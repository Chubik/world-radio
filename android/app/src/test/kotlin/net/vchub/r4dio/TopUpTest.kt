package net.vchub.r4dio

import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

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

    // "без навантаження" means waiting for a moment that costs the user nothing,
    // not taking smaller bites at an arbitrary one.
    @Test
    fun the_top_up_waits_for_wifi_and_charging() {
        assertTrue(topUpAllowed(unmetered = true, charging = true, held = 1000, ceiling = 20_000))
        assertFalse(topUpAllowed(unmetered = false, charging = true, held = 1000, ceiling = 20_000))
        assertFalse(topUpAllowed(unmetered = true, charging = false, held = 1000, ceiling = 20_000))
        assertFalse(topUpAllowed(unmetered = false, charging = false, held = 1000, ceiling = 20_000))
    }

    @Test
    fun the_top_up_stops_at_the_ceiling() {
        assertFalse(topUpAllowed(unmetered = true, charging = true, held = 20_000, ceiling = 20_000))
    }

    // one station short of the ceiling is still a reason to fetch: the boundary
    // belongs on the "stop" side only once it is actually reached.
    @Test
    fun the_top_up_runs_right_up_to_the_ceiling() {
        assertTrue(topUpAllowed(unmetered = true, charging = true, held = 19_999, ceiling = 20_000))
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

    // the ban is a product requirement on every ingest path, and the top-up is
    // the one path that pulls stations nobody asked for by name.
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
