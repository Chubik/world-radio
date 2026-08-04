package net.vchub.r4dio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class CatalogFilterTest {
    private fun station(uuid: String, country: String = "UA", name: String = "Name") =
        Station(uuid, name, "http://x/$uuid", country, "MP3", 128)

    @Test
    fun keeps_only_the_target_count() {
        val input = (1..50).map { station("u$it") }
        assertEquals(10, takeAllowed(input, emptySet(), 10).size)
    }

    @Test
    fun drops_banned_countries_before_taking_the_target() {
        val input = listOf(
            station("ru1", country = "RU"),
            station("by1", country = "BY"),
            station("ua1"),
            station("ua2"),
        )
        val out = takeAllowed(input, emptySet(), 4)
        assertEquals(listOf("ua1", "ua2"), out.map { it.uuid })
    }

    @Test
    fun drops_user_hidden_countries() {
        val input = listOf(station("de1", country = "DE"), station("ua1"))
        val out = takeAllowed(input, setOf("DE"), 4)
        assertEquals(listOf("ua1"), out.map { it.uuid })
    }

    @Test
    fun a_full_target_survives_interleaved_banned_entries() {
        // the point of over-fetching: banned entries must not eat into the target
        val input = (1..30).flatMap { listOf(station("ru$it", country = "RU"), station("ok$it")) }
        val out = takeAllowed(input, emptySet(), 20)
        assertEquals(20, out.size)
        assertTrue(out.none { it.country == "RU" })
    }

    @Test
    fun fewer_survivors_than_the_target_returns_all_survivors() {
        val input = listOf(station("ua1"), station("ru1", country = "RU"))
        assertEquals(listOf("ua1"), takeAllowed(input, emptySet(), 10).map { it.uuid })
    }

    @Test
    fun drops_stations_with_a_blank_url() {
        val input = listOf(Station("b", "Blank", "", "UA", "MP3", 128), station("ua1"))
        assertEquals(listOf("ua1"), takeAllowed(input, emptySet(), 10).map { it.uuid })
    }
}
