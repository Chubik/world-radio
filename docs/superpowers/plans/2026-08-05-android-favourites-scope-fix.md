# Android Favourites Scope Fix — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make FAVOURITES ONLY scope actually play the user's favourites after a device sync, instead of silently playing a random non-favourite.

**Architecture:** Favourites are stored twice — `fav_uuids` (the ids, which sync updates) and `cached_favs` (the full station objects, which only the local star button ever wrote). Playback reads the second, so synced favourites were invisible to it. We add one reconciler that makes `cached_favs` match `fav_uuids` — resolving stations from the catalogue cache first, then backfilling whatever is missing from radio-browser's `byuuid` endpoint — and call it from both sync paths. Separately, the silent all-catalogue fallback in `pickForScope` is made explicit so a future breakage cannot masquerade as normal shuffling.

**Tech Stack:** Kotlin, OkHttp, kotlinx.serialization, DataStore Preferences, JUnit4 + MockWebServer.

## Global Constraints

- All code, comments, logs and commit messages in **English, lowercase** log/comment style.
- No AI/assistant mention anywhere — commits, code, comments.
- Commit to `dev` only. Commit subjects are the public changelog — write them for users.
- RU/BY station ban is hardcoded and stays private — do not mention it in user-facing copy.
- Existing test suite is 93 tests and must stay green.
- Android module lives at `radio/android`; run gradle from there.
- Do not touch the user's real data dir. Emulator testing only, and shut the emulator down when done.

---

## Background: the two defects

Verified against the user's real data before planning:

1. **`applyMerged` never writes `cached_favs`** (`FavStore.kt:183-198`). It writes `keyFavs`, `keyBlocked`, `keyExcludedCountries`. The only writer of `keyCached` is `toggleFav` (`FavStore.kt:108`) — i.e. the local star button. So favourites starred on the desktop never become playable on the phone.
2. **`pickForScope` silently falls back to the whole catalogue** (`Catalog.kt:65`) when the favourites list is empty. This masks defect 1: instead of "no favourites", the user gets a random station while the pill still reads FAVOURITES ONLY.

Evidence gathered: desktop has 7 favourites, phone pill correctly showed `· 7` (so uuid sync works); **0 of 7** are in the top-1000 `clickcount` catalogue Android fetches (so resolving from the catalogue alone is insufficient); **7 of 7** resolve via `byuuid` (so backfill works).

---

## File Structure

- `app/src/main/kotlin/net/vchub/r4dio/Catalog.kt` — add `fetchByUuids`; make base URL injectable; make the fallback in `pickForScope` explicit.
- `app/src/main/kotlin/net/vchub/r4dio/FavSync.kt` — **new**. Pure reconciliation logic: given wanted uuids, known stations and fetched stations, decide what `cached_favs` should become and which uuids still need fetching.
- `app/src/main/kotlin/net/vchub/r4dio/FavStore.kt` — add `setCachedFavs`, and a `currentCachedFavs` already exists.
- `app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt` — call the reconciler after `applyMerged` in `syncNow`.
- `app/src/main/kotlin/net/vchub/r4dio/SyncActivity.kt` — the reconcile is triggered via the service, no direct call (see Task 5).
- `app/src/test/kotlin/net/vchub/r4dio/FavSyncTest.kt` — **new**. Pure-function tests.
- `app/src/test/kotlin/net/vchub/r4dio/CatalogByUuidTest.kt` — **new**. MockWebServer tests for `fetchByUuids`.
- `app/src/test/kotlin/net/vchub/r4dio/ShuffleTest.kt` — extend for the explicit-fallback change.

---

### Task 1: Pure reconciliation logic

Decides what the cache should contain. No IO, no Android — so it is testable without a harness. This is the same pattern as `HomeState.kt` and `WidgetState.kt`.

**Files:**
- Create: `app/src/main/kotlin/net/vchub/r4dio/FavSync.kt`
- Test: `app/src/test/kotlin/net/vchub/r4dio/FavSyncTest.kt`

