package net.vchub.r4dio.ui

import android.os.Bundle
import net.vchub.r4dio.EXTRA_CATALOG_FETCHING
import net.vchub.r4dio.EXTRA_CATALOG_GROWING
import net.vchub.r4dio.EXTRA_CATALOG_SIZE
import net.vchub.r4dio.EXTRA_FAV
import net.vchub.r4dio.EXTRA_FILTER_COUNTRIES
import net.vchub.r4dio.EXTRA_SCOPE
import net.vchub.r4dio.EXTRA_UUID
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

    // without the id a screen can name what is playing but cannot star or block
    // it — which is the whole point of the now-playing screen.
    @Test
    fun the_playing_stations_id_reaches_the_state() {
        val b = Bundle().apply { putString(EXTRA_UUID, "abc-123") }
        assertEquals("abc-123", uiStateFromExtras(b, UiState()).stationUuid)
    }

    @Test
    fun a_bundle_without_an_id_keeps_the_one_already_shown() {
        val previous = UiState(stationUuid = "abc-123")
        assertEquals("abc-123", uiStateFromExtras(Bundle(), previous).stationUuid)
    }

    @Test
    fun a_download_in_flight_reaches_the_state() {
        val b = Bundle().apply { putBoolean(EXTRA_CATALOG_FETCHING, true) }
        assertTrue(uiStateFromExtras(b, UiState()).catalogueFetching)
    }

    // the service publishes the fetching flag on its own, in a bundle holding
    // nothing else — that is what keeps it off the expensive path that counts
    // 59k stations. so the fold must carry everything the bundle omits, or the
    // pill would blank its own count the instant a download started.
    @Test
    fun the_fetching_flag_alone_does_not_blank_the_counts() {
        val previous = UiState(
            stationName = "Radio Trek",
            catalogueSize = 58932,
            favCount = 12,
            catalogueGrowing = true,
        )
        val b = Bundle().apply { putBoolean(EXTRA_CATALOG_FETCHING, true) }
        val s = uiStateFromExtras(b, previous)
        assertTrue(s.catalogueFetching)
        assertEquals(58932, s.catalogueSize)
        assertEquals(12, s.favCount)
        assertEquals("Radio Trek", s.stationName)
        assertTrue(s.catalogueGrowing)
    }
}
