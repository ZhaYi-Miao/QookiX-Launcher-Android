use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::sync::OnceLock;
use tokio::fs;
use anyhow::{Context, Result};
use crate::models::*;
use crate::version::MinecraftVersion;
use crate::java::{MultiRTManager, JreManager, JREValidator, get_device_arch};
use crate::natives::NativesExtractor;
use crate::jvm_launcher::JvmLauncher;

static GAME_PID: OnceLock<Mutex<Option<u32>>> = OnceLock::new();
static GAME_STATUS: OnceLock<Mutex<GameStatus>> = OnceLock::new();

fn init_statics() {
    GAME_PID.get_or_init(|| Mutex::new(None));
    GAME_STATUS.get_or_init(|| Mutex::new(GameStatus {
        is_running: false,
        instance_id: None,
        pid: None,
        exit_code: None,
        error: None,
    }));
}

pub async fn launch_game(instance_id: &str, account: Option<&Account>) -> Result<GameStatus> {
    crate::fsutil::validate_id(instance_id, "实例").map_err(|e| anyhow::anyhow!(e))?;
    init_statics();

    {
        let mut status = GAME_STATUS.get().unwrap().lock().unwrap();
        if status.is_running {
            return Err(anyhow::anyhow!("GAME_ALREADY_RUNNING"));
        }
        status.is_running = true;
        status.instance_id = Some(instance_id.to_string());
        status.pid = None;
        status.exit_code = None;
        status.error = None;
    }
    // 前端靠 launch://state 显示「运行中」并提供「关闭所有实例」
    crate::progress::emit_launch_state(instance_id, "running", 0, None);

    // 记录游戏启动前的工作目录，游戏退出后恢复（见下面的 chdir）

    let mut previous_dir: Option<std::path::PathBuf> = None;

    
    let launch_result = async {
        let data_dir = crate::settings::get_data_dir().await?;
        let instance_dir = Path::new(&data_dir)
            .join("instances")
            .join(instance_id);

        let instance_file = instance_dir.join("instance.json");
        let content = fs::read_to_string(&instance_file).await
            .context("Failed to read instance")?;

        let instance: MinecraftProfile = serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("INSTANCE_CONFIG_CORRUPT: {}", e))?;

        // 空实例兜底：没装游戏文件就启动，JVM 找不到 client jar 会直接失败 ——
        // 现象是「黑屏 / 秒退」，日志里只有 LWJGL 报错，用户完全看不出是没装。
        if !crate::version::is_version_installed(&instance.mc_version).await {
            return Err(anyhow::anyhow!("INSTANCE_NOT_INSTALLED"));
        }

        let version_info = crate::version::get_version_info(&instance.mc_version).await?;

        let required_java = JREValidator::get_required_version(&version_info).await?;

        crate::progress::emit_launch_progress(
            &format!("正在检查 Java {required_java} 运行时…"),
            20.0,
        );
        let runtime = match MultiRTManager::get_compatible_runtime(required_java).await? {
            Some(r) => r,
            None => {
                tracing::info!("No compatible JRE found, downloading Java {} for {}", required_java, get_device_arch());
                // 实际下载包约 27MB（解压后 150~250MB）。以前这里写「约 100MB」，差得太远。
                crate::progress::emit_launch_progress(
                    &format!("未找到 Java {required_java}，正在下载运行时（约 27MB）…"),
                    25.0,
                );
                // JRE 下载很慢，必须把百分比报给悬浮启动卡片，否则用户以为没反应。
                // `install()` 把整体进度分成两段：下载 0..0.7、解压 0.7..1.0，
                // 这里统一映射到启动流程的 25%..65%。
                let last_pct = std::sync::Arc::new(std::sync::atomic::AtomicI32::new(-100));
                let last_pct_for_phase = last_pct.clone();
                JreManager::install(
                    required_java,
                    get_device_arch(),
                    move |p: f32| {
                        let pct = 25.0 + p as f64 * 40.0;
                        let as_int = pct as i32;
                        if as_int - last_pct.load(std::sync::atomic::Ordering::Relaxed) >= 2 {
                            last_pct.store(as_int, std::sync::atomic::Ordering::Relaxed);
                            // 文案用阶段中性的说法：这个回调在「下载中」和
                            // 「解压刚结束（install 收尾报 100%）」两个时刻都会触发，
                            // 写死「正在下载」会在解压阶段显示成假信息。
                            crate::progress::emit_launch_progress("正在准备 Java 运行时…", pct);
                        }
                    },
                    move |phase: &str| {
                        // 解压这几十秒里没有任何百分比可报，至少把文案换对，
                        // 否则用户会以为还卡在下载。
                        last_pct_for_phase.store(63, std::sync::atomic::Ordering::Relaxed);
                        crate::progress::emit_launch_progress(phase, 63.0);
                    },
                )
                .await?
            }
        };

        if !JREValidator::validate(&runtime).await? {
            return Err(anyhow::anyhow!(
                "JRE 校验失败（{}）：{} 下找不到 libjli.so/libjvm.so",
                runtime.name,
                runtime.path
            ));
        }

        let libraries_dir = Path::new(&data_dir).join("libraries");
        let natives_cache_dir = Path::new(&data_dir)
            .join("cache")
            .join("natives")
            .join(instance_id);

        crate::progress::emit_launch_progress("正在解压运行库…", 70.0);
        let natives_dir = NativesExtractor::extract(&libraries_dir, get_device_arch(), &natives_cache_dir).await?;

        // 安卓端真正的基础原生库（liblwjgl / libgl4es / libopenal …）随 APK 分发。
        // 新版本安卓默认不从 APK 解出 .so，这里解析出（必要时从 APK 抽取）一个
        // 能让 JVM `System.loadLibrary` 成功的目录。
        #[cfg(target_os = "android")]
        let apk_native_dir = crate::android_env::resolve_native_lib_dir(&data_dir)
            .await
            .map_err(|e| anyhow::anyhow!("安卓原生库准备失败：{e}"))?;

        let vendor = load_vendor_jars(&data_dir, runtime.java_version).await?;

        let classpath = build_classpath(&version_info, &data_dir, &vendor).await?;

        let mut jvm_args = build_jvm_args(&instance, &runtime, &vendor);

        // 正版账号：启动前把 refresh_token 换成真正的 Minecraft access token。
        // 换不出来就明确失败 —— 不能拿 refresh token 冒充 access token 骗过启动流程
        // （那样游戏能起来，但一连正版服务器就 401/掉线，用户根本不知道发生了什么）。
        let ms_token = match account {
            Some(acc) if acc.account_type == crate::models::AccountType::Microsoft => Some(
                crate::accounts::minecraft_access_token(acc)
                    .await
                    .map_err(|e| anyhow::anyhow!("正版会话已过期，请重新登录微软账号：{e}"))?,
            ),
            _ => None,
        };
        let game_args = build_game_args(&instance, account, &data_dir, ms_token.as_deref())?;

        let game_dir = instance.game_dir.clone();

        // Minecraft 的 Log4j 不会自己创建 logs/ 目录，缺了会直接
        // `FileNotFoundException: logs/latest.log`（游戏仍会继续，但日志丢失）。
        for sub in [
            "logs",
            "crash-reports",
            "saves",
            "resourcepacks",
            "shaderpacks",
            "mods",
            "config",
        ] {
            let _ = fs::create_dir_all(Path::new(&game_dir).join(sub)).await;
        }

        #[cfg(target_os = "android")]
        jvm_args.extend(build_android_jvm_args(
            &instance,
            &runtime,
            &data_dir,
            &game_dir,
            &natives_dir.to_string_lossy(),
            &apk_native_dir,
        ));

        let mut env_map = HashMap::new();
        env_map.insert("JAVA_HOME".to_string(), runtime.path.clone());
        env_map.insert("HOME".to_string(), game_dir.clone());
        let renderer = renderer_setting();

        // 注意：**不要**设置 LD_LIBRARY_PATH。
        // 实测（1.8.9 与 1.18.2 都试过）覆盖它会让 libjvm.so 加载失败，
        // 于是 JLI 退化成 `exec bin/java`，而 Android 禁止执行应用私有目录 →
        // 报 `Permission denied / Error: trying to exec .../bin/java`，比不设更糟。
        // JDK 8 自带的原生库找不到依赖（libnio.so → libnet.so）改用
        // `preload_jre_libs` 在 JVM 启动前逐个 System.load 解决，见那里。

        env_map.insert("POJAV_NATIVEDIR".to_string(), natives_dir.to_string_lossy().to_string());
        // Pojav 的 Java 层会设这些变量，libpojavexec 的 pojavInitOpenGL 直接读它们：
        // 读不到就是 NULL，`strcmp(NULL,...)` 会段错误（不是可选优化，是必须项）。
        env_map.insert("POJAV_RENDERER".to_string(), renderer.clone());
        env_map.insert("FORCE_VSYNC".to_string(), "true".to_string());
        env_map.insert("POJAV_EMUI_ITERATOR_MITIGATE".to_string(), "false".to_string());
        env_map.insert("POJAV_VSYNC_IN_ZINK".to_string(), "false".to_string());
        env_map.insert("TMPDIR".to_string(), format!("{}/cache", data_dir.trim_end_matches('/')));

        if renderer == "vulkan_zink" {
            // Zink = Mesa 的「OpenGL on Vulkan」。C 层自己也会 setenv GALLIUM_DRIVER=zink，
            // 这里显式再设一遍更稳；MESA_GLSL_CACHE_DIR 让着色器缓存落到应用私有目录，
            // 否则每次进游戏都要重编译一遍（很慢）。
            env_map.insert("GALLIUM_DRIVER".to_string(), "zink".to_string());
            env_map.insert("MESA_LOADER_DRIVER_OVERRIDE".to_string(), "zink".to_string());
            let cache_dir = format!("{}/cache", data_dir.trim_end_matches('/'));
            env_map.insert("MESA_GLSL_CACHE_DIR".to_string(), cache_dir.clone());
            // 下面这几项是 Pojav 原版在 zink 路径上一定会设的，移植时漏了：
            // 前三个放宽 GLSL 版本/扩展限制（Minecraft 的着色器常用高版本语法），
            // VTEST_SOCKET_NAME 是 virgl/zink 用来放测试 socket 的位置。
            env_map.insert("force_glsl_extensions_warn".to_string(), "true".to_string());
            env_map.insert("allow_higher_compat_version".to_string(), "true".to_string());
            env_map.insert(
                "allow_glsl_extension_directive_midshader".to_string(),
                "true".to_string(),
            );
            env_map.insert("VTEST_SOCKET_NAME".to_string(), format!("{cache_dir}/.virgl_test"));
            // 注意 egl_bridge.c 里是**存在性**判断（`getenv(...) == NULL` 才跳过），
            // 所以给 GL4ES 时绝不能设这个变量，否则一样会去加载 Turnip。
            env_map.insert("POJAV_LOAD_TURNIP".to_string(), "1".to_string());
            env_map.insert("LIBGL_ES".to_string(), "3".to_string());
        } else {
            // GL4ES 专有：mipmap 与「忽略 GL 错误」（后者能躲掉整合包里的无效调用）
            env_map.insert("LIBGL_MIPMAP".to_string(), "3".to_string());
            env_map.insert("LIBGL_NOERROR".to_string(), "1".to_string());
            // 1.17+ 需要 OpenGL 3.2 才能渲染主界面全景/帧缓冲，
            // ES2 下这些会静默失败（表现就是主界面背景一片灰）。
            env_map.insert("LIBGL_ES".to_string(), "3".to_string());
        }

        for (key, value) in &env_map {
            std::env::set_var(key, value);
        }

        let main_class = version_info.main_class
            .unwrap_or_else(|| "net.minecraft.client.main.Main".to_string());

        let jre_home = Path::new(&runtime.path);

        // 必须先预加载 JRE 内部库，否则 JVM 走到 libnio.so 时必然
        // `library "libnet.so" not found`（安卓 linker 不查 JRE 的 lib 目录）
        #[cfg(target_os = "android")]
        crate::android_env::preload_jre_libraries(jre_home);

        // 悬浮启动卡片靠 launch://progress 显示步骤；这里不要用 launch://log，
        // 前端把任何 launch://log 都当作「启动成功」。
        crate::progress::emit_launch_progress(
            &format!("正在启动 JVM（Java {}）…", runtime.java_version),
            85.0,
        );

        // 安卓进程的 stdout 默认丢弃：把 fd 1/2 转到日志文件，并把新增行推给前端日志面板
        #[cfg(target_os = "android")]
        {
            let log_path = Path::new(&data_dir)
                .join("logs")
                .join(format!("launch-{instance_id}.log"));
            crate::android_env::redirect_output(&log_path)
                .with_context(|| format!("无法创建启动日志：{}", log_path.display()))?;
            crate::android_env::spawn_log_tail(log_path, instance_id.to_string());

            // 窗口尺寸决定触摸坐标换算是否准确，出问题时最需要看到它
            for arg in jvm_args.iter().filter(|a| a.starts_with("-Dglfwstub")) {
                tracing::info!("[launcher] {arg}");
            }
            tracing::info!(
                "[launcher] classpath 条目 {}，java {} @ {}",
                classpath.matches(':').count() + 1,
                runtime.java_version,
                runtime.path
            );
        }

        // 把**进程的工作目录**切到实例目录。
        //
        // 为什么必须做：游戏里大量相对路径是按「当前工作目录」解析的 ——
        // Minecraft 的 log4j 配置写的是 `logs/latest.log`，安卓应用的默认工作目录是 `/`，
        // 于是日志直接报 `FileNotFoundException: logs/latest.log (No such file or directory)`，
        // 游戏自己的日志/部分资源从此写不出来。
        // 只设 `-Duser.dir` 是不够的：那只是 Java 的一个属性，不改变 OS 层的 CWD。
        // JVM 与游戏都在本进程里，所以这里 chdir 一次就够；游戏退出后再切回去。
        // JDK 8 的原生库必须提前用绝对路径加载（见 android_bridge::preload_jre_libs）。
        // 只对 Java 8 及更早版本做 —— JDK 9+ 的库是扁平的，本来就能找到，不做无谓的事。
        if runtime.java_version <= 8 {
            match crate::android_bridge::preload_jre_libs(&runtime.path) {
                Ok(n) => tracing::info!("[launcher] 预加载 JRE 原生库 {n} 个"),
                Err(e) => tracing::warn!("[launcher] 预加载 JRE 原生库失败：{e}"),
            }
        }
        previous_dir = std::env::current_dir().ok();
        if let Err(e) = std::env::set_current_dir(&game_dir) {
            tracing::warn!("切换工作目录到实例目录失败（游戏内相对路径可能异常）：{e}");
        }

        // 在启动 JVM 之前在后台守候：等 JVM 起来后接管 `GL11C.nglGetString`，
        // 把 F3 里那行 `Display: … (厂商串)` 换成我们自己的（详见 gl_brand.rs）。
        crate::gl_brand::spawn_installer();

        let exit_code = JvmLauncher::launch(
            jre_home,
            &jvm_args,
            &classpath,
            &main_class,
            &game_args,
            &game_dir,
        )?;

        Ok::<i32, anyhow::Error>(exit_code)
    }.await;

    // 恢复原来的工作目录：启动器和游戏同进程，别把 CWD 留在实例目录里
    if let Some(dir) = previous_dir {
        let _ = std::env::set_current_dir(dir);
    }

    let exit_code = match launch_result {
        Ok(code) => code,
        Err(e) => {
            let mut status = GAME_STATUS.get().unwrap().lock().unwrap();
            status.is_running = false;
            status.error = Some(e.to_string());
            drop(status);
            crate::progress::emit_launch_progress(&format!("启动失败：{e}"), 100.0);
            crate::progress::emit_launch_exit();
            crate::progress::emit_launch_state(instance_id, "exited", 0, None);
            #[cfg(target_os = "android")]
            crate::android_env::stop_log_tail();
            return Err(e);
        }
    };

    {
        let mut status = GAME_STATUS.get().unwrap().lock().unwrap();
        status.is_running = false;
        status.exit_code = Some(exit_code);
    }
    #[cfg(target_os = "android")]
    crate::android_env::stop_log_tail();
    crate::progress::emit_launch_log(instance_id, "out", &format!("JVM 已退出，退出码 {exit_code}"));
    crate::progress::emit_launch_exit();
    crate::progress::emit_launch_state(instance_id, "exited", 0, Some(exit_code));

    if exit_code != 0 {
        let _ = crate::crash::report_crash(instance_id, exit_code).await;
    }

    Ok(get_game_status())
}

