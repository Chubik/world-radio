package net.vchub.r4dio

import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Test

class StationFieldsTest {
    private val json = Json { ignoreUnknownKeys = true }

    @Test
    fun the_api_gives_us_tags_and_language() {
        val body = """{"stationuuid":"a","name":"Jazz FM","url_resolved":"http://x",
            "countrycode":"UA","codec":"MP3","bitrate":128,
            "tags":"jazz,lounge","language":"english"}"""
        val s = json.decodeFromString(ApiStation.serializer(), body).toStation()
        assertEquals("jazz,lounge", s.tags)
        assertEquals("english", s.language)
    }

    // the cache is the only copy on disk; a field that survives the wire but not
    // the round trip through FavStation is lost the moment the app restarts.
    @Test
    fun tags_survive_the_trip_to_disk_and_back() {
        val s = Station("a", "Jazz FM", "http://x", "UA", "MP3", 128, "jazz,lounge", "english")
        val back = FavStation.of(s).toStation()
        assertEquals(s, back)
    }

    // a catalog.json written before this task has no tags key at all. it must
    // still load — the alternative is every user losing their catalogue.
    @Test
    fun a_cache_written_before_tags_existed_still_loads() {
        val old = """{"uuid":"a","name":"N","url":"u","country":"UA","codec":"MP3","bitrate":128}"""
        val s = json.decodeFromString(FavStation.serializer(), old).toStation()
        assertEquals("", s.tags)
        assertEquals("", s.language)
    }

    @Test
    fun genres_are_split_trimmed_and_lowercased() {
        val s = Station("a", "N", "u", "UA", "MP3", 128, "Jazz, LOUNGE ,, news")
        assertEquals(listOf("jazz", "lounge", "news"), s.genres())
    }

    @Test
    fun a_station_with_no_tags_has_no_genres() {
        assertEquals(emptyList<String>(), Station("a", "N", "u", "UA", "MP3", 128).genres())
    }
}
