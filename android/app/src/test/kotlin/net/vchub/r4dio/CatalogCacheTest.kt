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

    private fun tempSurvivors(): List<String> =
        tmp.root.list().orEmpty().filter { it.endsWith(".tmp") }

    @Test
    fun write_leaves_no_temp_file_behind() {
        val cache = CatalogCache(tmp.root)
        cache.write(listOf(station("a")))
        assertTrue(File(tmp.root, "catalog.json").exists())
        assertEquals(emptyList<String>(), tempSurvivors())
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
    fun write_does_not_refuse_an_empty_list_so_callers_must_guard_before_calling() {
        // CatalogCache has no opinion about what "empty" means — it will happily
        // clobber a full, good catalogue with nothing if a caller passes it an
        // empty list (e.g. a failed network fetch). this is why PlaybackService's
        // fetchAndStore/withReadyCatalog check fetched.isEmpty() BEFORE calling
        // write(); the guard lives at the call site because it cannot live here.
        val cache = CatalogCache(tmp.root)
        val goodCatalogue = (1..1000).map { station("s$it") }
        cache.write(goodCatalogue)
        assertEquals(1000, cache.read().size)

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
        assertEquals(emptyList<String>(), tempSurvivors())
    }

    @Test
    fun successful_write_reports_true() {
        assertTrue(CatalogCache(tmp.root).write(listOf(station("a"))))
    }

    @Test
    fun failed_write_reports_false_and_leaves_previous_cache_intact() {
        val cache = CatalogCache(tmp.root)
        val original = listOf(station("original"))
        cache.write(original)

        tmp.root.setWritable(false)
        try {
            assertFalse(cache.write(listOf(station("new"))))
        } finally {
            tmp.root.setWritable(true)
        }

        assertEquals(original, cache.read())
        assertEquals(emptyList<String>(), tempSurvivors())
    }

    @Test
    fun failed_rename_reports_false_and_leaves_no_temp_file() {
        val cache = CatalogCache(tmp.root)
        cache.write(listOf(station("a")))
        // a directory in the way of the rename makes the rename fail after the
        // temp file has already been created and filled.
        val blocker = File(tmp.root, "catalog.json")
        blocker.delete()
        blocker.mkdir()
        File(blocker, "keep").writeText("x")

        assertFalse(cache.write(listOf(station("b"))))
        assertEquals(emptyList<String>(), tempSurvivors())
    }

    @Test
    fun concurrent_writes_never_leave_the_catalogue_empty_or_partial() {
        val cache = CatalogCache(tmp.root)
        val first = (1..500).map { station("a$it") }
        val second = (1..500).map { station("b$it") }
        cache.write(first)

        repeat(20) {
            val start = java.util.concurrent.CountDownLatch(1)
            val done = java.util.concurrent.CountDownLatch(2)
            listOf(first, second).forEach { list ->
                Thread {
                    start.await()
                    cache.write(list)
                    done.countDown()
                }.start()
            }
            start.countDown()
            done.await()

            val read = cache.read()
            assertTrue("catalogue was $read", read == first || read == second)
            assertEquals(emptyList<String>(), tempSurvivors())
        }
    }

    @Test
    fun read_concurrent_with_writes_never_returns_an_empty_catalogue() {
        val cache = CatalogCache(tmp.root)
        val first = (1..500).map { station("a$it") }
        val second = (1..500).map { station("b$it") }
        cache.write(first)

        val stop = java.util.concurrent.atomic.AtomicBoolean(false)
        val writer = Thread {
            while (!stop.get()) {
                cache.write(second)
                cache.write(first)
            }
        }
        writer.start()
        try {
            repeat(200) {
                val read = cache.read()
                assertTrue("catalogue was ${read.size} stations", read == first || read == second)
            }
        } finally {
            stop.set(true)
            writer.join()
        }
    }

    @Test
    fun merging_adds_only_the_new_ones() {
        val cache = CatalogCache(tmp.root)
        cache.write(listOf(station("a")))
        val added = cache.merge(listOf(station("a"), station("b")))
        assertEquals(1, added)
        assertEquals(setOf("a", "b"), cache.read().map { it.uuid }.toSet())
    }

    // a merge must never shrink the catalogue: the pool is what shuffle draws
    // from, and losing stations mid-session is worse than gaining none.
    @Test
    fun merging_nothing_keeps_what_is_there() {
        val cache = CatalogCache(tmp.root)
        cache.write(listOf(station("a")))
        assertEquals(0, cache.merge(emptyList()))
        assertEquals(1, cache.read().size)
    }

    @Test
    fun merging_does_not_duplicate_a_station_already_held() {
        val cache = CatalogCache(tmp.root)
        cache.write(listOf(station("a")))
        cache.merge(listOf(station("a")))
        assertEquals(1, cache.read().size)
    }

    // the incoming copy is the fresher one from the api, but replacing the held
    // entry would churn the file on every top-up for no gain; keeping it also
    // means a merge can never quietly rewrite a station's url under a listener.
    @Test
    fun merging_keeps_the_entry_already_held_on_a_uuid_collision() {
        val cache = CatalogCache(tmp.root)
        cache.write(listOf(Station("a", "held", "http://held", "UA", "MP3", 128)))
        cache.merge(listOf(Station("a", "incoming", "http://incoming", "UA", "MP3", 128)))
        assertEquals("held", cache.read().single().name)
    }

    // merge is called from a background thread while the pick path reads; a
    // read-modify-write that is not inside the lock loses stations under load.
    @Test
    fun concurrent_merges_never_lose_a_station() {
        val cache = CatalogCache(tmp.root)
        cache.write(listOf(station("seed")))
        val threads = (1..8).map { t ->
            Thread { repeat(25) { i -> cache.merge(listOf(station("t${t}s$i"))) } }
        }
        threads.forEach { it.start() }
        threads.forEach { it.join() }
        assertEquals(201, cache.read().size)
    }

    // every station cached before genres existed has empty tags. one forced
    // refetch fills them; without it the genre filter is empty for a day.
    @Test
    fun a_catalogue_with_no_genres_at_all_wants_a_backfill() {
        val cache = CatalogCache(tmp.root)
        assertTrue(cache.needsGenreBackfill(listOf(station("a"), station("b"))))
    }

    // one station with tags is enough to prove the catalogue came from a build
    // that stores them — some stations genuinely have none.
    @Test
    fun a_catalogue_with_any_genre_does_not() {
        val cache = CatalogCache(tmp.root)
        val tagged = Station("c", "N", "u", "UA", "MP3", 128, "jazz")
        assertFalse(cache.needsGenreBackfill(listOf(station("a"), tagged)))
    }

    // an empty catalogue is a cold start, not a stale one — the ordinary fetch
    // path handles it, and claiming a backfill would double-fetch.
    @Test
    fun an_empty_catalogue_does_not_want_a_backfill() {
        assertFalse(CatalogCache(tmp.root).needsGenreBackfill(emptyList()))
    }

    // proves the exact surviving set, not just a count: the removed station is
    // genuinely gone, the untouched one survives, and the added one is present.
    @Test
    fun applying_a_delta_adds_removes_and_keeps_the_rest() {
        val cache = CatalogCache(tmp.root)
        cache.write(listOf(station("a"), station("b")))
        assertTrue(cache.read().any { it.uuid == "a" }) // "a" is genuinely there before removal

        val after = cache.applyDelta(added = listOf(station("c")), removed = setOf("a"))

        assertEquals(setOf("b", "c"), after?.map { it.uuid }?.toSet())
        assertFalse(after!!.any { it.uuid == "a" })
        assertEquals(setOf("b", "c"), cache.read().map { it.uuid }.toSet())
    }

    // a station that is both removed and re-added in the same delta must
    // survive — added is applied after removed, so the incoming copy wins.
    // held has two stations so the removal share (1/2) stays under the ceiling.
    @Test
    fun a_station_removed_and_readded_in_the_same_delta_survives() {
        val cache = CatalogCache(tmp.root)
        cache.write(
            listOf(
                Station("a", "old name", "http://old", "UA", "MP3", 128),
                station("other"),
            ),
        )

        val after = cache.applyDelta(
            added = listOf(Station("a", "new name", "http://new", "UA", "MP3", 128)),
            removed = setOf("a"),
        )

        assertEquals("new name", after?.single { it.uuid == "a" }?.name)
    }

    // above the removal-share ceiling the delta is refused: null, and the disk
    // is untouched — the caller falls back to a full download instead.
    @Test
    fun a_delta_that_removes_too_large_a_share_is_refused_and_leaves_the_cache_untouched() {
        val cache = CatalogCache(tmp.root)
        val held = listOf(station("a"), station("b"), station("c"), station("d"))
        cache.write(held)

        // 3 of 4 held = 75%, above the 50% ceiling.
        val after = cache.applyDelta(added = emptyList(), removed = setOf("a", "b", "c"))

        assertEquals(null, after)
        assertEquals(held, cache.read())
    }

    // exactly at the ceiling is still allowed — the guard is "more than half",
    // not "half or more".
    @Test
    fun a_delta_at_exactly_the_removal_ceiling_is_allowed() {
        val cache = CatalogCache(tmp.root)
        cache.write(listOf(station("a"), station("b")))

        val after = cache.applyDelta(added = emptyList(), removed = setOf("a"))

        assertEquals(setOf("b"), after?.map { it.uuid }?.toSet())
    }

    // an empty held catalogue has nothing for a delta to apply against — this
    // is the cold-start case, and the caller must fall to a full download.
    @Test
    fun applying_a_delta_to_an_empty_cache_is_refused() {
        val cache = CatalogCache(tmp.root)
        val after = cache.applyDelta(added = listOf(station("a")), removed = emptySet())
        assertEquals(null, after)
    }

    @Test
    fun leftover_backup_from_an_older_version_is_not_resurrected() {
        // older builds moved the cache aside to catalog.json.bak. such a file is
        // stale by definition now and must never come back as the catalogue.
        val cache = CatalogCache(tmp.root)
        cache.write(listOf(station("current")))
        // a well-formed stale backup produced in a separate directory, planted
        // exactly as an older build would have left it behind on upgrade.
        val other = tmp.newFolder()
        CatalogCache(other).write(listOf(station("stale")))
        File(other, "catalog.json").copyTo(File(tmp.root, "catalog.json.bak"))

        assertEquals(listOf(station("current")), cache.read())

        // with the current cache gone the stale backup must NOT stand in for it.
        File(tmp.root, "catalog.json").delete()
        File(other, "catalog.json").copyTo(File(tmp.root, "catalog.json.bak"), overwrite = true)
        assertEquals(emptyList<Station>(), cache.read())
    }
}
