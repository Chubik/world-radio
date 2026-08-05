package net.vchub.r4dio

import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class CatalogByUuidTest {
    private fun catalogFor(server: MockWebServer): Catalog =
        Catalog(baseUrl = server.url("/").toString().trimEnd('/'))

    @Test
    fun fetchByUuids_returnsStations() {
        val server = MockWebServer()
        server.enqueue(
            MockResponse().setBody(
                """[{"stationuuid":"a","name":"A FM","url_resolved":"http://a","countrycode":"DE","codec":"MP3","bitrate":128}]"""
            )
        )
        server.start()
        val out = catalogFor(server).fetchByUuids(listOf("a"))
        assertEquals(listOf("a"), out.map { it.uuid })
        assertEquals("A FM", out[0].name)
        server.shutdown()
    }

    @Test
    fun fetchByUuids_postsUuidsAsCsv() {
        val server = MockWebServer()
        server.enqueue(MockResponse().setBody("[]"))
        server.start()
        catalogFor(server).fetchByUuids(listOf("a", "b"))
        val req = server.takeRequest()
        assertEquals("/json/stations/byuuid", req.path)
        assertTrue(req.body.readUtf8().contains("uuids=a%2Cb"))
        server.shutdown()
    }

    @Test
    fun fetchByUuids_emptyInputMakesNoRequest() {
        val server = MockWebServer()
        server.start()
        assertEquals(emptyList<Station>(), catalogFor(server).fetchByUuids(emptyList()))
        assertEquals(0, server.requestCount)
        server.shutdown()
    }

    @Test
    fun fetchByUuids_serverErrorReturnsEmpty() {
        val server = MockWebServer()
        server.enqueue(MockResponse().setResponseCode(503))
        server.start()
        assertEquals(emptyList<Station>(), catalogFor(server).fetchByUuids(listOf("a")))
        server.shutdown()
    }

    @Test
    fun fetchByUuids_malformedBodyReturnsEmpty() {
        val server = MockWebServer()
        server.enqueue(MockResponse().setBody("not json"))
        server.start()
        assertEquals(emptyList<Station>(), catalogFor(server).fetchByUuids(listOf("a")))
        server.shutdown()
    }

    @Test
    fun fetchByUuids_appliesBannedStationFilter() {
        val server = MockWebServer()
        server.enqueue(
            MockResponse().setBody(
                """[{"stationuuid":"r","name":"Moscow FM","url_resolved":"http://r","countrycode":"RU","codec":"MP3","bitrate":128}]"""
            )
        )
        server.start()
        assertEquals(emptyList<Station>(), catalogFor(server).fetchByUuids(listOf("r")))
        server.shutdown()
    }
}