**Interfaces:**
- Consumes: `Station` (from `Station.kt`).
- Produces:
  - `FavSync.missingUuids(wanted: Set<String>, known: List<Station>): List<String>`
  - `FavSync.reconcile(wanted: Set<String>, known: List<Station>, fetched: List<Station>): List<Station>`

- [ ] **Step 1: Write the failing test**

```kotlin
package net.vchub.r4dio

import org.junit.Assert.assertEquals
import org.junit.Test

class FavSyncTest {
    private fun st(uuid: String, name: String = uuid) =
        Station(uuid, name, "http://$uuid", "DE", "MP3", 128)

    @Test
    fun missingUuids_returnsWantedNotPresentInKnown() {
        val known = listOf(st("a"), st("b"))
        assertEquals(listOf("c"), FavSync.missingUuids(setOf("a", "c"), known))
    }

    @Test
    fun missingUuids_emptyWhenAllKnown() {
        val known = listOf(st("a"), st("b"))
        assertEquals(emptyList<String>(), FavSync.missingUuids(setOf("a", "b"), known))
    }

    @Test
    fun reconcile_keepsOnlyWantedStations() {
        // "b" was un-starred on the other device — it must drop out of the cache
        val known = listOf(st("a"), st("b"))
        val out = FavSync.reconcile(setOf("a"), known, emptyList())
        assertEquals(listOf("a"), out.map { it.uuid })
    }

    @Test
    fun reconcile_addsFetchedStations() {
        val out = FavSync.reconcile(setOf("a", "c"), listOf(st("a")), listOf(st("c")))
        assertEquals(setOf("a", "c"), out.map { it.uuid }.toSet())
    }

    @Test
    fun reconcile_prefersKnownOverFetchedForSameUuid() {
        val known = listOf(st("a", name = "local name"))
        val fetched = listOf(st("a", name = "remote name"))
        val out = FavSync.reconcile(setOf("a"), known, fetched)
        assertEquals(listOf("local name"), out.map { it.name })
    }

    @Test
    fun reconcile_dropsUnresolvableUuids() {
        // "z" is in neither known nor fetched — a station deleted upstream.
        // it must not appear as a phantom entry.
        val out = FavSync.reconcile(setOf("a", "z"), listOf(st("a")), emptyList())
        assertEquals(listOf("a"), out.map { it.uuid })
    }

    @Test
    fun reconcile_dropsStationsWithoutUrl() {
        val fetched = listOf(Station("c", "no url", "", "DE", "MP3", 128))
        val out = FavSync.reconcile(setOf("c"), emptyList(), fetched)
        assertEquals(emptyList<String>(), out.map { it.uuid })
    }

    @Test
    fun reconcile_emptyWantedClearsEverything() {
        val out = FavSync.reconcile(emptySet(), listOf(st("a")), emptyList())
        assertEquals(emptyList<String>(), out.map { it.uuid })
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `./gradlew :app:testDebugUnitTest --tests '*FavSyncTest*'`
Expected: FAIL — unresolved reference `FavSync`.

- [ ] **Step 3: Write minimal implementation**

```kotlin
package net.vchub.r4dio

/**
 * favourites live in two places: the uuid set (which sync overwrites) and the
 * cached station objects (which playback picks from). sync only ever wrote the
 * first, so favourites starred on another device were never playable here.
 * these functions decide what the cache must become; the IO lives in the caller.
 */
object FavSync {
    fun missingUuids(wanted: Set<String>, known: List<Station>): List<String> {
        val have = known.map { it.uuid }.toSet()
        return wanted.filterNot { have.contains(it) }
    }