pub async fn kill_game() -> Result<()> {
    init_statics();

    crate::input_bridge::set_ready(false);
    crate::render_bridge::release();
    JvmLauncher::shutdown()?;

    let instance_id = {
        let mut status = GAME_STATUS.get().unwrap().lock().unwrap();
        status.is_running = false;
        status.pid = None;
        let id = status.instance_id.clone();
        drop(status);
        id
    };
    if let Some(id) = instance_id {
        crate::progress::emit_launch_state(&id, "exited", 0, None);
    }

    Ok(())
}

pub fn get_game_status() -> GameStatus {
    init_statics();
    GAME_STATUS.get().unwrap().lock().unwrap().clone()
}

async fn build_classpath(
    version_info: &MinecraftVersion,
    data_dir: &str,
    vendor: &VendorJars,
) -> Result<String> {
    let mut classpath = Vec::new();

    let version_jar = Path::new(data_dir)
        .join("versions")
        .join(&version_info.id)
        .join(format!("{}.jar", version_info.id));
    classpath.push(version_jar.to_string_lossy().to_string());

    if let Some(libraries) = &version_info.libraries {
        for lib in libraries {
            // 桌面版 LWJGL 在安卓上没得用（不带 arm64 natives），全部剔除，
            // 由下面的 Pojav 版顶替。
            if lib.name.starts_with("org.lwjgl:") {
                continue;
            }
            if let Some(downloads) = &lib.downloads {
                if let Some(artifact) = &downloads.artifact {
                    let lib_path = Path::new(data_dir)
                        .join("libraries")
                        .join(&artifact.path);
                    classpath.push(lib_path.to_string_lossy().to_string());
                }
            }
        }
    }

    // Pojav 版 LWJGL（官方 LWJGL 全部类 + 安卓版 GLFW stub）：
    // 放在最后也不会被顶掉，因为 org.lwjgl:* 已经在上面的循环里被剔除。
    // Caciocavallo 不放这里，它必须走 bootclasspath（见 build_jvm_args）。
    classpath.push(vendor.lwjgl.to_string_lossy().to_string());

    Ok(classpath.join(":"))
}

