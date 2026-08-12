package net.vchub.r4dio

import org.junit.Assert.assertEquals
import org.junit.Test

class FilterFetchTest {
    @Test
    fun a_filtered_country_not_yet_pulled_is_wanted() {
        assertEquals(setOf("UA"), countriesToPull(filter = setOf("UA"), alreadyPulled = emptySet()))
    }

    // the fetch is one request per country and the catalogue keeps what it got,
    // so pulling the same country twice in a session is pure waste.
    @Test
    fun a_country_already_pulled_is_not_wanted_again() {
        assertEquals(emptySet<String>(), countriesToPull(setOf("UA"), alreadyPulled = setOf("UA")))
    }

    @Test
    fun only_the_countries_still_missing_are_wanted() {
        assertEquals(setOf("PL"), countriesToPull(setOf("UA", "PL"), alreadyPulled = setOf("UA")))
    }

    // no filter means the user has expressed no narrow intent; the background
    // top-up is what serves them, not a burst of every-country requests.
    @Test
    fun an_empty_filter_wants_nothing() {
        assertEquals(emptySet<String>(), countriesToPull(emptySet(), alreadyPulled = emptySet()))
    }

    // the filter arrives from the desktop over the wire, so its case is not ours
    // to trust — "ua" and "UA" are the same country and must not fetch twice.
    @Test
    fun the_filter_is_matched_case_insensitively() {
        assertEquals(emptySet<String>(), countriesToPull(setOf("ua"), alreadyPulled = setOf("UA")))
        assertEquals(setOf("UA"), countriesToPull(setOf("ua"), alreadyPulled = emptySet()))
    }

    // a banned country must never be requested, even if one reaches the filter:
    // the ban is a product requirement on every ingest path.
    @Test
    fun a_banned_country_is_never_pulled() {
        assertEquals(emptySet<String>(), countriesToPull(setOf("RU", "BY"), alreadyPulled = emptySet()))
    }
}
