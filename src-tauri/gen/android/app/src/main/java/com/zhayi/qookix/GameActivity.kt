package com.zhayi.qookix

import android.content.Context
import android.content.Intent
import android.content.res.Configuration
import android.graphics.drawable.ColorDrawable
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.view.KeyEvent
import android.view.View
import android.view.WindowManager
import android.widget.AdapterView
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ListView
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.app.AlertDialog
import androidx.activity.OnBackPressedCallback
import androidx.core.content.ContextCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.drawerlayout.widget.DrawerLayout
import androidx.lifecycle.lifecycleScope
import com.kdt.LoggerView
import com.zhayi.qookix.control.MenuEntry
import com.zhayi.qookix.control.QookixMenuAdapter
import com.zhayi.qookix.services.GameService
import com.zhayi.qookix.tauri.TauriBridge
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.kdt.pojavlaunch.EfficientAndroidLWJGLKeycode
import net.kdt.pojavlaunch.Logger
import net.kdt.pojavlaunch.LwjglGlfwKeycode
import net.kdt.pojavlaunch.MinecraftGLSurface
import net.kdt.pojavlaunch.customcontrols.ControlButtonMenuListener
import net.kdt.pojavlaunch.customcontrols.ControlData
import net.kdt.pojavlaunch.customcontrols.ControlDrawerData
import net.kdt.pojavlaunch.customcontrols.ControlJoystickData
import net.kdt.pojavlaunch.customcontrols.ControlLayout
import net.kdt.pojavlaunch.customcontrols.CustomControls
import net.kdt.pojavlaunch.customcontrols.EditorExitable
import net.kdt.pojavlaunch.customcontrols.handleview.DrawerPullButton
import net.kdt.pojavlaunch.customcontrols.keyboard.LwjglCharSender
import net.kdt.pojavlaunch.customcontrols.keyboard.TouchCharInput
import net.kdt.pojavlaunch.customcontrols.mouse.GyroControl
import net.kdt.pojavlaunch.customcontrols.mouse.HotbarView
import net.kdt.pojavlaunch.customcontrols.mouse.Touchpad
import net.kdt.pojavlaunch.prefs.LauncherPreferences
import net.kdt.pojavlaunch.prefs.QuickSettingSideDialog
import net.kdt.pojavlaunch.utils.MCOptionUtils
import org.lwjgl.glfw.CallbackBridge
import java.io.File
import java.io.IOException

/**
 * 游戏运行界面 —— **进入游戏之后的逻辑完全照 PojavLauncher 的 `MainActivity`**。
 *
 * 视图层级、事件分发、抽屉菜单、生命周期处理全部沿用 Pojav 的实现（见
 * `res/layout/activity_basemain.xml` 与上游 `net.kdt.pojavlaunch.MainActivity`），
 * 与 QookiX 自研版本的关键差异：
 *
 * | 环节 | 旧的自研实现 | 现在（= Pojav） |
 * |---|---|---|
 * | 渲染载体 | `TextureView`（`GameSurfaceView`） | `MinecraftGLSurface`：内部自建 `SurfaceView`/`TextureView` 并加进 `ControlLayout` |
 * | 触摸 | 手写手势（点按/长按/双指） | `InGUIEventProcessor` / `InGameEventProcessor` + `Touchpad` + `AndroidPointerCapture` |
 * | 虚拟控件 | `VirtualControlsView` + 自研 JSON | `ControlLayout` + `ControlData`（Pojav 控制布局 v8 格式，支持摇杆/抽屉/导入导出） |
 * | 软键盘 | 无 | `TouchCharInput` + `LwjglCharSender` |
 * | 游戏内菜单 | `MaterialAlertDialog` 列表 | 右侧 `DrawerLayout` + `menu_ingame`（含快速设置侧栏与就地编辑器） |
 * | 窗口尺寸 | 只在 Surface 创建时同步一次 | `refreshSize()`：创建时 + 尺寸变化 + 回前台 500ms 后重算 |
 *
 * 本类同时充当 Pojav 里 `MainActivity` 的静态宿主角色：
 * `touchCharInput` / `switchKeyboardState()` / `toggleMouse()` 由控制层的
 * `ControlButton`、`MinecraftGLSurface` 直接调用。
 */
class GameActivity : AppCompatActivity(), ControlButtonMenuListener, EditorExitable {

