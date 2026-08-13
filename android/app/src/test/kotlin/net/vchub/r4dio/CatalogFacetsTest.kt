package net.vchub.r4dio

import org.junit.Assert.assertEquals
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
}
