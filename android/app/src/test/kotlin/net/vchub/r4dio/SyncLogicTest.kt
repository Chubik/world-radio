package net.vchub.r4dio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SyncLogicTest {
    @Test
    fun a_real_key_is_recognised() {
        assertTrue(isSyncKey("r4-abc123"))
        assertTrue(isSyncKey("  r4-abc123  "))
    }

    @Test
    fun anything_that_is_not_a_key_is_rejected() {
        assertFalse(isSyncKey(null))
        assertFalse(isSyncKey(""))
        assertFalse(isSyncKey("abc123"))
        // the prefix alone carries no account
        assertFalse(isSyncKey("r4-"))
        assertFalse(isSyncKey("https://example.com"))
    }

    // backing out of the camera is how someone changes their mind, so it must
    // not read as "that did not work".
    @Test
    fun a_cancelled_scan_is_not_a_failure() {
        assertEquals(ScanOutcome.Cancelled, scanOutcome(null))
    }

    @Test
    fun scanning_someone_elses_qr_says_so() {
        assertEquals(ScanOutcome.NotAKey, scanOutcome("https://example.com"))
    }

    @Test
    fun scanning_a_key_links_it_trimmed() {
        assertEquals(ScanOutcome.Linked("r4-abc123"), scanOutcome(" r4-abc123 "))
    }

    // a list nobody scrolls to the end of is a list nobody uses.
    @Test
    fun the_offered_countries_are_a_curated_list_in_order() {
        assertEquals(40, OFFERED_COUNTRY_CODES.size)
        assertEquals(OFFERED_COUNTRY_CODES.sorted(), OFFERED_COUNTRY_CODES)
        assertTrue(OFFERED_COUNTRY_CODES.contains("UA"))
    }

    // the ban is a product requirement everywhere, and a country the user could
    // "un-hide" would be a way back in.
    @Test
    fun banned_countries_are_not_offered() {
        assertFalse(OFFERED_COUNTRY_CODES.contains("RU"))
        assertFalse(OFFERED_COUNTRY_CODES.contains("BY"))
    }
}
