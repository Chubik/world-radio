package net.vchub.r4dio

import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

class CatalogTest {
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

    @Test
    fun a_country_fetch_asks_for_that_country_only() {
        server.enqueue(MockResponse().setBody("""[{"stationuuid":"a","name":"UA one","url_resolved":"http://x","countrycode":"UA"}]"""))
        val got = catalog.fetchCountry("UA")
        val asked = server.takeRequest().path.orEmpty()
        assertTrue("wrong endpoint: $asked", asked.contains("bycountrycodeexact/UA"))
        assertTrue("broken stations must not be asked for: $asked", asked.contains("hidebroken=true"))
        assertEquals(1, got.size)
    }

    // the ban is a product requirement on every ingest path, not a display rule.
    @Test
    fun a_country_fetch_still_drops_banned_countries() {
        server.enqueue(MockResponse().setBody("""[{"stationuuid":"r","name":"ru one","url_resolved":"http://x","countrycode":"RU"}]"""))
        assertTrue(catalog.fetchCountry("RU").isEmpty())
    }

    @Test
    fun a_blocked_station_never_arrives_from_a_country_fetch() {
        server.enqueue(MockResponse().setBody("""[{"stationuuid":"a","name":"UA one","url_resolved":"http://x","countrycode":"UA"}]"""))
        assertTrue(catalog.fetchCountry("UA", blocked = setOf("a")).isEmpty())
    }

    // a network failure must leave the catalogue alone rather than emptying it.
    @Test
    fun a_failed_country_fetch_returns_nothing_rather_than_throwing() {
        server.enqueue(MockResponse().setResponseCode(500))
        assertTrue(catalog.fetchCountry("UA").isEmpty())
    }
}
