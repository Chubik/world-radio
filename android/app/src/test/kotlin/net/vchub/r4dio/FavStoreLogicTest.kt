package net.vchub.r4dio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.random.Random

class FavStoreLogicTest {
    private fun st(uuid: String) = Station(uuid, uuid, "http://$uuid", "", "", 0)

    @Test
    fun toggle_addsWhenAbsent() {
        assertEquals(setOf("a"), FavLogic.toggle(emptySet(), "a"))
    }

    @Test
    fun toggle_removesWhenPresent() {
        assertEquals(emptySet<String>(), FavLogic.toggle(setOf("a"), "a"))
    }

    @Test
    fun pickFav_returnsNull_forEmpty() {
        assertNull(FavLogic.pickFav(emptyList()))
    }

    @Test
    fun pickFav_returnsOne() {
        val p = FavLogic.pickFav(listOf(st("a"), st("b")), Random(42))!!
        assertTrue(p.uuid == "a" || p.uuid == "b")
    }
}
