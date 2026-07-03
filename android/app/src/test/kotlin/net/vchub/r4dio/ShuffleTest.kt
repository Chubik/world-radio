package net.vchub.r4dio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.random.Random

class ShuffleTest {
    private fun st(uuid: String, url: String) = Station(uuid, uuid, url, "", "", 0)

    @Test
    fun pickRandom_returnsNull_forEmpty() {
        assertNull(pickRandom(emptyList()))
    }

    @Test
    fun pickRandom_skipsStationsWithBlankUrl() {
        val list = listOf(st("a", ""), st("b", "http://x"))
        assertEquals("b", pickRandom(list, Random(1))?.uuid)
    }

    @Test
    fun pickRandom_returnsAPlayableOne() {
        val list = listOf(st("a", "http://a"), st("b", "http://b"))
        val p = pickRandom(list, Random(42))!!
        assertTrue(p.uuid == "a" || p.uuid == "b")
        assertTrue(p.url.isNotBlank())
    }

    @Test
    fun pickForScope_all_usesCatalog() {
        val cat = listOf(st("a", "http://a"))
        val p = pickForScope(Scope.ALL, cat, emptyList(), Random(1))
        assertEquals("a", p?.uuid)
    }

    @Test
    fun pickForScope_favs_usesFavs() {
        val cat = listOf(st("a", "http://a"))
        val favs = listOf(st("f", "http://f"))
        val p = pickForScope(Scope.FAVS, cat, favs, Random(1))
        assertEquals("f", p?.uuid)
    }

    @Test
    fun pickForScope_favs_emptyReturnsNull() {
        assertNull(pickForScope(Scope.FAVS, listOf(st("a", "http://a")), emptyList()))
    }
}
