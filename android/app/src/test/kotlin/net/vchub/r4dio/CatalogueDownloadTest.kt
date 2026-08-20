package net.vchub.r4dio

import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/**
 * the whole catalogue in one request from our own server. the payload is
 * already in the on-disk shape, so these tests use that shape rather than
 * radio-browser's.
 */
class CatalogueDownloadTest {
    private lateinit var server: MockWebServer
    private lateinit var catalog: Catalog

    private fun url() = server.url("/catalog").toString()

    private fun payload(vararg rows: String) = "[${rows.joinToString(",")}]"

    private fun row(uuid: String, name: String, country: String) =
        """{"uuid":"$uuid","name":"$name","url":"http://x","country":"$country",
            "codec":"MP3","bitrate":128,"tags":"jazz","language":"english"}"""

    @Before
    fun setUp() {
        server = MockWebServer()
        server.start()
        catalog = Catalog()
    }

    @After
    fun tearDown() {
        server.shutdown()
    }

    @Test
    fun the_whole_catalogue_arrives_in_one_request() {
        server.enqueue(
            MockResponse().setBody(
                payload(row("a", "Jazz FM", "UA"), row("b", "Rock FM", "PL")),
            ),
        )
        val got = catalog.fetchCatalogue(url = url())
        assertEquals(listOf("a", "b"), got.map { it.uuid })
        assertEquals(1, server.requestCount)
    }

    // the payload is the cache's own shape, so every field has to survive the
    // round trip — a dropped codec or bitrate would show up as blank metadata
    // on every station at once.
    @Test
    fun every_field_survives_the_download() {
        server.enqueue(MockResponse().setBody(payload(row("a", "Jazz FM", "UA"))))
        val s = catalog.fetchCatalogue(url = url()).single()
        assertEquals("a", s.uuid)
        assertEquals("Jazz FM", s.name)
        assertEquals("http://x", s.url)
        assertEquals("UA", s.country)
        assertEquals("MP3", s.codec)
        assertEquals(128, s.bitrate)
        assertEquals("jazz", s.tags)
        assertEquals("english", s.language)
    }

    // the server filters these too, but this side does not get to assume that:
    // a stale or wrong server must never put a banned station in front of a
    // listener.
    @Test
    fun the_ban_is_applied_even_though_the_server_filters_too() {
        server.enqueue(
            MockResponse().setBody(payload(row("r", "ru one", "RU"), row("a", "ok", "UA"))),
        )
        assertEquals(listOf("a"), catalog.fetchCatalogue(url = url()).map { it.uuid })
    }

    @Test
    fun a_blocked_station_never_arrives() {
        server.enqueue(MockResponse().setBody(payload(row("a", "ok", "UA"))))
        assertTrue(catalog.fetchCatalogue(url = url(), blocked = setOf("a")).isEmpty())
    }

    // every failure returns empty rather than throwing, and the caller must
    // read empty as "keep what you have" — writing it would erase the cache.
    @Test
    fun a_server_error_returns_nothing_rather_than_throwing() {
        server.enqueue(MockResponse().setResponseCode(503))
        assertTrue(catalog.fetchCatalogue(url = url()).isEmpty())
    }

    @Test
    fun a_truncated_body_returns_nothing_rather_than_throwing() {
        server.enqueue(MockResponse().setBody("""[{"uuid":"a","name":"trunc"""))
        assertTrue(catalog.fetchCatalogue(url = url()).isEmpty())
    }

    @Test
    fun an_empty_body_returns_nothing_rather_than_throwing() {
        server.enqueue(MockResponse().setBody(""))
        assertTrue(catalog.fetchCatalogue(url = url()).isEmpty())
    }

    // the server may add fields later; an old app must not choke on them.
    @Test
    fun an_unknown_field_does_not_break_the_download() {
        server.enqueue(
            MockResponse().setBody(
                """[{"uuid":"a","name":"ok","url":"http://x","country":"UA","votes":42}]""",
            ),
        )
        assertEquals(listOf("a"), catalog.fetchCatalogue(url = url()).map { it.uuid })
    }

