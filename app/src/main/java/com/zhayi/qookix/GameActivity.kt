package com.zhayi.qookix

import android.app.AlertDialog
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.view.KeyEvent
import android.view.WindowManager
import android.widget.FrameLayout
import androidx.activity.ComponentActivity
import androidx.lifecycle.lifecycleScope
import com.zhayi.qookix.control.InputDispatcher
import com.zhayi.qookix.render.GameSurfaceView
import com.zhayi.qookix.services.GameService
import com.zhayi.qookix.tauri.TauriBridge
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class GameActivity : ComponentActivity() {

    private var instanceId: String = ""
    private var accountUuid: String = ""
    private var exitCode: Int = 0
    private lateinit var container: FrameLayout
    private lateinit var surfaceView: GameSurfaceView

    companion object {
        private const val EXTRA_INSTANCE_ID = "instance_id"
        private const val EXTRA_ACCOUNT_UUID = "account_uuid"

        fun start(context: Context, instanceId: String, accountUuid: String) {
            val intent = Intent(context, GameActivity::class.java).apply {
                putExtra(EXTRA_INSTANCE_ID, instanceId)
                putExtra(EXTRA_ACCOUNT_UUID, accountUuid)
                flags = Intent.FLAG_ACTIVITY_NEW_TASK
            }
            context.startActivity(intent)
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        instanceId = intent.getStringExtra(EXTRA_INSTANCE_ID) ?: ""
        accountUuid = intent.getStringExtra(EXTRA_ACCOUNT_UUID) ?: ""

        window.setFlags(
            WindowManager.LayoutParams.FLAG_FULLSCREEN,
            WindowManager.LayoutParams.FLAG_FULLSCREEN
        )
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)

        container = FrameLayout(this)
        surfaceView = GameSurfaceView(this)
        container.addView(surfaceView, FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT,
            FrameLayout.LayoutParams.MATCH_PARENT
        ))
        setContentView(container)

        GameService.start(this)
        launchGame()
    }

    private fun launchGame() {
        lifecycleScope.launch {
            InputDispatcher.setReady(false)

            exitCode = withContext(Dispatchers.IO) {
                TauriBridge.launchGame(instanceId, accountUuid)
            }

            InputDispatcher.setReady(false)

            if (exitCode != 0) {
                showCrashDialog()
            } else {
                finish()
            }
        }
    }

    private fun showCrashDialog() {
        AlertDialog.Builder(this)
            .setTitle("游戏崩溃")
            .setMessage("游戏异常退出，退出码: $exitCode")
            .setCancelable(false)
            .setPositiveButton("确定") { _, _ -> finish() }
            .show()
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent?): Boolean {
        if (keyCode == KeyEvent.KEYCODE_BACK) {
            return true
        }
        return super.onKeyDown(keyCode, event)
    }

    override fun onDestroy() {
        super.onDestroy()
        GameService.stop(this)
        InputDispatcher.setReady(false)
    }
}
