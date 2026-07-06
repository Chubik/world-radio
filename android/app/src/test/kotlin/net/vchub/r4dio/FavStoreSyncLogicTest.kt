package net.vchub.r4dio

import org.junit.Assert.assertEquals
import org.junit.Test

class FavStoreSyncLogicTest {
    @Test
    fun mergedFavs_unions_local_and_remote() {
        val out = SyncMerge.mergedFavs(setOf("a", "b"), listOf("b", "c"))
        assertEquals(setOf("a", "b", "c"), out)
    }

    @Test
    fun mergedFavs_emptyRemote_keepsLocal() {
        assertEquals(setOf("a"), SyncMerge.mergedFavs(setOf("a"), emptyList()))
    }

    @Test
    fun mergedFavs_emptyLocal_takesRemote() {
        assertEquals(setOf("x"), SyncMerge.mergedFavs(emptySet(), listOf("x")))
    }
}