    // this decodes ~59k stations in one parse, so a single station missing a
    // field must not cost the whole catalogue. it is the difference between one
    // odd station and an app with nothing in it.
    @Test
    fun one_station_missing_a_field_does_not_lose_the_whole_catalogue() {
        server.enqueue(
            MockResponse().setBody(
                """[{"uuid":"a","name":"no codec here","url":"http://x","country":"UA"},
                    {"uuid":"b","name":"complete","url":"http://x","country":"PL",
                     "codec":"MP3","bitrate":128}]""",
            ),
        )
        val got = catalog.fetchCatalogue(url = url())
        assertEquals(listOf("a", "b"), got.map { it.uuid })
        assertEquals("", got.first().codec)
        assertEquals(0, got.first().bitrate)
    }

    // a station with no url cannot be played, and the defaults above mean it now
    // parses instead of failing — so something else has to drop it.
    @Test
    fun a_station_without_a_url_is_dropped() {
        server.enqueue(
            MockResponse().setBody("""[{"uuid":"a","name":"urlless","country":"UA"}]"""),
        )
        assertTrue(catalog.fetchCatalogue(url = url()).isEmpty())
    }

    // the default points at our own server, never radio-browser: that is the
    // whole reason the download is 4.3 mb instead of 69 mb.
    @Test
    fun the_default_url_is_our_own_server() {
        assertEquals("https://r4dio.net/catalog", CATALOG_URL)
    }

    // a 304 must read as "unchanged", not as an empty catalogue — those two used
    // to collapse into the same empty list, and the caller could not tell a
    // revalidation apart from a failed download.
    @Test
    fun a_304_reports_unchanged_rather_than_an_empty_catalogue() {
        server.enqueue(MockResponse().setResponseCode(304))
        val result = catalog.fetchCatalogueResult(url = url(), etag = "\"abc\"")
        assertTrue(result is CatalogueResult.Unchanged)
        val sent = server.takeRequest()
        assertEquals("\"abc\"", sent.getHeader("If-None-Match"))
    }

    @Test
    fun a_200_returns_stations_and_the_new_etag() {
        server.enqueue(
            MockResponse()
                .setHeader("ETag", "\"def\"")
                .setBody(payload(row("a", "Jazz FM", "UA"))),
        )
        val result = catalog.fetchCatalogueResult(url = url(), etag = "")
        assertTrue(result is CatalogueResult.Fetched)
        result as CatalogueResult.Fetched
        assertEquals(listOf("a"), result.stations.map { it.uuid })
        assertEquals("\"def\"", result.etag)
    }

    // no etag held yet (first-ever fetch) must not send an empty If-None-Match
    // header, which some servers would treat as a match against everything.
    @Test
    fun no_held_etag_sends_no_if_none_match_header() {
        server.enqueue(MockResponse().setBody(payload(row("a", "Jazz FM", "UA"))))
        catalog.fetchCatalogueResult(url = url(), etag = "")
        val sent = server.takeRequest()
        assertEquals(null, sent.getHeader("If-None-Match"))
    }

    // a server error must report Failed, not Unchanged — retrying belongs to the
    // failure path, and an unchanged verdict would stamp the sync time as if the
    // catalogue were confirmed current.
    @Test
    fun a_server_error_reports_failed() {
        server.enqueue(MockResponse().setResponseCode(503))
        assertTrue(catalog.fetchCatalogueResult(url = url(), etag = "") is CatalogueResult.Failed)
    }

    // the ban still applies on the incremental path, same as on fetchCatalogue.
    @Test
    fun the_ban_is_applied_on_the_incremental_path_too() {
        server.enqueue(
            MockResponse().setBody(payload(row("r", "ru one", "RU"), row("a", "ok", "UA"))),
        )
        val result = catalog.fetchCatalogueResult(url = url(), etag = "") as CatalogueResult.Fetched
        assertEquals(listOf("a"), result.stations.map { it.uuid })
    }

    private fun deltaUrl() = server.url("/catalog/delta").toString()

