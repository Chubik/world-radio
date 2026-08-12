package net.vchub.r4dio.ui

import android.content.ComponentName
import android.content.Context
import android.os.Bundle
import androidx.media3.common.Player
import androidx.media3.session.MediaController
import androidx.media3.session.SessionCommand
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.MoreExecutors
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import net.vchub.r4dio.PlaybackService

/** the service packs country/codec/bitrate into one artist string. */
internal fun parseArtist(artist: String): Pair<String, String> {
    val parts = artist.split(" · ")
    val country = parts.getOrNull(0).orEmpty()
    val codec = parts.getOrNull(1).orEmpty()
    return country to codec
}

/** what [PlayerConnection] needs of a controller, so its lifecycle rules can be
 *  tested without a live MediaSession. */
interface ControllerHandle {
    val isPlaying: Boolean
    val sessionExtras: Bundle
    fun sendCustomCommand(command: String)
    fun release()
}

/**
 * the single controller for the whole app. every tab reads [state]; none of
 * them build a controller of their own, so there is one connect/release
 * lifecycle rather than one per screen.
 *
 * [connector] is a seam: production passes [mediaControllerConnector], tests
 * pass a fake they resolve by hand. the guard rules below are the ones worth
 * testing, and a live MediaSession cannot exercise them deterministically.
 */
class PlayerConnection(
    private val connector: (onReady: (ControllerHandle) -> Unit) -> Unit,
) {
    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

    private var controller: ControllerHandle? = null

    // the controller can arrive after release(); without this the callback would
    // hand us one nobody will ever close, keeping the session alive for the life
    // of the process.
    @Volatile private var released = false

    fun connect() {
        released = false
        connector { handle ->
            if (released) {
                handle.release()
                return@connector
            }
            controller = handle
            _state.value = uiStateFromExtras(handle.sessionExtras, _state.value)
                .copy(isPlaying = handle.isPlaying)
        }
    }

    fun release() {
        released = true
        controller?.release()
        controller = null
    }

    /** dropped when nothing is connected yet: a tab can be tapped before the
     *  controller resolves, and that is not a crash. */
    fun send(command: String) {
        controller?.sendCustomCommand(command)
    }

    internal fun onExtras(extras: Bundle) {
        _state.value = uiStateFromExtras(extras, _state.value)
    }

    internal fun onPlaying(isPlaying: Boolean) {
        _state.value = _state.value.copy(isPlaying = isPlaying)
    }

    internal fun onMetadata(station: String, artist: String) {
        val (country, codec) = parseArtist(artist)
        _state.value = _state.value.copy(stationName = station, country = country, codec = codec)
    }
}

/** the real connector wraps Media3 and forwards its callbacks into the three
 *  internal folds above. keeps the runCatching/directExecutor shape the
 *  activity used to own. */
fun mediaControllerConnector(
    context: Context,
    conn: () -> PlayerConnection,
): (onReady: (ControllerHandle) -> Unit) -> Unit = { onReady ->
    val token = SessionToken(context, ComponentName(context, PlaybackService::class.java))
    val future = MediaController.Builder(context, token)
        .setListener(object : MediaController.Listener {
            override fun onExtrasChanged(controller: MediaController, extras: Bundle) {
                conn().onExtras(extras)
            }
        })
        .buildAsync()
    future.addListener({
        val c = runCatching { future.get() }.getOrNull() ?: return@addListener
        c.addListener(object : Player.Listener {
            override fun onIsPlayingChanged(isPlaying: Boolean) = conn().onPlaying(isPlaying)
            override fun onMediaMetadataChanged(m: androidx.media3.common.MediaMetadata) =
                conn().onMetadata(
                    (m.station ?: m.title)?.toString().orEmpty(),
                    m.artist?.toString().orEmpty(),
                )
        })
        onReady(object : ControllerHandle {
            override val isPlaying get() = c.isPlaying
            override val sessionExtras get() = c.sessionExtras
            override fun sendCustomCommand(command: String) {
                c.sendCustomCommand(SessionCommand(command, Bundle.EMPTY), Bundle.EMPTY)
            }
            override fun release() = c.release()
        })
    }, MoreExecutors.directExecutor())
}