/// 安卓适配 jar 的落盘路径（非安卓平台为空结构，逻辑只有一处 cfg）。
struct VendorJars {
    lwjgl: std::path::PathBuf,
    cacio: Vec<std::path::PathBuf>,
}

async fn load_vendor_jars(data_dir: &str, java_version: i32) -> Result<VendorJars> {
    #[cfg(target_os = "android")]
    {
        let vendor = crate::android_env::ensure_vendor_jars(data_dir, java_version)
            .await
            .map_err(|e| anyhow::anyhow!("释放安卓适配 jar 失败：{e}"))?;
        Ok(VendorJars {
            lwjgl: vendor.lwjgl,
            cacio: vendor.cacio,
        })
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (data_dir, java_version);
        Ok(VendorJars {
            lwjgl: std::path::PathBuf::new(),
            cacio: Vec::new(),
        })
    }
}

/// 读取某个实例的启动日志（**磁盘上的那份**）。
///
/// 前端日志面板平时显示的是内存里收到的 `launch://log` 事件流；可游戏一旦崩溃，
/// 整个进程会被带走，重启后内存里的日志就没了 —— 于是「日志」页永远显示
/// 「暂无日志」，偏偏在最需要它的时候。<br>
/// 这里直接读磁盘：优先启动器那份（包含启动步骤与早期报错，例如
/// `JRE 校验失败`、`找不到 libjli.so` 这类在游戏启动前就失败的情况），
/// 如果游戏真的跑起来过，再附上 Minecraft 自己的 `logs/latest.log`。
/// 日志最多读多少字节（从**尾部**读）。
///
/// 以前是 `read_to_string` 整个读进来 —— `latest.log` 上到几百 MB 在手机上很常见，
/// 一次就把内存打满。崩溃现场都在最后，前面几百 MB 的早期输出没有读的必要。
const LOG_TAIL_MAX: u64 = 2 * 1024 * 1024;

