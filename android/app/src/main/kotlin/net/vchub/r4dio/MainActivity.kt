package net.vchub.r4dio

import android.Manifest
import android.content.ComponentName
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat
import androidx.media3.common.Player
import androidx.media3.session.MediaController
import androidx.media3.session.SessionCommand
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.ListenableFuture
import com.google.common.util.concurrent.MoreExecutors

class MainActivity : ComponentActivity() {
    private var controllerFuture: ListenableFuture<MediaController>? = null
    private var controller: MediaController? = null
    private var listener: Player.Listener? = null
    private val main = Handler(Looper.getMainLooper())
    private val closeGuard = Runnable { finish() }

    private val requestPermission =
        registerForActivityResult(ActivityResultContracts.RequestPermission()) {
            connect()
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        when (needsNotificationPermission()) {
            true -> requestPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
            false -> connect()
        }
    }

    private fun connect() {
        val token = SessionToken(this, ComponentName(this, PlaybackService::class.java))
        val future = MediaController.Builder(this, token).buildAsync()
        controllerFuture = future
        future.addListener({
            val c = runCatching { future.get() }.getOrNull()
            when (c == null) {
                true -> finish()
                false -> onConnected(c)
            }
        }, MoreExecutors.directExecutor())
    }

    private fun onConnected(c: MediaController) {
        controller = c
        when (c.isPlaying) {
            true -> finish()
            false -> waitForPlayback(c)
        }
    }

    private fun waitForPlayback(c: MediaController) {
        val l = object : Player.Listener {
            override fun onIsPlayingChanged(isPlaying: Boolean) {
                when (isPlaying) {
                    true -> finish()
                    false -> {}
                }
            }
        }
        listener = l
        c.addListener(l)
        when (c.mediaItemCount == 0) {
            true -> c.sendCustomCommand(SessionCommand(CMD_SHUFFLE, android.os.Bundle.EMPTY), android.os.Bundle.EMPTY)
            false -> {}
        }
        main.postDelayed(closeGuard, 15000)
    }

    override fun onDestroy() {
        main.removeCallbacks(closeGuard)
        listener?.let { controller?.removeListener(it) }
        listener = null
        controller = null
        controllerFuture?.let { MediaController.releaseFuture(it) }
        controllerFuture = null
        super.onDestroy()
    }

    private fun needsNotificationPermission(): Boolean {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
            return false
        }
        return ContextCompat.checkSelfPermission(
            this,
            Manifest.permission.POST_NOTIFICATIONS,
        ) != PackageManager.PERMISSION_GRANTED
    }
}
