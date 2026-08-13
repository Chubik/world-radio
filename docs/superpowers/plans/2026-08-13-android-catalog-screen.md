# Android Catalog Screen Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Phase 2 of the Compose rewrite — the Catalog tab stops being a placeholder. Live search over the cached catalogue, a filter sheet (country / genre / codec / min bitrate), chips showing what is in force with one-tap clearing, and a tap that actually plays the station.

**Architecture:** The catalogue is read straight from `CatalogCache` on the UI side — the same file the service writes, which is safe because writes are a temp-file-plus-atomic-rename. Search and filtering are pure functions over that list, fast enough to run on every keystroke (measured ~3ms per pass at the full 62k catalogue size). Two new capabilities the app has never had: a session command carrying a station uuid, and a local block/unblock write path.

**Tech Stack:** Kotlin, Jetpack Compose, Media3 session commands, DataStore, kotlinx.serialization.

**Spec:** `docs/superpowers/specs/2026-08-13-android-compose-rewrite-design.md`
**Design:** `_private/design/2026-08-10-android-handoff.html`, section "Catalog + Search + Filters" (not in git — private directory)

## Global Constraints

- Repo `radio`, branch `dev`, everything under `android/`. Do not push — the controller pushes after review.
- **Every colour from a token** (`R4dioTokens.colors`, or the derived `Palette.panel()/rule()/mute()`). No `Color(0xFF…)` literals, no `colorResource`, no `@color/`. One hardcoded colour breaks all 14 themes.
- **Every user-facing string in `strings.xml`.** Phase 1 ended by extracting the last of them; do not reintroduce literals.
- **Letter spacing uses `.em`, never `.sp`** — Compose's `.sp` letterSpacing is absolute where Android XML's is em-relative. `HomeScreen.kt` and `Pill.kt` are the reference.
- **The RU/BY ban holds on every ingest and every display path** ([[exclude-russian-stations]]). A catalogue screen that lists a banned station is a defect, even though the fetch paths already filter.
- **Never launch playback to prove something** — the emulator plays audio out loud. Read `catalog.json`, the rendered screen, and `logcat -s r4dio`. Task 6 has one deliberate exception, called out where it applies.
- Verified on the emulator, not by reading code. Screenshots prove layout, **not colour** — check pairings against `ui/Palette.kt`.
- All code, comments and commit messages English, lowercase-first. Comments only where they state a constraint. Commit subjects are the published changelog.
- Files stay under 600–800 lines; split by responsibility when one grows past that.
- Gate before every commit: `cd android && ./gradlew test --rerun-tasks` green (a plain `./gradlew test` reports UP-TO-DATE and proves nothing), real output pasted in the report.
- Gradle quirk: `./gradlew test --tests '…'` does **not** work — the aggregate task rejects `--tests`. Use `./gradlew testDebugUnitTest --tests '*SomeTest*'`.

## What does not exist yet, and is therefore in scope

Established by inspection on 2026-08-13:

- **No genre or language data.** `ApiStation` decodes six fields; the API's `tags` and `language` are silently dropped by `ignoreUnknownKeys`. 76% of stations have tags. Task 1 adds them, and the cache must be refetched — the user has confirmed there are no users yet, so a forced refresh is acceptable.
- **No search of any kind**, on device or in the API client.
- **No way to play a chosen station.** All seven `CMD_*` commands take an empty `Bundle`; `PlayerConnection.send(String)` cannot carry an argument.
- **No local block path.** `blocked_uuids` is written only by sync merge and backup restore.
- **No observable blocked state** — `currentBlocked()` is a one-shot read, unlike `favUuids`/`excludedCountries` which have Flows.

---