/// 只读文件**尾部**最多 `max_bytes`，截断时在开头标注。
fn read_tail(path: &Path, max_bytes: u64) -> std::io::Result<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path)?;
    let len = f.metadata()?.len();
    let truncated = len > max_bytes;
    if truncated {
        f.seek(SeekFrom::Start(len - max_bytes))?;
    }
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    let mut text = String::from_utf8_lossy(&buf).into_owned();
    if truncated {
        // 截断点多半落在某一行中间，把那半行丢掉
        if let Some(idx) = text.find('\n') {
            text.drain(..=idx);
        }
        text = format!("…（文件较大，仅显示最后 {} KB）\n{text}", max_bytes / 1024);
    }
    Ok(text)
}

pub async fn read_instance_log(instance_id: &str) -> anyhow::Result<String> {
    let data_dir = crate::settings::get_data_dir().await?;
    let id = instance_id.to_string();
    // 纯同步文件 IO、而且可能读几百 MB —— 必须挪出 tokio worker
    tokio::task::spawn_blocking(move || read_instance_log_sync(&id, &data_dir))
        .await
        .context("日志读取任务失败")?
}

fn read_instance_log_sync(instance_id: &str, data_dir: &str) -> anyhow::Result<String> {
    let launcher_log = Path::new(data_dir)
        .join("logs")
        .join(format!("launch-{instance_id}.log"));
    let game_log = Path::new(data_dir)
        .join("instances")
        .join(instance_id)
        .join("logs")
        .join("latest.log");

    let mut out = String::new();
    let mut found = false;

    if let Ok(text) = read_tail(&launcher_log, LOG_TAIL_MAX) {
        out.push_str(&format!("===== 启动器日志（{}）=====\n", launcher_log.display()));
        out.push_str(&text);
        found = true;
    }
    if let Ok(text) = read_tail(&game_log, LOG_TAIL_MAX) {
        if found {
            out.push_str("\n\n");
        }
        out.push_str(&format!("===== 游戏日志（{}）=====\n", game_log.display()));
        out.push_str(&text);
        found = true;
    }

    if !found {
        out.push_str("暂无日志：这个实例还没有启动过，或日志文件已被清理。");
    }
    Ok(out)
}
/// 当前渲染后端：`opengles2`（GL4ES，默认）或 `vulkan_zink`（Zink + Turnip）。
///
/// 这个设置和「按钮大小 / 鼠标速度」那些放一起，存在安卓侧 SharedPreferences，
/// 由「设置 → 游戏内 → 渲染器」写入，这里经原生桥读出来。
///
/// 任何一步失败都回退到 GL4ES —— 那条路径在所有设备上都能跑，
/// 而 Zink 需要 Adreno + Vulkan，选错就是黑屏，所以宁可保守。
fn renderer_setting() -> String {
    let Ok(json) = crate::android_bridge::read_pojav_prefs() else {
        return "opengles2".to_string();
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&json) else {
        return "opengles2".to_string();
    };
    match v.get("renderer").and_then(|r| r.as_str()) {
        Some("vulkan_zink") => "vulkan_zink".to_string(),
        // MobileGlues：返回库文件名 —— libpojavexec 对未知渲染器名会按 .so dlopen，
        // 这也是 FCL/Zalith 接入 MG 的同一机制（renderer 列表直接给 .so 文件名）。
        Some("mobileglues") => "libmobileglues.so".to_string(),
        _ => "opengles2".to_string(),
    }
}
/// 从设置文件里取整数项（兼容 snake_case 与 camelCase 两种键名）。
fn raw_i64(raw: &std::collections::HashMap<String, serde_json::Value>, keys: &[&str]) -> Option<i64> {
    keys.iter()
        .find_map(|k| raw.get(*k).and_then(|v| v.as_i64()))
}

