package net.vchub.r4dio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class CatalogFacetsTest {
    // the point of the curated list: the commonest tags in the real cache are
    // "radio", "fm", "méxico" and one station's own name, none of which is a
    // genre a person would filter by.
    @Test
    fun the_genre_rows_are_the_curated_list_not_the_commonest_tags() {
        val facets = listOf("radio" to 2304, "méxico" to 2024, "pop" to 5527, "jazz" to 1130)
        assertEquals(listOf("pop" to 5527, "jazz" to 1130), offeredGenreRows(facets))
    }

    // a row the catalogue cannot fill is a dead end, not a filter.
    @Test
    fun a_genre_the_catalogue_has_none_of_is_dropped() {
        assertEquals(listOf("rock" to 12), offeredGenreRows(listOf("rock" to 12)))
    }

    @Test
    fun the_genre_rows_keep_the_curated_order_not_the_count_order() {
        val facets = listOf("jazz" to 9000, "pop" to 1, "rock" to 5)
        assertEquals(listOf("pop", "rock", "jazz"), offeredGenreRows(facets).map { it.first })
    }

    // the cache carries "UNKNOWN" (an absence, not a choice) and video muxes;
    // a codec row must be something a listener would actually pick.
    @Test
    fun the_codec_rows_drop_unknown_and_the_video_muxes() {
        val facets = listOf(
            "MP3" to 40274, "AAC+" to 8436, "AAC" to 7480, "UNKNOWN" to 1965,
            "OGG" to 668, "AAC,H.264" to 67, "MP4" to 17, "FLV" to 16,
        )
        assertEquals(
            listOf("MP3", "AAC+", "AAC", "OGG"),
            offeredCodecRows(facets).map { it.first },
        )
    }

    @Test
    fun a_chip_exists_for_every_value_in_force() {
        val filters = CatalogFilters(
            countries = setOf("UA", "PL"),
            genres = setOf("jazz"),
            codecs = setOf("MP3"),
            minBitrate = 128,
        )
        assertEquals(
            listOf("PL", "UA", "JAZZ", "MP3", "≥128k"),
            activeChips(filters).map { it.label },
        )
    }

    @Test
    fun no_filters_means_no_chips() {
        assertTrue(activeChips(CatalogFilters()).isEmpty())
    }

    // dropping one chip must leave every other narrowing exactly as it was.
    @Test
    fun dropping_a_chip_removes_only_that_value() {
        val filters = CatalogFilters(countries = setOf("UA", "PL"), genres = setOf("jazz"), minBitrate = 128)
        val chip = activeChips(filters).first { it.label == "UA" }
        val next = withoutChip(filters, chip)
        assertEquals(setOf("PL"), next.countries)
        assertEquals(setOf("jazz"), next.genres)
        assertEquals(128, next.minBitrate)
    }

    @Test
    fun dropping_the_bitrate_chip_clears_the_minimum() {
        val filters = CatalogFilters(minBitrate = 256)
        assertEquals(0, withoutChip(filters, activeChips(filters).single()).minBitrate)
    }

    @Test
    fun toggling_a_value_adds_it_then_removes_it() {
        assertEquals(setOf("UA"), toggleValue(emptySet(), "UA"))
        assertEquals(emptySet<String>(), toggleValue(setOf("UA"), "UA"))
    }

    // the sheet builds its filter values from facet rows, and searchCatalog
    // compares them against the station's uppercased country and codec — a
    // lowercase value there matches nothing and reports no error.
    @Test
    fun facet_values_are_in_the_case_searchCatalog_compares_against() {
        val stations = listOf(
            Station("a", "A", "u", "ua", "mp3", 128, "jazz"),
            Station("b", "B", "u", "pl", "aac", 64, "pop"),
        )
        val country = countryFacets(stations).first().first
        val codec = codecFacets(stations).first().first
        assertEquals(1, searchCatalog(stations, "", CatalogFilters(countries = setOf(country))).size)
        assertEquals(1, searchCatalog(stations, "", CatalogFilters(codecs = setOf(codec))).size)
        val genre = genreFacets(stations).first().first
        assertEquals(1, searchCatalog(stations, "", CatalogFilters(genres = setOf(genre))).size)
    }

    @Test
    fun any_is_the_first_bitrate_step() {
        assertEquals(0, BITRATE_STEPS.first())
    }

    // the catalogue carries 240 country codes, 125 of them with fewer than 20
    // stations. an uncapped list buries the genre and codec groups under a
    // scroll nobody will finish, so the sheet shows the commonest and stops.
    @Test
    fun the_country_rows_are_capped() {
        val many = (1..300).map { "C$it" to (301 - it) }
        val rows = offeredCountryRows(many)
        assertEquals(COUNTRY_ROW_CAP, rows.size)
        assertEquals("C1", rows.first().first)
    }

    @Test
    fun a_short_country_list_is_untouched() {
        val few = listOf("UA" to 352, "PL" to 120)
        assertEquals(few, offeredCountryRows(few))
    }

    // a country with nothing in it would be a row that can only disappoint.
    @Test
    fun empty_countries_are_dropped() {
        assertEquals(listOf("UA" to 352), offeredCountryRows(listOf("UA" to 352, "ZZ" to 0)))
    }

    // one tap under the search field, where the same genre through the sheet is
    // four. they come from the head of the curated list, so they are the genres
    // covering most of the real catalogue.
    @Test
    fun the_quick_chips_offer_the_commonest_genres() {
        assertEquals(6, quickGenreChips(CatalogFilters()).size)
        assertEquals(OFFERED_GENRES.take(6), quickGenreChips(CatalogFilters()))
    }

    // an active genre already shows in the row as a chip with a ✕. offering it
    // again beside itself would let someone tap one and watch the other change.
    @Test
    fun a_genre_already_chosen_is_not_offered_again() {
        val out = quickGenreChips(CatalogFilters(genres = setOf("pop")))
        assertFalse(out.contains("pop"))
        assertEquals(5, out.size)
    }

    @Test
    fun a_genre_chosen_outside_the_quick_list_leaves_the_row_alone() {
        assertEquals(6, quickGenreChips(CatalogFilters(genres = setOf("reggae"))).size)
    }

    // a rotation must not silently drop a filter the user set. every field has
    // to survive the flatten/restore round trip, or the screen quietly forgets.
    @Test
    fun filters_survive_the_save_restore_round_trip() {
        val f = CatalogFilters(
            countries = setOf("UA", "PL"),
            genres = setOf("jazz"),
            codecs = setOf("AAC"),
            minBitrate = 128,
            sort = SortOrder.BITRATE,
        )
        assertEquals(f, filtersFromList(filtersToList(f)))
    }

    @Test
    fun an_empty_filter_set_round_trips_too() {
        assertEquals(CatalogFilters(), filtersFromList(filtersToList(CatalogFilters())))
    }

    @Test
    fun every_sort_order_survives_the_round_trip() {
        for (order in SortOrder.entries) {
            val f = CatalogFilters(sort = order)
            assertEquals(order, filtersFromList(filtersToList(f)).sort)
        }
    }

    // state saved by a build that had no sort control is four items long, and a
    // rotation right after an update must not crash on the missing fifth.
    @Test
    fun state_saved_before_sorting_existed_still_restores() {
        val old = listOf(listOf("UA"), listOf<String>(), listOf<String>(), 0)
        val restored = filtersFromList(old)
        assertEquals(setOf("UA"), restored.countries)
        assertEquals(SortOrder.POPULAR, restored.sort)
    }

    // a saved name this build does not know (a downgrade) is not a crash.
    @Test
    fun an_unknown_sort_name_falls_back_to_the_default() {
        val odd = listOf(listOf<String>(), listOf<String>(), listOf<String>(), 0, "SOMETHING_ELSE")
        assertEquals(SortOrder.POPULAR, filtersFromList(odd).sort)
    }
}
