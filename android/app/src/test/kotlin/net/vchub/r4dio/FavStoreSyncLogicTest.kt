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

    @Test
    fun mergedData_unions_each_field() {
        val local = SyncData(favs = listOf("a", "b"), blocked = emptyList(), excluded_countries = emptyList())
        val server = SyncData(favs = listOf("b", "c"), blocked = listOf("x"), excluded_countries = listOf("US"))
        val m = SyncMerge.mergedData(local, server)
        assertEquals(listOf("a", "b", "c"), m.favs)
        assertEquals(listOf("x"), m.blocked)
        assertEquals(listOf("US"), m.excluded_countries)
    }
}
