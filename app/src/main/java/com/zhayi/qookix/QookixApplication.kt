package com.zhayi.qookix

import android.app.Application
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob

class QookixApplication : Application() {

    val applicationScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    override fun onCreate() {
        super.onCreate()
        instance = this
        initDirectories()
        setupGlobalExceptionHandler()
    }

    private fun initDirectories() {
        val gameHome = getGameHomeDir()
        val dirs = arrayOf(
            "versions", "libraries", "assets", "assets/indexes",
            "assets/objects", "assets/skins", "custom_instances",
            "accounts", "runtimes", "cache", "cache/natives",
            "cache/icons", "controlmap", "caciocavallo", "lwjgl3", "crash-reports"
        )
        dirs.forEach {
            val dir = java.io.File(gameHome, it)
            if (!dir.exists()) dir.mkdirs()
        }
    }

    private fun setupGlobalExceptionHandler() {
        val defaultHandler = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, throwable ->
            writeCrashReport(throwable)
            defaultHandler?.uncaughtException(thread, throwable)
        }
    }

    private fun writeCrashReport(throwable: Throwable) {
        val crashFile = java.io.File(getGameHomeDir(), "latestcrash.txt")
        crashFile.bufferedWriter().use { writer ->
            writer.write("""
                |Timestamp: ${System.currentTimeMillis()}
                |Thread: ${Thread.currentThread().name}
                |Exception: ${throwable.javaClass.name}
                |Message: ${throwable.message}
                |
                |Stack Trace:
                |${throwable.stackTrace.joinToString("\n") { "\tat $it" }}
            """.trimMargin())
        }
    }

    fun getGameHomeDir(): java.io.File = java.io.File(getExternalFilesDir(null), "games/QookiX")
    fun getRuntimeDir(): java.io.File = java.io.File(filesDir, "runtimes")
    fun getCustomCacheDir(): java.io.File = java.io.File(filesDir, "cache")

    companion object {
        @JvmStatic
        lateinit var instance: QookixApplication
            private set
    }
}
