package net.vchub.r4dio

import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.session.CommandButton
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import androidx.media3.session.SessionCommand
import androidx.media3.session.SessionResult
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import kotlin.concurrent.thread

const val CMD_SHUFFLE = "net.vchub.r4dio.SHUFFLE"
const val CMD_STAR = "net.vchub.r4dio.STAR"

class PlaybackService : MediaSessionService() {
    private var session: MediaSession? = null
    private var exo: ExoPlayer? = null
    private val catalog = Catalog()
    @Volatile private var stations: List<Station> = emptyList()
    private val main = Handler(Looper.getMainLooper())

    override fun onCreate() {
        super.onCreate()
        val player = ExoPlayer.Builder(this).build()
        exo = player
        session = MediaSession.Builder(this, player)
            .setCallback(Callback())
            .build()
        loadStations()
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    override fun onTaskRemoved(rootIntent: android.content.Intent?) {
        val player = exo
        when (player != null && player.playWhenReady && player.mediaItemCount > 0) {
            true -> {}
            false -> pauseAllPlayersAndStopSelf()
        }
    }

    override fun onDestroy() {
        session?.release()
        exo?.release()
        session = null
        exo = null
        super.onDestroy()
    }

    private fun loadStations() {
        thread {
            val fetched = catalog.fetchStations()
            stations = fetched
            Log.i("r4dio", "loaded ${fetched.size} stations")
            val pick = pickRandom(fetched) ?: return@thread
            main.post { playPick(pick) }
        }
    }

    private fun shuffle() {
        val current = stations
        when (current.isEmpty()) {
            true -> thread {
                val fetched = catalog.fetchStations()
                stations = fetched
                Log.i("r4dio", "refetched ${fetched.size} stations for shuffle")
                val pick = pickRandom(fetched) ?: return@thread
                main.post { playPick(pick) }
            }
            false -> {
                val pick = pickRandom(current) ?: return
                playPick(pick)
            }
        }
    }

    private fun playPick(pick: Station) {
        val player = exo ?: return
        Log.i("r4dio", "playing ${pick.name} — ${pick.url}")
        val subtitle = listOf(pick.country, pick.codec, "${pick.bitrate}k")
            .filter { it.isNotBlank() && it != "0k" }
            .joinToString(" · ")
        val metadata = MediaMetadata.Builder()
            .setTitle(pick.name)
            .setArtist(subtitle)
            .setStation(pick.name)
            .setIsBrowsable(false)
            .setIsPlayable(true)
            .build()
        val item = MediaItem.Builder()
            .setUri(pick.url)
            .setMediaMetadata(metadata)
            .build()
        player.setMediaItem(item)
        player.prepare()
        player.play()
    }

    private inner class Callback : MediaSession.Callback {
        private val shuffleCommand = SessionCommand(CMD_SHUFFLE, android.os.Bundle.EMPTY)
        private val starCommand = SessionCommand(CMD_STAR, android.os.Bundle.EMPTY)

        private val shuffleButton = CommandButton.Builder(CommandButton.ICON_SHUFFLE_ON)
            .setDisplayName("shuffle")
            .setCustomIconResId(R.drawable.ic_shuffle)
            .setSessionCommand(shuffleCommand)
            .build()

        private val starButton = CommandButton.Builder(CommandButton.ICON_STAR_UNFILLED)
            .setDisplayName("favs")
            .setCustomIconResId(R.drawable.ic_star)
            .setSessionCommand(starCommand)
            .build()

        override fun onConnect(
            session: MediaSession,
            controller: MediaSession.ControllerInfo,
        ): MediaSession.ConnectionResult {
            val sessionCommands =
                MediaSession.ConnectionResult.DEFAULT_SESSION_AND_LIBRARY_COMMANDS.buildUpon()
                    .add(shuffleCommand)
                    .add(starCommand)
                    .build()
            return MediaSession.ConnectionResult.AcceptedResultBuilder(session)
                .setAvailableSessionCommands(sessionCommands)
                .setCustomLayout(listOf(shuffleButton, starButton))
                .build()
        }

        override fun onCustomCommand(
            session: MediaSession,
            controller: MediaSession.ControllerInfo,
            customCommand: SessionCommand,
            args: android.os.Bundle,
        ): ListenableFuture<SessionResult> {
            when (customCommand.customAction) {
                CMD_SHUFFLE -> {
                    shuffle()
                    return Futures.immediateFuture(SessionResult(SessionResult.RESULT_SUCCESS))
                }
                CMD_STAR -> {
                    Log.i("r4dio", "star toggled (favs not yet wired)")
                    return Futures.immediateFuture(SessionResult(SessionResult.RESULT_SUCCESS))
                }
            }
            return Futures.immediateFuture(SessionResult(SessionResult.RESULT_ERROR_NOT_SUPPORTED))
        }
    }
}
