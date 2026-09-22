//! Android 原生能力桥：屏幕方向锁定、文件选择结果（`content://`）落地等。
//!
//! 调用链：`Kotlin MainActivity` → `TauriBridge.nativeAttach()`（把 JavaVM 交给 Rust）
//! → Rust 通过 JNI 调用 `com.zhayi.qookix.MainActivity` 上的静态方法。
//! 非 Android 平台全部降级为 no-op，保证同一份代码在桌面端可编译。

/// 记录 JavaVM 与 MainActivity 的全局引用（由 `nativeAttach` 在 JNI 入口处调用）。
#[cfg(target_os = "android")]
pub fn init(vm: jni::JavaVM, env: &mut jni::JNIEnv, activity: &jni::objects::JObject) {
    android::set_vm(vm);
    android::set_activity(env, activity);

    // 应用启动就把 native 库抽取好（平时 stamp 未变时是零拷贝空跑，开销可忽略）。
    //
    // 这是「装包后第一次启动必失败」的根因：抽取原本发生在启动游戏的流程里，
    // 而 GameActivity 会更早去加载 `files/natives/libpojavexec.so` —— 第一次启动时
    // 抽取正在进行/尚未完成，ART 侧就会回退去加载 APK 里的另一份副本，
    // 两份各自的 `pojav_environ` 不共享，GL 初始化直接 SIGSEGV(fault addr 0x0)。
    // 提前在应用启动时做完，之后谁加载的都是同一份文件，竞态消失。
    std::thread::spawn(|| {
        let result = tauri::async_runtime::block_on(async {
            match crate::settings::get_data_dir().await {
                Ok(dir) => crate::android_env::resolve_native_lib_dir(&dir)
                    .await
                    .map(|_| ())
                    .map_err(|e| e.to_string()),
                Err(e) => Err(e.to_string()),
            }
        });
        match result {
            Ok(()) => tracing::info!("[launcher] 应用启动阶段 natives 已就绪"),
            Err(e) => tracing::warn!("[launcher] 应用启动阶段抽取 natives 失败：{e}"),
        }
    });
}

/// 锁定 / 跟随屏幕方向。`mode`：`system` | `portrait` | `landscape`。
#[cfg(target_os = "android")]
pub fn set_orientation(mode: &str) -> Result<(), String> {
    android::call_activity(
        "setOrientation",
        "(Ljava/lang/String;)V",
        Some(mode),
    )
    .map(|_| ())
    .ok_or_else(|| "设置屏幕方向失败（原生桥未就绪）".to_string())
}

/// 拉起游戏界面（GameActivity）。游戏必须跑在带 SurfaceView 的 Activity 里，
/// 否则 GL4ES 没有 Surface 可用，GLFW 创建窗口就会失败。
#[cfg(target_os = "android")]
pub fn start_game_activity(instance_id: &str, account_uuid: &str) -> Result<(), String> {
    android::call_activity2(
        "startGameActivity",
        "(Ljava/lang/String;Ljava/lang/String;)V",
        instance_id,
        account_uuid,
    )
    .map(|_| ())
    .ok_or_else(|| "无法拉起游戏界面（原生桥未就绪）".to_string())
}

#[cfg(not(target_os = "android"))]
pub fn start_game_activity(_instance_id: &str, _account_uuid: &str) -> Result<(), String> {
    Err("桌面端不需要 GameActivity".to_string())
}

/// 把文件选择器返回的 `content://` URI 落地成应用私有目录下的真实路径。
/// 已经是普通路径时原样返回。
#[cfg(target_os = "android")]
pub fn resolve_picked_path(uri_or_path: &str) -> Result<String, String> {
    android::call_activity(
        "resolvePickedUri",
        "(Ljava/lang/String;)Ljava/lang/String;",
        Some(uri_or_path),
    )
    .ok_or_else(|| "无法读取所选文件".to_string())
}

/// 把文本写回「另存为」选中的目标。
///
/// 安卓的文件选择器（`ACTION_CREATE_DOCUMENT`）返回的是 SAF 的 `content://` URI，
/// 不是路径 —— Rust 侧对 URI 做 `fs::write` 必然失败，导出日志/配置就一直报错。
/// 写入必须由 ContentResolver 完成，这里转交给原生。
///
/// 返回值约定：原生侧成功返回**空串**，失败返回错误信息，桥不可用返回 None。
#[cfg(target_os = "android")]
pub fn write_text_to_uri(uri_or_path: &str, content: &str) -> Result<(), String> {
    match android::call_activity_str2(
        "writeTextToUri",
        "(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
        uri_or_path,
        content,
    ) {
        Some(msg) if msg.is_empty() => Ok(()),
        Some(msg) => Err(msg),
        None => Err("原生桥不可用，无法写入所选位置".to_string()),
    }
}

#[cfg(not(target_os = "android"))]
pub fn write_text_to_uri(uri_or_path: &str, content: &str) -> Result<(), String> {
    let p = std::path::Path::new(uri_or_path);
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(p, content).map_err(|e| format!("写入文件失败: {e}"))
}