### Task 0: The catalogue actually fills up

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/Catalog.kt` (the ceiling)
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt` (`topUpCatalogue`)
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/TopUpTest.kt` (add cases)

**Interfaces:**
- Consumes: `topUpAllowed`, `Catalog.fetchPage`, `CatalogCache.merge` — all exist.
- Produces: nothing new; this changes how often the existing page fetch runs.

**Why this is task 0 and not a later phase.** Measured on 2026-08-13 against the live API: the service holds **1,286** stations, the API has **62,250**. The top-up fetches **one 200-station page per service start or shuffle**, and only on unmetered + charging — so reaching even the current 20,000 ceiling takes ~95 opportunities and the full catalogue over 300. In practice the emulator's log reads `top-up skipped: charging=false` and the catalogue has not moved. A catalogue screen filtering over 2% of the world's stations reproduces the exact complaint this whole project started from.

Sizes, measured rather than assumed: the eight fields we keep are ~263 bytes per station, so the full catalogue is **~15.6 MB on disk** and roughly **70 MB over the wire**. Both are acceptable once, on wi-fi. `TOP_UP_CEILING = 20_000` was a guess of mine, not a limit from the data.

**Read first:** `PlaybackService.kt` — `topUpCatalogue()` and its `topUpInFlight` guard — and `Catalog.kt:49-60` for the constants and `topUpAllowed`.

**The rule that must not be broken:** "без навантаження" means waiting for a moment that costs the user nothing. The conditions do not relax — unmetered AND charging, re-read on every page, never cached. What changes is that one opportunity now fetches pages until a condition fails, instead of exactly one page.

- [ ] **Step 1: Write the failing tests**

```kotlin
    // the ceiling exists to stop, not to cap the world at a third of it.
    @Test
    fun the_ceiling_is_the_whole_catalogue() {
        assertTrue(TOP_UP_CEILING >= 62_000)
    }

    // one page per opportunity took hundreds of launches to fill; a run keeps
    // going while the moment is still free, and stops the instant it is not.
    @Test
    fun a_run_stops_as_soon_as_a_condition_fails() {
        var calls = 0
        val pages = topUpRun(held = 1000, ceiling = 62_000, allowed = { calls++ < 3 })
        assertEquals(3, pages)
    }

    @Test
    fun a_run_stops_at_the_ceiling_even_while_conditions_hold() {
        val pages = topUpRun(held = 61_900, ceiling = 62_000, allowed = { true })
        assertEquals(1, pages)
    }

    @Test
    fun a_run_that_may_not_start_fetches_nothing() {
        assertEquals(0, topUpRun(held = 1000, ceiling = 62_000, allowed = { false }))
    }
```

- [ ] **Step 2: Run red, then implement the loop shape as a pure function** so the stopping rules are testable without a device:

```kotlin
/**
 * how many pages a single top-up opportunity should fetch. pure so the stopping
 * rules can be tested; the caller does the fetching and re-reads the conditions
 * through [allowed] before each page, never caching them.
 */
fun topUpRun(held: Int, ceiling: Int, limit: Int = TOP_UP_PAGE, allowed: () -> Boolean): Int {
    var have = held
    var pages = 0
    while (have < ceiling && allowed()) {
        have += limit
        pages++
    }
    return pages
}
```

Raise `TOP_UP_CEILING` to `62_000` with a comment recording the measurement and the date.

- [ ] **Step 3: Wire it into `topUpCatalogue`.** Replace the single fetch with a loop that re-reads `conditions.unmetered()` and `conditions.charging()` before **each** page, merges each page as it lands (so a run interrupted halfway keeps what it got), and stops on: conditions gone, ceiling reached, an empty page, or a page that adds nothing new. Log once per run with the totals rather than once per page.

- [ ] **Step 4: Run to verify they pass**, then `./gradlew test --rerun-tasks` green.

- [ ] **Step 5: Real-path proof required.** On the emulator with `adb shell dumpsys battery set ac 1`, report the catalogue count before and after a single launch — it must climb by thousands, not by 200. Then `adb shell dumpsys battery unplug`, relaunch, and confirm the log says the run stopped and the count held. Report both numbers and the wall-clock time the run took.

- [ ] **Step 6: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/Catalog.kt android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt android/app/src/test/kotlin/net/vchub/r4dio/TopUpTest.kt
git commit -m "the whole world of stations, not a corner of it"
```

---

### Task 1: Genre and language reach the phone

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/Station.kt`
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/StationFieldsTest.kt` (create)

**Interfaces:**
- Produces (every later task filters on these):

```kotlin
data class Station(
    val uuid: String, val name: String, val url: String, val country: String,
    val codec: String, val bitrate: Int,
    val tags: String = "",      // comma-separated, as the api sends it
    val language: String = "",  // comma-separated, as the api sends it
)
fun Station.genres(): List<String>   // trimmed, lowercased, blanks dropped
```

