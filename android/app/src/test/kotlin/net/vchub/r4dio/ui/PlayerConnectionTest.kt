package net.vchub.r4dio.ui

import android.os.Bundle
import net.vchub.r4dio.ARG_UUID
import net.vchub.r4dio.CMD_PLAY_UUID
import net.vchub.r4dio.CMD_SHUFFLE
import net.vchub.r4dio.EXTRA_CATALOG_SIZE
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

private class FakeHandle(
    override val isPlaying: Boolean = false,
    override val sessionExtras: Bundle = Bundle(),
    override val mediaItemCount: Int = 0,
) : ControllerHandle {
    var released = false
    val sent = mutableListOf<String>()
    val sentArgs = mutableListOf<Bundle>()
    override fun sendCustomCommand(command: String, args: Bundle) {
        sent.add(command)
        sentArgs.add(args)
    }
    override fun release() { released = true }
}

@RunWith(RobolectricTestRunner::class)
class PlayerConnectionTest {
    @Test
    fun connecting_publishes_the_extras_the_session_already_had() {
        val handle = FakeHandle(
            isPlaying = true,
            sessionExtras = Bundle().apply { putInt(EXTRA_CATALOG_SIZE, 1286) },
        )
        var ready: ((ControllerHandle) -> Unit)? = null
        val conn = PlayerConnection { onReady -> ready = onReady }
        conn.connect()
        ready!!(handle)
        assertEquals(1286, conn.state.value.catalogueSize)
        assertTrue(conn.state.value.isPlaying)
    }

    // the controller future can resolve after the screen is gone. without the
    // released guard the callback hands us a controller nobody will ever close,
    // and it keeps the session alive for the life of the process.
    @Test
    fun a_controller_arriving_after_release_is_closed_immediately() {
        val handle = FakeHandle()
        var ready: ((ControllerHandle) -> Unit)? = null
        val conn = PlayerConnection { onReady -> ready = onReady }
        conn.connect()
        conn.release()
        ready!!(handle)
        assertTrue("late controller must be released", handle.released)
        assertTrue("and never used", handle.sent.isEmpty())
    }

    @Test
    fun release_closes_a_controller_that_did_arrive() {
        val handle = FakeHandle()
        var ready: ((ControllerHandle) -> Unit)? = null
        val conn = PlayerConnection { onReady -> ready = onReady }
        conn.connect()
        ready!!(handle)
        conn.release()
        assertTrue(handle.released)
    }

    // the product is a radio that plays the moment it opens, without being
    // looked at. an empty player on connect means a cold start, and the app
    // must shuffle itself rather than sit silent on "— idle —".
    @Test
    fun an_empty_player_shuffles_itself_on_connect() {
        val handle = FakeHandle(mediaItemCount = 0)
        var ready: ((ControllerHandle) -> Unit)? = null
        val conn = PlayerConnection { onReady -> ready = onReady }
        conn.connect()
        ready!!(handle)
        assertEquals(listOf(CMD_SHUFFLE), handle.sent)
    }

    // but a controller reconnecting to a session that is already loaded must
    // not restart it — coming back from the background would change station.
    @Test
    fun a_loaded_player_is_left_alone_on_connect() {
        val handle = FakeHandle(mediaItemCount = 1)
        var ready: ((ControllerHandle) -> Unit)? = null
        val conn = PlayerConnection { onReady -> ready = onReady }
        conn.connect()
        ready!!(handle)
        assertTrue("must not disturb a loaded session", handle.sent.isEmpty())
    }

    // a tab can be tapped before the controller resolves; dropping the command
    // is correct, crashing is not.
    @Test
    fun a_command_before_connect_is_dropped_not_thrown() {
        val conn = PlayerConnection { }
        conn.send("net.vchub.r4dio.SHUFFLE")
    }

    @Test
    fun commands_reach_the_controller_once_connected() {
        // loaded, so the connect-time auto-shuffle does not fire and the only
        // command on the list is the one this test sends.
        val handle = FakeHandle(mediaItemCount = 1)
        var ready: ((ControllerHandle) -> Unit)? = null
        val conn = PlayerConnection { onReady -> ready = onReady }
        conn.connect()
        ready!!(handle)
        conn.send("net.vchub.r4dio.SHUFFLE")
        assertEquals(listOf("net.vchub.r4dio.SHUFFLE"), handle.sent)
    }

    // reconnecting after a release must clear the guard, or the app comes back
    // from the background with a permanently dead controller.
    @Test
    fun reconnecting_after_release_works() {
        val second = FakeHandle(mediaItemCount = 1)
        var ready: ((ControllerHandle) -> Unit)? = null
        val conn = PlayerConnection { onReady -> ready = onReady }
        conn.connect()
        conn.release()
        conn.connect()
        ready!!(second)
        conn.send("net.vchub.r4dio.TOGGLE")
        assertEquals(listOf("net.vchub.r4dio.TOGGLE"), second.sent)
    }

    @Test
    fun release_before_connect_does_not_throw() {
        val conn = PlayerConnection { }
        conn.release()
    }

    @Test
    fun a_second_release_does_not_throw() {
        val handle = FakeHandle()
        var ready: ((ControllerHandle) -> Unit)? = null
        val conn = PlayerConnection { onReady -> ready = onReady }
        conn.connect()
        ready!!(handle)
        conn.release()
        conn.release()
    }

    @Test
    fun a_command_can_carry_an_argument() {
        val handle = FakeHandle(mediaItemCount = 1)
        var ready: ((ControllerHandle) -> Unit)? = null
        val conn = PlayerConnection { onReady -> ready = onReady }
        conn.connect()
        ready!!(handle)
        conn.send(CMD_PLAY_UUID, Bundle().apply { putString(ARG_UUID, "abc") })
        assertEquals(listOf(CMD_PLAY_UUID), handle.sent)
        assertEquals("abc", handle.sentArgs.single().getString(ARG_UUID))
    }

    // the catalogue can be tapped before the controller resolves; dropping the
    // tap is correct, crashing is not.
    @Test
    fun an_argument_command_before_connect_is_dropped_not_thrown() {
        PlayerConnection { }.send(CMD_PLAY_UUID, Bundle().apply { putString(ARG_UUID, "abc") })
    }
}