/// 读取系统剪贴板文本。
///
/// 代码编辑器的「粘贴」在 WebView 里没有可用路径：`navigator.clipboard.readText()`
/// 需要 wry 未授予的 `clipboard-read` 权限，而 `document.execCommand("paste")`
/// 在 Chromium 里按设计永远失败。只能由原生读。
#[cfg(target_os = "android")]
pub fn read_clipboard() -> Result<String, String> {
    android::call_activity("readClipboardText", "()Ljava/lang/String;", None)
        .ok_or_else(|| "剪贴板为空或读取失败".to_string())
}

/// 桌面端浏览器有完整的 Clipboard API，不需要也不应该走原生。
#[cfg(not(target_os = "android"))]
pub fn read_clipboard() -> Result<String, String> {
    Err("当前平台请使用系统粘贴".to_string())
}

/// 写入系统剪贴板。
///
/// 同样是因为 WebView 没给 `clipboard-write` 权限（`writeText` 抛 NotAllowedError）。
/// 返回约定同 `write_text_to_uri`：原生侧成功返回**空串**。
#[cfg(target_os = "android")]
pub fn write_clipboard(text: &str) -> Result<(), String> {
    match android::call_activity(
        "writeClipboardText",
        "(Ljava/lang/String;)Ljava/lang/String;",
        Some(text),
    ) {
        Some(msg) if msg.is_empty() => Ok(()),
        Some(msg) => Err(msg),
        // 桥不可用时 `call_activity` 返回 None
        None => Err("写入剪贴板失败（原生桥未就绪）".to_string()),
    }
}

#[cfg(not(target_os = "android"))]
pub fn write_clipboard(_text: &str) -> Result<(), String> {
    Err("当前平台请使用系统复制".to_string())
}

/// 读取当前**系统代理**（Android 上是 VPN / Wi-Fi 下发的 HTTP 代理，例如 Clash 的 127.0.0.1:7890）。
/// 返回形如 `http://127.0.0.1:7890` / `socks5://127.0.0.1:1080`；没有代理时返回 None。
///
/// 原生 socket 不会自动走系统代理，只有显式套用这个地址，`proxy_mode = "system"`
/// 才会和 WebView 的联网行为一致。
#[cfg(target_os = "android")]
pub fn system_proxy() -> Option<String> {
    android::call_activity("systemProxy", "()Ljava/lang/String;", None)
        .filter(|s| !s.trim().is_empty())
}

/// 预加载 JRE 自带的原生库（JDK 8 必需）。
///
/// 见 Kotlin 侧 `preloadJreLibs` 的说明：Android 的 linker 只按命名空间路径解析
/// 裸文件名，JDK 8 的库之间靠裸名字互相依赖（libnio.so → libnet.so），
/// 必须在 JVM 启动前用绝对路径先加载进来。
#[cfg(target_os = "android")]
pub fn preload_jre_libs(jre_home: &str) -> Result<i32, String> {
    android::call_activity("preloadJreLibs", "(Ljava/lang/String;)I", Some(jre_home))
        .and_then(|s| s.trim().parse::<i32>().ok())
        .ok_or_else(|| "预加载 JRE 原生库失败（原生桥未就绪）".to_string())
}

#[cfg(not(target_os = "android"))]
pub fn preload_jre_libs(_jre_home: &str) -> Result<i32, String> {
    Ok(0)
}
/// 读取移植过来的 Pojav 控制层设置（整份 SharedPreferences，JSON 字符串）。
///
/// 这些设置（分辨率缩放 / 按钮大小 / 鼠标速度 / 忽略刘海 / 长按判定 / 陀螺仪…）
/// 由 `LauncherPreferences` 从安卓侧读取，QookiX 的 settings.json 管不到它们，
/// 所以必须过这座桥。
#[cfg(target_os = "android")]
pub fn read_pojav_prefs() -> Result<String, String> {
    android::call_activity("readPojavPrefs", "()Ljava/lang/String;", None)
        .ok_or_else(|| "读取游戏内设置失败（原生桥未就绪）".to_string())
}

/// 写入 Pojav 设置（JSON）。Kotlin 侧写完会重载 LauncherPreferences，
/// 所以不需要重启应用即可生效。
#[cfg(target_os = "android")]
pub fn write_pojav_prefs(json: &str) -> Result<(), String> {
    android::call_activity(
        "writePojavPrefs",
        "(Ljava/lang/String;)V",
        Some(json),
    )
    .map(|_| ())
    .ok_or_else(|| "保存游戏内设置失败（原生桥未就绪）".to_string())
}

#[cfg(not(target_os = "android"))]
pub fn read_pojav_prefs() -> Result<String, String> {
    Ok("{}".to_string())
}

