package com.zhayi.qookix.control

import com.zhayi.qookix.tauri.TauriBridge

object InputDispatcher {

    private const val EVENT_KEY = 1005
    private const val EVENT_MOUSE_BUTTON = 1006
    private const val EVENT_CURSOR_POS = 1003
    private const val EVENT_SCROLL = 1007

    @Volatile
    private var ready = false

    fun setReady(ready: Boolean) {
        this.ready = ready
    }

    fun dispatchKey(keyCode: Int, pressed: Boolean) {
        dispatchKeyFull(keyCode, 0, if (pressed) 1 else 0, 0)
    }

    fun dispatchKeyFull(key: Int, scancode: Int, action: Int, mods: Int) {
        if (!ready) return
        val data = "$key,$scancode,$action,$mods"
        TauriBridge.sendInput(EVENT_KEY, data)
    }

    fun dispatchMouseButton(button: Int, pressed: Boolean) {
        dispatchMouseButtonFull(button, if (pressed) 1 else 0, 0)
    }

    fun dispatchMouseButtonFull(button: Int, action: Int, mods: Int) {
        if (!ready) return
        val data = "$button,$action,$mods"
        TauriBridge.sendInput(EVENT_MOUSE_BUTTON, data)
    }

    fun dispatchCursorPos(x: Double, y: Double) {
        if (!ready) return
        val data = "$x,$y"
        TauriBridge.sendInput(EVENT_CURSOR_POS, data)
    }

    fun dispatchScroll(x: Double, y: Double) {
        if (!ready) return
        val data = "$x,$y"
        TauriBridge.sendInput(EVENT_SCROLL, data)
    }
}