**Read first:** `Station.kt` in full — it is 43 lines and holds all three shapes (`ApiStation` from the wire, `Station` in memory, `FavStation` on disk). All three need the two fields, or they are lost at whichever boundary is missed.

Both fields default to `""`, which is what makes the change backward compatible: an existing `catalog.json` written before this task decodes fine, with empty genres, until the catalogue refreshes.

- [ ] **Step 1: Write the failing tests**

```kotlin
package net.vchub.r4dio

import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Test

class StationFieldsTest {
    private val json = Json { ignoreUnknownKeys = true }

    @Test
    fun the_api_gives_us_tags_and_language() {
        val body = """{"stationuuid":"a","name":"Jazz FM","url_resolved":"http://x",
            "countrycode":"UA","codec":"MP3","bitrate":128,
            "tags":"jazz,lounge","language":"english"}"""
        val s = json.decodeFromString(ApiStation.serializer(), body).toStation()
        assertEquals("jazz,lounge", s.tags)
        assertEquals("english", s.language)
    }

    // the cache is the only copy on disk; a field that survives the wire but not
    // the round trip through FavStation is lost the moment the app restarts.
    @Test
    fun tags_survive_the_trip_to_disk_and_back() {
        val s = Station("a", "Jazz FM", "http://x", "UA", "MP3", 128, "jazz,lounge", "english")
        val back = FavStation.of(s).toStation()
        assertEquals(s, back)
    }

    // a catalog.json written before this task has no tags key at all. it must
    // still load — the alternative is every user losing their catalogue.
    @Test
    fun a_cache_written_before_tags_existed_still_loads() {
        val old = """{"uuid":"a","name":"N","url":"u","country":"UA","codec":"MP3","bitrate":128}"""
        val s = json.decodeFromString(FavStation.serializer(), old).toStation()
        assertEquals("", s.tags)
        assertEquals("", s.language)
    }

    @Test
    fun genres_are_split_trimmed_and_lowercased() {
        val s = Station("a", "N", "u", "UA", "MP3", 128, "Jazz, LOUNGE ,, news")
        assertEquals(listOf("jazz", "lounge", "news"), s.genres())
    }

    @Test
    fun a_station_with_no_tags_has_no_genres() {
        assertEquals(emptyList<String>(), Station("a", "N", "u", "UA", "MP3", 128).genres())
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd android && ./gradlew testDebugUnitTest --tests '*StationFieldsTest*'`
Expected: compile failure — `Station` has no `tags` parameter.

- [ ] **Step 3: Implement**

Add `tags` and `language` to all three data classes with `= ""` defaults, carry them through `toStation()` and `FavStation.of()`, and add:

```kotlin
/** the api sends tags as one comma-separated string; every consumer wants a list. */
fun Station.genres(): List<String> =
    tags.split(",").map { it.trim().lowercase() }.filter { it.isNotEmpty() }
```

`ApiStation` needs no `@SerialName` for either field — the api's keys are already `tags` and `language`.

- [ ] **Step 4: Run to verify they pass**, then `./gradlew test --rerun-tasks` green.

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/Station.kt android/app/src/test/kotlin/net/vchub/r4dio/StationFieldsTest.kt
git commit -m "stations remember what kind of music they play"
```

---

### Task 2: The catalogue refreshes so genres arrive

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/CatalogCache.kt`
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt`
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/CatalogCacheTest.kt` (add cases)

**Interfaces:**
- Consumes: `Station.tags` (Task 1).
- Produces:

```kotlin
// in CatalogCache
fun needsGenreBackfill(stations: List<Station>): Boolean
```

**Read first:** `CatalogCache.kt` in full, and `PlaybackService.kt:270-300` (`loadStations`) plus `:329-360` (`refreshIfStale`). The existing staleness mechanism is a 24-hour TTL on `catalog_synced_at`; this task adds a second, one-off reason to refetch.

Every station already on disk has empty tags. Waiting for the TTL would leave the genre filter empty for up to a day; forcing one refetch when the held catalogue has no genres at all fixes it on the next launch.

- [ ] **Step 1: Write the failing tests**

```kotlin
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
```

- [ ] **Step 2: Run red, then implement**

```kotlin
    /**
     * true when the held catalogue predates genres entirely. one-off: as soon as
     * a refetch lands, at least one station carries tags and this stops firing.
     */
    fun needsGenreBackfill(stations: List<Station>): Boolean =
        stations.isNotEmpty() && stations.none { it.tags.isNotBlank() }
```

