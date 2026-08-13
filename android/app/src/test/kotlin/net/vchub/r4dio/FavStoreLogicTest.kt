package net.vchub.r4dio

import android.content.Context
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringSetPreferencesKey
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import kotlin.random.Random
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking

@RunWith(RobolectricTestRunner::class)
class FavStoreLogicTest {
    private lateinit var context: Context

    @Before
    fun setup() {
        context = RuntimeEnvironment.getApplication()
    }

    private fun st(uuid: String) = Station(uuid, uuid, "http://$uuid", "", "", 0)

    @Test
    fun toggle_addsWhenAbsent() {
        assertEquals(setOf("a"), FavLogic.toggle(emptySet(), "a"))
    }

    @Test
    fun toggle_removesWhenPresent() {
        assertEquals(emptySet<String>(), FavLogic.toggle(setOf("a"), "a"))
    }

    @Test
    fun pickFav_returnsNull_forEmpty() {
        assertNull(FavLogic.pickFav(emptyList()))
    }

    @Test
    fun pickFav_returnsOne() {
        val p = FavLogic.pickFav(listOf(st("a"), st("b")), rng = Random(42))!!
        assertTrue(p.uuid == "a" || p.uuid == "b")
    }

    @Test
    fun blocking_and_unblocking_a_station_toggles_the_set() {
        assertEquals(setOf("a"), FavLogic.toggle(emptySet(), "a"))
        assertEquals(emptySet<String>(), FavLogic.toggle(setOf("a"), "a"))
    }

    @Test
    fun blocking_does_not_touch_the_favourite_set() = runBlocking {
        val store = FavStore(context)
        val uuid = "test-station-uuid"

        store.applyMerged(
            favs = setOf(uuid),
            blocked = emptySet(),
            excluded = emptySet(),
        )

        store.toggleBlocked(uuid)

        val favs = store.currentFavUuids()
        val blocked = store.currentBlocked()

        assertEquals("favourite set must not be touched", setOf(uuid), favs)
        assertEquals("uuid must be blocked", setOf(uuid), blocked)
    }
}
