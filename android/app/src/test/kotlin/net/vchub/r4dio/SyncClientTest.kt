package net.vchub.r4dio

import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SyncClientTest {
    private fun clientFor(server: MockWebServer): SyncClient =
        SyncClient(baseUrl = server.url("/").toString().trimEnd('/'))

    @Test
    fun createAccount_returnsKey() {
        val server = MockWebServer()
        server.enqueue(MockResponse().setBody("""{"key":"r4-abc"}"""))
        server.start()
        assertEquals("r4-abc", clientFor(server).createAccount())
        server.shutdown()
    }

    @Test
    fun createAccount_serverError_returnsNull() {
        val server = MockWebServer()
        server.enqueue(MockResponse().setResponseCode(500))
        server.start()
        assertNull(clientFor(server).createAccount())
        server.shutdown()
    }

    @Test
    fun pull_returnsData() {
        val server = MockWebServer()
        server.enqueue(MockResponse().setBody("""{"favs":["a","b"],"blocked":["x"]}"""))
        server.start()
        val d = clientFor(server).pull("r4-k")
        assertEquals(SyncData(listOf("a", "b"), listOf("x")), d)
        server.shutdown()
    }

    @Test
    fun pull_401_returnsNull() {
        val server = MockWebServer()
        server.enqueue(MockResponse().setResponseCode(401))
        server.start()
        assertNull(clientFor(server).pull("r4-bad"))
        server.shutdown()
    }

    @Test
    fun push_returnsServerStateVerbatim() {
        val server = MockWebServer()
        server.enqueue(MockResponse().setBody("""{"favs":["c"],"blocked":[]}"""))
        server.start()
        val d = clientFor(server).push("r4-k", SyncData(listOf("c"), emptyList()))
        assertEquals(SyncData(listOf("c"), emptyList()), d)
        server.shutdown()
    }

    @Test
    fun delete_204_true() {
        val server = MockWebServer()
        server.enqueue(MockResponse().setResponseCode(204))
        server.start()
        assertTrue(clientFor(server).delete("r4-k"))
        server.shutdown()
    }

    // the compatibility requirement, measured on the bytes the client really sends
    // rather than on a locally-configured serializer that could drift from it: a
    // device that has never touched the profile must look exactly like the old one
    // to a pre-upgrade server.
    @Test
    fun push_fromAnUntouchedDevice_isBytewiseThePreProfilePayload() {
        val server = MockWebServer()
        server.enqueue(MockResponse().setBody("""{"favs":[],"blocked":[]}"""))
        server.start()
        val out = SyncProfile().outgoing(listOf("f1"), emptyList(), emptyList(), emptyList())
        clientFor(server).push("r4-k", out)
        assertEquals(
            """{"favs":["f1"],"blocked":[],"excluded_countries":[]}""",
            server.takeRequest().body.readUtf8(),
        )
        server.shutdown()
    }

    @Test
    fun push_fromATouchedDevice_carriesTheProfileOnTheWire() {
        val server = MockWebServer()
        server.enqueue(MockResponse().setBody("""{"favs":[],"blocked":[]}"""))
        server.start()
        val out = SyncProfile(
            countries = listOf("UA"),
            countriesAt = 10,
            scope = "favorites",
            scopeAt = 20,
            theme = "nord",
            themeAt = 30,
        ).outgoing(emptyList(), emptyList(), emptyList(), listOf(HistoryRecord("s1", 40, false)))
        clientFor(server).push("r4-k", out)
        val body = server.takeRequest().body.readUtf8()
        assertTrue(body, body.contains(""""shuffle_filter":{"value":{"countries":["UA"]},"at":10}"""))
        assertTrue(body, body.contains(""""scope":{"value":"favorites","at":20}"""))
        assertTrue(body, body.contains(""""theme":{"value":"nord","at":30}"""))
        assertTrue(body, body.contains(""""history":[{"id":"s1","at":40,"gone":false}]"""))
        server.shutdown()
    }

    // a server that has never heard of the profile must leave everything alone
    @Test
    fun pull_withoutTheProfileKeys_parsesAndChangesNothing() {
        val server = MockWebServer()
        server.enqueue(MockResponse().setBody("""{"favs":["a"],"blocked":[]}"""))
        server.start()
        val d = clientFor(server).pull("r4-k")!!
        assertNull(d.shuffle_filter)
        assertNull(d.scope)
        assertNull(d.theme)
        assertTrue(d.history.isEmpty())
        server.shutdown()
    }

    @Test
    fun pull_parsesTheProfileKeys() {
        val server = MockWebServer()
        server.enqueue(
            MockResponse().setBody(
                """{"favs":[],"blocked":[],"shuffle_filter":{"value":{"countries":["UA"]},"at":10},""" +
                    """"scope":{"value":"dead","at":20},"theme":{"value":"nord","at":30},""" +
                    """"history":[{"id":"s1","at":40,"gone":false}]}""",
            ),
        )
        server.start()
        val d = clientFor(server).pull("r4-k")!!
        assertEquals(10L, d.shuffle_filter?.at)
        assertEquals(20L, d.scope?.at)
        assertEquals(30L, d.theme?.at)
        assertEquals(listOf(HistoryRecord("s1", 40, false)), d.history)
        server.shutdown()
    }

    @Test
    fun push_badJson_returnsNull() {
        val server = MockWebServer()
        server.enqueue(MockResponse().setBody("not json"))
        server.start()
        assertNull(clientFor(server).push("r4-k", SyncData(emptyList(), emptyList())))
        server.shutdown()
    }
}