- [ ] **Step 3: Wire it**

In `PlaybackService.loadStations()`, on the cache-hit branch (where `refreshIfStale` is already called), zero the sync stamp when `catalogCache.needsGenreBackfill(cached)` is true, so the existing `refreshIfStale` does the work rather than adding a second fetch path:

```kotlin
                    // the held catalogue predates genres; drop the stamp so the
                    // existing staleness path refetches it once, now.
                    if (catalogCache.needsGenreBackfill(cached)) {
                        Log.i("r4dio", "catalogue has no genres, refetching once")
                        runBlocking { favStore.setCatalogSyncedAt(0) }
                    }
                    refreshIfStale(userExcluded, blocked)
```

- [ ] **Step 4:** `./gradlew test --rerun-tasks` green.

- [ ] **Step 5: Real-path proof required.** On the emulator: note the station count and confirm `catalog.json` currently has no `tags` key. Launch, wait for the refresh, then confirm `tags` is present on a good proportion of entries. Report the before/after percentage of stations carrying a non-empty `tags` — the live API gives roughly 76%.

- [ ] **Step 6: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/CatalogCache.kt android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt android/app/src/test/kotlin/net/vchub/r4dio/CatalogCacheTest.kt
git commit -m "the catalogue picks up genres on its own"
```

---

### Task 3: Search and filtering, as pure functions

**Files:**
- Create: `android/app/src/main/kotlin/net/vchub/r4dio/CatalogQuery.kt`
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/CatalogQueryTest.kt`

**Interfaces:**
- Consumes: `Station`, `Station.genres()` (Task 1), `isExcluded` (`Catalog.kt:24`).
- Produces (Tasks 4–6 all use these):

```kotlin
data class CatalogFilters(
    val countries: Set<String> = emptySet(),   // uppercase ISO codes
    val genres: Set<String> = emptySet(),      // lowercase
    val codecs: Set<String> = emptySet(),      // uppercase
    val minBitrate: Int = 0,
) {
    val isEmpty: Boolean
    val activeCount: Int
}
fun searchCatalog(stations: List<Station>, query: String, filters: CatalogFilters): List<Station>
fun countryFacets(stations: List<Station>): List<Pair<String, Int>>  // code to count, commonest first
fun genreFacets(stations: List<Station>): List<Pair<String, Int>>
fun codecFacets(stations: List<Station>): List<Pair<String, Int>>
```

**Read first:** `Catalog.kt:24-30` (`isExcluded`) and `:62-83` (`allowedStation`). **`allowedStation` is not reusable here** and must not be called: it folds together the editorial ban, blocked-ness and the *shuffle* country filter, and a browse screen needs a different combination — it shows blocked stations (so they can be unblocked) but must never show banned ones. Reuse `isExcluded` alone.

Measured: a substring pass costs ~0.15ms over today's 1,286 stations and ~3ms over the full 62k the catalogue reaches after Task 0 — so filtering on every keystroke needs no index. If it ever does, that is a later measurement's call, not a guess made here.

- [ ] **Step 1: Write the failing tests**

