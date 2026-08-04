package net.vchub.r4dio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class WidgetStateTest {
    @Test
    fun narrow_widgets_use_the_compact_layout() {
        assertTrue(usesCompactLayout(110))
        assertTrue(usesCompactLayout(WIDGET_SMALL_MAX_WIDTH_DP))
    }

    @Test
    fun wide_widgets_use_the_full_layout() {
        assertFalse(usesCompactLayout(WIDGET_SMALL_MAX_WIDTH_DP + 1))
        assertFalse(usesCompactLayout(320))
    }

    // a launcher that reports nothing must not collapse to the cramped layout
    @Test
    fun a_zero_width_report_falls_back_to_the_full_layout() {
        assertFalse(usesCompactLayout(0))
    }

    @Test
    fun station_label_shows_the_station_when_there_is_one() {
        assertEquals("Radio Paradise", widgetStationLabel("Radio Paradise", "— idle —"))
    }

    @Test
    fun station_label_falls_back_to_idle_when_blank() {
        assertEquals("— idle —", widgetStationLabel("", "— idle —"))
        assertEquals("— idle —", widgetStationLabel("   ", "— idle —"))
    }

    @Test
    fun meta_label_joins_the_parts_it_has() {
        assertEquals("US · MP3 · 128k", widgetMetaLabel("US", "MP3", 128))
    }

    // the api leaves any of these empty on plenty of stations
    @Test
    fun meta_label_skips_missing_parts_without_a_dangling_separator() {
        assertEquals("MP3 · 128k", widgetMetaLabel("", "MP3", 128))
        assertEquals("US · 128k", widgetMetaLabel("US", "", 128))
        assertEquals("US · MP3", widgetMetaLabel("US", "MP3", 0))
        assertEquals("US", widgetMetaLabel("US", "", 0))
        assertEquals("", widgetMetaLabel("", "", 0))
    }
}
