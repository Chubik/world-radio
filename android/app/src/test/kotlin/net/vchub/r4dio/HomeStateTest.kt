package net.vchub.r4dio

import org.junit.Assert.assertFalse
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

    @Test
    fun no_warn_when_catalogue_not_loaded_even_with_many_hidden_countries() {
        assertFalse(isAllHiddenWarn(playableCount = 0, hiddenCount = 40, scope = "all", catalogLoaded = false))
    }

    // once the catalogue lands the same inputs must flip the warn on: this is not
    // a permanently disabled check, only deferred until there is something to judge
    @Test
    fun warn_appears_once_catalogue_loads_with_same_filters() {
        assertFalse(isAllHiddenWarn(playableCount = 0, hiddenCount = 3, scope = "all", catalogLoaded = false))
        assertTrue(isAllHiddenWarn(playableCount = 0, hiddenCount = 3, scope = "all", catalogLoaded = true))
    }

    @Test
    fun no_warn_when_catalogue_not_loaded_and_no_countries_hidden() {
        assertFalse(isAllHiddenWarn(playableCount = 0, hiddenCount = 0, scope = "all", catalogLoaded = false))
    }
}
