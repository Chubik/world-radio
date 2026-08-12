package net.vchub.r4dio.ui

import android.os.Bundle
import net.vchub.r4dio.EXTRA_CATALOG_GROWING
import net.vchub.r4dio.EXTRA_CATALOG_SIZE
import net.vchub.r4dio.EXTRA_FAV
import net.vchub.r4dio.EXTRA_FILTER_COUNTRIES
import net.vchub.r4dio.EXTRA_SCOPE
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.robolectric.RobolectricTestRunner
import org.junit.runner.RunWith

@RunWith(RobolectricTestRunner::class)
class PlayerStateTest {
    @Test
    fun extras_become_state() {
        val b = Bundle().apply {
            putBoolean(EXTRA_FAV, true)
            putString(EXTRA_SCOPE, "favs")
            putInt(EXTRA_CATALOG_SIZE, 1286)
            putBoolean(EXTRA_CATALOG_GROWING, true)
            putStringArray(EXTRA_FILTER_COUNTRIES, arrayOf("UA", "PL"))
        }
        val s = uiStateFromExtras(b, UiState())
        assertTrue(s.isFav)
        assertEquals("favs", s.scope)
        assertEquals(1286, s.catalogueSize)
        assertTrue(s.catalogueGrowing)
        assertEquals(listOf("UA", "PL"), s.filterCountries)
    }

    // the service publishes extras and player metadata on separate channels, so
    // a extras-only update must not wipe the station name the metadata channel
    // set — that would blank the screen every time a count changed.
    @Test
    fun an_extras_update_keeps_the_station_already_shown() {
        val previous = UiState(stationName = "Radio Trek", country = "UA", isPlaying = true)
        val s = uiStateFromExtras(Bundle(), previous)
        assertEquals("Radio Trek", s.stationName)
        assertEquals("UA", s.country)
        assertTrue(s.isPlaying)
    }

    @Test
    fun a_missing_scope_reads_as_all() {
        assertEquals("all", uiStateFromExtras(Bundle(), UiState()).scope)
        assertFalse(uiStateFromExtras(Bundle(), UiState()).isFav)
    }
}
