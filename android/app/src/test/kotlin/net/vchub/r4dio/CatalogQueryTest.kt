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

    // measured on the live catalogue: searching names alone found 26 stations
    // for "french" where 2,226 actually broadcast in french, and 494 for "news"
    // against 3,920. genre and language have been in the cache since v1.18.6.
    @Test
    fun search_finds_stations_by_genre_and_language_not_just_name() {
        val wide = all + listOf(
            st("e", "Le Poste", country = "FR", tags = "chanson").copy(language = "french"),
            st("f", "Morning Report", country = "GB", tags = "news,talk"),
        )
        assertEquals(listOf("e"), searchCatalog(wide, "french", CatalogFilters()).map { it.uuid })
        // "b" and "c" carry the news tag, "f" carries it too
        assertEquals(
            setOf("b", "c", "f"),
            searchCatalog(wide, "news", CatalogFilters()).map { it.uuid }.toSet(),
        )
    }

    // the reason ranking exists. a two-letter query matches a country code, and
    // on real data "us" pulled 7,714 american stations — which must not bury the
    // stations actually called something starting with "us".
    @Test
    fun a_name_match_outranks_a_country_match() {
        val wide = listOf(
            st("far", "Radio Warsaw", country = "US"),
            st("near", "USV Radio", country = "RO"),
        )
        assertEquals(listOf("near", "far"), searchCatalog(wide, "us", CatalogFilters()).map { it.uuid })
    }

    @Test
    fun a_name_beginning_with_the_query_comes_before_one_merely_containing_it() {
        val wide = listOf(
            st("inside", "Smooth Jazz Lounge"),
            st("start", "Jazz Lounge"),
        )
        assertEquals(listOf("start", "inside"), searchCatalog(wide, "jazz", CatalogFilters()).map { it.uuid })
    }

    // the whole ordering, in one assertion: name-prefix, name-substring, genre,
    // language, country.
    @Test
    fun the_ranking_runs_name_then_genre_then_language_then_country() {
        val wide = listOf(
            st("country", "Nothing Alike", country = "PO"),
            st("language", "Nothing Alike 2", country = "FR").copy(language = "po"),
            st("genre", "Nothing Alike 3", country = "FR", tags = "po"),
            st("anywhere", "Radio Po"),
            st("prefix", "Po Radio"),
        )
        assertEquals(
            listOf("prefix", "anywhere", "genre", "language", "country"),
            searchCatalog(wide, "po", CatalogFilters()).map { it.uuid },
        )
    }

    // a genre is a whole tag, never a substring: "pop" inside "popular" is not a
    // genre match, and a loosely compared country code turns every query into a
    // country query.
    @Test
    fun genre_and_country_match_whole_values_not_substrings() {
        val wide = listOf(
            st("tagged", "Station One", tags = "popular"),
            st("exact", "Station Two", tags = "pop"),
        )
        assertEquals(listOf("exact"), searchCatalog(wide, "pop", CatalogFilters()).map { it.uuid })
    }

    // stations that match nothing must not ride along at the end of the list.
    @Test
    fun a_station_matching_nothing_is_left_out() {
        assertTrue(searchCatalog(all, "zzzznothing", CatalogFilters()).isEmpty())
    }

    // filters still apply to a ranked search; ranking only orders what survives.
    @Test
    fun ranking_does_not_bypass_the_filters() {
        val f = CatalogFilters(countries = setOf("PL"))
        assertEquals(listOf("d"), searchCatalog(all, "jazz", f).map { it.uuid })
    }

    // the catalogue arrives ranked by upstream clickcount, which is the only
    // ordering carrying real information — so the default must not disturb it.
    @Test
    fun the_default_sort_leaves_the_catalogue_order_alone() {
        assertEquals(
            all.map { it.uuid },
            searchCatalog(all, "", CatalogFilters()).map { it.uuid },
        )
    }

    @Test
    fun sorting_by_name_is_alphabetical_and_case_insensitive() {
        val out = searchCatalog(all, "", CatalogFilters(sort = SortOrder.NAME))
        assertEquals(listOf("Jazz Cafe", "Kyiv Talk", "Radio Trek", "Warsaw Jazz"), out.map { it.name })
    }

    @Test
    fun sorting_by_bitrate_puts_the_best_stream_first() {
        val out = searchCatalog(all, "", CatalogFilters(sort = SortOrder.BITRATE))
        assertEquals(listOf(256, 128, 128, 64), out.map { it.bitrate })
    }

    // someone who typed a name wants the closest name, whatever sort is set.
    @Test
    fun a_query_outranks_the_chosen_sort() {
        val wide = listOf(
            st("loud", "Smooth Jazz", bitrate = 320),
            st("quiet", "Jazz Hall", bitrate = 32),
        )
        val out = searchCatalog(wide, "jazz", CatalogFilters(sort = SortOrder.BITRATE))
        // "Jazz Hall" starts with the query, so it leads despite the lower bitrate
        assertEquals(listOf("quiet", "loud"), out.map { it.uuid })
    }

    // within one rank the chosen sort still decides.
    @Test
    fun the_sort_breaks_ties_between_equally_relevant_stations() {
        val wide = listOf(
            st("quiet", "Jazz Hall", bitrate = 32),
            st("loud", "Jazz Club", bitrate = 320),
        )
        val out = searchCatalog(wide, "jazz", CatalogFilters(sort = SortOrder.BITRATE))
        assertEquals(listOf("loud", "quiet"), out.map { it.uuid })
    }

    // sort never hides a station, so it must not read as an active filter — the
    // chip row and CLEAR ALL both key off these.
    @Test
    fun choosing_a_sort_is_not_an_active_filter() {
        val f = CatalogFilters(sort = SortOrder.NAME)
        assertTrue(f.isEmpty)
        assertEquals(0, f.activeCount)
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
