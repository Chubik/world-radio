package net.vchub.r4dio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class HomeStateTest {
    @Test
    fun pill_is_hidden_when_the_user_has_hidden_no_countries() {
        assertFalse(showsHiddenPill(hiddenCount = 0, scope = "all"))
    }

    @Test
    fun pill_shows_in_all_scope_when_countries_are_hidden() {
        assertTrue(showsHiddenPill(hiddenCount = 1, scope = "all"))
        assertTrue(showsHiddenPill(hiddenCount = 40, scope = "all"))
    }

    // favourites deliberately ignore the country filter, so the pill would lie there
    @Test
    fun pill_is_hidden_in_favs_scope_even_when_countries_are_hidden() {
        assertFalse(showsHiddenPill(hiddenCount = 3, scope = "favs"))
    }

    @Test
    fun warn_when_filters_leave_nothing_playable() {
        assertTrue(isAllHiddenWarn(playableCount = 0, hiddenCount = 3, scope = "all", catalogLoaded = true))
    }

    // an empty catalogue with no filters set is a network problem, not a filter problem
    @Test
    fun no_warn_when_nothing_is_playable_but_no_country_is_hidden() {
        assertFalse(isAllHiddenWarn(playableCount = 0, hiddenCount = 0, scope = "all", catalogLoaded = true))
    }

    @Test
    fun no_warn_while_stations_remain_playable() {
        assertFalse(isAllHiddenWarn(playableCount = 1, hiddenCount = 3, scope = "all", catalogLoaded = true))
        assertFalse(isAllHiddenWarn(playableCount = 1000, hiddenCount = 40, scope = "all", catalogLoaded = true))
    }

    // in favs scope pickForScope falls back to the catalogue, so the filter is not
    // what is stopping playback; the existing no-favourites warn owns that case
    @Test
    fun no_warn_in_favs_scope() {
        assertFalse(isAllHiddenWarn(playableCount = 0, hiddenCount = 3, scope = "favs", catalogLoaded = true))
    }

    @Test
    fun unknown_scope_string_is_treated_as_all() {
        assertTrue(showsHiddenPill(hiddenCount = 2, scope = "something-else"))
        assertTrue(isAllHiddenWarn(playableCount = 0, hiddenCount = 2, scope = "something-else", catalogLoaded = true))
    }

    // cold start: loadStations() and syncNow() race with no ordering, so the sync
    // round-trip can publish playableCount = 0 before the catalogue thread has read
    // or fetched anything. hidden countries set from a previous session must not
    // produce a false "all hidden" warning at the moment the app opens.
    @Test
    fun no_warn_on_cold_start_before_the_catalogue_has_loaded() {
        assertFalse(isAllHiddenWarn(playableCount = 0, hiddenCount = 3, scope = "all", catalogLoaded = false))
    }

    // once the catalogue lands the same inputs must flip the warn on: this is not
    // a permanently disabled check, only deferred until there is something to judge
    @Test
    fun warn_appears_once_catalogue_loads_with_same_filters() {
        assertFalse(isAllHiddenWarn(playableCount = 0, hiddenCount = 3, scope = "all", catalogLoaded = false))
        assertTrue(isAllHiddenWarn(playableCount = 0, hiddenCount = 3, scope = "all", catalogLoaded = true))
    }

    // the actual bug this guard exists for: the user's filters were aggressive enough
    // that the fetch itself returned nothing allowed (Catalog.takeAllowed filters before
    // PlaybackService ever sees the result), so the load resolved with zero stations.
    // that is still a load that happened, not a load that hasn't happened yet, and the
    // warn must fire. fails under a `stations.isNotEmpty()`-style signal, which would
    // wrongly read this state as "not loaded" and suppress the warn forever.
    @Test
    fun warn_when_the_filters_emptied_the_fetch_itself() {
        assertTrue(isAllHiddenWarn(playableCount = 0, hiddenCount = 30, scope = "all", catalogLoaded = true))
    }

    // playableCount is counted from the loaded station list, so a non-zero count can
    // never coexist with "not loaded" — documents that this combination is unreachable.
    @Test
    fun no_warn_when_playable_without_a_loaded_catalogue_is_impossible_in_practice() {
        assertFalse(isAllHiddenWarn(playableCount = 5, hiddenCount = 3, scope = "all", catalogLoaded = false))
    }

    @Test
    fun no_filter_means_no_filter_pill() {
        assertNull(filterPillLabel(emptyList(), scope = "all"))
    }

    @Test
    fun the_filter_pill_names_the_countries() {
        assertEquals("FILTER: UA", filterPillLabel(listOf("UA"), scope = "all"))
        assertEquals("FILTER: UA·PL", filterPillLabel(listOf("UA", "PL"), scope = "all"))
        assertEquals("FILTER: UA·PL·DE", filterPillLabel(listOf("UA", "PL", "DE"), scope = "all"))
    }

    // a long list would push the scope pill off the row, so it counts the rest
    @Test
    fun a_long_filter_is_summarised_after_three_codes() {
        assertEquals("FILTER: UA·PL·DE +1", filterPillLabel(listOf("UA", "PL", "DE", "FR"), scope = "all"))
        assertEquals(
            "FILTER: UA·PL·DE +3",
            filterPillLabel(listOf("UA", "PL", "DE", "FR", "IT", "ES"), scope = "all"),
        )
    }

    // favourites bypass the filter (FavLogic.pickFav ignores it), so it is not in
    // force there — but hiding it taught the user that a filter they had set had
    // vanished. it stays on screen and is shown as not applying instead.
    @Test
    fun the_filter_pill_still_shows_in_favs_scope() {
        assertEquals("FILTER: UA", filterPillLabel(listOf("UA"), scope = "favs"))
    }

    @Test
    fun the_filter_is_in_force_only_outside_favs_scope() {
        assertTrue(filterIsInForce(listOf("UA"), scope = "all"))
        assertFalse(filterIsInForce(listOf("UA"), scope = "favs"))
    }

    // with no filter set there is nothing to be in force, in either scope.
    @Test
    fun an_empty_filter_is_never_in_force() {
        assertFalse(filterIsInForce(emptyList(), scope = "all"))
        assertFalse(filterIsInForce(emptyList(), scope = "favs"))
    }

    // the catalogue is no longer either "the top-1000" or "everything" — it grows,
    // and the user asked to be able to see where it has got to.
    @Test
    fun the_pill_names_how_many_stations_are_held() {
        assertEquals("1 240 STATIONS", catalogueLabel(1240, growing = false))
    }

    @Test
    fun a_growing_catalogue_says_so() {
        assertEquals("1 240 STATIONS +", catalogueLabel(1240, growing = true))
    }

    // before the first fetch resolves there is no number worth showing.
    @Test
    fun an_unknown_count_shows_nothing() {
        assertEquals("", catalogueLabel(0, growing = false))
    }

    // a download is the sharper fact: "+" only says more exists somewhere, this
    // says it is arriving right now.
    @Test
    fun a_download_in_flight_outranks_the_plus() {
        assertEquals(
            "1 240 STATIONS · LOADING…",
            catalogueLabel(1240, growing = true, fetching = true),
        )
    }

    // a fresh install has nothing to count, and silence there reads as a broken
    // app rather than a working one.
    @Test
    fun the_very_first_download_says_something_rather_than_nothing() {
        assertEquals("LOADING STATIONS…", catalogueLabel(0, growing = false, fetching = true))
    }

    // once it lands the label goes back to being a plain count.
    @Test
    fun a_finished_download_leaves_no_trace_in_the_label() {
        assertEquals("58 932 STATIONS", catalogueLabel(58932, growing = false, fetching = false))
    }

    // the separator is a space, not a comma or a dot: this screen is read at a
    // glance in a car, and both of those read as decimals in some locales.
    @Test
    fun thousands_are_grouped_with_a_space() {
        assertEquals("999 STATIONS", catalogueLabel(999, growing = false))
        assertEquals("1 000 STATIONS", catalogueLabel(1000, growing = false))
        assertEquals("20 000 STATIONS", catalogueLabel(20000, growing = false))
    }

    @Test
    fun the_awake_label_reflects_the_state() {
        assertEquals("\u2600 AWAKE", keepAwakeLabel(true))
        assertEquals("\u263e SLEEPS", keepAwakeLabel(false))
    }

    // the toggle is the only writer of this state, so tapping it must always be the
    // inverse of what is stored — never of what the window currently happens to do.
    @Test
    fun tapping_the_toggle_inverts_the_stored_state() {
        assertTrue(nextKeepAwake(false))
        assertFalse(nextKeepAwake(true))
    }
}
