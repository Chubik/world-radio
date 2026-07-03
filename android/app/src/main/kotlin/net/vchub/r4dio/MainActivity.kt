package net.vchub.r4dio

import android.Manifest
import android.content.ComponentName
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.*
import androidx.core.content.ContextCompat
import androidx.media3.common.Player
import androidx.media3.session.MediaController
import androidx.media3.session.SessionCommand
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.MoreExecutors

class MainActivity : ComponentActivity() {
    private var controller: MediaController? = null
    private var controllerFuture: com.google.common.util.concurrent.ListenableFuture<MediaController>? = null
    private val favStore by lazy { FavStore(this) }

    private val requestPermission =
        registerForActivityResult(ActivityResultContracts.RequestPermission()) { render() }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        when (needsNotificationPermission()) {
            true -> requestPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
            false -> render()
        }
    }

    private fun render() {
        val token = SessionToken(this, ComponentName(this, PlaybackService::class.java))
        val future = MediaController.Builder(this, token).buildAsync()
        controllerFuture = future
        future.addListener({
            if (isDestroyed || isFinishing) return@addListener
            val c = future.get()
            controller = c
            if (c.mediaItemCount == 0) {
                c.sendCustomCommand(SessionCommand(CMD_SHUFFLE, android.os.Bundle.EMPTY), android.os.Bundle.EMPTY)
            }
            setContent { Ui(c) }
        }, MoreExecutors.directExecutor())
    }

    @Composable
    private fun Ui(c: MediaController) {
        var isPlaying by remember { mutableStateOf(c.isPlaying) }
        var title by remember { mutableStateOf(c.mediaMetadata.title?.toString() ?: "") }
        var artist by remember { mutableStateOf(c.mediaMetadata.artist?.toString() ?: "") }
        DisposableEffect(c) {
            val l = object : Player.Listener {
                override fun onIsPlayingChanged(playing: Boolean) { isPlaying = playing }
                override fun onMediaMetadataChanged(m: androidx.media3.common.MediaMetadata) {
                    title = m.title?.toString() ?: ""
                    artist = m.artist?.toString() ?: ""
                }
            }
            c.addListener(l)
            onDispose { c.removeListener(l) }
        }
        val favs by favStore.favUuids.collectAsState(initial = emptySet())
        val scope by favStore.scope.collectAsState(initial = Scope.ALL)
        val meta = artist
        val currentUuid = c.currentMediaItem?.mediaId ?: ""
        val hint = when {
            scope == Scope.FAVS && favs.isEmpty() -> "no favourites yet"
            else -> null
        }
        NowPlayingScreen(
            state = NowPlayingState(
                station = title.ifBlank { "r4dio" },
                subtitle = "",
                meta = meta,
                isPlaying = isPlaying,
                isFav = favs.contains(currentUuid),
                scope = scope,
                hint = hint,
            ),
            onShuffle = { c.sendCustomCommand(SessionCommand(CMD_SHUFFLE, android.os.Bundle.EMPTY), android.os.Bundle.EMPTY) },
            onPlayPause = { if (c.isPlaying) c.pause() else c.play() },
            onStar = { c.sendCustomCommand(SessionCommand(CMD_STAR, android.os.Bundle.EMPTY), android.os.Bundle.EMPTY) },
            onScope = { target ->
                val b = android.os.Bundle().apply { putString("scope", target.name) }
                c.sendCustomCommand(SessionCommand(CMD_SCOPE, android.os.Bundle.EMPTY), b)
            },
        )
    }

    override fun onDestroy() {
        controller?.release()
        controller = null
        controllerFuture?.let { MediaController.releaseFuture(it) }
        controllerFuture = null
        super.onDestroy()
    }

    private fun needsNotificationPermission(): Boolean {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return false
        return ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
    }
}
