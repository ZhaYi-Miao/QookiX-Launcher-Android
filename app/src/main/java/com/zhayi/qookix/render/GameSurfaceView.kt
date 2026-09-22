package com.zhayi.qookix.render

import android.content.Context
import android.util.AttributeSet
import android.view.SurfaceHolder
import android.view.SurfaceView
import com.zhayi.qookix.tauri.TauriBridge

class GameSurfaceView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
    defStyleAttr: Int = 0
) : SurfaceView(context, attrs, defStyleAttr), SurfaceHolder.Callback {

    init {
        holder.addCallback(this)
        setZOrderOnTop(true)
    }

    override fun surfaceCreated(holder: SurfaceHolder) {
        TauriBridge.setupSurface(holder.surface)
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        TauriBridge.setupSurface(holder.surface)
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        TauriBridge.releaseSurface()
    }
}
