package com.zhayi.qookix.tauri

import android.util.Log
import android.view.Surface

object TauriBridge {

    private const val TAG = "TauriBridge"
    private var nativeLoaded = false

    init {
        try {
            System.loadLibrary("qookix_lib")
            nativeLoaded = true
            Log.i(TAG, "Native library loaded successfully")
        } catch (e: UnsatisfiedLinkError) {
            nativeLoaded = false
            Log.w(TAG, "Native library not available, using Kotlin fallback")
        }
    }

    fun isNativeAvailable(): Boolean = nativeLoaded

    external fun nativeGetVersionList(): String
    external fun nativeLaunchGame(instanceId: String, accountUuid: String): Int
    external fun nativeKillGame(): Int
    external fun nativeGetGameStatus(): String
    external fun nativeSendInput(eventType: Int, data: String)
    external fun nativeSetupSurface(surface: Surface)
    external fun nativeReleaseSurface()

    fun launchGame(instanceId: String, accountUuid: String): Int {
        return if (nativeLoaded) {
            nativeLaunchGame(instanceId, accountUuid)
        } else {
            Log.e(TAG, "Native library not loaded, cannot launch game")
            -1
        }
    }

    fun killGame(): Boolean {
        return if (nativeLoaded) {
            nativeKillGame() == 1
        } else {
            false
        }
    }

    fun sendInput(eventType: Int, data: String) {
        if (nativeLoaded) {
            nativeSendInput(eventType, data)
        }
    }

    fun setupSurface(surface: Surface) {
        if (nativeLoaded) {
            nativeSetupSurface(surface)
        }
    }

    fun releaseSurface() {
        if (nativeLoaded) {
            nativeReleaseSurface()
        }
    }
}