    // a 409 means the held snapshot is too old for a delta at all — the caller
    // falls back to a full download instead of retrying the delta. it must be
    // distinguishable from every other outcome, not merely "not Changed".
    @Test
    fun a_409_means_unavailable_specifically() {
        server.enqueue(MockResponse().setResponseCode(409).setBody("""{"full":"/catalog"}"""))
        val result = catalog.fetchDelta(url = deltaUrl(), since = "\"old\"")
        assertTrue(result is DeltaResult.Unavailable)
        assertFalse(result is DeltaResult.Changed)
        assertFalse(result is DeltaResult.Unchanged)
    }

    // a 304 on the delta endpoint means the snapshot already held is current —
    // distinct from Unavailable, which means "give up and download everything".
    @Test
    fun a_304_means_unchanged() {
        server.enqueue(MockResponse().setResponseCode(304))
        val result = catalog.fetchDelta(url = deltaUrl(), since = "\"old\"")
        assertTrue(result is DeltaResult.Unchanged)
    }

    @Test
    fun a_delta_returns_what_was_added_and_removed() {
        server.enqueue(
            MockResponse().setResponseCode(200).setBody(
                """{"id":"\"new\"","added":[${row("2", "B", "DE")}],"removed":["1"]}""",
            ),
        )
        val result = catalog.fetchDelta(url = deltaUrl(), since = "\"old\"")
        assertTrue(result is DeltaResult.Changed)
        result as DeltaResult.Changed
        assertEquals(listOf("2"), result.added.map { it.uuid })
        assertEquals(setOf("1"), result.removed)
        assertEquals("\"new\"", result.id)
    }

    // the ban applies to a delta's added stations exactly as it does on a full
    // fetch — the server filters too, but this side does not get to assume that.
    @Test
    fun the_ban_is_applied_to_a_deltas_added_stations() {
        server.enqueue(
            MockResponse().setResponseCode(200).setBody(
                """{"id":"\"new\"","added":[${row("r", "ru one", "RU")},${row("a", "ok", "UA")}],
                    "removed":[]}""",
            ),
        )
        val result = catalog.fetchDelta(url = deltaUrl(), since = "\"old\"") as DeltaResult.Changed
        assertEquals(listOf("a"), result.added.map { it.uuid })
    }

    @Test
    fun a_blocked_station_never_arrives_via_a_delta() {
        server.enqueue(
            MockResponse().setResponseCode(200).setBody(
                """{"id":"\"new\"","added":[${row("a", "ok", "UA")}],"removed":[]}""",
            ),
        )
        val result = catalog.fetchDelta(
            url = deltaUrl(),
            since = "\"old\"",
            blocked = setOf("a"),
        ) as DeltaResult.Changed
        assertTrue(result.added.isEmpty())
    }

    // no held snapshot means there is nothing to ask a delta against — the
    // caller must fall straight to a full download, without a request going out.
    @Test
    fun a_blank_since_is_unavailable_and_sends_no_request() {
        val result = catalog.fetchDelta(url = deltaUrl(), since = "")
        assertTrue(result is DeltaResult.Unavailable)
        assertEquals(0, server.requestCount)
    }

    // a server error on the delta endpoint is Unavailable, not a crash — same
    // fallback as a 409, reached by a different failure.
    @Test
    fun a_server_error_on_delta_is_unavailable() {
        server.enqueue(MockResponse().setResponseCode(503))
        val result = catalog.fetchDelta(url = deltaUrl(), since = "\"old\"")
        assertTrue(result is DeltaResult.Unavailable)
    }

    // the since value must reach the server unmodified — it is the whole basis
    // for "what changed since I last synced".
    @Test
    fun since_is_sent_as_a_query_parameter() {
        server.enqueue(
            MockResponse().setResponseCode(200).setBody("""{"id":"\"new\"","added":[],"removed":[]}"""),
        )
        catalog.fetchDelta(url = deltaUrl(), since = "\"old-tag\"")
        val sent = server.takeRequest()
        assertTrue(sent.path.orEmpty().contains("since="))
    }
}
