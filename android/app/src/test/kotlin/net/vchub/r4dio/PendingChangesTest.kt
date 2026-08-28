package net.vchub.r4dio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class PendingChangesTest {
    @Test
    fun a_removal_is_recorded_as_gone() {
        val p = PendingChanges().note(ChangeSet.FAVS, "abc", gone = true)
        assertEquals(listOf(Change("abc", true)), p.favs)
    }

    @Test
    fun the_latest_change_for_an_id_replaces_the_earlier_one() {
        // star, unstar, star again must leave ONE entry, not three
        val p = PendingChanges()
            .note(ChangeSet.FAVS, "abc", gone = false)
            .note(ChangeSet.FAVS, "abc", gone = true)
            .note(ChangeSet.FAVS, "abc", gone = false)
        assertEquals(listOf(Change("abc", false)), p.favs)
    }

    @Test
    fun sets_do_not_bleed_into_each_other() {
        val p = PendingChanges()
            .note(ChangeSet.FAVS, "a", gone = true)
            .note(ChangeSet.BLOCKED, "b", gone = false)
        assertEquals(listOf(Change("a", true)), p.favs)
        assertEquals(listOf(Change("b", false)), p.blocked)
        assertTrue(p.excluded_countries.isEmpty())
    }

    @Test
    fun clearing_what_was_pushed_leaves_the_rest() {
        val pushed = PendingChanges().note(ChangeSet.FAVS, "a", gone = true)
        val current = pushed.note(ChangeSet.FAVS, "b", gone = true)
        assertEquals(listOf(Change("b", true)), current.clearPushed(pushed).favs)
    }

    @Test
    fun an_edit_written_during_the_round_trip_survives_the_clear() {
        // "a" was pushed as gone; while the request was in flight the user
        // starred it again. clearing the push must NOT drop that newer edit.
        val pushed = PendingChanges().note(ChangeSet.FAVS, "a", gone = true)
        val current = pushed.note(ChangeSet.FAVS, "a", gone = false)
        assertEquals(listOf(Change("a", false)), current.clearPushed(pushed).favs)
    }

    @Test
    fun empty_means_nothing_to_send() {
        assertTrue(PendingChanges().isEmpty())
        assertTrue(!PendingChanges().note(ChangeSet.FAVS, "a", gone = true).isEmpty())
    }
}
