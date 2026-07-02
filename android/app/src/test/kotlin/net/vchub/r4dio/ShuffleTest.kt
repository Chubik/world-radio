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
}
