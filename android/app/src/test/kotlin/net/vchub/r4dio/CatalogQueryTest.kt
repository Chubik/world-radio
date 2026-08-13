package net.vchub.r4dio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class CatalogQueryTest {
    private fun st(
        uuid: String, name: String, country: String = "UA",
        codec: String = "MP3", bitrate: Int = 128, tags: String = "",
    ) = Station(uuid, name, "http://x/$uuid", country, codec, bitrate, tags)

    private val all = listOf(
        st("a", "Jazz Cafe", tags = "jazz,lounge"),
        st("b", "Radio Trek", country = "UA", tags = "news"),
        st("c", "Kyiv Talk", country = "UA", codec = "AAC", bitrate = 64, tags = "talk,news"),
        st("d", "Warsaw Jazz", country = "PL", codec = "AAC", bitrate = 256, tags = "jazz"),
    )

    @Test
    fun an_empty_query_and_no_filters_returns_everything() {
        assertEquals(4, searchCatalog(all, "", CatalogFilters()).size)
    }

    @Test
    fun search_matches_the_name_case_insensitively() {
        assertEquals(listOf("a", "d"), searchCatalog(all, "jazz", CatalogFilters()).map { it.uuid })
        assertEquals(listOf("a", "d"), searchCatalog(all, "JAZZ", CatalogFilters()).map { it.uuid })
    }

    // a substring, not a prefix: "trek" must find "Radio Trek", because that is
    // how a person looks for a station whose full name they half remember.
    @Test
    fun search_matches_anywhere_in_the_name() {
        assertEquals(listOf("b"), searchCatalog(all, "trek", CatalogFilters()).map { it.uuid })
    }

    @Test
    fun surrounding_whitespace_in_a_query_is_ignored() {
        assertEquals(2, searchCatalog(all, "  jazz  ", CatalogFilters()).size)
    }

    @Test
    fun the_country_filter_narrows_to_those_countries() {
        val f = CatalogFilters(countries = setOf("PL"))
        assertEquals(listOf("d"), searchCatalog(all, "", f).map { it.uuid })
    }

    @Test
    fun several_countries_are_an_or() {
        val f = CatalogFilters(countries = setOf("PL", "UA"))
        assertEquals(4, searchCatalog(all, "", f).size)
    }

    @Test
    fun the_genre_filter_matches_one_of_a_stations_tags() {
        val f = CatalogFilters(genres = setOf("news"))
        assertEquals(listOf("b", "c"), searchCatalog(all, "", f).map { it.uuid })
    }

    @Test
    fun the_codec_and_bitrate_filters_narrow_too() {
        assertEquals(listOf("c", "d"), searchCatalog(all, "", CatalogFilters(codecs = setOf("AAC"))).map { it.uuid })
        assertEquals(listOf("d"), searchCatalog(all, "", CatalogFilters(minBitrate = 256)).map { it.uuid })
    }

    // groups are ANDed, values within a group ORed — the same rule the cli uses,
    // and the one the filter sheet's "Show N stations" count depends on.
    @Test
    fun groups_combine_with_and() {
        val f = CatalogFilters(countries = setOf("PL"), genres = setOf("jazz"), codecs = setOf("AAC"))
        assertEquals(listOf("d"), searchCatalog(all, "", f).map { it.uuid })
        assertTrue(searchCatalog(all, "", f.copy(countries = setOf("UA"))).isEmpty())
    }

    @Test
    fun a_query_and_a_filter_combine() {
        val f = CatalogFilters(countries = setOf("PL"))
        assertEquals(listOf("d"), searchCatalog(all, "jazz", f).map { it.uuid })
    }

    // the ban is a product requirement on every surface, including one that is
    // only browsing. a banned station in a list is as wrong as one playing.
    @Test
    fun banned_stations_never_appear_however_you_search() {
        val banned = listOf(
            st("r", "Russia Today", country = "RU"),
            st("m", "Moscow FM", country = "UA"),
        )
        assertTrue(searchCatalog(banned, "", CatalogFilters()).isEmpty())
        assertTrue(searchCatalog(banned, "russia", CatalogFilters()).isEmpty())
    }

    @Test
    fun a_filter_set_knows_whether_it_is_empty_and_how_many_groups_are_active() {
        assertTrue(CatalogFilters().isEmpty)
        assertEquals(0, CatalogFilters().activeCount)
        val f = CatalogFilters(countries = setOf("UA"), minBitrate = 128)
        assertFalse(f.isEmpty)
        assertEquals(2, f.activeCount)
    }

    @Test
    fun facets_count_what_is_there_commonest_first() {
        assertEquals(listOf("UA" to 3, "PL" to 1), countryFacets(all))
        assertEquals("jazz" to 2, genreFacets(all).first())
        assertEquals(listOf("AAC" to 2, "MP3" to 2), codecFacets(all).sortedBy { it.first })
    }

    // facets drive the filter sheet's option list; offering a genre nothing
    // carries would be a dead row the user can only be disappointed by.
    @Test
    fun facets_ignore_banned_stations_too() {
        assertTrue(countryFacets(listOf(st("r", "X", country = "RU"))).isEmpty())
    }
}