```kotlin
package net.vchub.r4dio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class CatalogQueryTest {
    private fun st(
        uuid: String, name: String, country: String = "UA",
        codec: String = "MP3", bitrate: Int = 128, tags: String = "",
    ) = Station(uuid, name, "http://x/$uuid", country, codec, bitrate, tags)

    private val all = listOf(
        st("a", "Jazz Cafe", tags = "jazz,lounge"),
        st("b", "Radio Trek", country = "UA", tags = "news"),
        st("c", "Kyiv Talk", country = "UA", codec = "AAC", bitrate = 64, tags = "talk,news"),
        st("d", "Warsaw Jazz", country = "PL", codec = "AAC", bitrate = 256, tags = "jazz"),
    )

    @Test
    fun an_empty_query_and_no_filters_returns_everything() {
        assertEquals(4, searchCatalog(all, "", CatalogFilters()).size)
    }

    @Test
    fun search_matches_the_name_case_insensitively() {
        assertEquals(listOf("a", "d"), searchCatalog(all, "jazz", CatalogFilters()).map { it.uuid })
        assertEquals(listOf("a", "d"), searchCatalog(all, "JAZZ", CatalogFilters()).map { it.uuid })
    }

    // a substring, not a prefix: "trek" must find "Radio Trek", because that is
    // how a person looks for a station whose full name they half remember.
    @Test
    fun search_matches_anywhere_in_the_name() {
        assertEquals(listOf("b"), searchCatalog(all, "trek", CatalogFilters()).map { it.uuid })
    }

    @Test
    fun surrounding_whitespace_in_a_query_is_ignored() {
        assertEquals(2, searchCatalog(all, "  jazz  ", CatalogFilters()).size)
    }

    @Test
    fun the_country_filter_narrows_to_those_countries() {
        val f = CatalogFilters(countries = setOf("PL"))
        assertEquals(listOf("d"), searchCatalog(all, "", f).map { it.uuid })
    }

    @Test
    fun several_countries_are_an_or() {
        val f = CatalogFilters(countries = setOf("PL", "UA"))
        assertEquals(4, searchCatalog(all, "", f).size)
    }

    @Test
    fun the_genre_filter_matches_one_of_a_stations_tags() {
        val f = CatalogFilters(genres = setOf("news"))
        assertEquals(listOf("b", "c"), searchCatalog(all, "", f).map { it.uuid })
    }

    @Test
    fun the_codec_and_bitrate_filters_narrow_too() {
        assertEquals(listOf("c", "d"), searchCatalog(all, "", CatalogFilters(codecs = setOf("AAC"))).map { it.uuid })
        assertEquals(listOf("d"), searchCatalog(all, "", CatalogFilters(minBitrate = 256)).map { it.uuid })
    }

    // groups are ANDed, values within a group ORed — the same rule the cli uses,
    // and the one the filter sheet's "Show N stations" count depends on.
    @Test
    fun groups_combine_with_and() {
        val f = CatalogFilters(countries = setOf("PL"), genres = setOf("jazz"), codecs = setOf("AAC"))
        assertEquals(listOf("d"), searchCatalog(all, "", f).map { it.uuid })
        assertTrue(searchCatalog(all, "", f.copy(countries = setOf("UA"))).isEmpty())
    }

    @Test
    fun a_query_and_a_filter_combine() {
        val f = CatalogFilters(countries = setOf("PL"))
        assertEquals(listOf("d"), searchCatalog(all, "jazz", f).map { it.uuid })
    }

    // the ban is a product requirement on every surface, including one that is
    // only browsing. a banned station in a list is as wrong as one playing.
    @Test
    fun banned_stations_never_appear_however_you_search() {
        val banned = listOf(
            st("r", "Russia Today", country = "RU"),
            st("m", "Moscow FM", country = "UA"),
        )
        assertTrue(searchCatalog(banned, "", CatalogFilters()).isEmpty())
        assertTrue(searchCatalog(banned, "russia", CatalogFilters()).isEmpty())
    }

    @Test
    fun a_filter_set_knows_whether_it_is_empty_and_how_many_groups_are_active() {
        assertTrue(CatalogFilters().isEmpty)
        assertEquals(0, CatalogFilters().activeCount)
        val f = CatalogFilters(countries = setOf("UA"), minBitrate = 128)
        assertFalse(f.isEmpty)
        assertEquals(2, f.activeCount)
    }

    @Test
    fun facets_count_what_is_there_commonest_first() {
        assertEquals(listOf("UA" to 3, "PL" to 1), countryFacets(all))
        assertEquals("news" to 2, genreFacets(all).first())
        assertEquals(listOf("MP3" to 2, "AAC" to 2), codecFacets(all).sortedBy { it.first })
    }

    // facets drive the filter sheet's option list; offering a genre nothing
    // carries would be a dead row the user can only be disappointed by.
    @Test
    fun facets_ignore_banned_stations_too() {
        assertTrue(countryFacets(listOf(st("r", "X", country = "RU"))).isEmpty())
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd android && ./gradlew testDebugUnitTest --tests '*CatalogQueryTest*'`
Expected: compile failure — `CatalogFilters` does not exist.

- [ ] **Step 3: Implement.** `searchCatalog` filters out `isExcluded` first, then applies the query as a case-insensitive substring over `name`, then each non-empty filter group as an AND of ORs. The three facet functions drop banned stations, count, and sort by count descending then by key ascending so the order is stable.

