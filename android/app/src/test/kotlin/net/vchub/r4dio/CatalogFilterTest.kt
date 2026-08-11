package net.vchub.r4dio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.random.Random

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

    @Test
    fun a_blocked_station_is_not_allowed() {
        val s = station("abc", country = "NL")
        assertTrue(allowedStation(s))
        assertFalse(allowedStation(s, blocked = setOf("abc")))
    }

    @Test
    fun takeAllowed_drops_blocked_stations_and_still_fills_the_target() {
        val input = (1..10).map { station("s$it", country = "NL") }
        val kept = takeAllowed(input, userExcluded = emptySet(), blocked = setOf("s1", "s2"), target = 5)
        assertEquals(5, kept.size)
        assertTrue(kept.none { it.uuid in setOf("s1", "s2") })
    }

    @Test
    fun pickRandom_never_picks_a_blocked_station() {
        val input = listOf(station("bad"), station("ok"))
        repeat(20) {
            val p = pickRandom(input, blocked = setOf("bad"), rng = Random(it.toLong()))!!
            assertEquals("ok", p.uuid)
        }
    }

    @Test
    fun blocking_beats_favouriting() {
        // a blocked favourite must not play: blocking is a pointed "never this one",
        // unlike an excluded country, which a favourite still outranks.
        val favs = listOf(station("fav1", country = "DE"), station("fav2"))
        repeat(20) {
            val p = FavLogic.pickFav(favs, blocked = setOf("fav1"), rng = Random(it.toLong()))!!
            assertEquals("fav2", p.uuid)
        }
        assertEquals(null, FavLogic.pickFav(favs, blocked = setOf("fav1", "fav2")))
    }

    @Test
    fun an_excluded_country_still_loses_to_a_favourite() {
        // guards the deliberate asymmetry above: country filters do not apply in favs scope
        val favs = listOf(station("fav1", country = "DE"))
        assertEquals("fav1", FavLogic.pickFav(favs)?.uuid)
    }

    @Test
    fun favs_scope_skips_a_blocked_favourite_rather_than_falling_back() {
        val cat = listOf(station("cat1"))
        val favs = listOf(station("fav1"), station("fav2"))
        val out = pickForScopeDetailed(
            Scope.FAVS,
            cat,
            favs,
            blocked = setOf("fav1"),
            rng = Random(7),
        )
        assertEquals("fav2", out.station?.uuid)
        assertFalse(out.usedFallback)
    }

    @Test
    fun favs_scope_falls_back_to_the_catalogue_when_every_favourite_is_blocked() {
        val cat = listOf(station("cat1"))
        val favs = listOf(station("fav1"))
        val out = pickForScopeDetailed(
            Scope.FAVS,
            cat,
            favs,
            blocked = setOf("fav1"),
            rng = Random(7),
        )
        assertEquals("cat1", out.station?.uuid)
        assertTrue(out.usedFallback)
    }

    @Test
    fun a_blocked_station_is_not_picked_from_the_catalogue_in_all_scope() {
        val cat = listOf(station("bad"), station("ok"))
        repeat(20) {
            val out = pickForScopeDetailed(
                Scope.ALL,
                cat,
                emptyList(),
                blocked = setOf("bad"),
                rng = Random(it.toLong()),
            )
            assertEquals("ok", out.station?.uuid)
        }
    }

    @Test
    fun empty_filter_is_unrestricted() {
        val s = station("de1", country = "DE")
        assertTrue(allowedStation(s, included = emptySet()))
    }

    @Test
    fun the_filter_keeps_only_its_own_countries() {
        assertTrue(allowedStation(station("ua1", country = "UA"), included = setOf("UA")))
        assertFalse(allowedStation(station("de1", country = "DE"), included = setOf("UA")))
    }

    @Test
    fun the_filter_matches_regardless_of_case() {
        assertTrue(allowedStation(station("ua1", country = "ua"), included = setOf("UA")))
    }

    @Test
    fun filter_restricts_all_scope_picks() {
        val cat = listOf(
            station("ua1", country = "UA"),
            station("de1", country = "DE"),
            station("pl1", country = "PL"),
        )
        repeat(20) {
            val out = pickForScopeDetailed(
                Scope.ALL,
                cat,
                emptyList(),
                included = setOf("UA"),
                rng = Random(it.toLong()),
            )
            assertEquals("ua1", out.station?.uuid)
        }
    }

    @Test
    fun pickRandom_honours_the_filter() {
        val cat = listOf(station("ua1", country = "UA"), station("de1", country = "DE"))
        repeat(20) {
            assertEquals("ua1", pickRandom(cat, included = setOf("UA"), rng = Random(it.toLong()))?.uuid)
        }
    }

    // the filter is a taste filter, exactly like an excluded country — an explicit
    // star outranks it, same asymmetry blocking deliberately does not share.
    @Test
    fun favs_scope_ignores_the_filter() {
        val cat = listOf(station("cat1", country = "UA"))
        val favs = listOf(station("fav1", country = "DE"))
        repeat(20) {
            val out = pickForScopeDetailed(
                Scope.FAVS,
                cat,
                favs,
                included = setOf("UA"),
                rng = Random(it.toLong()),
            )
            assertEquals("fav1", out.station?.uuid)
            assertFalse(out.usedFallback)
        }
    }

    // a filter that matches nothing must report the fallback rather than pretend
    // it picked under the filter — the same honesty the favs fallback already has.
    @Test
    fun a_filter_matching_nothing_leaves_all_scope_with_no_pick() {
        val cat = listOf(station("de1", country = "DE"))
        val out = pickForScopeDetailed(Scope.ALL, cat, emptyList(), included = setOf("UA"))
        assertNull(out.station)
    }

    // blocking still outranks the filter: an included country does not resurrect
    // a station the user pointedly blocked.
    @Test
    fun a_blocked_station_in_a_filtered_country_still_never_plays() {
        assertFalse(
            allowedStation(station("ua1", country = "UA"), included = setOf("UA"), blocked = setOf("ua1")),
        )
    }

    // and an excluded country still wins over an included one, so a country the
    // user hid cannot come back through the shuffle filter.
    @Test
    fun an_excluded_country_beats_the_filter() {
        assertFalse(
            allowedStation(station("ua1", country = "UA"), userExcluded = setOf("UA"), included = setOf("UA")),
        )
    }
}
