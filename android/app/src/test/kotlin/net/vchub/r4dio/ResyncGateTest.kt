package net.vchub.r4dio

import java.util.concurrent.atomic.AtomicBoolean
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ResyncGateTest {
    @Test
    fun a_doorbell_claims_the_slot() {
        val queued = AtomicBoolean(false)
        assertTrue(ResyncGate.claim(queued))
    }

    // the debounce: a burst while a sync is still queued must not pile up one
    // re-sync per event.
    @Test
    fun two_rapid_doorbells_claim_the_slot_only_once() {
        val queued = AtomicBoolean(false)
        assertTrue(ResyncGate.claim(queued))
        assertFalse(ResyncGate.claim(queued))
        assertFalse(ResyncGate.claim(queued))
    }

    // once the sync has run the slot must free up again, or live updates would
    // stop after the very first one.
    @Test
    fun a_doorbell_after_the_sync_ran_claims_again() {
        val queued = AtomicBoolean(false)
        assertTrue(ResyncGate.claim(queued))
        ResyncGate.release(queued)
        assertTrue(ResyncGate.claim(queued))
    }
}