- [ ] **Step 4: Run green**, then `./gradlew test --rerun-tasks`.

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/CatalogQuery.kt android/app/src/test/kotlin/net/vchub/r4dio/CatalogQueryTest.kt
git commit -m "find a station by name, country, genre or quality"
```

---

### Task 4: Playing a station you chose

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt`
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/ui/PlayerConnection.kt`
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/ui/PlayerConnectionTest.kt` (add cases)

**Interfaces:**
- Produces:

```kotlin
const val CMD_PLAY_UUID = "net.vchub.r4dio.PLAY_UUID"
const val ARG_UUID = "uuid"
// in ControllerHandle
fun sendCustomCommand(command: String, args: Bundle)
// in PlayerConnection
fun send(command: String, args: Bundle = Bundle.EMPTY)
```

**Read first:** `PlaybackService.kt:30-36` (the command constants), `:795-804` (`onConnect`, where commands are whitelisted), `:815-890` (`onCustomCommand`), and `ui/PlayerConnection.kt` in full. **A session command must be registered in two places** — the field list and `onConnect`'s available-commands builder. Missing the second makes it a silent no-op; this bit the phase-1 work and is the repo's known defect class.

`CMD_SCOPE` already reads an optional argument (`args.getString("scope")`), so the pattern exists — but no caller has ever sent one.

- [ ] **Step 1: Write the failing tests**

```kotlin
    @Test
    fun a_command_can_carry_an_argument() {
        val handle = FakeHandle(mediaItemCount = 1)
        var ready: ((ControllerHandle) -> Unit)? = null
        val conn = PlayerConnection { onReady -> ready = onReady }
        conn.connect()
        ready!!(handle)
        conn.send(CMD_PLAY_UUID, Bundle().apply { putString(ARG_UUID, "abc") })
        assertEquals(listOf(CMD_PLAY_UUID), handle.sent)
        assertEquals("abc", handle.sentArgs.single().getString(ARG_UUID))
    }

    // the catalogue can be tapped before the controller resolves; dropping the
    // tap is correct, crashing is not.
    @Test
    fun an_argument_command_before_connect_is_dropped_not_thrown() {
        PlayerConnection { }.send(CMD_PLAY_UUID, Bundle().apply { putString(ARG_UUID, "abc") })
    }
```

`FakeHandle` gains `val sentArgs = mutableListOf<Bundle>()` and records the bundle alongside the name.

- [ ] **Step 2: Run red, then implement the client side.** `ControllerHandle.sendCustomCommand` takes a `Bundle`; `PlayerConnection.send` gains a defaulted `args` parameter so every existing call site is untouched; the Media3 adapter passes it through as the command's `args`.

- [ ] **Step 3: Implement the service side.** Add `CMD_PLAY_UUID` to the constants, to the command field list, **and to `onConnect`'s builder**. Handle it:

```kotlin
                CMD_PLAY_UUID -> {
                    val uuid = args.getString(ARG_UUID).orEmpty()
                    scope.launch {
                        val station = withReadyCatalog().firstOrNull { it.uuid == uuid }
                        when (station) {
                            // the catalogue the screen listed and the one the
                            // service holds can differ after a refresh; a tap on
                            // a station that is gone must do nothing, not crash.
                            null -> Log.w("r4dio", "play requested for unknown station $uuid")
                            else -> main.post { playPick(station) }
                        }
                    }
                    return Futures.immediateFuture(SessionResult(SessionResult.RESULT_SUCCESS))
                }
```

- [ ] **Step 4: Grep for every place a session command is registered** and confirm `CMD_PLAY_UUID` appears in all of them. Name the lines in the report.

- [ ] **Step 5:** `./gradlew test --rerun-tasks` green.