/// 从设置文件里取文本项（兼容两种键名）。
fn raw_text(
    raw: &std::collections::HashMap<String, serde_json::Value>,
    keys: &[&str],
) -> Option<String> {
    keys.iter()
        .find_map(|k| raw.get(*k).and_then(|v| v.as_str()).map(|s| s.to_string()))
}

/// 解析实际使用的 `-Xmx` / `-Xms`（单位 MB）。
///
/// 修的是两个真问题：
/// 1. 以前只读 `instance.max_memory_mb.unwrap_or(2048)`，**设置界面里配的全局内存
///    完全不生效** —— 每个实例都固定 2048M；
/// 2. 没有任何范围校验：`0` / 负数会拼出 `-Xmx0M`，`min > max` 会让 JVM 直接拒绝启动。
fn resolve_memory_limits(
    instance: &MinecraftProfile,
    raw: &std::collections::HashMap<String, serde_json::Value>,
) -> (i32, i32) {
    // 手机上的实际上限；再高只会换来 OOM 和系统杀进程
    const HARD_MAX_MB: i32 = 8192;
    const HARD_MIN_MB: i32 = 256;

    let max_mb = instance
        .max_memory_mb
        .or_else(|| raw_i64(raw, &["max_memory_mb", "maxMemoryMb"]).map(|v| v as i32))
        .unwrap_or(2048)
        .clamp(512, HARD_MAX_MB);

    let min_mb = raw_i64(raw, &["min_memory_mb", "minMemoryMb"])
        .map(|v| v as i32)
        .unwrap_or(512)
        .clamp(HARD_MIN_MB, max_mb);

    (max_mb, min_mb)
}
fn build_jvm_args(instance: &MinecraftProfile, runtime: &Runtime, vendor: &VendorJars) -> Vec<String> {
    let mut args = Vec::new();

    // 设置文件只读一次（load_raw_settings_sync 是同步磁盘读，别在热路径里反复调）
    let raw = crate::settings::load_raw_settings_sync();

    let (max_mem, min_mem) = resolve_memory_limits(instance, &raw);
    args.push(format!("-Xmx{}M", max_mem));
    args.push(format!("-Xms{}M", min_mem));

    // 设置界面里的「JVM 参数」以前从来没被读过 —— 改了没有任何效果。
    if let Some(global_jvm_args) = raw_text(&raw, &["jvm_args", "jvmArgs"]) {
        if !global_jvm_args.trim().is_empty() {
            args.extend(global_jvm_args.split_whitespace().map(|s| s.to_string()));
        }
    }

    // Caciocavallo（安卓上的 AWT 实现）。
    //
    // 现状：**关闭**。Pojav 版 JRE 的 lib 目录里只有 libawt_headless.so，
    // 没有 libawt_xawt.so；而 `-Djava.awt.headless=false` 会让 java.awt.Toolkit
    // 的静态初始化去加载 libawt_xawt.so 并直接 UnsatisfiedLinkError。
    // 原版 Minecraft 不需要 AWT 界面（LWJGL 自带窗口），保持 headless 即可；
    // 将来若换用带 xawt 的 JRE，把这里改成 true 就能启用（参数表已照抄 Pojav）。
    const ENABLE_CACIOCAVALLO: bool = false;

    let (width, height) = instance.resolution.unwrap_or((854, 480));
    if !ENABLE_CACIOCAVALLO {
        args.push("-Djava.awt.headless=true".to_string());
        if let Some(custom_args) = &instance.java_args {
            args.extend(custom_args.split_whitespace().map(|s| s.to_string()));
        }
        return args;
    }

    if runtime.java_version == 8 {
        args.push("-Dawt.toolkit=net.java.openjdk.cacio.ctc.CTCToolkit".to_string());
        args.push(
            "-Djava.awt.graphicsenv=net.java.openjdk.cacio.ctc.CTCGraphicsEnvironment".to_string(),
        );
    } else {
        args.push("-Dawt.toolkit=com.github.caciocavallosilano.cacio.ctc.CTCToolkit".to_string());
        args.push(
            "-Djava.awt.graphicsenv=com.github.caciocavallosilano.cacio.ctc.CTCGraphicsEnvironment"
                .to_string(),
        );
        args.push(
            "-Djava.system.class.loader=com.github.caciocavallosilano.cacio.ctc.CTCPreloadClassLoader"
                .to_string(),
        );
        for flag in [
            "--add-exports=java.desktop/java.awt=ALL-UNNAMED",
            "--add-exports=java.desktop/java.awt.peer=ALL-UNNAMED",
            "--add-exports=java.desktop/sun.awt.image=ALL-UNNAMED",
            "--add-exports=java.desktop/sun.java2d=ALL-UNNAMED",
            "--add-exports=java.desktop/java.awt.dnd.peer=ALL-UNNAMED",
            "--add-exports=java.desktop/sun.awt=ALL-UNNAMED",
            "--add-exports=java.desktop/sun.awt.event=ALL-UNNAMED",
            "--add-exports=java.desktop/sun.awt.datatransfer=ALL-UNNAMED",
            "--add-exports=java.desktop/sun.font=ALL-UNNAMED",
            "--add-exports=java.base/sun.security.action=ALL-UNNAMED",
            "--add-opens=java.base/java.util=ALL-UNNAMED",
            "--add-opens=java.desktop/java.awt=ALL-UNNAMED",
            "--add-opens=java.desktop/sun.font=ALL-UNNAMED",
            "--add-opens=java.desktop/sun.java2d=ALL-UNNAMED",
            "--add-opens=java.base/java.lang.reflect=ALL-UNNAMED",
            "--add-opens=java.base/java.net=ALL-UNNAMED",
        ] {
            args.push(flag.to_string());
        }
    }

    args.push("-Djava.awt.headless=false".to_string());
    args.push(format!("-Dcacio.managed.screensize={width}x{height}"));
    args.push("-Dcacio.font.fontmanager=sun.awt.X11FontManager".to_string());
    args.push("-Dcacio.font.fontscaler=sun.font.FreetypeFontScaler".to_string());
    args.push("-Dswing.defaultlaf=javax.swing.plaf.metal.MetalLookAndFeel".to_string());

    // 关键：Caciocavallo 必须挂在引导类路径上。
    // `-Djava.system.class.loader` 指定的 CTCPreloadClassLoader 由引导加载器解析，
    // 只放进 -cp 会报 ClassNotFoundException / JVM_FindClassFromBootLoader。
    if !vendor.cacio.is_empty() {
        let joined = vendor
            .cacio
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(":");
        let kind = if runtime.java_version == 8 { "p" } else { "a" };
        args.push(format!("-Xbootclasspath/{kind}:{joined}"));
    }

    if let Some(custom_args) = &instance.java_args {
        args.extend(custom_args.split_whitespace().map(|s| s.to_string()));
    }

    args
}