    fun reconcile(
        wanted: Set<String>,
        known: List<Station>,
        fetched: List<Station>,
    ): List<Station> {
        // known wins over fetched: a locally starred station already carries the
        // metadata the user saw, and re-fetching can return a renamed entry.
        val byUuid = LinkedHashMap<String, Station>()
        for (s in fetched) byUuid[s.uuid] = s
        for (s in known) byUuid[s.uuid] = s
        return wanted.mapNotNull { byUuid[it] }.filter { it.url.isNotBlank() }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `./gradlew :app:testDebugUnitTest --tests '*FavSyncTest*'`
Expected: PASS, 8 tests.

- [ ] **Step 5: Commit**

```bash
git add app/src/main/kotlin/net/vchub/r4dio/FavSync.kt app/src/test/kotlin/net/vchub/r4dio/FavSyncTest.kt
git commit -m "add favourites reconciliation logic"
```

---

### Task 2: Fetch stations by uuid

`byuuid` is the endpoint that resolves a favourite the top-1000 catalogue does not contain. Verified against production: 7/7 of the user's favourites resolve.

**Files:**
- Modify: `app/src/main/kotlin/net/vchub/r4dio/Catalog.kt:68-99`
- Test: `app/src/test/kotlin/net/vchub/r4dio/CatalogByUuidTest.kt`

**Interfaces:**
- Consumes: `ApiStation`, `Station`, `toStation()`, `allowedStation()` — all existing in `Station.kt` / `Catalog.kt`.
- Produces: `Catalog.fetchByUuids(uuids: List<String>): List<Station>`; `Catalog` constructor gains a second parameter `baseUrl: String = "https://all.api.radio-browser.info"`.

- [ ] **Step 1: Write the failing test**

```kotlin
package net.vchub.r4dio

import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class CatalogByUuidTest {
    private fun catalogFor(server: MockWebServer): Catalog =
        Catalog(baseUrl = server.url("/").toString().trimEnd('/'))

    @Test
    fun fetchByUuids_returnsStations() {
        val server = MockWebServer()
        server.enqueue(
            MockResponse().setBody(
                """[{"stationuuid":"a","name":"A FM","url_resolved":"http://a","countrycode":"DE","codec":"MP3","bitrate":128}]"""
            )
        )
        server.start()
        val out = catalogFor(server).fetchByUuids(listOf("a"))
        assertEquals(listOf("a"), out.map { it.uuid })
        assertEquals("A FM", out[0].name)
        server.shutdown()
    }

    @Test
    fun fetchByUuids_postsUuidsAsCsv() {
        val server = MockWebServer()
        server.enqueue(MockResponse().setBody("[]"))
        server.start()
        catalogFor(server).fetchByUuids(listOf("a", "b"))
        val req = server.takeRequest()
        assertEquals("/json/stations/byuuid", req.path)
        assertTrue(req.body.readUtf8().contains("uuids=a%2Cb"))
        server.shutdown()
    }

    @Test
    fun fetchByUuids_emptyInputMakesNoRequest() {
        val server = MockWebServer()
        server.start()
        assertEquals(emptyList<Station>(), catalogFor(server).fetchByUuids(emptyList()))
        assertEquals(0, server.requestCount)
        server.shutdown()
    }

    @Test
    fun fetchByUuids_serverErrorReturnsEmpty() {
        val server = MockWebServer()
        server.enqueue(MockResponse().setResponseCode(503))
        server.start()
        assertEquals(emptyList<Station>(), catalogFor(server).fetchByUuids(listOf("a")))
        server.shutdown()
    }

    @Test
    fun fetchByUuids_malformedBodyReturnsEmpty() {
        val server = MockWebServer()
        server.enqueue(MockResponse().setBody("not json"))
        server.start()
        assertEquals(emptyList<Station>(), catalogFor(server).fetchByUuids(listOf("a")))
        server.shutdown()
    }

    @Test
    fun fetchByUuids_appliesBannedStationFilter() {
        val server = MockWebServer()
        server.enqueue(
            MockResponse().setBody(
                """[{"stationuuid":"r","name":"Moscow FM","url_resolved":"http://r","countrycode":"RU","codec":"MP3","bitrate":128}]"""
            )
        )
        server.start()
        assertEquals(emptyList<Station>(), catalogFor(server).fetchByUuids(listOf("r")))
        server.shutdown()
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `./gradlew :app:testDebugUnitTest --tests '*CatalogByUuidTest*'`
Expected: FAIL — `Catalog` has no `baseUrl` parameter and no `fetchByUuids`.

- [ ] **Step 3: Write minimal implementation**

Change the class declaration and `fetchOnce` to use the injected base, then add the new method. Replace lines 68-99 of `Catalog.kt` with:

```kotlin
class Catalog(
    private val client: OkHttpClient = OkHttpClient(),
    private val baseUrl: String = "https://all.api.radio-browser.info",
) {
    private val json = Json { ignoreUnknownKeys = true }

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

    /**
     * resolves specific stations by id. the catalogue is the top-1000 by clickcount,
     * so a favourite starred on the desktop is usually absent from it — this is the
     * only way to get its stream url. user-excluded countries are deliberately NOT
     * applied: an explicit favourite outranks a country filter, same as pickFav.
     */
    fun fetchByUuids(uuids: List<String>): List<Station> {
        if (uuids.isEmpty()) return emptyList()
        val body = FormBody.Builder().add("uuids", uuids.joinToString(",")).build()
        val request = Request.Builder()
            .url("$baseUrl/json/stations/byuuid")
            .header("User-Agent", "world-radio-android/1.0")
            .post(body)
            .build()
        return runCatching {
            client.newCall(request).execute().use { resp ->
                val text = resp.body?.string().orEmpty()
                if (!resp.isSuccessful || text.isBlank()) return emptyList()
                json.decodeFromString<List<ApiStation>>(text)
                    .map { it.toStation() }
                    .filter { allowedStation(it) }
            }
        }.getOrDefault(emptyList())
    }

    private fun fetchOnce(limit: Int): List<Station> {
        val url =
            "$baseUrl/json/stations/search" +
                "?limit=$limit&hidebroken=true&order=clickcount&reverse=true"
        val request = Request.Builder()
            .url(url)
            .header("User-Agent", "world-radio-android/1.0")
            .build()
        client.newCall(request).execute().use { resp ->
            val body = resp.body?.string().orEmpty()
            if (!resp.isSuccessful || body.isBlank()) return emptyList()
            val api = json.decodeFromString<List<ApiStation>>(body)
            return api.map { it.toStation() }.filter { allowedStation(it) }
        }
    }
}
```

Add the import at the top of the file, next to the other okhttp imports:

```kotlin
import okhttp3.FormBody
```

- [ ] **Step 4: Run test to verify it passes**

Run: `./gradlew :app:testDebugUnitTest --tests '*CatalogByUuidTest*'`
Expected: PASS, 6 tests.

Then confirm nothing else broke:

Run: `./gradlew :app:testDebugUnitTest`
Expected: PASS, all tests.

- [ ] **Step 5: Commit**

```bash
git add app/src/main/kotlin/net/vchub/r4dio/Catalog.kt app/src/test/kotlin/net/vchub/r4dio/CatalogByUuidTest.kt
git commit -m "resolve stations by id so favourites outside the top list can play"
```

---

### Task 3: Persist reconciled favourites

`FavStore` needs a way to write the whole cached list at once. Today only `toggleFav` writes it, one station at a time.

**Files:**
- Modify: `app/src/main/kotlin/net/vchub/r4dio/FavStore.kt` (add method after `toggleFav`, around line 110)

**Interfaces:**
- Consumes: `FavStation.of()`, `keyCached` (both existing in this file).
- Produces: `FavStore.setCachedFavs(stations: List<Station>)` — suspend.

- [ ] **Step 1: Write the implementation**

There is no unit test for this step: `FavStore` wraps DataStore and the project has deliberately avoided a Robolectric harness (see the deferred note in the widget work). The behaviour is covered by Task 1's pure logic and the Task 6 device test. Add after `toggleFav`:

```kotlin
    /**
     * replaces the cached station objects wholesale. sync overwrites the uuid set,
     * so the cache has to be rebuilt from it rather than edited one star at a time.
     */
    suspend fun setCachedFavs(stations: List<Station>) {
        val encoded = json.encodeToString(
            ListSerializer(FavStation.serializer()),
            stations.map { FavStation.of(it) },
        )
        store.edit { it[keyCached] = encoded }
    }
```

- [ ] **Step 2: Verify it compiles**

Run: `./gradlew :app:compileDebugKotlin`
Expected: BUILD SUCCESSFUL.

- [ ] **Step 3: Run the full suite**

Run: `./gradlew :app:testDebugUnitTest`
Expected: PASS, all tests.

- [ ] **Step 4: Commit**

```bash
git add app/src/main/kotlin/net/vchub/r4dio/FavStore.kt
git commit -m "allow the favourites cache to be rebuilt in one write"
```

---

### Task 4: Make the scope fallback explicit

The silent fallback is what turned defect 1 into "plays a random station under a FAVOURITES ONLY pill". Keep the fallback (stopping playback dead is worse) but make it observable, so it can never again hide a broken favourites path.

**Files:**
- Modify: `app/src/main/kotlin/net/vchub/r4dio/Catalog.kt:54-66`
- Test: `app/src/test/kotlin/net/vchub/r4dio/ShuffleTest.kt`

**Interfaces:**
- Consumes: `Scope`, `Station`, `FavLogic.pickFav`, `pickRandom`.
- Produces: `ScopePick` data class with fields `station: Station?` and `usedFallback: Boolean`; `pickForScopeDetailed(scope, catalog, favs, userExcluded, rng): ScopePick`. `pickForScope` stays as-is in signature and behaviour so existing callers and tests are unaffected.

- [ ] **Step 1: Write the failing test**

Append to `ShuffleTest.kt`:

```kotlin
    @Test
    fun detailed_favsWithFavourites_doesNotUseFallback() {
        val favs = listOf(Station("f", "Fav", "http://f", "DE", "MP3", 128))
        val cat = listOf(Station("c", "Cat", "http://c", "DE", "MP3", 128))
        val out = pickForScopeDetailed(Scope.FAVS, cat, favs)
        assertEquals("f", out.station?.uuid)
        assertEquals(false, out.usedFallback)
    }

    @Test
    fun detailed_favsWithoutFavourites_flagsFallback() {
        val cat = listOf(Station("c", "Cat", "http://c", "DE", "MP3", 128))
        val out = pickForScopeDetailed(Scope.FAVS, cat, emptyList())
        assertEquals("c", out.station?.uuid)
        assertEquals(true, out.usedFallback)
    }

    @Test
    fun detailed_allScope_neverFlagsFallback() {
        val cat = listOf(Station("c", "Cat", "http://c", "DE", "MP3", 128))
        val out = pickForScopeDetailed(Scope.ALL, cat, emptyList())
        assertEquals(false, out.usedFallback)
    }

    @Test
    fun detailed_nothingPlayable_returnsNullWithoutFallback() {
        val out = pickForScopeDetailed(Scope.FAVS, emptyList(), emptyList())
        assertEquals(null, out.station)
        assertEquals(true, out.usedFallback)
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `./gradlew :app:testDebugUnitTest --tests '*ShuffleTest*'`
Expected: FAIL — unresolved reference `pickForScopeDetailed`.

- [ ] **Step 3: Write minimal implementation**

Replace lines 54-66 of `Catalog.kt`:

```kotlin
data class ScopePick(val station: Station?, val usedFallback: Boolean)

/**
 * in favs mode with no resolvable favourites we still play something from the
 * full catalogue — stopping dead is worse. but the caller needs to know it
 * happened: a silent fallback here is what made a broken favourites sync look
 * like normal shuffling under a FAVOURITES ONLY pill.
 */
fun pickForScopeDetailed(
    scope: Scope,
    catalog: List<Station>,
    favs: List<Station>,
    userExcluded: Set<String> = emptySet(),
    rng: Random = Random.Default,
): ScopePick =
    when (scope) {
        Scope.ALL -> ScopePick(pickRandom(catalog, userExcluded, rng), false)
        Scope.FAVS -> when (val fav = FavLogic.pickFav(favs, rng)) {
            null -> ScopePick(pickRandom(catalog, userExcluded, rng), true)
            else -> ScopePick(fav, false)
        }
    }

fun pickForScope(
    scope: Scope,
    catalog: List<Station>,
    favs: List<Station>,
    userExcluded: Set<String> = emptySet(),
    rng: Random = Random.Default,
): Station? = pickForScopeDetailed(scope, catalog, favs, userExcluded, rng).station
```

- [ ] **Step 4: Run test to verify it passes**

Run: `./gradlew :app:testDebugUnitTest --tests '*ShuffleTest*'`
Expected: PASS, 18 tests.

- [ ] **Step 5: Commit**

```bash
git add app/src/main/kotlin/net/vchub/r4dio/Catalog.kt app/src/test/kotlin/net/vchub/r4dio/ShuffleTest.kt
git commit -m "make the favourites fallback visible instead of silent"
```

---

### Task 5: Wire reconciliation into the sync path

This is the task that actually fixes the user-visible bug. Both sync paths funnel through `PlaybackService.syncNow()` — `SyncActivity.linkAndMerge` calls `triggerSync()` at line 58, which starts the service action. So one call site is enough, and adding a second one in the activity would create a second writer of the same state.

**Files:**
- Modify: `app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt:355-390`

**Interfaces:**
- Consumes: `FavSync.missingUuids`, `FavSync.reconcile` (Task 1); `Catalog.fetchByUuids` (Task 2); `FavStore.setCachedFavs` (Task 3); existing `catalog`, `favStore`, `stations`, `catalogCache` fields.
- Produces: `PlaybackService.reconcileFavCache()` — private suspend, no return.

- [ ] **Step 1: Add the reconcile function**

Insert directly above `syncNow()` at line 355:

```kotlin
    /**
     * sync replaces the favourite uuid set but knows nothing about station objects,
     * and playback picks from the cached objects — so without this a favourite
     * starred on another device can never play here. resolve from the catalogue
     * first (free), then fetch whatever is left (most favourites are outside the
     * top-1000 the catalogue holds).
     */
    private suspend fun reconcileFavCache() {
        val wanted = favStore.currentFavUuids()
        val known = favStore.currentCachedFavs() + stations
        val missing = FavSync.missingUuids(wanted, known)
        val fetched = when (missing.isEmpty()) {
            true -> emptyList()
            else -> withContext(Dispatchers.IO) {
                runCatching { catalog.fetchByUuids(missing) }.getOrDefault(emptyList())
            }
        }
        val next = FavSync.reconcile(wanted, known, fetched)
        favStore.setCachedFavs(next)
        Log.i("r4dio", "fav cache reconciled: ${next.size}/${wanted.size} resolved")
    }
```

- [ ] **Step 2: Call it in BOTH branches of syncNow**

`syncNow()` has two branches: `key == null` (no linked device) and `else` (merge with the server). The reconcile belongs in both.

Putting it only in the `else` branch would leave a cache that is already out of step un-repairable once the device is unlinked — the uuid set would keep being the source of truth for the pill while the stale objects kept being the source of truth for playback, with nothing left to reconcile them. It is cheap when there is nothing to do: `missingUuids` returns empty and no request is made.

In the `null` branch, add it as the first statement, before `refreshIfStale(...)`:

```kotlin
                null -> {
                    reconcileFavCache()
                    refreshIfStale(favStore.currentExcluded())
                    refreshCustomLayout()
                }
```

In the `else` branch, add it immediately after the `favStore.applyMerged(...)` call (which ends at line 378) and before the `refreshIfStale(...)` line:

```kotlin
                    reconcileFavCache()
```

- [ ] **Step 3: Verify the field names used above actually exist**

Run: `./gradlew :app:compileDebugKotlin`
Expected: BUILD SUCCESSFUL.

The field names were verified while writing this plan: the `Catalog` instance is `catalog` (`PlaybackService.kt:74`) and the cache is `catalogCache` (line 75). `stations` is the in-memory catalogue list. If any of these disagree with the code you see, grep for the real name rather than inventing one.

- [ ] **Step 4: Run the full suite**

Run: `./gradlew :app:testDebugUnitTest`
Expected: PASS, all tests.

- [ ] **Step 5: Commit**

```bash
git add app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt
git commit -m "play the favourites you starred on another device"
```

---

### Task 6: Verify on the emulator against the real path

The repo's dominant defect class is "correct code never reached on the path that matters", and a previous emulator test passed against a broken build because the fixture bypassed the real code path. So this test must drive the **real sync**, not a hand-written `cached_favs`.

**Files:** none modified — this is verification.

- [ ] **Step 1: Build and install**

```bash
cd /Users/vchub/dev/projects/world-radio/radio/android
./gradlew :app:assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

- [ ] **Step 2: Confirm the pre-fix behaviour is gone, using a real sync key**

Start the app, open SYNC, and link using a key that has favourites which are **not** in the top-1000 catalogue. Do not hand-write DataStore contents.

```bash
adb logcat -c
adb shell am start -n net.vchub.r4dio/.MainActivity
# link the device through the UI, then:
adb logcat -d | grep "fav cache reconciled"
```

Expected: a line like `fav cache reconciled: 7/7 resolved`. A `0/7` means the fetch failed — investigate before proceeding, do not accept it.

- [ ] **Step 3: Verify the scope actually plays a favourite**

Switch scope to FAVOURITES ONLY and shuffle several times.

Expected on screen: the station name changes between the synced favourites only, and the metadata line reads `★ saved` — **not** `☆ not saved`. The pill count and the played stations must agree.

- [ ] **Step 4: Verify the fallback is no longer silent**

With scope FAVOURITES ONLY and zero favourites (unlink or clear them), shuffle.

```bash
adb logcat -d | grep -i "fallback\|nothing to play"
```

Expected: playback still happens (by design), and the fallback is visible in the log rather than indistinguishable from a normal pick.

- [ ] **Step 5: Shut the emulator down**

```bash
adb emu kill
```

- [ ] **Step 6: Commit nothing, report findings**

If any step failed, return to systematic-debugging rather than patching forward.

---

## Self-Review

**Spec coverage:**
- Defect 1 (sync never updates the station cache) → Tasks 1, 2, 3, 5.
- Defect 2 (silent fallback masking it) → Task 4.
- Evidence that catalogue-only resolution is insufficient (0/7 in top-1000) → Task 2 exists precisely because of this.
- Verification against the real path → Task 6.

**Type consistency check:**
- `FavSync.missingUuids(Set<String>, List<Station>): List<String>` — defined Task 1, used Task 5. ✓
- `FavSync.reconcile(Set<String>, List<Station>, List<Station>): List<Station>` — defined Task 1, used Task 5. ✓
- `Catalog.fetchByUuids(List<String>): List<Station>` — defined Task 2, used Task 5. ✓
- `FavStore.setCachedFavs(List<Station>)` — defined Task 3, used Task 5. ✓
- `ScopePick(station, usedFallback)` / `pickForScopeDetailed` — defined Task 4, not consumed by Task 5 (the service keeps calling `pickForScope`; the detailed variant exists for the log/test surface). ✓
- `Catalog(client, baseUrl)` — the existing `Catalog()` call in `PlaybackService` still compiles because both parameters have defaults. ✓

**Known limitation, deliberately accepted:** a favourite deleted upstream from radio-browser resolves nowhere and silently drops out of the cache (Task 1, `reconcile_dropsUnresolvableUuids`). Carrying station data inside the sync payload would fix that, but it is a protocol change across three repos — recorded as the follow-up option "C" from the investigation, not built here.
