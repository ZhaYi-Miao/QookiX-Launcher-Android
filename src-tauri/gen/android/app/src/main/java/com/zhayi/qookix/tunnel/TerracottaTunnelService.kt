package com.zhayi.qookix.tunnel

import android.app.Service
import android.content.Intent
import android.os.IBinder
import android.util.Log
import net.burningtnt.terracotta.TerracottaAndroidAPI
import java.io.BufferedReader
import java.io.InputStreamReader
import java.io.OutputStream
import java.net.InetSocketAddress
import java.net.ServerSocket
import java.net.Socket
import java.nio.charset.StandardCharsets
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean

/**
 * 陶瓦联机（Terracotta）隧道服务 —— **必须跑在独立进程**（`:tunnel`，见 AndroidManifest）。
 *
 * ## 为什么要在独立进程里
 *
 * Terracotta 的 Android 产物是原生库 `libterracotta.so`（AGPL-3.0-or-later，
 * 但上游 README 给了例外：只要「打包未修改的二进制且**不静态/动态链接**」，
 * 或「通过 IPC 与未修改的程序交互并在界面署名」，就不被 AGPL 涵盖）。
 *
 * 我们采用例外条款里的**进程隔离**做法：`.so` 只在这个进程里 `System.loadLibrary`，
 * 主进程（启动器本体）**完全不链接**它，只通过 `127.0.0.1` 上的 HTTP 通信。
 * 这样启动器保持 GPL-3.0。署名见「设置 → 关于 → 第三方声明」。
 *
 * 顺带的好处：隧道崩溃/被杀不会带走启动器和游戏（我们刚踩过游戏 JVM 把整个进程带走的坑）。
 *
 * ## 通信协议（极简 HTTP，主进程用 fetch 调）
 *
 *   GET  /state              → 当前状态 JSON（Terracotta 的 getState 原样透传）
 *   POST /host?player=NAME   → 开始开房并扫描局域网里的「开放局域网世界」
 *   POST /guest?room=CODE&player=NAME → 加入别人的房间
 *   POST /waiting            → 回到等待状态
 *   POST /shutdown           → 停止隧道
 *   GET  /logs               → 最近日志（Terracotta 自己的 application.log 尾部）
 */
class TerracottaTunnelService : Service() {

