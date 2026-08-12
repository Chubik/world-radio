package net.vchub.r4dio.ui

import android.os.Bundle
import net.vchub.r4dio.EXTRA_CATALOG_SIZE
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

private class FakeHandle(
    override val isPlaying: Boolean = false,
    override val sessionExtras: Bundle = Bundle(),
) : ControllerHandle {
    var released = false
    val sent = mutableListOf<String>()
    override fun sendCustomCommand(command: String) { sent.add(command) }
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

    // a tab can be tapped before the controller resolves; dropping the command
    // is correct, crashing is not.
    @Test
    fun a_command_before_connect_is_dropped_not_thrown() {
        val conn = PlayerConnection { }
        conn.send("net.vchub.r4dio.SHUFFLE")
    }

    @Test
    fun commands_reach_the_controller_once_connected() {
        val handle = FakeHandle()
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
        val second = FakeHandle()
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
}
