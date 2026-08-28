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

    // PlaybackService shares one AtomicBoolean between the doorbell
    // (onStreamEvent) and ACTION_SYNC_NOW (onStartCommand) precisely so this
    // holds: two syncs from different triggers racing each other is the same
    // clearPushedPending/applyMerged hazard as two doorbells racing, not a
    // separate concern. a claim dropped here costs nothing durable — see
    // PendingChangesTest for why the pending change survives regardless of
    // which sync attempt gets collapsed.
    @Test
    fun a_ui_triggered_sync_and_a_doorbell_sync_share_the_same_slot() {
        val queued = AtomicBoolean(false)
        assertTrue("the doorbell claims first", ResyncGate.claim(queued))
        assertFalse(
            "a UI-triggered sync arriving while the doorbell's is in flight must not also run",
            ResyncGate.claim(queued),
        )
        ResyncGate.release(queued)
        assertTrue("once released, either source can claim again", ResyncGate.claim(queued))
    }
}