/// 安卓端必须追加的 JVM 参数（对照 PojavLauncher 的 `JREUtils.getJavaArgs`）。
/// 这些参数少一个都可能表现为「JVM 起来了但游戏立刻崩」，所以尽量照抄：
/// 关键几项是 `java.library.path`（找到 liblwjgl/libgl4es）、`org.lwjgl.opengl.libname`
/// （走 GL4ES 把 OpenGL 翻成 GLES）、以及 `glfwstub.*`（安卓窗口尺寸）。
#[cfg(target_os = "android")]
fn build_android_jvm_args(
    instance: &MinecraftProfile,
    runtime: &Runtime,
    data_dir: &str,
    game_dir: &str,
    natives_dir: &str,
    native_lib_dir: &str,
) -> Vec<String> {
    let mut args = Vec::new();

    let mut library_path = natives_dir.to_string();
    if !native_lib_dir.is_empty() {
        library_path.push(':');
        library_path.push_str(&native_lib_dir);
    }
    args.push(format!("-Djava.library.path={library_path}"));
    args.push(format!("-Djna.boot.library.path={native_lib_dir}"));
    args.push(format!("-Djava.home={}", runtime.path));
    args.push(format!(
        "-Djava.io.tmpdir={}/cache",
        data_dir.trim_end_matches('/')
    ));
    args.push(format!("-Duser.home={game_dir}"));
    args.push(format!("-Duser.dir={game_dir}"));
    args.push("-Dos.name=Linux".to_string());
    args.push(format!(
        "-Dos.version={}",
        crate::android_env::android_release()
    ));

    // 渲染后端决定 LWJGL 去加载哪个 OpenGL 实现：
    //   GL4ES（opengles2）→ libgl4es_114.so：OpenGL → GLES 翻译层，兼容性最好
    //   Zink（vulkan_zink）→ libOSMesa.so：Mesa 的 OSMesa + Zink，OpenGL → Vulkan
    let renderer = renderer_setting();
    let gl_libname = if renderer == "vulkan_zink" {
        "libOSMesa.so"
    } else if renderer == "libmobileglues.so" {
        "libmobileglues.so"
    } else {
        "libgl4es_114.so"
    };
    args.push(format!("-Dorg.lwjgl.opengl.libname={gl_libname}"));
    args.push("-Dorg.lwjgl.vulkan.libname=libvulkan.so".to_string());
    if !native_lib_dir.is_empty() {
        args.push(format!(
            "-Dorg.lwjgl.freetype.libname={native_lib_dir}/libfreetype.so"
        ));
    }

    // 安卓 GLFW stub 的窗口尺寸**必须等于 Surface 的实际像素尺寸**。
    // 触摸坐标是 Surface 像素：若游戏以为窗口是 1280x720 而 Surface 是 2424x1080，
    // 所有点击都会被换算到错误位置（画面正常但按钮点不动）。
    // Surface 尺寸由 GameActivity 在 surfaceChanged 时同步过来；
    // 尚未同步（例如桌面端调用路径）时退回实例配置的分辨率。
    let (width, height) = crate::android_env::surface_size()
        .or(instance.resolution)
        .unwrap_or((1280, 720));
    args.push(format!("-Dglfwstub.windowWidth={width}"));
    args.push(format!("-Dglfwstub.windowHeight={height}"));
    args.push("-Dglfwstub.initEgl=false".to_string());

    args.push("-Dlog4j2.formatMsgNoLookups=true".to_string());
    args.push("-Dfml.earlyprogresswindow=false".to_string());
    args.push("-Dloader.disable_forked_guis=true".to_string());
    // 安卓没有 jspawnhelper，POSIX_SPAWN 会让 ProcessBuilder 直接失败
    args.push("-Djdk.lang.Process.launchMechanism=FORK".to_string());
    args.push(format!(
        "-XX:ActiveProcessorCount={}",
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    ));

    args
}

