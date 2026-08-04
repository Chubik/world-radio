package net.vchub.r4dio

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class CatalogTtlTest {
    private val day = 86_400L

    @Test
    fun never_synced_is_stale() {
        assertTrue(catalogIsStale(0L, now = 1_000_000L, ttlSecs = day))
    }

    @Test
    fun just_synced_is_fresh() {
        assertFalse(catalogIsStale(1_000_000L - 600, now = 1_000_000L, ttlSecs = day))
    }

    @Test
    fun older_than_the_ttl_is_stale() {
        assertTrue(catalogIsStale(1_000_000L - 25 * 3600, now = 1_000_000L, ttlSecs = day))
    }

    @Test
    fun exactly_at_the_ttl_is_stale() {
        assertTrue(catalogIsStale(1_000_000L - day, now = 1_000_000L, ttlSecs = day))
    }
}
