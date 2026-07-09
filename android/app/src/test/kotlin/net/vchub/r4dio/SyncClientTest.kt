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
    fun push_returnsMerged() {
        val server = MockWebServer()
        server.enqueue(MockResponse().setBody("""{"favs":["a","b","c"],"blocked":[]}"""))
        server.start()
        val d = clientFor(server).push("r4-k", SyncData(listOf("c"), emptyList()))
        assertEquals(SyncData(listOf("a", "b", "c"), emptyList()), d)
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

    @Test
    fun push_badJson_returnsNull() {
        val server = MockWebServer()
        server.enqueue(MockResponse().setBody("not json"))
        server.start()
        assertNull(clientFor(server).push("r4-k", SyncData(emptyList(), emptyList())))
        server.shutdown()
    }
}
