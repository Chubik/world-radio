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
 * the rules deciding when the catalogue may be fetched, plus the paged walk that
 * survives only as the fallback for when our own server cannot be reached.
 */
class TopUpTest {
    private lateinit var server: MockWebServer
    private lateinit var catalog: Catalog

    @Before
    fun setUp() {
        server = MockWebServer()
        server.start()
        catalog = Catalog(baseUrl = server.url("/").toString().trimEnd('/'))
    }

    @After
    fun tearDown() {
        server.shutdown()
    }

    // wi-fi never needed a reason: it costs the user nothing, and data saver
    // only ever restrains mobile data.
    @Test
    fun wifi_always_fetches() {
        assertTrue(
            catalogueFetchAllowed(unmetered = true, dataSaver = false, onMobileAllowed = true),
        )
        assertTrue(
            catalogueFetchAllowed(unmetered = true, dataSaver = true, onMobileAllowed = false),
        )
    }

    // the whole point of the change: a phone that never sees wi-fi used to sit
    // on 1,000 stations forever.
    @Test
    fun mobile_data_fetches_when_the_user_allows_it() {
        assertTrue(
            catalogueFetchAllowed(unmetered = false, dataSaver = false, onMobileAllowed = true),
        )
    }

    @Test
    fun mobile_data_is_left_alone_when_the_user_says_no() {
        assertFalse(
            catalogueFetchAllowed(unmetered = false, dataSaver = false, onMobileAllowed = false),
        )
    }

    // when android has been told to hold back background data, an app does not
    // get to decide its own download is the exception.
    @Test
    fun data_saver_outranks_the_users_own_toggle() {
        assertFalse(
            catalogueFetchAllowed(unmetered = false, dataSaver = true, onMobileAllowed = true),
        )
    }

    // the ceiling is the whole catalogue, not a guessed-at fraction of it.
    @Test
    fun the_ceiling_is_the_whole_catalogue() {
        assertTrue(TOP_UP_CEILING >= 62_000)
    }

    // a real full fetch landed at 58,938 on a device: the ban and unplayable
    // streams take ~3,300 off what upstream reports. measuring "is it whole"
    // against the upstream ceiling left the pill saying "+" forever on a
    // catalogue that was already complete.
    @Test
    fun a_complete_catalogue_is_below_the_upstream_ceiling() {
        assertTrue(CATALOGUE_WHOLE < TOP_UP_CEILING)
        assertTrue("a real full fetch of 58938 must count as whole", 58_938 >= CATALOGUE_WHOLE)
    }

    @Test
    fun a_page_asks_for_the_next_slice() {
        server.enqueue(MockResponse().setBody("[]"))
        catalog.fetchPage(offset = 1000, limit = 200)
        val asked = server.takeRequest().path.orEmpty()
        assertTrue(asked, asked.contains("offset=1000"))
        assertTrue(asked, asked.contains("limit=200"))
        assertTrue(asked, asked.contains("order=clickcount"))
        assertTrue(asked, asked.contains("hidebroken=true"))
    }

    // the ban is a product requirement on every ingest path, and the fallback
    // walk is one that pulls stations nobody asked for by name.
    @Test
    fun a_page_still_drops_banned_countries() {
        server.enqueue(
            MockResponse().setBody(
                """[{"stationuuid":"r","name":"ru one","url_resolved":"http://x","countrycode":"RU"},
                    {"stationuuid":"a","name":"ok","url_resolved":"http://x","countrycode":"PL"}]""",
            ),
        )
        assertEquals(listOf("a"), catalog.fetchPage(0, 10).map { it.uuid })
    }

    @Test
    fun a_failed_page_returns_nothing_rather_than_throwing() {
        server.enqueue(MockResponse().setResponseCode(500))
        assertTrue(catalog.fetchPage(0, 10).isEmpty())
    }
}
