package net.vchub.r4dio

import org.junit.Assert.assertEquals
import org.junit.Test

class FavSyncTest {
    private fun st(uuid: String, name: String = uuid) =
        Station(uuid, name, "http://$uuid", "DE", "MP3", 128)

    @Test
    fun missingUuids_returnsWantedNotPresentInKnown() {
        val known = listOf(st("a"), st("b"))
        assertEquals(listOf("c"), FavSync.missingUuids(setOf("a", "c"), known))
    }

    @Test
    fun missingUuids_emptyWhenAllKnown() {
        val known = listOf(st("a"), st("b"))
        assertEquals(emptyList<String>(), FavSync.missingUuids(setOf("a", "b"), known))
    }

    @Test
    fun reconcile_keepsOnlyWantedStations() {
        // "b" was un-starred on the other device — it must drop out of the cache
        val known = listOf(st("a"), st("b"))
        val out = FavSync.reconcile(setOf("a"), known, emptyList())
        assertEquals(listOf("a"), out.map { it.uuid })
    }

    @Test
    fun reconcile_addsFetchedStations() {
        val out = FavSync.reconcile(setOf("a", "c"), listOf(st("a")), listOf(st("c")))
        assertEquals(setOf("a", "c"), out.map { it.uuid }.toSet())
    }

    @Test
    fun reconcile_prefersKnownOverFetchedForSameUuid() {
        val known = listOf(st("a", name = "local name"))
        val fetched = listOf(st("a", name = "remote name"))
        val out = FavSync.reconcile(setOf("a"), known, fetched)
        assertEquals(listOf("local name"), out.map { it.name })
    }

    @Test
    fun reconcile_dropsUnresolvableUuids() {
        // "z" is in neither known nor fetched — a station deleted upstream.
        // it must not appear as a phantom entry.
        val out = FavSync.reconcile(setOf("a", "z"), listOf(st("a")), emptyList())
        assertEquals(listOf("a"), out.map { it.uuid })
    }

    @Test
    fun reconcile_dropsStationsWithoutUrl() {
        val fetched = listOf(Station("c", "no url", "", "DE", "MP3", 128))
        val out = FavSync.reconcile(setOf("c"), emptyList(), fetched)
        assertEquals(emptyList<String>(), out.map { it.uuid })
    }

    @Test
    fun reconcile_emptyWantedClearsEverything() {
        val out = FavSync.reconcile(emptySet(), listOf(st("a")), emptyList())
        assertEquals(emptyList<String>(), out.map { it.uuid })
    }
}
