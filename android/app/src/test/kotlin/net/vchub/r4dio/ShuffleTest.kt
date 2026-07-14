package net.vchub.r4dio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.random.Random

class ShuffleTest {
    private fun st(uuid: String, url: String) = Station(uuid, uuid, url, "", "", 0)
    private fun stc(uuid: String, country: String, name: String = uuid) =
        Station(uuid, name, "http://$uuid", country, "", 0)

    @Test
    fun pickRandom_returnsNull_forEmpty() {
        assertNull(pickRandom(emptyList()))
    }

    @Test
    fun isExcluded_blocksRussiaByCountry() {
        assertTrue(isExcluded(stc("a", "RU")))
        assertTrue(isExcluded(stc("b", "ru")))
    }

    @Test
    fun isExcluded_blocksBelarusByCountry() {
        assertTrue(isExcluded(stc("a", "BY")))
    }

    @Test
    fun isExcluded_blocksByNameSubstring() {
        assertTrue(isExcluded(stc("a", "DE", "Radio Moscow")))
        assertTrue(isExcluded(stc("b", "US", "Русское Радио")))
    }

    @Test
    fun isExcluded_allowsOthers() {
        assertTrue(!isExcluded(stc("a", "UA", "Radio Ukraine")))
        assertTrue(!isExcluded(stc("b", "DE", "Antenne Bayern")))
    }

    @Test
    fun pickRandom_neverPicksExcluded() {
        val list = listOf(stc("ru", "RU"), stc("ua", "UA"))
        repeat(20) {
            val p = pickRandom(list, rng = Random(it.toLong()))!!
            assertEquals("ua", p.uuid)
        }
    }

    @Test
    fun pickRandom_skipsStationsWithBlankUrl() {
        val list = listOf(st("a", ""), st("b", "http://x"))
        assertEquals("b", pickRandom(list, rng = Random(1))?.uuid)
    }

    @Test
    fun pickRandom_returnsAPlayableOne() {
        val list = listOf(st("a", "http://a"), st("b", "http://b"))
        val p = pickRandom(list, rng = Random(42))!!
        assertTrue(p.uuid == "a" || p.uuid == "b")
        assertTrue(p.url.isNotBlank())
    }

    @Test
    fun pickForScope_all_usesCatalog() {
        val cat = listOf(st("a", "http://a"))
        val p = pickForScope(Scope.ALL, cat, emptyList(), rng = Random(1))
        assertEquals("a", p?.uuid)
    }

    @Test
    fun pickForScope_favs_usesFavs() {
        val cat = listOf(st("a", "http://a"))
        val favs = listOf(st("f", "http://f"))
        val p = pickForScope(Scope.FAVS, cat, favs, rng = Random(1))
        assertEquals("f", p?.uuid)
    }

    @Test
    fun pickForScope_favs_emptyReturnsNull() {
        assertNull(pickForScope(Scope.FAVS, listOf(st("a", "http://a")), emptyList()))
    }

    @Test
    fun userExcludedCountryIsNeverPicked() {
        val stations = listOf(stc("1", "FR", "FR one"), stc("2", "US", "US one"))
        repeat(50) {
            val pick = pickRandom(stations, userExcluded = setOf("US"))
            assertNotNull(pick)
            assertEquals("US is user-excluded, must never be picked", "FR", pick!!.country)
        }
    }

    @Test
    fun ruByStillExcludedRegardlessOfUserSet() {
        val stations = listOf(stc("1", "RU", "ru"))
        assertNull(pickRandom(stations, userExcluded = emptySet()))
    }
}