    private val running = AtomicBoolean(false)
    private var serverSocket: ServerSocket? = null
    private var ready = false
    private var lastError: String? = null
    /** 房间码是否已经写进通知（只写一次，避免每次轮询都更新通知） */
    @Volatile
    private var roomNotified = false
    /** 房间码监听线程（见 [startRoomWatcher]） */
    private var roomWatcher: Thread? = null
    @Volatile
    private var watchingRoom = false

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) {
            shutdownTerracotta()
            stopSelf()
            return START_NOT_STICKY
        }
        if (!running.get()) startEverything()
        return START_STICKY
    }

    private fun startEverything() {
        running.set(true)
        // 加载 + 启动 Terracotta 放后台线程：initialize() 内部要初始化 EasyTier，
        // 阻塞约 1 秒，放主线程会卡住启动。
        Thread({
            try {
                System.loadLibrary("terracotta")
                val cb = TerracottaAndroidAPI.VpnServiceCallback {
                    // EasyTier 需要 TUN（VPN）时回调。这里不申请 VPN：
                    // 陶瓦的「开房」只需要把 MC 那个端口映射进 mesh。
                    Log.w(TAG, "收到 VpnService 请求，本次不申请（走端口映射模式）")
                }
                val meta = TerracottaAndroidAPI.initialize(applicationContext, cb)
                Log.i(TAG, "Terracotta 就绪: $meta")
                ready = true
            } catch (t: Throwable) {
                lastError = t.toString()
                Log.e(TAG, "Terracotta 初始化失败", t)
            }
        }, "tc-init").start()

        // 本地 HTTP 服务（主进程唯一的通信入口）
        Thread({
            try {
                val ss = ServerSocket(0) // 让系统分配空闲端口
                serverSocket = ss
                // 端口写进共享数据目录，主进程（Rust）读文件即可，无需新增 JNI
                java.io.File(filesDir, PORT_FILE).writeText(ss.localPort.toString())
                Log.i(TAG, "隧道 HTTP 监听 127.0.0.1:${ss.localPort}")
                while (!ss.isClosed) {
                    val client = ss.accept()
                    Thread { handle(client) }.start()
                }
            } catch (t: Throwable) {
                Log.e(TAG, "HTTP 服务退出", t)
            }
        }, "tc-http").start()
    }

    private fun shutdownTerracotta() {
        ready = false
        runCatching { serverSocket?.close() }
        serverSocket = null
        running.set(false)
        runCatching { stopForeground(true) }
    }

    /** 开房时晋升前台（带通知），保证切后台不被回收 */
    private fun promoteToForeground() {
        runCatching {
            if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.O) {
                val nm = getSystemService(NOTIFICATION_SERVICE) as android.app.NotificationManager
                if (nm.getNotificationChannel(CHANNEL_ID) == null) {
                    nm.createNotificationChannel(
                        android.app.NotificationChannel(
                            CHANNEL_ID, "联机房间", android.app.NotificationManager.IMPORTANCE_LOW
                        )
                    )
                }
            }
            val n = android.app.Notification.Builder(this, CHANNEL_ID)
                .setSmallIcon(android.R.drawable.ic_dialog_info)
                .setContentTitle("正在等待「对局域网开放」")
                .setContentText("在游戏里对局域网开放世界后，房间码会显示在这里")
                .setOngoing(true)
                .build()
            startForegroundCompat(n)
        }.onFailure { Log.w(TAG, "晋升前台失败（房间可能不稳定）", it) }
    }

    /**
     * 带类型的前台服务晋升（与 ServerService 同一套写法）。
     *
     * Android 14（API 34）起，不带类型调用 `startForeground` 会抛
     * `MissingForegroundServiceTypeException`：实测现象就是**通知不出现** ——
     * 房间码写在通知里，于是玩家在游戏里什么都看不到（异常被 runCatching 吞掉，
     * 只在 logcat 留一行 W/TcTunnel）。
     */
    private fun startForegroundCompat(n: android.app.Notification) {
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.Q) {
            startForeground(
                NOTIFICATION_ID,
                n,
                android.content.pm.ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC,
            )
        } else {
            startForeground(NOTIFICATION_ID, n)
        }
    }

    private fun demoteForeground() {
        stopRoomWatcher()
        runCatching { stopForeground(true) }
    }

    /** 从状态 JSON 里抠出房间码（字段名各版本不同，挨个试） */
    private fun extractRoomCode(stateJson: String): String? {
        for (key in listOf("\"room\"", "\"code\"", "\"room_code\"", "\"roomCode\"")) {
            val i = stateJson.indexOf(key)
            if (i < 0) continue
            val colon = stateJson.indexOf(':', i + key.length)
            if (colon < 0) continue
            val rest = stateJson.substring(colon + 1).trimStart()
            if (!rest.startsWith('"')) continue
            val end = rest.indexOf('"', 1)
            if (end <= 1) continue
            val v = rest.substring(1, end)
            if (v.isNotBlank()) return v
        }
        return null
    }

    /** 把通知换成「房间码 xxx」，这样玩家在游戏里也能看到并念给朋友 */
    private fun updateNotificationWithRoom(code: String) {
        runCatching {
            val n = android.app.Notification.Builder(this, CHANNEL_ID)
                .setSmallIcon(android.R.drawable.ic_dialog_info)
                .setContentTitle("联机房间 $code")
                .setContentText("把这个房间码发给朋友即可加入")
                .setOngoing(true)
                .build()
            startForegroundCompat(n)
            Log.i(TAG, "房间码已就绪: $code")
        }.onFailure { Log.w(TAG, "更新房间码通知失败", it) }
    }

    /**
     * 开房扫描期间自己盯着状态，一拿到房间码就写进通知栏。
     *
     * 为什么必须由本服务自己轮询：玩家在游戏里时启动器在后台，WebView 的定时器被挂起，
     * 之前唯一会调 `/state` 的 `MultiplayerView` 早就停了 —— 实测现象就是
     * 房间其实开好了（HostOk），通知栏却什么都没有，而游戏里玩家只能看通知栏。
     * 另外 Android 14 起 `startForeground` 不带类型会抛异常（通知直接不出现），
     * 那条已由 [startForegroundCompat] 修掉。
     */
    private fun startRoomWatcher() {
        if (roomWatcher != null) return
        roomNotified = false
        watchingRoom = true
        roomWatcher = Thread {
            val deadline = System.currentTimeMillis() + 30 * 60 * 1000L
            while (watchingRoom && !roomNotified && System.currentTimeMillis() < deadline) {
                try {
                    val s = TerracottaAndroidAPI.getState()
                    if (s.contains("host")) {
                        extractRoomCode(s)?.let { code ->
                            roomNotified = true
                            updateNotificationWithRoom(code)
                        }
                    }
                } catch (t: Throwable) {
                    // 还没初始化完 / EasyTier 正在起，都属正常，继续重试
                    Log.w(TAG, "读取房间状态失败（继续重试）", t)
                }
                try {
                    Thread.sleep(2000)
                } catch (e: InterruptedException) {
                    break
                }
            }
        }.also {
            it.isDaemon = true
            it.name = "room-watcher"
            it.start()
        }
    }

    private fun stopRoomWatcher() {
        watchingRoom = false
        roomWatcher = null
    }

    override fun onDestroy() {
        stopRoomWatcher()
        shutdownTerracotta()
        super.onDestroy()
    }

    // ── HTTP 处理（只支持最简的 GET/POST，够用即可）──────────────────────

    private fun handle(client: Socket) {
        try {
            client.use { sock ->
                sock.soTimeout = 5000
                val input = BufferedReader(InputStreamReader(sock.getInputStream(), StandardCharsets.UTF_8))
                val requestLine = input.readLine() ?: return
                // 把请求头读干净
                while (true) {
                    val line = input.readLine() ?: break
                    if (line.isEmpty()) break
                }
                val parts = requestLine.split(" ")
                if (parts.size < 2) return
                val method = parts[0]
                val target = parts[1]
                val path = target.substringBefore('?')
                val query = parseQuery(target.substringAfter('?', ""))

                val body: String = try {
                    withState {
                        when {
                            path == "/state" -> {
                                val s = TerracottaAndroidAPI.getState()
                                // 状态里出现房间码就把通知更新掉（用户在游戏里只看得到通知栏）
                                if (s.contains("host") && !roomNotified) {
                                    extractRoomCode(s)?.let { code ->
                                        roomNotified = true
                                        updateNotificationWithRoom(code)
                                    }
                                }
                                s
                            }
                            path == "/host" -> {
                                // 开房时才需要「一直活着」：晋升为前台服务（带通知），
                                // 否则切后台被系统回收，房间就断了。
                                promoteToForeground()
                                startRoomWatcher()
                                TerracottaAndroidAPI.setScanning(null, query["player"])
                                "{\"ok\":true,\"state\":\"host-scanning\"}"
                            }
                            path == "/guest" -> {
                                val room = query["room"]
                                if (room.isNullOrBlank()) "{\"ok\":false,\"error\":\"缺少房间码\"}"
                                else {
                                    // 先退出自己这边的房间/扫描：正开着房时 setGuesting 会直接失败，
                                    // 而调用方（游戏内联机面板、启动器多人页）只看返回 JSON，
                                    // 表现出来的就是「点了加入房间毫无反应」—— 真机踩过。
                                    runCatching { TerracottaAndroidAPI.setWaiting() }
                                    val ok = TerracottaAndroidAPI.setGuesting(room, query["player"])
                                    "{\"ok\":$ok,\"state\":\"${if (ok) "guesting" else "failed"}\"}"
                                }
                            }
                            path == "/waiting" -> {
                                TerracottaAndroidAPI.setWaiting()
                                demoteForeground()
                                "{\"ok\":true,\"state\":\"waiting\"}"
                            }
                            path == "/autohost" -> {
                                // 游戏启动时调用：直接进开房扫描。这样用户**不用回启动器点按钮**
                                // —— 在游戏里「开放局域网世界」即可，房间码会出现在通知栏。
                                val name = query["player"]
                                promoteToForeground()
                                startRoomWatcher()
                                TerracottaAndroidAPI.setScanning(name, name)
                                "{\"ok\":true,\"state\":\"host-scanning\"}"
                            }
                            path == "/logs" -> tailLogs()
                            path == "/ping" -> "{\"ok\":true,\"ready\":$ready}"
                            else -> "{\"ok\":false,\"error\":\"未知接口 $method $path\"}"
                        }
                    }
                } catch (t: Throwable) {
                    Log.e(TAG, "处理 $path 失败", t)
                    "{\"ok\":false,\"error\":${jsonEscape(t.toString())}}"
                }

                val bytes = body.toByteArray(StandardCharsets.UTF_8)
                sock.getOutputStream().apply {
                    write("HTTP/1.1 200 OK\r\n".toByteArray())
                    write("Content-Type: application/json; charset=utf-8\r\n".toByteArray())
                    write("Content-Length: ${bytes.size}\r\n".toByteArray())
                    write("Connection: close\r\n\r\n".toByteArray())
                    write(bytes)
                    flush()
                }
            }
        } catch (_: Throwable) {
        }
    }

    /** Terracotta 未就绪时的兜底：直接报错而不是让调用方等 */
    private inline fun <T> withState(block: () -> T): T {
        if (!ready) {
            Thread.sleep(300) // 给 initialize 一点时间
        }
        if (!ready) throw IllegalStateException("Terracotta 还在启动中，请稍候重试")
        return block()
    }

    private fun tailLogs(): String {
        val f = java.io.File(filesDir, "net.burningtnt.terracotta/application.log")
        if (!f.exists()) return "{\"ok\":true,\"logs\":\"\"}"
        val text = try {
            f.readText()
        } catch (t: Throwable) {
            ""
        }
        val tail = if (text.length > 4000) text.takeLast(4000) else text
        return "{\"ok\":true,\"logs\":${jsonEscape(tail)}}"
    }

    private fun parseQuery(q: String): Map<String, String> =
        q.split('&').filter { it.contains('=') }.associate {
            val k = it.substringBefore('=')
            val v = it.substringAfter('=')
            k to java.net.URLDecoder.decode(v, "UTF-8")
        }

    private fun jsonEscape(s: String): String {
        val sb = StringBuilder("\"")
        for (c in s) {
            when (c) {
                '"' -> sb.append("\\\"")
                '\\' -> sb.append("\\\\")
                '\n' -> sb.append("\\n")
                '\r' -> sb.append("\\r")
                '\t' -> sb.append("\\t")
                else -> if (c < ' ') sb.append("\\u%04x".format(c.code)) else sb.append(c)
            }
        }
        return sb.append("\"").toString()
    }

    companion object {
        private const val TAG = "TcTunnel"
        private const val CHANNEL_ID = "tunnel_channel"
        private const val NOTIFICATION_ID = 4202

        /** 动作常量（供主进程用 Intent 触发） */
        const val ACTION_STOP = "com.zhayi.qookix.tunnel.STOP"

        /** 隧道 HTTP 端口文件（放在 filesDir，主进程 Rust 读它拿端口） */
        const val PORT_FILE = "tunnel_port"
    }
}