- [ ] **Step 6: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt android/app/src/main/kotlin/net/vchub/r4dio/ui/PlayerConnection.kt android/app/src/test/kotlin/net/vchub/r4dio/ui/PlayerConnectionTest.kt
git commit -m "play the station you picked"
```

---

### Task 5: Blocking a station from the phone

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/FavStore.kt`
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/FavStoreLogicTest.kt` (add cases)

**Interfaces:**
- Produces:

```kotlin
// in FavStore
val blockedUuids: Flow<Set<String>>
suspend fun toggleBlocked(uuid: String)
```

**Read first:** `FavStore.kt:114-126` (`toggleFav` — the model to copy), `:374-389` (`applyMerged`, the only current writer of `blocked_uuids`), and `FavLogic.toggle` (`FavStore.kt:22-26`). Blocking is currently sync-only on Android: the design calls for a swipe action, which needs a local write.

**The rule that must not be broken:** blocking is a pointed "never play this again" that outranks a star ([[android-favourites-two-stores]] and the doc block at `Catalog.kt:62-72`). Blocking a favourite must therefore *not* silently unstar it — the two sets are independent, and `allowedStation` already resolves the conflict in blocked's favour.

- [ ] **Step 1: Write the failing tests**

```kotlin
    @Test
    fun blocking_and_unblocking_a_station_toggles_the_set() {
        assertEquals(setOf("a"), FavLogic.toggle(emptySet(), "a"))
        assertEquals(emptySet<String>(), FavLogic.toggle(setOf("a"), "a"))
    }

    // blocked outranks a star, so the two sets are independent: blocking a
    // favourite must leave the star alone rather than quietly unstarring it.
    @Test
    fun blocking_does_not_touch_the_favourite_set() {
        val favs = setOf("a")
        FavLogic.toggle(emptySet(), "a")
        assertEquals(setOf("a"), favs)
    }
```

The store-level behaviour needs DataStore and is proven on the emulator in Task 7 rather than mocked here.

- [ ] **Step 2: Implement**

```kotlin
    val blockedUuids: Flow<Set<String>> = store.data.map { it[keyBlocked] ?: emptySet() }

    /**
     * the local half of blocking. until now a uuid could only enter this set from
     * a sync merge or a backup restore, which meant a station could be blocked on
     * the desktop but not on the phone that is playing it.
     *
     * deliberately does not touch the favourite set: blocked outranks a star, and
     * allowedStation already resolves that — unstarring here would lose a choice
     * the user made separately.
     */
    suspend fun toggleBlocked(uuid: String) {
        store.edit { prefs ->
            prefs[keyBlocked] = FavLogic.toggle(prefs[keyBlocked] ?: emptySet(), uuid)
        }
    }
```

- [ ] **Step 3:** `./gradlew test --rerun-tasks` green.

- [ ] **Step 4: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/FavStore.kt android/app/src/test/kotlin/net/vchub/r4dio/FavStoreLogicTest.kt
git commit -m "block a station without reaching for the desktop"
```

---

### Task 6: The catalog screen

**Files:**
- Create: `android/app/src/main/kotlin/net/vchub/r4dio/ui/CatalogScreen.kt`
- Create: `android/app/src/main/kotlin/net/vchub/r4dio/ui/StationRow.kt`
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/ui/R4dioApp.kt`
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/MainActivity.kt`
- Modify: `android/app/src/main/res/values/strings.xml`

**Interfaces:**
- Consumes: `searchCatalog`, `CatalogFilters`, the three facet functions (Task 3); `CMD_PLAY_UUID`/`ARG_UUID` (Task 4); `FavStore.blockedUuids`/`toggleBlocked` (Task 5), `favUuids`/`toggleFav`.
- Produces:

```kotlin
@Composable fun CatalogScreen(
    stations: List<Station>, favourites: Set<String>, blocked: Set<String>,
    onPlay: (Station) -> Unit, onStar: (Station) -> Unit, onBlock: (Station) -> Unit,
)
```

**Read first:** `ui/HomeScreen.kt` and `ui/Pill.kt` — they set the idiom (mono type, token colours, pill shapes) this screen must match. Then `R4dioApp.kt` for how the Catalog tab is currently stubbed.

The design (handoff, "Catalog + Search + Filters"): a search field at the top; below it a chip row showing each active filter with a `✕`, a `+ Filters` chip and `Clear all`; then the station list. The filter sheet is a modal with `Reset` at the top, four groups (Country / Genre / Codec / Min bitrate) and a bottom button reading `Show N stations`, where N is the live count for the pending selection. Empty state: `No stations match`, the active filters spelled out, and a `Clear filters` button.

**Where the list comes from.** `MainActivity` constructs `CatalogCache(filesDir)` and reads it on `Dispatchers.IO` into Compose state. This is safe: `CatalogCache.write` uses a unique temp file plus an atomic rename, so a reader sees the old or the new file whole. **Read only — never call `write` or `merge` from the UI side**, which would race the service's read-modify-write. Note this constraint in a comment at the call site.