#[cfg(not(target_os = "android"))]
pub fn write_pojav_prefs(_json: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "android"))]
pub fn set_orientation(_mode: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "android"))]
pub fn resolve_picked_path(uri_or_path: &str) -> Result<String, String> {
    Ok(uri_or_path.to_string())
}

#[cfg(not(target_os = "android"))]
pub fn system_proxy() -> Option<String> {
    None
}

#[cfg(target_os = "android")]
mod android {
    use jni::objects::{JObject, JString, JValue};
    use jni::JavaVM;
    use std::sync::atomic::{AtomicPtr, Ordering};

    /// 应用进程的 JavaVM（进程内唯一，且生命周期与进程一致，无需释放）。
    static JAVA_VM: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
    /// MainActivity 实例的全局引用（由 `nativeAttach` 在 JNI 入口处创建）。
    ///
    /// 这里存实例而不是类名：从原生线程 `FindClass` 用的是系统类加载器，看不到应用类，
    /// 在开启 `-Xcheck:jni` 的设备上会直接 abort 掉进程。实例方法调用则完全避开这个问题。
    static ACTIVITY: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

    pub fn set_vm(vm: JavaVM) {
        JAVA_VM.store(
            vm.get_java_vm_pointer() as *mut std::ffi::c_void,
            Ordering::Release,
        );
    }

    pub fn set_activity(env: &mut jni::JNIEnv, activity: &JObject) {
        if let Ok(global) = env.new_global_ref(activity) {
            let raw = global.as_obj().as_raw() as *mut std::ffi::c_void;
            // 进程级引用：故意不释放（释放需要 JNIEnv，且应用退出前一直有效）
            std::mem::forget(global);
            ACTIVITY.store(raw, Ordering::Release);
        }
    }

    fn java_vm() -> Option<JavaVM> {
        let ptr = JAVA_VM.load(Ordering::Acquire) as *mut jni::sys::JavaVM;
        if ptr.is_null() {
            return None;
        }
        unsafe { JavaVM::from_raw(ptr).ok() }
    }

    fn activity() -> Option<JObject<'static>> {
        let raw = ACTIVITY.load(Ordering::Acquire) as jni::sys::jobject;
        if raw.is_null() {
            return None;
        }
        Some(unsafe { JObject::from_raw(raw) })
    }

    /// 两个字符串参数的版本（例如 startGameActivity(instanceId, accountUuid)）。
    pub fn call_activity2(name: &str, sig: &str, first: &str, second: &str) -> Option<String> {
        let vm = java_vm()?;
        let mut env = vm.attach_current_thread_as_daemon().ok()?;
        let obj = activity()?;

        let arg1: JString = env.new_string(first).ok()?;
        let arg2: JString = env.new_string(second).ok()?;
        env.call_method(
            &obj,
            name,
            sig,
            &[JValue::Object(&arg1), JValue::Object(&arg2)],
        )
        .ok()?;
        Some(String::new())
    }

    /// 两个字符串参数、**并取回字符串结果**的版本。
    ///
    /// 与 `call_activity2` 的区别：那个是「调用完就算成功」的（`startGameActivity`
    /// 那种不需要回执），这里要拿回执才能知道写文件到底成没成。
    pub fn call_activity_str2(name: &str, sig: &str, first: &str, second: &str) -> Option<String> {
        let vm = java_vm()?;
        let mut env = vm.attach_current_thread_as_daemon().ok()?;
        let obj = activity()?;

        let arg1: JString = env.new_string(first).ok()?;
        let arg2: JString = env.new_string(second).ok()?;
        let result = env
            .call_method(
                &obj,
                name,
                sig,
                &[JValue::Object(&arg1), JValue::Object(&arg2)],
            )
            .ok()?;

        match result {
            jni::objects::JValueOwned::Object(ret) => {
                if ret.is_null() {
                    return None;
                }
                let jstr = JString::from(ret);
                env.get_string(&jstr)
                    .ok()
                    .map(|s| s.to_string_lossy().into_owned())
            }
            _ => Some(String::new()),
        }
    }

    /// 调用 MainActivity 上的实例方法并返回字符串结果（返回 null 时得到 None）。
    pub fn call_activity(name: &str, sig: &str, arg: Option<&str>) -> Option<String> {
        let vm = java_vm()?;
        let mut env = vm.attach_current_thread_as_daemon().ok()?;
        let obj = activity()?;

        let result = match arg {
            Some(value) => {
                let jvalue: JString = env.new_string(value).ok()?;
                env.call_method(&obj, name, sig, &[JValue::Object(&jvalue)])
                    .ok()?
            }
            None => env.call_method(&obj, name, sig, &[]).ok()?,
        };

        match result {
            jni::objects::JValueOwned::Object(ret) => {
                if ret.is_null() {
                    return None;
                }
                let jstr = JString::from(ret);
                env.get_string(&jstr)
                    .ok()
                    .map(|s| s.to_string_lossy().into_owned())
            }
            // void / 基本类型返回：调用成功即可，用空串表示「成功但没有返回值」
            _ => Some(String::new()),
        }
    }
}
