# Android Catalogue Cache Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the Android app a persistent station cache so shuffle plays instantly, works with no network, and draws from 1000 genuinely usable stations.

**Architecture:** Three independent pieces. A new `CatalogCache` reads and atomically writes the station list as JSON in `filesDir` (never DataStore — it holds its whole contents in memory). `Catalog.fetchStations` over-fetches and filters so the stored list is a full 1000 after exclusions. `PlaybackService` loads from the cache on start and refreshes in the background only when the TTL has expired.

**Tech Stack:** Kotlin, kotlinx.serialization (already a dependency), DataStore Preferences (already a dependency, used only for the small timestamp), OkHttp, JUnit.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-08-01-android-catalog-cache-design.md`.
- Comments in English, lowercase. ONLY where genuinely non-obvious — explain "why", never "what". No trivial comments.
- Logs in English and lowercase.
- **Do not use `else if` chains.** Use `when` or early returns.
- No AI / Claude / Anthropic mention anywhere. No personal data.
- Never blind-overwrite an existing file: `Read` it first, then `Edit`.
- **Do NOT add Room, SQLite, or WorkManager.** All three are explicit non-goals.
- **Do NOT put the catalogue in DataStore.** It holds its entire contents in memory and rewrites the whole file per edit; a ~173 KB catalogue there is the bug this design exists to avoid. Only the small `catalog_synced_at` timestamp belongs in DataStore.
- **Do NOT touch the CLI (`crates/`) or the sync server.** Android only.
- **Do NOT weaken the RU/BY ban.** `isExcluded` must keep being applied at fetch time so banned stations never reach the cache file.
- TDD: write the failing test first, run it, watch it fail, then implement.
- Tests are JVM unit tests in `android/app/src/test/kotlin/net/vchub/r4dio/` (six already exist there — follow their style). No device or emulator in Tasks 1-3; Task 4 is the device pass and is controller-run.

---

### Task 1: The cache file — atomic write, total read

**Files:**
- Create: `android/app/src/main/kotlin/net/vchub/r4dio/CatalogCache.kt`
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/CatalogCacheTest.kt`

**Interfaces:**
- Consumes: `Station` and `FavStation` from `Station.kt`. **`FavStation` already exists and is exactly the serialisable form needed** — same six fields (`uuid`, `name`, `url`, `country`, `codec`, `bitrate`), with `FavStation.of(station)` and `favStation.toStation()` converters. Reuse it; do NOT declare a second station DTO.
- Produces, for Tasks 2 and 3:
  - `class CatalogCache(private val dir: File)`
  - `fun read(): List<Station>` — never throws; any failure yields an empty list.
  - `fun write(stations: List<Station>)` — atomic; never throws.

- [ ] **Step 1: Write the failing test**

Create `android/app/src/test/kotlin/net/vchub/r4dio/CatalogCacheTest.kt`:

```kotlin
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
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd /Users/vchub/dev/projects/world-radio/radio/android && ./gradlew testDebugUnitTest --tests "*CatalogCacheTest*" 2>&1 | tail -15`
Expected: FAIL — `unresolved reference: CatalogCache`.

- [ ] **Step 3: Implement**

Create `android/app/src/main/kotlin/net/vchub/r4dio/CatalogCache.kt`:

```kotlin
package net.vchub.r4dio

import android.util.Log
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.Json
import java.io.File

private const val CACHE_FILE = "catalog.json"

/**
 * the station catalogue on disk. deliberately a plain file rather than datastore:
 * datastore keeps its whole contents in memory and rewrites the file on every
 * edit, which a ~173kb catalogue would make expensive for unrelated settings.
 */
class CatalogCache(private val dir: File) {
    private val json = Json { ignoreUnknownKeys = true }
    private val file get() = File(dir, CACHE_FILE)

    fun read(): List<Station> {
        if (!file.exists()) {
            return emptyList()
        }
        return runCatching {
            json.decodeFromString(ListSerializer(FavStation.serializer()), file.readText())
                .map { it.toStation() }
        }.getOrElse {
            Log.w("r4dio", "catalog cache unreadable, refetching")
            emptyList()
        }
    }

    fun write(stations: List<Station>) {
        runCatching {
            val raw = json.encodeToString(
                ListSerializer(FavStation.serializer()),
                stations.map { FavStation.of(it) },
            )
            // write-then-rename so a process killed mid-write never leaves a
            // half-file where a reader can see it
            val tmp = File(dir, "$CACHE_FILE.tmp")
            tmp.writeText(raw)
            when (tmp.renameTo(file)) {
                true -> {}
                false -> {
                    file.writeText(raw)
                    tmp.delete()
                }
            }
        }.onFailure { Log.w("r4dio", "catalog cache write failed: ${it.message}") }
    }
}
```