    companion object {
        private const val EXTRA_INSTANCE_ID = "instance_id"
        private const val EXTRA_ACCOUNT_UUID = "account_uuid"

        /**
         * SharedPreferences 名，与 Pojav 一致，偏好文件可直接沿用。
         *
         * **必须与 `MainActivity.PREFS_POJAV` 完全一致**（都是 `launcher_preferences`）。
         * 这里曾经写的是 `pojav_prefs`，与启动器设置页写的文件**不是同一个** ——
         * 后果是设置页里「渲染分辨率缩放 / 按钮大小 / 鼠标速度 / 渲染器…」写进去的值
         * 游戏侧完全读不到（`LauncherPreferences` 从本 Activity 指定的文件加载），
         * 表现为「改了不生效」；反过来游戏内快速设置改的值启动器也看不到（不同步）。
         */
        private const val PREF_NAME = "launcher_preferences"

        /** 历史遗留的偏好文件名，仅在迁移时读取，见 [migrateLegacyPrefs]。 */
        private const val LEGACY_PREF_NAME = "pojav_prefs"

        private const val TAG = "QookiXGame"

        /**
         * 内置控制布局的**视觉版本**。
         *
         * 默认布局写在 `<files>/controlmap/default.json`，只在文件缺失时写入 ——
         * 所以改版后老用户看不到新皮肤。这里用一个独立标记文件记录当前皮肤版本：
         * 版本不一致就**覆盖一次**内置 default.json（玩家自己另存/另选的布局文件不受影响，
         * 只有名为 default.json 的这一份会被刷新）。
         */
        private const val CONTROL_STYLE_VERSION = 2
        private const val STYLE_MARKER = ".qk-control-style"

        /** `R.array.menu_ingame` 六项对应的图标（顺序必须与数组一致）。 */
        private val GAME_MENU_ICONS = intArrayOf(
            R.drawable.ic_qk_power,     // 强制关闭
            R.drawable.ic_qk_log,       // 日志输出
            R.drawable.ic_qk_key,       // 发送自定义键码
            R.drawable.ic_qk_sliders,   // 快速设置
            R.drawable.ic_qk_gamepad,   // 自定义控制布局
            R.drawable.ic_qk_camera,    // 截图
        )

        /** `R.array.menu_customcontrol` 七项对应的图标（顺序必须与数组一致）。 */
        private val EDITOR_MENU_ICONS = intArrayOf(
            R.drawable.ic_qk_plus,      // 添加按键
            R.drawable.ic_qk_plus,      // 添加组合键
            R.drawable.ic_qk_plus,      // 添加摇杆
            R.drawable.ic_qk_folder,    // 加载
            R.drawable.ic_qk_save,      // 保存
            R.drawable.ic_qk_star,      // 选择默认控制布局
            R.drawable.ic_qk_close,     // 退出编辑器
        )

        /** 控制层持有它（`MinecraftGLSurface.processKeyEvent` 转发软键盘事件用）。 */
        @JvmField
        var touchCharInput: TouchCharInput? = null

        /** 控制层持有它（`MainActivity.toggleMouse` 的等价物）。 */
        @JvmField
        var touchpad: Touchpad? = null

        /** Pojav `MainActivity.switchKeyboardState()` 的等价物。 */
        @JvmStatic
        fun switchKeyboardState() {
            touchCharInput?.switchKeyboardState()
        }

        /** Pojav `MainActivity.toggleMouse(Context)` 的等价物。 */
        @JvmStatic
        fun toggleMouse(ctx: Context) {
            // 游戏抓着鼠标时虚拟鼠标没有意义（Pojav 同样直接 return）
            if (CallbackBridge.isGrabbing()) return
            val showing = touchpad?.switchState() ?: false
            Toast.makeText(
                ctx,
                if (showing) R.string.control_mouseon else R.string.control_mouseoff,
                Toast.LENGTH_SHORT
            ).show()
        }

        fun start(context: Context, instanceId: String, accountUuid: String) {
            val intent = Intent(context, GameActivity::class.java).apply {
                putExtra(EXTRA_INSTANCE_ID, instanceId)
                putExtra(EXTRA_ACCOUNT_UUID, accountUuid)
                // NEW_DOCUMENT + intoRecents：游戏在多任务里占一张**独立卡片**，
                // 启动器自己的卡片仍在 —— 玩家可以随时切回启动器（页面/状态原样保留），
                // 再从多任务切回游戏。之前两张界面挤在同一个任务里，
                // 多任务里只剩游戏一张卡，启动器「消失」了（用户反馈）。
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_NEW_DOCUMENT)
            }
            context.startActivity(intent)
        }
    }

    private var instanceId: String = ""
    private var accountUuid: String = ""

    private lateinit var mControlLayout: ControlLayout
    private lateinit var minecraftGLView: MinecraftGLSurface
    private lateinit var loggerView: LoggerView
    private lateinit var drawerLayout: DrawerLayout
    /** 抽屉里真正承载列表的 ListView（id 沿用 Pojav 的 main_navigation_view）。 */
    private lateinit var navDrawer: ListView
    /** 抽屉容器：它才是 DrawerLayout 的直接子 View，openDrawer() 必须传它。 */
    private lateinit var navContainer: LinearLayout
    private lateinit var menuTitle: TextView
    private lateinit var menuSubtitle: TextView
    private lateinit var mDrawerPullButton: DrawerPullButton
    private lateinit var mHotbarView: HotbarView
    private var mGyroControl: GyroControl? = null
    private var mQuickSettingSideDialog: QuickSettingSideDialog? = null

    private lateinit var gameMenuAdapter: QookixMenuAdapter
    private lateinit var gameActionClickListener: AdapterView.OnItemClickListener
    private var editorMenuAdapter: QookixMenuAdapter? = null
    private var editorMenuListener: AdapterView.OnItemClickListener? = null

    private var isInEditor = false
    private var gameStarted = false

    // ------------------------------------------------------------------ 生命周期

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        instanceId = intent.getStringExtra(EXTRA_INSTANCE_ID) ?: ""
        accountUuid = intent.getStringExtra(EXTRA_ACCOUNT_UUID) ?: ""

        // 偏好：控制层（ControlData / Touchpad / QuickSettingSideDialog）直接读这些静态字段
        migrateLegacyPrefs()
        LauncherPreferences.DEFAULT_PREF = getSharedPreferences(PREF_NAME, MODE_PRIVATE)
        LauncherPreferences.loadPreferences(this)

        // 沉浸式全屏必须在 setContentView 之前：View 尺寸在这里定下来，
        // 而游戏窗口尺寸随后由 MinecraftGLSurface.refreshSize() 取 View 尺寸算出。
        applyImmersiveMode()
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)

        // 游戏目录（options.txt 读写用），与 Rust 侧 instances::get_game_dir 一致
        val gameDir = File(filesDir, "instances/$instanceId")
        net.kdt.pojavlaunch.Tools.DIR_GAME_NEW = gameDir.absolutePath
        MCOptionUtils.load(gameDir.absolutePath)

        // 尽早拉起前台服务，避免切后台被 lowmemorykiller 杀掉
        GameService.start(this)

        setContentView(R.layout.activity_basemain)
        bindValues()

        // ── 控制层接线（照 Pojav MainActivity.onCreate）──────────────────────
        CallbackBridge.addGrabListener(touchpad)
        CallbackBridge.addGrabListener(minecraftGLView)

        mGyroControl = GyroControl(this)

        // TextureView 上设置透明窗口背景会渲染成一片白，Pojav 同款判断
        if (LauncherPreferences.PREF_USE_ALTERNATE_SURFACE) {
            window.setBackgroundDrawable(null)
        } else {
            window.setBackgroundDrawable(ColorDrawable(android.graphics.Color.BLACK))
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            window.setSustainedPerformanceMode(LauncherPreferences.PREF_SUSTAINED_PERFORMANCE)
        }

        // 就地控制编辑器（抽屉里点「自定义控制布局」后切换过去）
        editorMenuAdapter = QookixMenuAdapter(
            this,
            entriesOf(R.array.menu_customcontrol, EDITOR_MENU_ICONS)
        )
        editorMenuListener = AdapterView.OnItemClickListener { _, _, position, _ ->
            when (position) {
                0 -> mControlLayout.addControlButton(ControlData("New"))
                1 -> mControlLayout.addDrawer(ControlDrawerData())
                2 -> mControlLayout.addJoystickButton(ControlJoystickData())
                3 -> mControlLayout.openLoadDialog()
                4 -> mControlLayout.openSaveDialog(this)
                5 -> mControlLayout.openSetDefaultDialog()
                6 -> mControlLayout.openExitDialog(this)
            }
        }

        // 快捷栏缩放跟随游戏内 GUI Scale 选项
        MCOptionUtils.addMCOptionListener { MCOptionUtils.getMcScale() }
        mControlLayout.setModifiable(false)

        mControlLayout.setMenuListener(this)
        mDrawerPullButton.setOnClickListener { onClickedMenu() }
        // 只允许通过「下拉按钮 / 菜单键」打开抽屉：否则屏幕边缘滑动会误触
        drawerLayout.setDrawerLockMode(DrawerLayout.LOCK_MODE_LOCKED_CLOSED)

        // 返回键兜底：**绝不 finish 这个界面**。
        //
        // Pojav 只在 dispatchKeyEvent 里把返回键映射成 GLFW_KEY_ESCAPE；如果那条路径
        // 因为任何原因没消费掉事件（软键盘 enabled 状态、预测式返回手势、对话框刚关闭…），
        // AppCompatActivity 的默认实现就会 finish 掉本 Activity ——
        // 留下「游戏 JVM 还在跑、界面却没了」的进程，多任务里看不到它，
        // 再点启动又会被判成「已在运行」。这里显式接管，行为与 Pojav 一致（发 Esc）。
        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                if (isInEditor) {
                    mControlLayout.askToExit(this@GameActivity)
                    return
                }
                CallbackBridge.sendKeyPress(LwjglGlfwKeycode.GLFW_KEY_ESCAPE.toInt())
            }
        })

        // latestlog.txt 只留给 Logger.appendToLog（未找到调用方，但保留以免上游代码初始化失败）。
        // 注意：**这里不再重定向 fd1/2** —— 那种做法会与 Rust 的 redirect_output 争抢 fd，
        // 结果两边都不完整（游戏内日志面板恒空）。fd 重定向统一由 Rust 负责，
        // 写进 files/logs/launch-<实例>.log，游戏内面板直接跟随该文件。
        try {
            val logFile = File(filesDir, "logs/latestlog.txt")
            logFile.parentFile?.mkdirs()
            if (!logFile.exists() && !logFile.createNewFile()) {
                throw IOException("Failed to create a new log file")
            }
            Logger.begin(logFile.absolutePath)
        } catch (e: Throwable) {
            android.util.Log.w("GameActivity", "日志文件初始化失败，游戏内日志面板将为空", e)
        }
        touchCharInput?.setCharacterSender(LwjglCharSender())

        // 1.13+ 必须开输入队列模式，且要在游戏启动前（Pojav: isInputStackCall = versionInfo.arguments != null）
        try {
            CallbackBridge.nativeSetUseInputStackQueue(true)
        } catch (e: Throwable) {
            android.util.Log.w("GameActivity", "nativeSetUseInputStackQueue 失败", e)
        }

        // ── 游戏内菜单（右侧抽屉）────────────────────────────────────────────
        gameMenuAdapter = QookixMenuAdapter(
            this,
            entriesOf(R.array.menu_ingame, GAME_MENU_ICONS)
        )
        gameActionClickListener = AdapterView.OnItemClickListener { _, _, position, _ ->
            when (position) {
                0 -> dialogForceClose()
                1 -> openLogOutput()
                2 -> dialogSendCustomKey()
                3 -> openQuickSettings()
                4 -> openCustomControls()
                5 -> takeScreenshot()
            }
            drawerLayout.closeDrawers()
        }
        navDrawer.adapter = gameMenuAdapter
        navDrawer.onItemClickListener = gameActionClickListener
        drawerLayout.closeDrawers()

        // ── Surface 就绪 → 启动 JVM ─────────────────────────────────────────
        minecraftGLView.setSurfaceReadyListener {
            // 虚拟鼠标按偏好预开启（Pojav: 启动前 setup virtual mouse）
            if (LauncherPreferences.PREF_VIRTUAL_MOUSE_START) {
                touchpad?.post { touchpad?.switchState() }
            }
            startGameOnce()
        }

        // isAlreadyRunning：JVM 可能还在后台跑（用户之前退到启动器但没杀进程），
        // 这时只重绑 Surface，不再启动一次。Pojav 用 GameService.LocalBinder.isActive 判断，
        // QookiX 直接问 Rust 侧的游戏状态。
        val alreadyRunning = TauriBridge.isGameRunning()
        touchpad?.let { minecraftGLView.start(alreadyRunning, it) }
            ?: run {
                android.util.Log.e("GameActivity", "touchpad 未绑定，无法启动渲染面")
                finish()
            }
    }

    /** 视图绑定（Pojav `MainActivity.bindValues` 的等价物，id 完全沿用上游布局）。 */
    private fun bindValues() {
        mControlLayout = findViewById(R.id.main_control_layout)
        minecraftGLView = findViewById(R.id.main_game_render_view)
        touchpad = findViewById(R.id.main_touchpad)
        drawerLayout = findViewById(R.id.main_drawer_options)
        navDrawer = findViewById(R.id.main_navigation_view)
        navContainer = findViewById(R.id.main_nav_container)
        menuTitle = findViewById(R.id.qk_menu_title)
        menuSubtitle = findViewById(R.id.qk_menu_subtitle)
        loggerView = findViewById(R.id.mainLoggerView)
        // 游戏内「日志输出」面板跟随 **Rust** 写的启动日志。
        // 不要再改回 `Logger.setLogListener` —— 那套走 fd1/2 自建管道，会被 Rust 的
        // redirect_output 顶掉写端，回调永远不触发（面板恒空）。详见 LoggerView 的类注释。
        loggerView.setLogFile(File(filesDir, "logs/launch-$instanceId.log"))
        touchCharInput = findViewById(R.id.mainTouchCharInput)
        mDrawerPullButton = findViewById(R.id.drawer_button)
        mHotbarView = findViewById(R.id.hotbar_view)
    }

    /**
     * 把 Pojav 的字符串数组与 QookiX 的图标一一配对。
     *
     * 文案仍取上游数组（`menu_ingame` / `menu_customcontrol`，含 zh-rCN 翻译），
     * 这样点击回调的 position 语义与 Pojav 完全一致。
     */
    private fun entriesOf(arrayRes: Int, icons: IntArray): List<MenuEntry> =
        QookixMenuAdapter.entriesOf(this, arrayRes, icons)

    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        // 布局完成后再取显示度量，否则刘海/窗口尺寸都是错的
        LauncherPreferences.computeNotchSize(this)
        mControlLayout.post {
            net.kdt.pojavlaunch.Tools.getDisplayMetrics(this)
            loadControls()
        }
    }

    /**
     * 加载控制布局。默认走 `LauncherPreferences.PREF_DEFAULTCTRL_PATH`
     * （首次运行 / 皮肤版本升级时由 [seedDefaultControlLayout] 写入内置布局）；
     * 失败则回落默认布局，绝不因为一个坏 JSON 就起不来游戏。
     */
    private fun loadControls() {
        seedDefaultControlLayout()
        try {
            mControlLayout.loadLayout(LauncherPreferences.PREF_DEFAULTCTRL_PATH)
        } catch (e: IOException) {
            android.util.Log.w("GameActivity", "控制布局加载失败，改用默认布局", e)
            try {
                mControlLayout.loadLayout(net.kdt.pojavlaunch.Tools.CTRLDEF_FILE)
            } catch (io: Throwable) {
                net.kdt.pojavlaunch.Tools.showError(this, io)
            }
        } catch (th: Throwable) {
            net.kdt.pojavlaunch.Tools.showError(this, th)
        }
        // 布局自带「菜单」按钮时就不需要下拉按钮（Pojav 同款）
        mDrawerPullButton.visibility =
            if (mControlLayout.hasMenuButton()) View.GONE else View.VISIBLE
        mControlLayout.toggleControlVisible()
    }

    /**
     * 把内置默认控制布局写到 `<files>/controlmap/default.json`。
     *
     * 内容与 PojavLauncher 的 `assets/default.json` 位置一致，但换了 QookiX 的视觉：
     * 圆角、半透明深色填充 + 强调色描边、中文标签。
     *
     * **升级语义**：只在「文件不存在」或「皮肤版本变了」时写入，
     * 版本标记放在同目录的 [STYLE_MARKER] 里。因此玩家自己另存的控制布局不受影响，
     * 只有内置的 `default.json` 会被刷新一次。
     */
    private fun seedDefaultControlLayout() {
        val target = File(net.kdt.pojavlaunch.Tools.CTRLDEF_FILE)
        target.parentFile?.mkdirs()

        val marker = File(target.parentFile, STYLE_MARKER)
        val installed = try {
            marker.readText().trim()
        } catch (_: Throwable) {
            ""
        }
        val wanted = CONTROL_STYLE_VERSION.toString()

        if (target.isFile && target.length() > 0L && installed == wanted) return

        try {
            resources.openRawResource(R.raw.pojav_default_control).use { input ->
                target.outputStream().use { output -> input.copyTo(output) }
            }
            marker.writeText(wanted)
            android.util.Log.i(
                "GameActivity",
                "内置控制布局已更新到皮肤版本 $wanted（原版本 '${installed.ifEmpty { "无" }}'）"
            )
        } catch (e: Throwable) {
            android.util.Log.w("GameActivity", "默认控制布局写入失败", e)
        }
    }

    // ------------------------------------------------------------------ 游戏启动

    private fun startGameOnce() {
        if (gameStarted) return
        gameStarted = true

        if (TauriBridge.isGameRunning()) {
            android.util.Log.i("GameActivity", "JVM 已在运行，仅重新绑定 Surface")
            return
        }

        // Rust 侧启动 JVM 时用这个尺寸当 glfwstub 的游戏窗口尺寸，
        // 与 refreshSize() 写进 CallbackBridge 的值必须一致，否则触摸坐标整体错位。
        TauriBridge.setSurfaceSize(CallbackBridge.windowWidth, CallbackBridge.windowHeight)

        lifecycleScope.launch {
            val exitCode = withContext(Dispatchers.IO) {
                TauriBridge.launchGame(instanceId, accountUuid)
            }
            if (exitCode != 0) showCrashDialog(exitCode) else finish()
        }
    }

    // ------------------------------------------------------------------ 游戏内菜单

    /** 抽屉入口：Pojav `MainActivity.onClickedMenu()`（容器才是 DrawerLayout 的抽屉 View）。 */
    override fun onClickedMenu() {
        drawerLayout.openDrawer(navContainer)
        navContainer.requestLayout()
    }

    /**
     * 游戏内截图。
     *
     * 为什么需要：手机上没有 F2，抽屉里也没有入口 —— 于是 `<实例目录>/screenshots/`
     * 永远是空的，实例详情里的「截图」tab 永远没东西看。
     * 这里等价于按一下 F2：Minecraft 自己的截图逻辑会把图写进 screenshots/。
     *
     * 走 `CallbackBridge.sendKeyPress` 而不是自己去抓帧/写文件，是为了复用游戏内
     * 已经绑定的截图键位（玩家改过键位也照样有效），也不用关心渲染后端是 GL4ES 还是 Vulkan。
     */
    private fun takeScreenshot() {
        try {
            // GLFW_KEY_F2 在 LwjglGlfwKeycode 里是 short，sendKeyPress 收 int
            CallbackBridge.sendKeyPress(net.kdt.pojavlaunch.LwjglGlfwKeycode.GLFW_KEY_F2.toInt())
            Toast.makeText(
                this,
                "已触发截图（保存在实例目录的 screenshots 文件夹）",
                Toast.LENGTH_SHORT
            ).show()
        } catch (e: Throwable) {
            android.util.Log.w("GameActivity", "触发截图失败", e)
            Toast.makeText(this, "截图失败：${e.message}", Toast.LENGTH_SHORT).show()
        }
    }

    private fun openLogOutput() {
        loggerView.visibility = View.VISIBLE
    }

    /** Pojav `MainActivity.dialogSendCustomKey()`：把按键表列出来直接发。 */
    private fun dialogSendCustomKey() {
        AlertDialog.Builder(this, R.style.QookixAlertDialog)
            .setTitle(R.string.control_customkey)
            .setItems(EfficientAndroidLWJGLKeycode.generateKeyName()) { _, position ->
                EfficientAndroidLWJGLKeycode.execKeyIndex(position)
            }
            .show()
    }

    /** Pojav `MainActivity.openQuickSettings()`：右侧滑出的快速设置面板。 */
    private fun openQuickSettings() {
        if (mQuickSettingSideDialog == null) {
            mQuickSettingSideDialog = object : QuickSettingSideDialog(this, mControlLayout) {
                override fun onResolutionChanged() {
                    minecraftGLView.refreshSize()
                    mHotbarView.onResolutionChanged()
                }

                override fun onGyroStateChanged() {
                    mGyroControl?.updateOrientation()
                    if (LauncherPreferences.PREF_ENABLE_GYRO) mGyroControl?.enable()
                    else mGyroControl?.disable()
                }
            }
        }
        mQuickSettingSideDialog?.appear(true)
    }

    /** Pojav `MainActivity.openCustomControls()`：把抽屉换成布局编辑器菜单。 */
    private fun openCustomControls() {
        val adapter = editorMenuAdapter ?: return
        val listener = editorMenuListener ?: return
        mControlLayout.setModifiable(true)
        navDrawer.adapter = adapter
        navDrawer.onItemClickListener = listener
        menuTitle.setText(R.string.qk_menu_title_editor)
        menuSubtitle.setText(R.string.qk_menu_subtitle_editor)
        mDrawerPullButton.visibility = View.VISIBLE
        isInEditor = true
    }

    /** Pojav `MainActivity.exitEditor()`：丢弃编辑内容并重新加载布局。 */
    override fun exitEditor() {
        try {
            mControlLayout.loadLayout(null as CustomControls?)
            mControlLayout.setModifiable(false)
            System.gc()
            mControlLayout.loadLayout(LauncherPreferences.PREF_DEFAULTCTRL_PATH)
        } catch (e: Throwable) {
            net.kdt.pojavlaunch.Tools.showError(this, e)
        }
        navDrawer.adapter = gameMenuAdapter
        navDrawer.onItemClickListener = gameActionClickListener
        menuTitle.setText(R.string.qk_menu_title_ingame)
        menuSubtitle.setText(R.string.qk_menu_subtitle_hint)
        mDrawerPullButton.visibility =
            if (mControlLayout.hasMenuButton()) View.GONE else View.VISIBLE
        isInEditor = false
    }

    /** 「强制关闭游戏」：先让 Rust 优雅结束 JVM（要落存档），再退界面。 */
    private fun dialogForceClose() {
        AlertDialog.Builder(this, R.style.QookixAlertDialog)
            .setMessage(R.string.mcn_exit_confirm)
            .setNegativeButton(android.R.string.cancel, null)
            .setPositiveButton(android.R.string.ok) { _, _ ->
                lifecycleScope.launch {
                    withContext(Dispatchers.IO) { TauriBridge.killGame() }
                    finish()
                }
            }
            .show()
    }

    // ------------------------------------------------------------------ 按键

    /**
     * 照 Pojav `MainActivity.dispatchKeyEvent`：
     * 编辑模式下返回键 = 询问是否退出编辑；否则交给 [MinecraftGLSurface.processKeyEvent]，
     * 未被消费的返回键映射成 `GLFW_KEY_ESCAPE` 送给游戏（游戏内表现为打开暂停菜单）。
     */
    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        if (isInEditor) {
            if (event.keyCode == KeyEvent.KEYCODE_BACK) {
                if (event.action == KeyEvent.ACTION_DOWN) mControlLayout.askToExit(this)
                return true
            }
            return super.dispatchKeyEvent(event)
        }
        val handleEvent = minecraftGLView.processKeyEvent(event)
        if (!handleEvent) {
            if (event.keyCode == KeyEvent.KEYCODE_BACK && touchCharInput?.isEnabled != true) {
                if (event.action != KeyEvent.ACTION_UP) return true
                CallbackBridge.sendKeyPress(LwjglGlfwKeycode.GLFW_KEY_ESCAPE.toInt())
                return true
            }
        }
        return handleEvent
    }

    // ------------------------------------------------------------------ 生命周期（GLFW 窗口属性）

    /**
     * 切后台 / 锁屏 / 切应用时把 GLFW 窗口标记为「不可见」。
     *
     * 注意：这与 Pojav 完全一致 —— Pojav 在 `onStop` 里**同样**置 `GLFW_VISIBLE=0`。
     * 黑屏的真正原因不在这个 flag，而在渲染面：
     * 回前台时必须拿到**新的** Android Surface 并重新 `setupBridgeWindow`，
     * 原生侧才会做 `eglMakeCurrent` 切换（`gl_bridge.c` 的 `gl_swap_surface`）。
     * QookiX 旧实现用 TextureView 且回前台从不重绑，于是 EGLSurface 一直指向死掉的窗口。
     * 现在由 [MinecraftGLSurface] 的 SurfaceView 路径覆盖（`surfaceCreated` 里重绑）。
     */
    override fun onStart() {
        super.onStart()
        setWindowAttrib(LwjglGlfwKeycode.GLFW_VISIBLE, 1)
    }

    override fun onStop() {
        setWindowAttrib(LwjglGlfwKeycode.GLFW_VISIBLE, 0)
        super.onStop()
    }

    override fun onResume() {
        super.onResume()
        if (LauncherPreferences.PREF_ENABLE_GYRO) mGyroControl?.enable()
        setWindowAttrib(LwjglGlfwKeycode.GLFW_HOVERED, 1)
    }

    override fun onPause() {
        mGyroControl?.disable()
        // 游戏还抓着鼠标时先发一个 Esc：让它释放鼠标并进入暂停菜单，
        // 同时单机存档顺带自动暂停。缺了这一步，回来时游戏状态是乱的。
        if (CallbackBridge.isGrabbing()) {
            CallbackBridge.sendKeyPress(LwjglGlfwKeycode.GLFW_KEY_ESCAPE.toInt())
        }
        mQuickSettingSideDialog?.cancel()
        setWindowAttrib(LwjglGlfwKeycode.GLFW_HOVERED, 0)
        super.onPause()
    }

    override fun onPostResume() {
        super.onPostResume()
        // 从后台/其他界面回来时系统栏可能已被恢复，重新应用沉浸式
        applyImmersiveMode()
        // Pojav 同款：延迟重算一次窗口尺寸。切前台后 View 尺寸可能刚变化，
        // 不重算的话画面会被拉伸或者停在旧分辨率（表现为「回来一片黑」）。
        mControlLayout.postDelayed({ minecraftGLView.refreshSize() }, 500)
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        mGyroControl?.updateOrientation()
        mControlLayout.requestLayout()
        mControlLayout.post {
            minecraftGLView.refreshSize()
            net.kdt.pojavlaunch.Tools.updateWindowSize(this)
            mControlLayout.refreshControlButtonPositions()
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        try {
            CallbackBridge.removeGrabListener(touchpad)
            CallbackBridge.removeGrabListener(minecraftGLView)
        } catch (_: Throwable) {
        }
        GameService.stop(this)
        touchCharInput = null
        touchpad = null
    }

    private fun setWindowAttrib(attrib: Int, value: Int) {
        try {
            // 原生实现在 libpojavexec.so：把属性写进 JVM 侧 GLFW 的 windowAttribs。
            // 游戏还没显示窗口时它是空操作（pojav_environ->showingWindow == 0）。
            CallbackBridge.nativeSetWindowAttrib(attrib, value)
        } catch (_: Throwable) {
            // 原生库还没加载时忽略（比如游戏还没启动）
        }
    }

    // ------------------------------------------------------------------ 沉浸式全屏

    private val immersiveFlags = (
        View.SYSTEM_UI_FLAG_LAYOUT_STABLE
            or View.SYSTEM_UI_FLAG_FULLSCREEN
            or View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
            or View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
            or View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
            or View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
        )

    /**
     * 进入沉浸式全屏，与 PojavLauncher 的 `Tools.setFullscreen` 一致。
     *
     * 为什么要注册 `OnSystemUiVisibilityChangeListener` 反复设置：用户从底部/顶部边缘
     * 滑出系统栏后，系统会**清掉**这些 flag，不重新设置就永久退回「有导航条」状态。
     * Pojav 正是靠这个监听器恢复的。
     *
     * 刘海/挖孔照抄 Pojav 的 `Tools.ignoreNotch`：`SHORT_EDGES` 让内容铺满挖孔区域，
     * 否则横屏时那一条会被裁掉（表现为屏幕一侧的黑边）。
     */
    /**
     * 把历史遗留的 [LEGACY_PREF_NAME] 偏好合并进 [PREF_NAME]。
     *
     * 早期版本游戏侧读的是 `pojav_prefs`、启动器设置页写的是 `launcher_preferences`，
     * 两个文件互不相见（见 [PREF_NAME] 的说明）。统一文件名后，首次运行把老文件里
     * **新文件还没有** 的键搬过来 —— 只补缺、不覆盖，用户之前在设置页里的选择优先。
     */
    private fun migrateLegacyPrefs() {
        val legacy = getSharedPreferences(LEGACY_PREF_NAME, MODE_PRIVATE)
        if (legacy.all.isEmpty()) return
        val target = getSharedPreferences(PREF_NAME, MODE_PRIVATE)
        val editor = target.edit()
        var moved = 0
        for ((key, value) in legacy.all) {
            if (target.contains(key)) continue
            when (value) {
                is Boolean -> editor.putBoolean(key, value)
                is Int -> editor.putInt(key, value)
                is Long -> editor.putLong(key, value)
                is Float -> editor.putFloat(key, value)
                is String -> editor.putString(key, value)
                else -> {}
            }
            moved++
        }
        if (moved > 0) {
            editor.apply()
            Log.i(TAG, "已从 $LEGACY_PREF_NAME 合并 $moved 项偏好到 $PREF_NAME")
        }
    }

    private fun applyImmersiveMode() {
        // 刘海按用户的「忽略刘海」开关**双向分派**（问题清单 P0-13）：
        // 开 → SHORT_EDGES（内容铺满挖孔区）；关 → NEVER（不与挖孔区重叠）。
        // 之前无论开关都写死 SHORT_EDGES，而窗口尺寸那边又按「刘海不算」扣宽，
        // 导致坐标系与可视区不一致。
        // 键名与设置页写入的 `ignoreNotch` 一致（历史上这里写的是 `ignore_notch`，
        // 与启动器设置页的驼峰键对不上 → 开关一直无效）。同时兼容读取老键名。
        val prefs = getSharedPreferences(PREF_NAME, Context.MODE_PRIVATE)
        val ignoreNotch = prefs.getBoolean(
            "ignoreNotch",
            prefs.getBoolean("ignore_notch", false)
        )

        // API 30+ 用 WindowInsetsControllerCompat：`systemUiVisibility` 自 API 30 起废弃，
        // Android 15+ 强制 edge-to-edge 后不再可靠（问题清单 P1-26）。
        WindowCompat.setDecorFitsSystemWindows(window, false)
        val controller = WindowInsetsControllerCompat(window, window.decorView)
        controller.systemBarsBehavior =
            WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        controller.hide(WindowInsetsCompat.Type.systemBars())

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            val attrs = window.attributes
            attrs.layoutInDisplayCutoutMode = if (ignoreNotch) {
                WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
            } else {
                WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_NEVER
            }
            window.attributes = attrs
        }

        // Android 10+ 的「系统栏对比度保护」：手势导航时系统会在底部导航条区域
        // 叠一层半透明黑，游戏里就表现为「底部一条黑的把画面盖住」。关掉它。
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            window.isNavigationBarContrastEnforced = false
            window.isStatusBarContrastEnforced = false
        }
        // API 30 以下由旧的 flag 体系兜底（30+ 已交给上面的 controller）
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) {
            val decor = window.decorView
            decor.setOnSystemUiVisibilityChangeListener { visibility ->
                if (visibility and View.SYSTEM_UI_FLAG_FULLSCREEN == 0) {
                    decor.systemUiVisibility = immersiveFlags
                }
            }
            decor.systemUiVisibility = immersiveFlags
        }
        window.setFlags(
            WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN,
            WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN
        )
    }

    // ------------------------------------------------------------------ 崩溃诊断

    private fun showCrashDialog(exitCode: Int) {
        val saved = dumpDiagnostics()
        AlertDialog.Builder(this, R.style.QookixAlertDialog)
            .setTitle("游戏异常退出（退出码 $exitCode）")
            .setMessage(
                saved
                    ?: "无法读取日志，可用 adb logcat 查看（游戏崩溃时会保存崩溃报告到实例目录 crash-reports/）"
            )
            .setCancelable(false)
            .setPositiveButton("知道了") { _, _ -> finish() }
            .show()
    }

    /**
     * 把游戏日志与崩溃报告复制到外部私有目录
     * （`Android/data/com.zhayi.qookix/files/diagnostics/`），
     * 这样不用 adb 也能用文件管理器取出来发给开发者。
     */
    private fun dumpDiagnostics(): String? {
        return try {
            val external = getExternalFilesDir(null) ?: return null
            val target = File(external, "diagnostics")
            target.mkdirs()

            val logFile = File(filesDir, "logs/launch-$instanceId.log")
            if (logFile.isFile) logFile.copyTo(File(target, "launch-$instanceId.log"), true)

            val crashDir = File(filesDir, "instances/$instanceId/crash-reports")
            crashDir.listFiles()?.sortedByDescending { it.lastModified() }?.firstOrNull()?.let {
                it.copyTo(File(target, it.name), true)
            }

            "日志与崩溃报告已保存到：\n$target"
        } catch (e: Exception) {
            null
        }
    }
}
