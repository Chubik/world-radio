package net.vchub.r4dio

import android.content.Context
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment

@RunWith(RobolectricTestRunner::class)
class FavStorePendingTest {
    private lateinit var context: Context

    @Before
    fun setup() {
        context = RuntimeEnvironment.getApplication()
    }

    private fun st(uuid: String) = Station(uuid, uuid, "http://$uuid", "", "", 0)

    @Test
    fun starring_records_a_change_with_gone_false() = runBlocking {
        val store = FavStore(context)
        store.toggleFav(st("a"))
        assertEquals(listOf(Change("a", false)), store.currentPending().favs)
    }

    @Test
    fun unstarring_records_a_tombstone() = runBlocking {
        val store = FavStore(context)
        // after toggleFav removes a uuid, currentPending must contain
        // Change(uuid, gone = true) — that is the whole point of the fix
        store.toggleFav(st("a")) // star
        store.toggleFav(st("a")) // unstar
        assertEquals(listOf(Change("a", true)), store.currentPending().favs)
    }

    @Test
    fun blocking_and_unblocking_records_into_the_blocked_set() = runBlocking {
        val store = FavStore(context)
        store.toggleBlocked("a")
        assertEquals(listOf(Change("a", false)), store.currentPending().blocked)

        store.toggleBlocked("a")
        assertEquals(listOf(Change("a", true)), store.currentPending().blocked)
    }

    @Test
    fun changing_countries_records_both_directions() = runBlocking {
        // starting from an already-excluded {UA, PL} (pending cleared, as if already
        // synced), moving to {PL, DE} must record UA gone=true and DE gone=false —
        // PL is untouched by the second call and must not appear.
        val store = FavStore(context)
        store.setExcluded(setOf("UA", "PL"))
        store.clearPushedPending(store.currentPending())

        store.setExcluded(setOf("PL", "DE"))

        val pending = store.currentPending().excluded_countries
        assertEquals(setOf(Change("UA", true), Change("DE", false)), pending.toSet())
        assertTrue(pending.none { it.id == "PL" })
    }

    @Test
    fun clearing_pushed_pending_leaves_what_changed_since() = runBlocking {
        val store = FavStore(context)
        store.toggleFav(st("a"))
        val pushed = store.currentPending()

        store.toggleFav(st("b"))

        store.clearPushedPending(pushed)

        assertEquals(listOf(Change("b", false)), store.currentPending().favs)
    }
}
