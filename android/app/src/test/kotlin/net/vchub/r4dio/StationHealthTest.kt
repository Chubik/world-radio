package net.vchub.r4dio

import org.junit.Assert.assertFalse
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class StationHealthTest {
    @Test
    fun blames_the_station_for_stream_side_errors() {
        // invalid content type, bad http status, file not found
        assertTrue(shouldBlame(2003))
        assertTrue(shouldBlame(2004))
        assertTrue(shouldBlame(2005))
        // parsing errors (3xxx)
        assertTrue(shouldBlame(3001))
        assertTrue(shouldBlame(3004))
        // decoding errors (4xxx)
        assertTrue(shouldBlame(4001))
        assertTrue(shouldBlame(4003))
    }

    @Test
    fun never_blames_the_device_network() {
        assertFalse(shouldBlame(2000)) // io unspecified
        assertFalse(shouldBlame(2001)) // network connection failed
        assertFalse(shouldBlame(2002)) // network connection timeout
    }

    @Test
    fun unknown_codes_do_not_blame() {
        assertFalse(shouldBlame(0))
        assertFalse(shouldBlame(1000)) // generic unspecified
        assertFalse(shouldBlame(2006)) // io no permission — device side
        assertFalse(shouldBlame(5001))
        assertFalse(shouldBlame(-1))
    }

    @Test
    fun genuine_failure_hides_on_first_strike() {
        val t = HealthTracker()
        assertTrue(t.onError(blame = true))
    }

    @Test
    fun network_failure_never_hides() {
        val t = HealthTracker()
        assertFalse(t.onError(blame = false))
    }

    @Test
    fun budget_stops_hiding_after_five_strikes_without_success() {
        val t = HealthTracker(budget = 5)
        repeat(5) { assertTrue(t.onError(blame = true)) }
        assertFalse(t.onError(blame = true))
        assertFalse(t.onError(blame = true))
    }

    @Test
    fun success_resets_the_budget() {
        val t = HealthTracker(budget = 5)
        repeat(5) { t.onError(blame = true) }
        assertFalse(t.onError(blame = true))
        t.onSuccess()
        assertTrue(t.onError(blame = true))
    }

    @Test
    fun network_errors_do_not_consume_the_budget() {
        val t = HealthTracker(budget = 2)
        repeat(10) { assertFalse(t.onError(blame = false)) }
        assertTrue(t.onError(blame = true))
        assertTrue(t.onError(blame = true))
        assertFalse(t.onError(blame = true))
    }

    @Test
    fun prune_keeps_only_uuids_still_reachable() {
        val hidden = setOf("dead1", "gone2", "fav3")
        val keep = setOf("dead1", "fav3", "alive4")
        assertEquals(setOf("dead1", "fav3"), pruneHidden(hidden, keep))
    }

    @Test
    fun hidden_uuids_unioned_into_blocked_never_get_picked() {
        val cat = listOf(
            Station("dead1", "Dead FM", "http://x/1", "UA", "MP3", 128),
            Station("alive2", "Alive FM", "http://x/2", "UA", "MP3", 128),
        )
        val blocked = emptySet<String>()
        val hidden = setOf("dead1")
        repeat(20) {
            val pick = pickForScope(Scope.ALL, cat, emptyList(), emptySet(), blocked + hidden)
            assertEquals("alive2", pick?.uuid)
        }
    }
}
