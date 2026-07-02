package net.vchub.r4dio

import androidx.media3.common.ForwardingPlayer
import androidx.media3.common.MediaItem
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import kotlin.concurrent.thread

class PlaybackService : MediaSessionService() {
    private var session: MediaSession? = null
    private var exo: ExoPlayer? = null
    private val catalog = Catalog()
    @Volatile private var stations: List<Station> = emptyList()

    override fun onCreate() {
        super.onCreate()
        val player = ExoPlayer.Builder(this).build()
        exo = player
        val forwarding = object : ForwardingPlayer(player) {
            override fun seekToNext() = shuffle()
            override fun seekToNextMediaItem() = shuffle()
            override fun hasNextMediaItem() = true
        }
        session = MediaSession.Builder(this, forwarding).build()
        loadStations()
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    override fun onDestroy() {
        session?.release()
        exo?.release()
        session = null
        exo = null
        super.onDestroy()
    }

    private fun loadStations() {
        thread {
            stations = catalog.fetchStations()
        }
    }

    private fun shuffle() {
        val pick = pickRandom(stations) ?: return
        val player = exo ?: return
        player.setMediaItem(MediaItem.fromUri(pick.url))
        player.prepare()
        player.play()
    }
}