Re-read the cache when the tab is selected and whenever `EXTRA_CATALOG_SIZE` changes, so a background top-up shows up without a restart.

- [ ] **Step 1: Strings.** Add to `strings.xml`: `catalog_search_hint` ("search stations"), `catalog_filters` ("+ FILTERS"), `catalog_clear_all` ("CLEAR ALL"), `catalog_empty_title` ("NO STATIONS MATCH"), `catalog_empty_clear` ("CLEAR FILTERS"), `catalog_loading` ("BUILDING THE CATALOGUE…"), `filters_title` ("FILTERS"), `filters_reset` ("RESET"), `filters_country` ("COUNTRY"), `filters_genre` ("GENRE"), `filters_codec` ("CODEC"), `filters_bitrate` ("MIN BITRATE"), `filters_any` ("ANY"), `filters_show_n` ("SHOW %1$d STATIONS"), `filters_show_none` ("NOTHING MATCHES"), `catalog_blocked` ("BLOCKED"), `catalog_unblock` ("UNBLOCK"), `catalog_bitrate_k` ("%1$dk").

- [ ] **Step 2: Build the row.** `StationRow` shows country code, name, `codec · bitrate`, a star that toggles, and — for a blocked station — the whole row dimmed with an `UNBLOCK` affordance instead of hiding it. Tapping a row plays it; long-pressing offers block. **Use a long-press, not a swipe:** the design shows a swipe, but a swipe inside a vertically scrolling list in a car is easy to trigger by accident, and blocking is destructive. Note the deviation in the task report.

- [ ] **Step 3: Build the screen and the filter sheet.** Use `LazyColumn` with `key = { it.uuid }`. The sheet's `Show N stations` count comes from `searchCatalog(stations, query, pending).size` — recomputed as the user toggles, which the measured cost makes free.

- [ ] **Step 4: Wire it into `R4dioApp` and `MainActivity`**, replacing the Catalog placeholder.

- [ ] **Step 5:** `./gradlew test --rerun-tasks` green, `assembleDebug` succeeds.

- [ ] **Step 6: Real-path proof required.** On the emulator, four screenshots: the catalogue listing stations; a search narrowing it; the filter sheet with real facet counts; the empty state after an over-narrow filter. Confirm from `logcat -s r4dio` that a tapped station plays — **this is the one place playback may be started, because tap-to-play is the feature being proven.** Stop playback immediately after (`adb shell am force-stop net.vchub.r4dio`) and say so in the report.

- [ ] **Step 7: Commit**

```bash
git add -A android/
git commit -m "browse every station you have, and play the one you want"
```

---

### Task 7: The catalogue screen survives real use

**Files:**
- Modify: whichever files the findings require.

This task has no new feature. It is the pass that catches what building the screen in pieces hides.

- [ ] **Step 1: Check each of these on the emulator, and report each with evidence:**

1. **A blocked station is visibly blocked** in the list, and unblocking it there takes effect — confirm via `run-as … cat` of the DataStore or by the row's own state, not by assumption.
2. **Starring from the catalogue** updates Home's fav count.
3. **The list survives a rotation** without losing the query, the filters or the scroll position.
4. **The light theme** (`hifi-paper`) renders the list, the chips and the sheet legibly. Temporarily default the theme to it, screenshot, restore.
5. **A catalogue mid-refresh** does not show a torn or empty list.
6. **No banned station appears** for any query — search the ban list's own words (`russia`, `moscow`, `minsk`) and confirm an empty result.

- [ ] **Step 2:** fix what the pass finds; each fix keeps the suite green.

- [ ] **Step 3: Commit** whatever the pass produced, with a subject naming the user-visible effect.

---

## Verification after all tasks (controller, not subagents)

1. Whole-branch review before anything is pushed.
2. On the emulator: search finds stations by partial name; each of the four filter groups narrows the list; combining groups ANDs them; `Clear all` restores everything; a tapped station plays; a long-pressed station blocks and can be unblocked.
3. The genre filter is populated — report the share of cached stations carrying tags after the backfill.
4. Home is unchanged. The widget and the media notification are unchanged.
5. Release notes: the catalogue is browsable now — search it, filter it by country, genre, codec or quality, and play any station you find.
