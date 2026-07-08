package net.vchub.r4dio

import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class MirrorClientTest {
    private fun clientFor(server: MockWebServer): MirrorClient =
        MirrorClient(baseUrl = server.url("/").toString().trimEnd('/'))

    @Test
    fun parse_sse_data_reads_event() {
        val c = MirrorClient()
        val line = """data: {"uuid":"u1","name":"One","url":"http://x/1","origin":"devA","seq":3}"""
        val e = c.parseSseData(line)!!
        assertEquals("u1", e.uuid)
        assertEquals(3L, e.seq)
        assertEquals("devA", e.origin)
    }

    @Test
    fun parse_sse_data_ignores_non_data() {
        val c = MirrorClient()
        assertNull(c.parseSseData("event: play"))
        assertNull(c.parseSseData(": keep-alive"))
        assertNull(c.parseSseData(""))
    }

    @Test
    fun play_returns_seq() {
        val server = MockWebServer()
        server.enqueue(MockResponse().setBody("""{"seq":7}"""))
        server.start()
        assertEquals(7L, clientFor(server).play("r4-k", "u1", "One", "http://x/1", "devA"))
        server.shutdown()
    }

    @Test
    fun play_error_returns_null() {
        val server = MockWebServer()
        server.enqueue(MockResponse().setResponseCode(401))
        server.start()
        assertNull(clientFor(server).play("r4-bad", "u", "n", "u", "d"))
        server.shutdown()
    }

    @Test
    fun events_delivers_parsed_events() {
        val server = MockWebServer()
        server.enqueue(
            MockResponse()
                .setHeader("Content-Type", "text/event-stream")
                .setBody("event: play\ndata: {\"uuid\":\"u1\",\"name\":\"One\",\"url\":\"http://x/1\",\"origin\":\"devA\",\"seq\":1}\n\n")
        )
        server.start()
        val got = mutableListOf<MirrorEvent>()
        clientFor(server).events("r4-k") { got.add(it) }
        assertEquals(1, got.size)
        assertEquals("u1", got[0].uuid)
        server.shutdown()
    }
}
