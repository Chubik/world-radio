package net.vchub.r4dio

import android.content.ComponentName
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.Text
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.session.MediaController
import androidx.media3.session.SessionCommand
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.MoreExecutors

class MainActivity : ComponentActivity() {
    private var controller: MediaController? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val token = SessionToken(this, ComponentName(this, PlaybackService::class.java))
        val future = MediaController.Builder(this, token).buildAsync()

        setContent {
            var isPlaying by remember { mutableStateOf(false) }
            var title by remember { mutableStateOf("Nothing playing") }

            future.addListener({
                val c = future.get()
                controller = c
                c.addListener(object : Player.Listener {
                    override fun onIsPlayingChanged(playing: Boolean) {
                        isPlaying = playing
                    }
                    override fun onMediaMetadataChanged(m: MediaMetadata) {
                        title = m.title?.toString() ?: "World Radio"
                    }
                })
            }, MoreExecutors.directExecutor())

            Column(
                modifier = Modifier.fillMaxSize().background(Color(0xFF15100B)).padding(24.dp),
                verticalArrangement = Arrangement.Center,
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Text(text = "▌ r4dio", color = Color(0xFFFFC457))
                Spacer(Modifier.height(8.dp))
                Text(text = title, color = Color(0xFFFFF0C0))
                Spacer(Modifier.height(24.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    Button(onClick = {
                        val c = controller ?: return@Button
                        c.sendCustomCommand(
                            SessionCommand(CMD_SHUFFLE, Bundle.EMPTY),
                            Bundle.EMPTY,
                        )
                    }) { Text("⇄ SHUFFLE") }
                    Button(onClick = {
                        val c = controller ?: return@Button
                        if (c.isPlaying) c.pause() else c.play()
                    }) { Text(if (isPlaying) "⏸" else "▶") }
                }
            }
        }
    }

    override fun onDestroy() {
        controller?.release()
        controller = null
        super.onDestroy()
    }
}
