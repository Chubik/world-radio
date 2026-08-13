package net.vchub.r4dio.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class TabsTest {
    // the strip is what carries now-playing on every tab except home, where
    // the station is already the biggest thing on the screen.
    @Test
    fun the_mini_player_shows_on_every_tab_but_home() {
        assertFalse(showsMiniPlayer(Tab.HOME, "Radio Trek"))
        assertTrue(showsMiniPlayer(Tab.CATALOG, "Radio Trek"))
        assertTrue(showsMiniPlayer(Tab.LIBRARY, "Radio Trek"))
        assertTrue(showsMiniPlayer(Tab.SETTINGS, "Radio Trek"))
    }

    // nothing playing yet: a strip naming no station is just a bar of noise.
    @Test
    fun the_mini_player_hides_when_nothing_is_playing() {
        assertFalse(showsMiniPlayer(Tab.CATALOG, ""))
    }

    @Test
    fun home_is_the_first_tab() {
        assertEquals(Tab.HOME, Tab.entries.first())
    }
}