Note on the rename fallback: `File.renameTo` can fail on some filesystems when the
target exists. The fallback writes directly and cleans the temp file, so a failed
rename degrades to a non-atomic write rather than losing the cache entirely.

`android.util.Log` is not available in plain JVM unit tests and throws
"not mocked". If the tests fail for that reason, add this to
`android/app/build.gradle.kts` inside the `android { }` block rather than removing
the logging:

```kotlin
    testOptions {
        unitTests.isReturnDefaultValues = true
    }
```

- [ ] **Step 4: Run the tests**

Run: `cd /Users/vchub/dev/projects/world-radio/radio/android && ./gradlew testDebugUnitTest --tests "*CatalogCacheTest*" 2>&1 | tail -15`
Expected: PASS, 7 tests.

- [ ] **Step 5: Commit**

```bash
cd /Users/vchub/dev/projects/world-radio/radio
git add android/app/src/main/kotlin/net/vchub/r4dio/CatalogCache.kt android/app/src/test/kotlin/net/vchub/r4dio/CatalogCacheTest.kt android/app/build.gradle.kts
git commit -m "feat(android): store the station list on disk so it survives a restart"
```

---

### Task 2: Over-fetch so the stored list is a full 1000

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/Catalog.kt` (`fetchStations`, `fetchOnce`)
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/CatalogFilterTest.kt` (create)

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces, for Task 3:
  - `fun takeAllowed(stations: List<Station>, userExcluded: Set<String>, target: Int): List<Station>` — a pure, testable top-level function in `Catalog.kt`.
  - `Catalog.fetchStations(target: Int = 1000, userExcluded: Set<String> = emptySet()): List<Station>` — requests 1.5x `target` from the API (see the OVERFETCH constants in Step 3) and returns at most `target` allowed stations.

**Why:** the radio-browser API can include a country (`countrycode=X`) but cannot exclude one, so exclusions must be applied client-side. Fetching exactly 1000 and then dropping the banned ones leaves fewer than 1000 usable stations.

- [ ] **Step 1: Write the failing test**

Create `android/app/src/test/kotlin/net/vchub/r4dio/CatalogFilterTest.kt`:

```kotlin
package net.vchub.r4dio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class CatalogFilterTest {
    private fun station(uuid: String, country: String = "UA", name: String = "Name") =
        Station(uuid, name, "http://x/$uuid", country, "MP3", 128)

    @Test
    fun keeps_only_the_target_count() {
        val input = (1..50).map { station("u$it") }
        assertEquals(10, takeAllowed(input, emptySet(), 10).size)
    }

    @Test
    fun drops_banned_countries_before_taking_the_target() {
        val input = listOf(
            station("ru1", country = "RU"),
            station("by1", country = "BY"),
            station("ua1"),
            station("ua2"),
        )
        val out = takeAllowed(input, emptySet(), 4)
        assertEquals(listOf("ua1", "ua2"), out.map { it.uuid })
    }

    @Test
    fun drops_user_hidden_countries() {
        val input = listOf(station("de1", country = "DE"), station("ua1"))
        val out = takeAllowed(input, setOf("DE"), 4)
        assertEquals(listOf("ua1"), out.map { it.uuid })
    }

    @Test
    fun a_full_target_survives_interleaved_banned_entries() {
        // the point of over-fetching: banned entries must not eat into the target
        val input = (1..30).flatMap { listOf(station("ru$it", country = "RU"), station("ok$it")) }
        val out = takeAllowed(input, emptySet(), 20)
        assertEquals(20, out.size)
        assertTrue(out.none { it.country == "RU" })
    }

    @Test
    fun fewer_survivors_than_the_target_returns_all_survivors() {
        val input = listOf(station("ua1"), station("ru1", country = "RU"))
        assertEquals(listOf("ua1"), takeAllowed(input, emptySet(), 10).map { it.uuid })
    }

    @Test
    fun drops_stations_with_a_blank_url() {
        val input = listOf(Station("b", "Blank", "", "UA", "MP3", 128), station("ua1"))
        assertEquals(listOf("ua1"), takeAllowed(input, emptySet(), 10).map { it.uuid })
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd /Users/vchub/dev/projects/world-radio/radio/android && ./gradlew testDebugUnitTest --tests "*CatalogFilterTest*" 2>&1 | tail -15`
Expected: FAIL — `unresolved reference: takeAllowed`.

- [ ] **Step 3: Implement**

