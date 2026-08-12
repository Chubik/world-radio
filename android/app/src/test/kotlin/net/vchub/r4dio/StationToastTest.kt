package net.vchub.r4dio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class StationToastTest {
    @Test
    fun the_line_carries_the_station_and_its_country() {
        assertEquals("Radio ROKS · UA", toastText("Radio ROKS", "UA"))
    }

    // a station with no country must not read "name · ".
    @Test
    fun a_station_without_a_country_is_named_alone() {
        assertEquals("Radio ROKS", toastText("Radio ROKS", ""))
        assertEquals("Radio ROKS", toastText("Radio ROKS", "   "))
    }

    @Test
    fun a_different_station_is_announced() {
        assertTrue(shouldAnnounce(previousUuid = "a", nextUuid = "b", appIsInForeground = false))
    }

    // the whole point is playback the user cannot see; over r4dio's own screen
    // the name is already on display and a panel would be noise.
    @Test
    fun nothing_is_announced_over_our_own_screen() {
        assertFalse(shouldAnnounce(previousUuid = "a", nextUuid = "b", appIsInForeground = true))
    }

    // a pause, a resume, or a re-buffer after a network blip all reach the play
    // path with the same station. announcing those teaches the user to ignore
    // the panel.
    @Test
    fun the_same_station_again_is_not_a_change() {
        assertFalse(shouldAnnounce(previousUuid = "a", nextUuid = "a", appIsInForeground = false))
    }

    @Test
    fun the_first_station_of_a_session_is_announced() {
        assertTrue(shouldAnnounce(previousUuid = null, nextUuid = "a", appIsInForeground = false))
    }
}
