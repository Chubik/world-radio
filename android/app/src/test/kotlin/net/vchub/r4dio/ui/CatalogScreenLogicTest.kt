package net.vchub.r4dio.ui

import net.vchub.r4dio.CatalogFilters
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class CatalogScreenLogicTest {
    // a station that reported neither field must not leave a bare separator
    // sitting under its name.
    @Test
    fun the_now_playing_meta_skips_what_the_station_did_not_report() {
        assertEquals("UA · MP3", nowPlayingMeta("UA", "MP3"))
        assertEquals("UA", nowPlayingMeta("UA", ""))
        assertEquals("MP3", nowPlayingMeta("", "MP3"))
        assertTrue(nowPlayingMeta("", "").isEmpty())
    }

    @Test
    fun the_meta_line_drops_an_unknown_bitrate_without_a_dangling_dot() {
        assertEquals("MP3", stationMeta("mp3", ""))
        assertEquals("MP3 · 128k", stationMeta("mp3", "128k"))
    }

    @Test
    fun a_station_with_neither_field_shows_no_meta_line() {
        assertTrue(stationMeta("", "").isEmpty())
    }

    // the empty screen has to name its own cause: the chip row may have scrolled
    // the offending filter out of sight.
    @Test
    fun the_empty_state_spells_out_the_query_and_every_filter() {
        val detail = emptyStateDetail("trek", CatalogFilters(countries = setOf("UA"), minBitrate = 320))
        assertEquals("\"trek\" · UA · ≥320k", detail)
    }

    @Test
    fun the_empty_state_detail_is_blank_when_nothing_is_narrowing() {
        assertTrue(emptyStateDetail("", CatalogFilters()).isEmpty())
    }
}
