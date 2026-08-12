package net.vchub.r4dio.ui

import org.junit.Assert.assertEquals
import org.junit.Test

class ThemeResolveTest {
    @Test
    fun a_synced_theme_wins() {
        assertEquals("nord", resolveTheme(synced = "nord", current = "amber-crt"))
    }

    // a slug this build does not know can only come from a newer client. the
    // user's current theme survives it; resetting to a default would be a
    // visible change nobody asked for.
    @Test
    fun an_unknown_synced_theme_keeps_what_we_have() {
        assertEquals("gruvbox", resolveTheme(synced = "plasma-9000", current = "gruvbox"))
    }

    // nothing synced yet: the account has never set a theme.
    @Test
    fun an_empty_synced_theme_keeps_what_we_have() {
        assertEquals("gruvbox", resolveTheme(synced = "", current = "gruvbox"))
    }

    // first run, nothing stored anywhere.
    @Test
    fun with_nothing_at_all_we_land_on_amber() {
        assertEquals("amber-crt", resolveTheme(synced = "", current = ""))
    }
}