/// 资源索引的 id **不等于**游戏版本号。
///
/// 例如 **1.18.2 的资源索引 id 就是 `1.18`**（Mojang 在同一小版本系列里复用同一份索引），
/// 启动器下载到的文件也是 `assets/indexes/1.18.json`。
/// 而以前这里直接传 `instance.mc_version`（"1.18.2"），游戏于是去找
/// `assets/indexes/1.18.2.json` —— 永远找不到（日志里那三条报错的来源）。
/// 正确做法是读版本 JSON 里的 `assetIndex.id`。
fn asset_index_id(instance: &MinecraftProfile, data_dir: &str) -> String {
    let json_file = Path::new(data_dir)
        .join("versions")
        .join(&instance.mc_version)
        .join(format!("{}.json", instance.mc_version));
    if let Ok(text) = std::fs::read_to_string(&json_file) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(id) = v
                .get("assetIndex")
                .and_then(|a| a.get("id"))
                .and_then(|s| s.as_str())
            {
                return id.to_string();
            }
        }
    }
    // 读不到版本 JSON 就退回版本号：至少不会比修复前更差
    instance.mc_version.clone()
}
fn build_game_args(
    instance: &MinecraftProfile,
    account: Option<&Account>,
    data_dir: &str,
    ms_access_token: Option<&str>,
) -> Result<Vec<String>> {
    let mut args = Vec::new();

    let (username, uuid, access_token) = match account {
        Some(acc) => {
            let token = match acc.account_type {
                // 正版：必须是**换来的** Minecraft access token。
                // 以前这里是 `acc.msa_refresh_token`，等于把刷新令牌当访问令牌用 ——
                // 游戏能起来，但一连正版服务器就 401/掉线，用户完全看不出原因。
                crate::models::AccountType::Microsoft => ms_access_token
                    .filter(|t| !t.is_empty())
                    .ok_or_else(|| {
                        anyhow::anyhow!("正版会话已过期，请重新登录微软账号后再启动")
                    })?
                    .to_string(),
                // 离线账号没有正版会话，通行做法就是填 `0`
                _ => "0".to_string(),
            };
            (acc.username.clone(), acc.uuid.clone(), token)
        }
        None => ("Steve".to_string(), "00000000-0000-0000-0000-000000000000".to_string(), "0".to_string()),
    };

    args.push("--username".to_string());
    args.push(username);
    args.push("--uuid".to_string());
    args.push(uuid);
    args.push("--accessToken".to_string());
    args.push(access_token);
    args.push("--version".to_string());
    args.push(instance.mc_version.clone());
    args.push("--gameDir".to_string());
    args.push(instance.game_dir.clone());
    // --assetsDir 必须是**绝对路径**。以前传的是相对路径 "assets"，
    // 而游戏进程的工作目录并不是数据目录（安卓应用的工作目录是 "/"），
    // 于是游戏永远找不到资源 —— 日志里那几条
    // `Can't find the resource index file: assets/indexes/1.18.2.json` 就是这个原因。
    let assets_dir = Path::new(data_dir).join("assets");
    args.push("--assetsDir".to_string());
    args.push(assets_dir.to_string_lossy().to_string());
    args.push("--assetIndex".to_string());
    args.push(asset_index_id(instance, data_dir));

    if let Some((w, h)) = instance.resolution {
        args.push("--width".to_string());
        args.push(w.to_string());
        args.push("--height".to_string());
        args.push(h.to_string());
    }

    if let Some(custom_args) = &instance.game_args {
        args.extend(custom_args.split_whitespace().map(|s| s.to_string()));
    }

    // 全局「游戏参数」同样从前端设置里有，但以前没人读。
    let raw = crate::settings::load_raw_settings_sync();
    if let Some(global_game_args) = raw_text(&raw, &["game_args", "gameArgs"]) {
        if !global_game_args.trim().is_empty() {
            args.extend(global_game_args.split_whitespace().map(|s| s.to_string()));
        }
    }

    Ok(args)
}
