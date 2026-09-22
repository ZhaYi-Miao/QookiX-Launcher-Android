package com.zhayi.qookix

import android.Manifest
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.content.pm.ActivityInfo
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.OpenableColumns
import android.util.Log
import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import androidx.activity.OnBackPressedCallback
import androidx.activity.enableEdgeToEdge
import com.zhayi.qookix.nativebridge.PojavShim
import net.kdt.pojavlaunch.prefs.LauncherPreferences
import com.zhayi.qookix.tauri.TauriBridge
import org.json.JSONObject
import java.io.File

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    // 把 JavaVM 与本实例交给 Rust：之后方向锁定 / 文件选择落地 / 系统代理都由 Rust 回调本类
    TauriBridge.attach(this)
    // 给 Pojav 输入桥（org.lwjgl.glfw.CallbackBridge）准备剪贴板等平台能力
    PojavShim.init(this)

    applyStoredOrientation()
    requestNotificationPermission()
    disableWebViewZoom()
    setupBackNavigation()
  }

  /**
   * 关掉 WebView 的双指缩放。
   *
   * 为什么非做不可：启动器界面是按「可用高度」精确排版的（标题栏 + 内容 + 底部导航，
   * 列表内部自己滚）。一旦被双指放大，界面就会超出屏幕、按钮点不到，而且缩不回去。
   *
   * 为什么不能只靠 index.html 里的 `user-scalable=no`：wry 生成的 WebView 没有打开
   * `useWideViewPort`，这种情况下 WebView 并不采纳页面里的 viewport meta；
   * 真正决定能不能捏合的是 WebView 自己的 `settings.supportZoom`（默认 true）。
   *
   * 为什么写在这里而不是 RustWebView.kt：那个文件是 wry 自动生成的
   * （文件头写着 DO NOT MODIFY），每次构建都可能被覆盖；MainActivity 是我们自己维护的。
   * WebView 由 Rust 在 onCreate 之后创建，所以要重试查找。
   */
  /**
   * 返回键 / 返回手势。
   *
   * 为什么必须自己接：wry 自带的返回处理被 Tauri 模板显式关掉了
   * （`TauriActivity.handleBackNavigation = false`），项目此前也没注册回调 ——
   * 于是任何页面按返回都落到 Activity 默认行为 `finish()`，**整个启动器被关掉**。
   *
   * 顺序（同 Pojav `LauncherActivity.onBackPressed` 的「先退一层再退到底」）：
   *  1. 先问前端：派发可取消的 `qk-back`，前端做应用内后退时 `preventDefault()`；
   *  2. 前端没处理 → WebView 历史能退就 `goBack()`；
   *  3. 都不行 → 才 `finish()`。
   *
   * 不要改 `TauriActivity.kt`：那是生成物（文件头写着 DO NOT MODIFY）。
   */
  private fun setupBackNavigation() {
    onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
      override fun handleOnBackPressed() {
        Log.i(TAG, "返回键：dispatcher 通路")
        handleBack()
      }
    })
  }

  /**
   * 旧返回键通路（`onBackPressed`）。
   *
   * Android 13+ 走预测返回时框架**不再调用**它，只有声明了
   * `android:enableOnBackInvokedCallback="true"` 之后 dispatcher 才接管；
   * 这里保留一份兜底，两条通路最终都汇集到 [handleBack]。
   */
  @Deprecated("Deprecated in Java")
  override fun onBackPressed() {
    Log.i(TAG, "返回键：onBackPressed 通路")
    handleBack()
  }

  private var lastBackHandledAt = 0L

  /** 返回键的**唯一**处理入口（两条通路汇到这里，做 300ms 去重防止处理两次）。 */
  private fun handleBack() {
    val now = System.currentTimeMillis()
    if (now - lastBackHandledAt < 300) return
    lastBackHandledAt = now

    val wv = findWebView(window.decorView)
    Log.i(TAG, "返回键：webview=${wv != null}")
    if (wv == null) {
      finish()
      return
    }
    // 派发一个**可取消**的 qk-back：前端关掉弹层、或做应用内后退时会
    // preventDefault()，表示「我已经处理了」。
    wv.evaluateJavascript(
      "(function(){try{var e=new CustomEvent('qk-back',{cancelable:true});" +
        "window.dispatchEvent(e);return e.defaultPrevented?'handled':'pass';}" +
        "catch(err){return 'pass';}})()"
    ) { result ->
      Log.i(TAG, "返回键：前端结果=$result canGoBack=${wv.canGoBack()}")
      if (result != null && result.contains("handled")) return@evaluateJavascript
      if (wv.canGoBack()) wv.goBack() else finish()
    }
  }

  private fun disableWebViewZoom(attempt: Int = 0) {
    val wv = findWebView(window.decorView)
    if (wv == null) {
      if (attempt < 50) window.decorView.postDelayed({ disableWebViewZoom(attempt + 1) }, 100)
      return
    }
    try {
      // 注意：supportZoom 在 SDK 里只有 supportZoom() 取值器，Kotlin 不合成属性，
      // 必须按方法调用 setSupportZoom(...)；另外两个才能当属性赋值。
      wv.settings.setSupportZoom(false)
      wv.settings.builtInZoomControls = false
      wv.settings.displayZoomControls = false
      // 跟随系统字体放大会把按固定字号排版的界面撑破（标题栏/底栏溢出）。
      // 锁到 100%：界面缩放由应用自己的「界面缩放」设置负责。
      wv.settings.textZoom = 100
      // 前端资源全部来自 APK 内置 assets（tauri:// 本地协议），没有联网价值；
      // 而 WebView 会把 `tauri://localhost/index.html` 及带 hash 的 chunk 缓存下来，
      // 覆盖安装后仍然吐旧界面（实测：新关于页不出现、加载的是旧 chunk 文件名）。
      // 全部走网络加载语义（LOAD_NO_CACHE）就能根治这类「更新了却看不到」的问题。
      wv.settings.cacheMode = android.webkit.WebSettings.LOAD_NO_CACHE
      Log.i(TAG, "已关闭 WebView 缩放，并禁用资源缓存（LOAD_NO_CACHE）")
    } catch (e: Exception) {
      Log.w(TAG, "关闭 WebView 缩放失败", e)
    }
  }

  /**
   * 读取移植过来的 Pojav 控制层设置。
   *
   * 为什么需要这座桥：Pojav 的那些设置（按钮大小、鼠标速度、**渲染分辨率缩放**、
   * 忽略刘海、长按判定、陀螺仪、禁用手势、备选渲染表面…）全部是
   * `LauncherPreferences` 从 `SharedPreferences("launcher_preferences")` 读的，
   * 而 QookiX 自己的设置存在 `files/settings.json`（Rust 侧）—— 两边不通，
   * 于是这些项永远停在默认值，用户根本没得调。
   * 这里把整份 SharedPreferences 以 JSON 读出来给前端做界面。
   */
  /**
   * 预加载 JRE 自带的原生库（主要给 JDK 8 用）。
   *
   * 为什么需要：Android 的 linker 只按**命名空间里的路径**解析裸文件名，
   * 而应用私有目录（files/runtimes/...）不在其中。JDK 8 的原生库互相依赖用的是
   * 裸名字（libnio.so → libnet.so），于是 JVM 刚跑 main 就
   * `UnsatisfiedLinkError: library "libnet.so" not found`。
   * 而 bionic 在解析 DT_NEEDED 时会**先在已加载的库里按 soname 匹配**，
   * 所以这里用 System.load(绝对路径) 把它们提前加载进来即可。
   *
   * 库之间有依赖，顺序无法预知，因此多轮尝试：一轮里没有任何新库加载成功就停。
   * 加载失败**不抛异常**（依赖未就绪很正常，下一轮再试）。
   */
  fun preloadJreLibs(jreHome: String): Int {
    val dirs = mutableListOf<java.io.File>()
    for (arch in arrayOf("aarch64", "arm64", "arm", "arm32", "x86_64", "x86")) {
      dirs.add(java.io.File(jreHome, "lib/$arch"))
      dirs.add(java.io.File(jreHome, "lib/$arch/jli"))
      dirs.add(java.io.File(jreHome, "lib/$arch/server"))
    }
    dirs.add(java.io.File(jreHome, "lib"))
    dirs.add(java.io.File(jreHome, "lib/server"))
    dirs.add(java.io.File(jreHome, "lib/jli"))

    val pending = mutableListOf<java.io.File>()
    for (d in dirs) {
      d.listFiles { f -> f.isFile && f.name.endsWith(".so") }?.let { pending.addAll(it) }
    }
    // jar 里可能带同名库，去重
    val uniq = pending.distinctBy { it.absolutePath }

    var loaded = 0
    var progress = true
    var rounds = 0
    val still = uniq.toMutableList()
    while (progress && rounds < 8) {
      progress = false
      rounds++
      val it = still.iterator()
      while (it.hasNext()) {
        val f = it.next()
        try {
          System.load(f.absolutePath)
          loaded++
          it.remove()
          progress = true
        } catch (e: Throwable) {
          // 依赖还没加载，下一轮再试
        }
      }
    }
    Log.i(TAG, "预加载 JRE 原生库：成功 $loaded 个，仍失败 ${still.size} 个（$rounds 轮）")
    return loaded
  }
  fun readPojavPrefs(): String {
    return try {
      val sp = getSharedPreferences(PREFS_POJAV, MODE_PRIVATE)
      val obj = JSONObject()
      for ((k, v) in sp.all) obj.put(k, v)
      obj.toString()
    } catch (e: Exception) {
      Log.w(TAG, "读取 Pojav 设置失败", e)
      "{}"
    }
  }

  /**
   * 写入 Pojav 设置并**立刻刷新内存里的静态字段**。
   *
   * 只写 SharedPreferences 是不够的：`LauncherPreferences` 在启动时读一次就缓存成
   * 静态字段（`PREF_SCALE_FACTOR` 等），游戏内各处读的都是缓存值。
   * 写完必须再调一次 `loadPreferences`，否则要等下次冷启动才生效。
   */
  fun writePojavPrefs(json: String) {
    try {
      val sp = getSharedPreferences(PREFS_POJAV, MODE_PRIVATE)
      val obj = JSONObject(json)
      val editor = sp.edit()
      for (key in obj.keys()) {
        when (val v = obj.get(key)) {
          is Boolean -> editor.putBoolean(key, v)
          is Int -> editor.putInt(key, v)
          is Long -> editor.putLong(key, v)
          is Double -> editor.putFloat(key, v.toFloat())
          is String -> editor.putString(key, v)
          else -> {}
        }
      }
      editor.apply()
      // loadPreferences 只认 `DEFAULT_PREF`，启动器进程里它通常还是 null ——
      // 不先赋值的话 loadPreferences 会直接返回（日志里那句
      // 「DEFAULT_PREF 为空，使用默认值」就是这个），静态字段根本没刷新。
      LauncherPreferences.DEFAULT_PREF = sp
      LauncherPreferences.loadPreferences(this)
      Log.i(TAG, "已写入 Pojav 设置：${obj.keys().asSequence().joinToString(",")}")
    } catch (e: Exception) {
      Log.w(TAG, "写入 Pojav 设置失败", e)
    }
  }

  private fun findWebView(v: View): WebView? {
    if (v is WebView) return v
    if (v is ViewGroup) {
      for (i in 0 until v.childCount) {
        findWebView(v.getChildAt(i))?.let { return it }
      }
    }
    return null
  }

  /**
   * Android 13+ 前台服务的通知需要用户授权才会显示。不申请的话，游戏运行时
   * 通知栏里那条「停止」通知根本看不到，用户就没有正常途径结束游戏。
   */
  private fun requestNotificationPermission() {
    if (Build.VERSION.SDK_INT < 33) return
    if (checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED) return
    try {
      requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1001)
    } catch (e: Exception) {
      Log.w(TAG, "申请通知权限失败", e)
    }
  }

  /**
   * 拉起游戏界面（由 Rust 的 launch_game 命令调用）。
   *
   * 游戏必须跑在 [GameActivity] 里：它挂着 GameSurfaceView，GL4ES 才有真正的 Surface 可画。
   * 真正的 JVM 启动由 GameActivity → TauriBridge.launchGame 触发。
   */
  fun startGameActivity(instanceId: String, accountUuid: String) {
    runOnUiThread {
      try {
        GameActivity.start(this, instanceId, accountUuid)
      } catch (e: Exception) {
        Log.e(TAG, "启动游戏界面失败", e)
      }
    }
  }

  /** 启动时读取 settings.json 里的方向设置并应用，让上次的选择立刻生效。 */
  private fun applyStoredOrientation() {
    try {
      val file = File(filesDir, "settings.json")
      if (!file.isFile) return
      val mode = JSONObject(file.readText()).optString("orientation", "landscape")
      setOrientation(mode)
    } catch (e: Exception) {
      Log.w(TAG, "读取屏幕方向设置失败", e)
    }
  }

  /** 锁定 / 跟随屏幕方向。可在任意线程调用。 */
  fun setOrientation(mode: String) {
    val requested = when (mode) {
      "portrait" -> ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
      "landscape" -> ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
      // 跟随系统用 USER：清单里写的是 sensorLandscape，
      // UNSPECIFIED 会继续沿用清单值而无法真正“跟随系统”。
      else -> ActivityInfo.SCREEN_ORIENTATION_USER
    }
    runOnUiThread {
      if (requestedOrientation != requested) {
        requestedOrientation = requested
      }
    }
  }

  /**
   * 把文件选择器返回的 `content://` URI 复制到应用缓存目录并返回真实路径。
   * Rust 侧的所有文件命令都只认普通路径，无法直接读取 content URI。
   */
  fun resolvePickedUri(uriOrPath: String): String? {
    if (!uriOrPath.startsWith("content://")) {
      return uriOrPath
    }
    return try {
      val uri = Uri.parse(uriOrPath)

      // 文件名完全由外部 provider 提供（DISPLAY_NAME），**必须净化**：
      // 形如 "../../files/settings.json" 的会被 `File(dir, name)` 规约到应用私有
      // 目录并覆盖 settings.json（里面存着账号 token / 代理设置）—— 路径穿越漏洞。
      val raw = queryDisplayName(uri) ?: "picked"
      val safeName = sanitizeFileName(raw)
      if (safeName.isEmpty()) {
        Log.e(TAG, "拒绝空文件名: $raw")
        return null
      }

      // 落在 <files>/picked/ 而不是 cacheDir：cache 会被系统回收，导入后引用成死链。
      val dir = File(filesDir, "picked").apply { mkdirs() }
      val dest = File(dir, "${System.currentTimeMillis()}_$safeName")

      // 二次校验：规约后的路径必须仍在目标目录内
      val canonicalDir = dir.canonicalPath + File.separator
      if (!dest.canonicalPath.startsWith(canonicalDir)) {
        Log.e(TAG, "拒绝可疑文件名（路径穿越）: $raw")
        return null
      }

      contentResolver.openInputStream(uri).use { input ->
        if (input == null) return null
        dest.outputStream().use { output -> input.copyTo(output) }
      }
      dest.absolutePath
    } catch (e: Exception) {
      Log.e(TAG, "解析所选文件失败: $uriOrPath", e)
      null
    }
  }

  /**
   * 读取系统剪贴板文本。
   *
   * 为什么必须走原生：代码编辑器的「粘贴」在 WebView 里两条路都是死的 ——
   *   ① `navigator.clipboard.readText()` 需要 `clipboard-read` 权限，wry 没有授予 → 抛错；
   *   ② `document.execCommand("paste")` 在 Chromium 里按设计**永远返回 false**。
   * 于是点了「粘贴」既不报错也不插入内容。这里用 ClipboardManager 直接读。
   *
   * 返回 null 表示剪贴板为空或读取失败 —— 调用方应据此提示用户，
   * 而不是像以前那样静默什么都不发生。
   */
  fun readClipboardText(): String? {
    return try {
      val cm = getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
      if (cm == null || !cm.hasPrimaryClip()) return null
      val clip = cm.primaryClip ?: return null
      if (clip.itemCount == 0) return null
      clip.getItemAt(0).coerceToText(this)?.toString()
    } catch (e: Throwable) {
      // Android 10+ 只有前台应用能读剪贴板；极少数机型/时机下仍会抛。
      Log.w(TAG, "读取剪贴板失败", e)
      null
    }
  }

  /**
   * 写入系统剪贴板。与 [readClipboardText] 对称。
   *
   * 实测 WebView 连 `clipboard-write` 权限也没授予：
   * `navigator.clipboard.writeText()` 抛 `NotAllowedError: Write permission denied`。
   * 所以编辑器的「复制」以前只能靠 `execCommand("copy")` 兜底 —— 那条路依赖用户手势，
   * 从工具栏点击时勉强能成，但不该把唯一入口押在兜底上。
   *
   * 返回约定：成功返回**空串**，失败返回错误信息（同 `writeTextToUri`）。
   */
  fun writeClipboardText(text: String): String {
    return try {
      val cm = getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
        ?: return "系统剪贴板不可用"
      cm.setPrimaryClip(ClipData.newPlainText("QookiX", text))
      ""
    } catch (e: Throwable) {
      Log.w(TAG, "写入剪贴板失败", e)
      e.message ?: e.toString()
    }
  }

  /**
   * 把文本写回「另存为」选中的目标。
   *
   * 为什么必须走原生：安卓的文件选择器（`ACTION_CREATE_DOCUMENT`）返回的是 SAF 的
   * `content://` URI，不是文件路径 —— Rust 侧对它做 `fs::write` 必然失败，
   * 表现就是「导出日志/配置点了没反应或直接报错」。写入只能由 ContentResolver 完成。
   *
   * 返回约定：成功返回**空串**；失败返回错误信息（给用户看，所以不带堆栈）。
   */
  fun writeTextToUri(uriOrPath: String, content: String): String {
    return try {
      if (!uriOrPath.startsWith("content://")) {
        val f = File(uriOrPath)
        f.parentFile?.mkdirs()
        f.writeText(content)
        return ""
      }
      val uri = Uri.parse(uriOrPath)
      // 模式必须是 "wt"（write + truncate）：只写 "w" 时部分 provider 会在原内容
      // 后面追加，导出的日志会变成「旧内容 + 新内容」两份。
      val out = contentResolver.openOutputStream(uri, "wt")
        ?: return "无法打开目标文件（系统未授予写权限？）"
      out.use { os ->
        os.write(content.toByteArray(Charsets.UTF_8))
        os.flush()
      }
      ""
    } catch (e: Throwable) {
      Log.w(TAG, "写入所选位置失败: $uriOrPath", e)
      e.message ?: e.toString()
    }
  }

  /** 只保留纯文件名：去掉任何目录部分，非法字符替换为 `_`。 */
  private fun sanitizeFileName(name: String): String {
    val base = name.substringAfterLast('/').substringAfterLast('\\')
    return base
      .map { ch -> if (ch.isLetterOrDigit() || ch in "._- ()[]（）") ch else '_' }
      .joinToString("")
      .take(120)
  }

  private fun queryDisplayName(uri: Uri): String? {
    return try {
      contentResolver
        .query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
        ?.use { cursor ->
          if (!cursor.moveToFirst()) return@use null
          val idx = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
          if (idx >= 0) cursor.getString(idx) else null
        }
    } catch (e: Exception) {
      null
    }
  }

  /**
   * 返回当前系统（含 VPN / Wi-Fi）下发的 HTTP 代理，形如 `http://127.0.0.1:7890`。
   *
   * 原生 socket 不会自动走系统代理，只有把这里拿到的地址显式交给 HTTP 客户端，
   * 设置里的「系统代理」才和 WebView 的联网行为一致（Clash / v2ray 等 TUN 环境下尤其关键）。
   */
  fun systemProxy(): String? {
    // VPN（Clash / v2ray 的 TUN 模式）已经接管整机流量，再叠加系统 HTTP 代理
    // 就是**双重代理**：实测表现为「网络抽风」——版本清单超时、下载半途失败，
    // 而且时好时坏极难排查（用户设备上就是这个状态）。
    // VPN 在跑就直接返回 null，让请求走 TUN。
    if (isVpnActive()) {
      Log.i(TAG, "检测到 VPN 生效，忽略系统 HTTP 代理（避免双重代理）")
      return null
    }
    try {
      java.net.ProxySelector.getDefault()?.let { selector ->
        val proxies = selector.select(java.net.URI("https://piston-meta.mojang.com/"))
        for (p in proxies) {
          val addr = p.address() as? java.net.InetSocketAddress ?: continue
          val host = addr.hostString
          if (host.isNullOrEmpty()) continue
          when (p.type()) {
            java.net.Proxy.Type.HTTP -> return "http://$host:${addr.port}"
            java.net.Proxy.Type.SOCKS -> return "socks5://$host:${addr.port}"
            else -> {}
          }
        }
      }
    } catch (e: Exception) {
      Log.w(TAG, "读取系统代理失败", e)
    }
    // 兜底：框架会依据默认网络为 java.net 设置这两个属性
    val host = System.getProperty("http.proxyHost")
    if (!host.isNullOrEmpty()) {
      val port = System.getProperty("http.proxyPort")?.toIntOrNull() ?: 80
      return "http://$host:$port"
    }
    return null
  }

  /** 当前是否有 VPN 生效（含 Clash 的 TUN 模式）。 */
  private fun isVpnActive(): Boolean {
    return try {
      val cm = getSystemService(Context.CONNECTIVITY_SERVICE) as? ConnectivityManager ?: return false
      var found = false
      cm.activeNetwork?.let { n ->
        if (cm.getNetworkCapabilities(n)?.hasTransport(NetworkCapabilities.TRANSPORT_VPN) == true) {
          found = true
        }
      }
      if (!found) {
        // VPN 有时不是 activeNetwork，扫一遍全部网络兜底
        found = cm.allNetworks.any { n ->
          cm.getNetworkCapabilities(n)?.hasTransport(NetworkCapabilities.TRANSPORT_VPN) == true
        }
      }
      found
    } catch (e: Throwable) {
      Log.w(TAG, "检测 VPN 状态失败", e)
      false
    }
  }

  private companion object {
    const val TAG = "MainActivity"

    /** Pojav 控制层读的那份 SharedPreferences 文件名（LauncherPreferences 里定的）。 */
    const val PREFS_POJAV = "launcher_preferences"
  }
}
