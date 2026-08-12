package net.vchub.r4dio.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test

class PaletteTest {
    // amber-crt is the current release's colours.xml. if this drifts, every
    // existing screenshot and the widget stop matching the app.
    @Test
    fun amber_crt_is_todays_palette() {
        val p = paletteFor("amber-crt")!!
        assertEquals(0xFF15100B, p.bg)
        assertEquals(0xFFD49A3A, p.fg)
        assertEquals(0xFFFFC457, p.accent)
        assertEquals(0xFFFF8A3D, p.hot)
        assertEquals(0xFF6E5430, p.dim)
        assertEquals(0xFF9EC074, p.ok)
        assertEquals(0xFFD96A5A, p.err)
        assertEquals(0xFF6FB0C8, p.info)
        assertEquals(0xFFFFF0C0, p.peak)
    }

    @Test
    fun all_fourteen_themes_resolve() {
        assertEquals(14, THEME_SLUGS.size)
        THEME_SLUGS.forEach { assertNotNull("no palette for $it", paletteFor(it)) }
    }

    // an unknown slug means a newer client chose a theme this build does not
    // have. the caller keeps its current theme; it must not be handed a default
    // that would silently overwrite the user's choice.
    @Test
    fun an_unknown_slug_resolves_to_nothing() {
        assertNull(paletteFor("solarized-light-extra"))
        assertNull(paletteFor(""))
    }

    // hifi-paper is the one light theme. a build that assumes a dark background
    // renders dark-on-dark here, and this is the test that says so.
    @Test
    fun hifi_paper_is_light() {
        val p = paletteFor("hifi-paper")!!
        assertEquals(0xFFEFE6CC, p.bg)
        assertEquals(0xFF0F0A04, p.peak)
    }

    @Test
    fun every_palette_is_fully_opaque() {
        THEME_SLUGS.mapNotNull { paletteFor(it) }.forEach { p ->
            listOf(p.bg, p.fg, p.accent, p.hot, p.dim, p.ok, p.err, p.info, p.peak)
                .forEach { assertEquals(0xFF000000, it and 0xFF000000) }
        }
    }
}