Read `android/app/src/main/kotlin/net/vchub/r4dio/Catalog.kt` first. Add the pure
helper next to the other top-level functions (near `pickRandom`):

```kotlin
// the api can include a country but not exclude one, so exclusions are applied
// here — over-fetching is what keeps the kept list a full `target` afterwards.
fun takeAllowed(
    stations: List<Station>,
    userExcluded: Set<String>,
    target: Int,
): List<Station> = stations.filter { allowedStation(it, userExcluded) }.take(target)
```

Then change `fetchStations` and `fetchOnce`. The current code is:

```kotlin
    fun fetchStations(limit: Int = 1000): List<Station> {
        repeat(2) { attempt ->
            val result = runCatching { fetchOnce(limit) }.getOrDefault(emptyList())
            if (result.isNotEmpty()) return result
        }
        return runCatching { fetchOnce(limit) }.getOrDefault(emptyList())
    }
```

Replace with:

```kotlin
    fun fetchStations(
        target: Int = DEFAULT_TARGET,
        userExcluded: Set<String> = emptySet(),
    ): List<Station> {
        val ask = target * OVERFETCH_NUMERATOR / OVERFETCH_DENOMINATOR
        repeat(2) {
            val result = runCatching { fetchOnce(ask) }.getOrDefault(emptyList())
            if (result.isNotEmpty()) return takeAllowed(result, userExcluded, target)
        }
        val last = runCatching { fetchOnce(ask) }.getOrDefault(emptyList())
        return takeAllowed(last, userExcluded, target)
    }
```

And add these constants at the top of the file, beside `EXCLUDED_COUNTRYCODES`:

```kotlin
const val DEFAULT_TARGET = 1000

// ask for half again as many as we keep, so banned and hidden-country stations
// do not eat into the target. expressed as a ratio because `3 / 2` as a single
// integer constant would evaluate to 1.
private const val OVERFETCH_NUMERATOR = 3
private const val OVERFETCH_DENOMINATOR = 2
```

and in `fetchStations` compute the request size as:

```kotlin
        val ask = target * OVERFETCH_NUMERATOR / OVERFETCH_DENOMINATOR
```

`fetchOnce` keeps its existing `.filter { allowedStation(it) }` — the RU/BY ban must
stay applied there so banned stations never reach the cache even if a caller
forgets to filter. Do not remove it.

- [ ] **Step 4: Run the tests**

Run: `cd /Users/vchub/dev/projects/world-radio/radio/android && ./gradlew testDebugUnitTest 2>&1 | tail -12`
Expected: PASS — the new `CatalogFilterTest` plus every existing test (`ShuffleTest` and the others must stay green; if a call site of `fetchStations(limit)` broke, fix it to the new signature).

- [ ] **Step 5: Commit**

```bash
cd /Users/vchub/dev/projects/world-radio/radio
git add android/app/src/main/kotlin/net/vchub/r4dio/Catalog.kt android/app/src/test/kotlin/net/vchub/r4dio/CatalogFilterTest.kt
git commit -m "feat(android): fill the station list with 1000 stations you can actually play"
```

---

