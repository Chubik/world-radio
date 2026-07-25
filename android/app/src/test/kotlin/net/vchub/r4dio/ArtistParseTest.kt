package net.vchub.r4dio

import org.junit.Assert.assertEquals
import org.junit.Test

class ArtistParseTest {
    @Test fun parses_country_and_codec() {
        assertEquals("SA" to "MP3", parseArtist("SA · MP3 · 128k"))
    }
    @Test fun handles_null_and_blank() {
        assertEquals(null to null, parseArtist(null))
        assertEquals(null to null, parseArtist(""))
    }
    @Test fun handles_missing_tokens() {
        assertEquals("US" to null, parseArtist("US"))
    }
}
