package net.vchub.r4dio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

class CatalogCacheTest {
    @get:Rule
    val tmp = TemporaryFolder()

    private fun station(uuid: String) =
        Station(uuid, "Name $uuid", "http://x/$uuid", "UA", "MP3", 128)

    @Test
    fun round_trips_stations() {
        val cache = CatalogCache(tmp.root)
        val stations = listOf(station("a"), station("b"))
        cache.write(stations)
        assertEquals(stations, cache.read())
    }

    @Test
    fun missing_file_reads_as_empty() {
        assertEquals(emptyList<Station>(), CatalogCache(tmp.root).read())
    }

    @Test
    fun corrupt_file_reads_as_empty_and_does_not_throw() {
        File(tmp.root, "catalog.json").writeText("{ this is not valid json")
        assertEquals(emptyList<Station>(), CatalogCache(tmp.root).read())
    }

    @Test
    fun truncated_json_reads_as_empty() {
        File(tmp.root, "catalog.json").writeText("""[{"uuid":"a","name":"N""")
        assertEquals(emptyList<Station>(), CatalogCache(tmp.root).read())
    }

    @Test
    fun write_leaves_no_temp_file_behind() {
        val cache = CatalogCache(tmp.root)
        cache.write(listOf(station("a")))
        assertTrue(File(tmp.root, "catalog.json").exists())
        assertFalse(File(tmp.root, "catalog.json.tmp").exists())
    }

    @Test
    fun write_replaces_previous_contents() {
        val cache = CatalogCache(tmp.root)
        cache.write(listOf(station("a"), station("b")))
        cache.write(listOf(station("c")))
        assertEquals(listOf(station("c")), cache.read())
    }

    @Test
    fun writing_an_empty_list_is_readable_as_empty() {
        val cache = CatalogCache(tmp.root)
        cache.write(listOf(station("a")))
        cache.write(emptyList())
        assertEquals(emptyList<Station>(), cache.read())
    }

    @Test
    fun empty_file_reads_as_empty() {
        File(tmp.root, "catalog.json").writeText("")
        assertEquals(emptyList<Station>(), CatalogCache(tmp.root).read())
    }

    @Test
    fun json_object_instead_of_array_reads_as_empty() {
        File(tmp.root, "catalog.json").writeText("{}")
        assertEquals(emptyList<Station>(), CatalogCache(tmp.root).read())
    }

    @Test
    fun write_replaces_previous_contents_atomically() {
        val cache = CatalogCache(tmp.root)
        cache.write(listOf(station("a"), station("b")))
        // write again with different content; proves delete-then-rename path works
        cache.write(listOf(station("c")))
        // result must be the NEW content, fully readable
        assertEquals(listOf(station("c")), cache.read())
        // the old stations must not be there
        assertFalse(cache.read().any { it.uuid == "a" || it.uuid == "b" })
    }

    @Test
    fun write_leaves_no_temp_file_behind_after_overwrite() {
        val cache = CatalogCache(tmp.root)
        cache.write(listOf(station("a")))
        cache.write(listOf(station("b")))
        assertFalse(File(tmp.root, "catalog.json.tmp").exists())
    }

    @Test
    fun when_write_fails_previous_cache_remains_readable() {
        val cache = CatalogCache(tmp.root)
        val original = listOf(station("original"))
        cache.write(original)

        // make the directory unwritable to force the write to fail on all operations
        // (including temp file creation, rename, etc.)
        tmp.root.setWritable(false)

        // try to write new content; this will fail because the directory is unwritable
        cache.write(listOf(station("new")))

        // restore permissions so we can read the cache
        tmp.root.setWritable(true)

        // the PREVIOUS cache must still be readable, not destroyed or truncated.
        // this test verifies that write() does not truncate the existing file in place.
        assertEquals(original, cache.read())
    }
}