### Task 3: Play from the cache, refresh in the background

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/FavStore.kt` (add the timestamp key + accessors)
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt` (`loadStations`, `withReadyCatalog`)
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/CatalogTtlTest.kt` (create)

**Interfaces:**
- Consumes: `CatalogCache(dir).read()` / `.write(stations)` from Task 1; `Catalog.fetchStations(target, userExcluded)` from Task 2.
- Produces: nothing — this is the last code task.

- [ ] **Step 1: Write the failing test for the TTL rule**

Create `android/app/src/test/kotlin/net/vchub/r4dio/CatalogTtlTest.kt`:

```kotlin
package net.vchub.r4dio

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class CatalogTtlTest {
    private val day = 86_400L

    @Test
    fun never_synced_is_stale() {
        assertTrue(catalogIsStale(0L, now = 1_000_000L, ttlSecs = day))
    }

    @Test
    fun just_synced_is_fresh() {
        assertFalse(catalogIsStale(1_000_000L - 600, now = 1_000_000L, ttlSecs = day))
    }

    @Test
    fun older_than_the_ttl_is_stale() {
        assertTrue(catalogIsStale(1_000_000L - 25 * 3600, now = 1_000_000L, ttlSecs = day))
    }

    @Test
    fun exactly_at_the_ttl_is_stale() {
        assertTrue(catalogIsStale(1_000_000L - day, now = 1_000_000L, ttlSecs = day))
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd /Users/vchub/dev/projects/world-radio/radio/android && ./gradlew testDebugUnitTest --tests "*CatalogTtlTest*" 2>&1 | tail -12`
Expected: FAIL — `unresolved reference: catalogIsStale`.

- [ ] **Step 3: Implement the TTL helper**

Add to `android/app/src/main/kotlin/net/vchub/r4dio/CatalogCache.kt` as a top-level function:

```kotlin
const val CATALOG_TTL_SECS = 86_400L

fun catalogIsStale(syncedAt: Long, now: Long, ttlSecs: Long = CATALOG_TTL_SECS): Boolean =
    now - syncedAt >= ttlSecs
```

A `syncedAt` of 0 means "never synced", which is always stale — the subtraction
handles that without a special case.

- [ ] **Step 4: Run the tests**

Run: `cd /Users/vchub/dev/projects/world-radio/radio/android && ./gradlew testDebugUnitTest --tests "*CatalogTtlTest*" 2>&1 | tail -12`
Expected: PASS, 4 tests.

- [ ] **Step 5: Add the timestamp to DataStore**

Read `android/app/src/main/kotlin/net/vchub/r4dio/FavStore.kt` first. Beside the
other keys (around line 53-59) add:

```kotlin
    private val keyCatalogSyncedAt = longPreferencesKey("catalog_synced_at")
```

Add the import `androidx.datastore.preferences.core.longPreferencesKey` next to the
existing `stringPreferencesKey` import.

Then add the two accessors, following the style of the neighbouring suspend
functions:

```kotlin
    suspend fun catalogSyncedAt(): Long = store.data.first()[keyCatalogSyncedAt] ?: 0L

    suspend fun setCatalogSyncedAt(epochSecs: Long) {
        store.edit { it[keyCatalogSyncedAt] = epochSecs }
    }
```

Only the timestamp goes here. The catalogue itself must NOT go into DataStore.

- [ ] **Step 6: Wire the service to the cache**

Read `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt` first. It
currently has, around line 189:

```kotlin
    private fun loadStations() {
        thread {
            val fetched = catalog.fetchStations()
            stations = fetched
            Log.i("r4dio", "loaded ${fetched.size} stations")
            val userExcluded = runBlocking { favStore.currentExcluded() }
            val pick = pickRandom(fetched, userExcluded) ?: return@thread
            main.post { playPick(pick) }
        }
    }
```

Add a cache field beside the existing `catalog` field:

```kotlin
    private val catalogCache by lazy { CatalogCache(filesDir) }
```

Replace `loadStations` with a cache-first version:

```kotlin
    private fun loadStations() {
        thread {
            val userExcluded = runBlocking { favStore.currentExcluded() }
            val cached = catalogCache.read()
            when (cached.isEmpty()) {
                true -> fetchAndStore(userExcluded)?.let { startFrom(it, userExcluded) }
                false -> {
                    stations = cached
                    Log.i("r4dio", "loaded ${cached.size} stations from cache")
                    startFrom(cached, userExcluded)
                    refreshIfStale(userExcluded)
                }
            }
        }
    }

    private fun startFrom(list: List<Station>, userExcluded: Set<String>) {
        val pick = pickRandom(list, userExcluded) ?: return
        main.post { playPick(pick) }
    }

    /** returns the fetched list, or null when the network gave us nothing. */
    private fun fetchAndStore(userExcluded: Set<String>): List<Station>? {
        val fetched = catalog.fetchStations(userExcluded = userExcluded)
        if (fetched.isEmpty()) {
            Log.w("r4dio", "catalog fetch returned nothing")
            return null
        }
        stations = fetched
        catalogCache.write(fetched)
        runBlocking { favStore.setCatalogSyncedAt(nowSecs()) }
        Log.i("r4dio", "fetched ${fetched.size} stations")
        return fetched
    }

    // a stale cache still plays; the refresh only replaces it if it succeeds.
    private fun refreshIfStale(userExcluded: Set<String>) {
        val syncedAt = runBlocking { favStore.catalogSyncedAt() }
        if (!catalogIsStale(syncedAt, nowSecs())) {
            return
        }
        thread { fetchAndStore(userExcluded) }
    }

    private fun nowSecs(): Long = System.currentTimeMillis() / 1000
```

Then update `withReadyCatalog` (around line 322). It currently fetches when the
in-memory list is empty; make it consult the cache first so a shuffle triggered
before `loadStations` finishes does not hit the network:

```kotlin
    private suspend fun withReadyCatalog(): List<Station> {
        val cur = stations
        if (cur.isNotEmpty()) return cur
        val cached = withContext(Dispatchers.IO) { catalogCache.read() }
        if (cached.isNotEmpty()) {
            stations = cached
            return cached
        }
        val userExcluded = withContext(Dispatchers.IO) { favStore.currentExcluded() }
        val fetched = withContext(Dispatchers.IO) {
            catalog.fetchStations(userExcluded = userExcluded)
        }
        stations = fetched
        withContext(Dispatchers.IO) { catalogCache.write(fetched) }
        Log.i("r4dio", "fetched ${fetched.size} stations for shuffle")
        return fetched
    }
```

Do not change `shuffle()`, `playPick()`, or anything else in the file.

- [ ] **Step 7: Build and run the whole suite**

Run: `cd /Users/vchub/dev/projects/world-radio/radio/android && ./gradlew assembleDebug testDebugUnitTest 2>&1 | tail -15`
Expected: `BUILD SUCCESSFUL`, all tests pass.

- [ ] **Step 8: Commit**

```bash
cd /Users/vchub/dev/projects/world-radio/radio
git add android/app/src/main/kotlin/net/vchub/r4dio/CatalogCache.kt android/app/src/main/kotlin/net/vchub/r4dio/FavStore.kt android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt android/app/src/test/kotlin/net/vchub/r4dio/CatalogTtlTest.kt
git commit -m "feat(android): start playing instantly and keep working without a connection"
```

---

### Task 4: Emulator verification (controller-run, not a subagent)

**Files:** none — verification only.

This task is run by the controller. It drives a real emulator and must never touch
the user's real data. The emulator must be shut down when the run finishes.

- [ ] **Step 1: Boot, build, install**

```bash
export PATH="$PATH:$HOME/Library/Android/sdk/emulator:$HOME/Library/Android/sdk/platform-tools"
nohup emulator -avd Pixel_7 -no-snapshot-load > /tmp/emu.log 2>&1 &
# wait for `adb shell getprop sys.boot_completed` to return 1, then:
cd /Users/vchub/dev/projects/world-radio/radio/android && ./gradlew assembleDebug
adb uninstall net.vchub.r4dio 2>/dev/null
adb install app/build/outputs/apk/debug/app-debug.apk
```

- [ ] **Step 2: First launch writes the cache**

```bash
adb logcat -c
adb shell am start -n net.vchub.r4dio/.MainActivity
# allow notifications if prompted, wait ~25s, then:
adb logcat -d -s r4dio:V | tail -10
adb shell run-as net.vchub.r4dio ls -la files/
```
Expected: a "fetched N stations" line, and `files/catalog.json` present with a
non-zero size. No `catalog.json.tmp` left behind.

- [ ] **Step 3: Second launch plays from the cache**

```bash
adb shell am force-stop net.vchub.r4dio
adb logcat -c
adb shell am start -n net.vchub.r4dio/.MainActivity
sleep 15
adb logcat -d -s r4dio:V | tail -10
```
Expected: a "loaded N stations from cache" line and playback starting. Because the
timestamp was just written, there must be NO second fetch this launch.

- [ ] **Step 4: Airplane mode still shuffles**

```bash
adb shell svc wifi disable && adb shell svc data disable
adb shell am force-stop net.vchub.r4dio
adb shell am start -n net.vchub.r4dio/.MainActivity
sleep 12
adb logcat -d -s r4dio:V | tail -8
adb shell svc wifi enable && adb shell svc data enable
```
Expected: "loaded N stations from cache" — the app still has a catalogue and picks
a station, even though the stream itself cannot play without network.

- [ ] **Step 5: No banned stations in the cache**

```bash
adb shell run-as net.vchub.r4dio cat files/catalog.json > /tmp/catalog.json
python3 -c "
import json
d = json.load(open('/tmp/catalog.json'))
print('stations:', len(d))
bad = [s for s in d if s['country'].upper() in ('RU','BY')]
print('banned present:', len(bad))
"
```
Expected: a count at or near 1000, and **zero** banned stations.

- [ ] **Step 6: Clean up**

```bash
adb uninstall net.vchub.r4dio
adb emu kill
rm -f /tmp/catalog.json
```

---

## Notes for the reviewer

The highest-risk area is Task 3's threading. `loadStations` already runs inside
`thread { }`, and the new `refreshIfStale` starts a second one; confirm the
background refresh cannot race the initial pick into playing two stations at once,
and that `stations` (a `@Volatile` field) is only ever assigned whole lists.

Second: confirm the RU/BY ban is still applied inside `fetchOnce`, not only in
`takeAllowed`. Losing it there would let a banned station reach the cache file
through any future caller that forgets to filter.

Third: confirm nothing put the catalogue itself into DataStore — only
`catalog_synced_at` belongs there.
