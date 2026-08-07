package net.vchub.r4dio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class BackupFileTest {
    private fun st(uuid: String) = Station(uuid, "name-$uuid", "http://$uuid", "DE", "MP3", 128)

    @Test
    fun a_backup_round_trips() {
        val out = BackupFile.encode(
            key = "r4-abc",
            favs = setOf("a", "b"),
            cached = listOf(st("a")),
            blocked = setOf("x"),
            excluded = setOf("DE"),
        )
        val back = BackupFile.decode(out)!!
        assertEquals("r4-abc", back.key)
        assertEquals(setOf("a", "b"), back.favs)
        assertEquals(listOf("a"), back.cached.map { it.uuid })
        assertEquals(setOf("x"), back.blocked)
        assertEquals(setOf("DE"), back.excluded)
    }

    @Test
    fun a_backup_without_a_key_is_still_valid() {
        // an unlinked device has favourites worth keeping even with no key
        val out = BackupFile.encode(null, setOf("a"), listOf(st("a")), emptySet(), emptySet())
        val back = BackupFile.decode(out)!!
        assertNull(back.key)
        assertEquals(setOf("a"), back.favs)
    }

    @Test
    fun garbage_decodes_to_null_rather_than_throwing() {
        assertNull(BackupFile.decode("not json"))
        assertNull(BackupFile.decode(""))
    }

    @Test
    fun another_apps_json_is_rejected() {
        // a well-formed json file that is not ours must not be silently accepted,
        // or restoring it would wipe the user's real state with empty values.
        assertNull(BackupFile.decode("""{"hello":"world"}"""))
    }

    @Test
    fun a_future_version_is_rejected() {
        val bumped = """{"version":99,"app":"r4dio","favs":["a"]}"""
        assertNull(BackupFile.decode(bumped))
    }

    @Test
    fun a_file_missing_the_marker_is_rejected() {
        assertNull(BackupFile.decode("""{"version":1,"favs":["a"]}"""))
    }
}
