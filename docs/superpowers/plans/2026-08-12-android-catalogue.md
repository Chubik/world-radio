# Android Catalogue Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A country filter on Android has something to filter — today a UA filter leaves 7 stations out of the 351 that exist, because the phone only ever holds the world top-1000.

**Architecture:** Two independent mechanisms feeding one cache. Selecting a filter pulls those countries in full, immediately (`bycountrycodeexact`, one request per country). Separately, a background top-up pages past the first 1000 by descending clickcount, but only on unmetered network while charging. Both merge into the existing `CatalogCache`, so the pick path filters newly-arrived stations through the shared rule with no extra wiring.

**Tech Stack:** Kotlin, OkHttp, kotlinx.serialization, DataStore. **No new dependencies** — WorkManager is deliberately not added (see Global Constraints).

**Spec:** `docs/superpowers/specs/2026-08-12-android-catalogue-design.md`

## Global Constraints

- Repo `radio`, branch `dev`, everything under `android/`. Do not push — the controller pushes after review.
- **Measured baseline to beat, from the live API on 2026-08-12:** the world top-1000 holds **7** UA stations; **351** exist. The top-1000 skews DE 111, FR 99, US 90, IN 66, GB 59, IT 53. Any claim of improvement is measured against these numbers, on the emulator, against the real API.
- **Both mechanisms are required, and the reason is the user's own correction:** a filter-triggered fetch alone helps nobody, because at the start no filter is set. Do not quietly drop the background top-up as "phase two".
- **"Без навантаження" means waiting for a moment that costs nothing** — unmetered network AND charging — not a smaller burst at an arbitrary time.
- **No new Gradle dependency.** WorkManager is the textbook answer and is the wrong trade here: it is a large addition for one periodic job in a service that is already running whenever playback is. Use the existing service and its coroutine scope. If you conclude the constraint is wrong, stop and escalate rather than adding the dependency.
- **The RU/BY ban applies to every ingest path**, including both new ones. It is enforced today in `allowedStation`/`Catalog.kt`; new fetches must pass through the same filter, not around it.
- Fetch paths are never filtered by the *country filter* — that applies at pick time only. A fetch pulls what it pulls; the pick decides what plays.
- Verified on the emulator, not by reading code. **Never launch playback to prove something** — the emulator plays audio out loud on the user's machine. Read the cache file, the rendered screen, and logcat.
- All code, comments, strings and commit messages English, lowercase-first. Comments only where they state a constraint. Commit subjects are the published changelog.
- Gate before every commit: `cd android && ./gradlew test` green, real output pasted in the report.

---

### Task 1: Fetch a country in full

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/Catalog.kt` (add `fetchCountry`)
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/CatalogTest.kt` (or the existing catalogue test file — follow what is there)

**Interfaces:**
- Consumes: the existing `fetchOnce`/`allowedStation` shape in `Catalog.kt:161-175`, `ApiStation.toStation()`.
- Produces (Tasks 2 and 3 call this):

```kotlin
fun fetchCountry(
    code: String,
    blocked: Set<String> = emptySet(),
): List<Station>
```

**Read first:** `Catalog.kt:120-175`. `fetchOnce` is the model to follow — same client, same user-agent, same `runCatching` shape, same `allowedStation` filter on the way out. The endpoint is
`$baseUrl/json/stations/bycountrycodeexact/UA?hidebroken=true` (verified live: returns the full list for a country).

- [ ] **Step 1: Write the failing tests**

Follow the existing test file's MockWebServer setup verbatim:

```kotlin
    @Test
    fun a_country_fetch_asks_for_that_country_only() {
        server.enqueue(MockResponse().setBody("""[{"stationuuid":"a","name":"UA one","url_resolved":"http://x","countrycode":"UA"}]"""))
        val got = catalog.fetchCountry("UA")
        val asked = server.takeRequest().path.orEmpty()
        assertTrue("wrong endpoint: $asked", asked.contains("bycountrycodeexact/UA"))
        assertTrue("broken stations must not be asked for: $asked", asked.contains("hidebroken=true"))
        assertEquals(1, got.size)
    }

    // the ban is a product requirement on every ingest path, not a display rule.
    @Test
    fun a_country_fetch_still_drops_banned_countries() {
        server.enqueue(MockResponse().setBody("""[{"stationuuid":"r","name":"ru one","url_resolved":"http://x","countrycode":"RU"}]"""))
        assertTrue(catalog.fetchCountry("RU").isEmpty())
    }

    @Test
    fun a_blocked_station_never_arrives_from_a_country_fetch() {
        server.enqueue(MockResponse().setBody("""[{"stationuuid":"a","name":"UA one","url_resolved":"http://x","countrycode":"UA"}]"""))
        assertTrue(catalog.fetchCountry("UA", blocked = setOf("a")).isEmpty())
    }

    // a network failure must leave the catalogue alone rather than emptying it.
    @Test
    fun a_failed_country_fetch_returns_nothing_rather_than_throwing() {
        server.enqueue(MockResponse().setResponseCode(500))
        assertTrue(catalog.fetchCountry("UA").isEmpty())
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd android && ./gradlew test --tests '*CatalogTest*'`
Expected: compile failure — `fetchCountry` does not exist.

- [ ] **Step 3: Implement**

```kotlin
    /**
     * every station in one country. the top-1000 by clickcount holds only a
     * handful of any country outside the big few — 7 of ukraine's 351 — so a
     * filter set to one is filtering almost nothing until this runs.
     */
    fun fetchCountry(code: String, blocked: Set<String> = emptySet()): List<Station> {
        val url = "$baseUrl/json/stations/bycountrycodeexact/$code?hidebroken=true"
        val request = Request.Builder()
            .url(url)
            .header("User-Agent", "world-radio-android/1.0")
            .build()
        return runCatching {
            client.newCall(request).execute().use { resp ->
                val body = resp.body?.string().orEmpty()
                if (!resp.isSuccessful || body.isBlank()) return emptyList()
                json.decodeFromString<List<ApiStation>>(body)
                    .map { it.toStation() }
                    .filter { allowedStation(it, blocked = blocked) }
            }
        }.getOrDefault(emptyList())
    }
```

- [ ] **Step 4: Run to verify they pass**, then `./gradlew test` green.

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/Catalog.kt android/app/src/test
git commit -m "fetch every station in a country, not just the popular few"
```

---

### Task 2: A filter pulls its countries in

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/CatalogCache.kt` (add a merge)
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt` (call it when the filter changes)
- Test: the cache's test file + a service-level test if one exists

**Interfaces:**
- Consumes: `Catalog.fetchCountry` (Task 1); `CatalogCache.read()`/`write()` as they exist today.
- Produces (Task 3 reuses this):

```kotlin
// in CatalogCache
fun merge(incoming: List<Station>): Int   // returns how many were genuinely new
```

**Read first:** `CatalogCache.kt` in full — it is small, uses a plain file and an instance lock, and both must be respected. Then `PlaybackService.kt:260-290` (where the filter arrives from a sync) and `:329-355` (`refreshIfStale`, the existing background-fetch shape to copy for threading).

- [ ] **Step 1: Write the failing tests**

```kotlin
    @Test
    fun merging_adds_only_the_new_ones() {
        cache.write(listOf(st("a", "UA")))
        val added = cache.merge(listOf(st("a", "UA"), st("b", "UA")))
        assertEquals(1, added)
        assertEquals(setOf("a", "b"), cache.read().map { it.uuid }.toSet())
    }

    // a merge must never shrink the catalogue: the pool is what shuffle draws
    // from, and losing stations mid-session is worse than gaining none.
    @Test
    fun merging_nothing_keeps_what_is_there() {
        cache.write(listOf(st("a", "UA")))
        assertEquals(0, cache.merge(emptyList()))
        assertEquals(1, cache.read().size)
    }

    @Test
    fun merging_does_not_duplicate_a_station_already_held() {
        cache.write(listOf(st("a", "UA")))
        cache.merge(listOf(st("a", "UA")))
        assertEquals(1, cache.read().size)
    }
```

- [ ] **Step 2: Run red**, implement `merge` (union by uuid, keep the existing entry on collision, write once), run green.

- [ ] **Step 3: Wire the real path**

In `PlaybackService`, when an applied profile brings a non-empty country filter, fetch each country on a background thread and merge. Guard it the way `refreshIfStale` guards itself (`refreshInFlight`, `PlaybackService.kt:84,337`) so a burst of sync events costs one fetch, not one each. After a merge that added anything, refresh the screen the same way `refreshIfStale` does — otherwise the count the user sees is stale.

**Do not** fetch when the filter is empty, and **do not** re-fetch a country already pulled this session.

- [ ] **Step 4: Grep for every place the filter can change** and confirm each reaches the fetch — a sync response, and any local path. This repo's known defect class is correct code the real path never reaches; name in the report which line does it.

- [ ] **Step 5:** `./gradlew test` green, commit `a country filter now pulls that country's stations in`.

**Real-path proof required:** on the emulator, set a UA filter on the account, and report how many UA stations the cache holds before and after. The baseline is 7. Read `catalog.json`, do not launch playback.

---

### Task 3: The background top-up

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/Catalog.kt` (paged fetch)
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt` (when it runs)
- Create: a small conditions helper next to the service, so the rule is testable without Android
- Test: unit tests for the conditions and the paging

**Interfaces:**
- Consumes: `CatalogCache.merge` (Task 2).
- Produces: nothing downstream.

```kotlin
fun fetchPage(offset: Int, limit: Int, blocked: Set<String> = emptySet()): List<Station>

// pure, so it can be tested without a device
fun topUpAllowed(unmetered: Boolean, charging: Boolean, held: Int, ceiling: Int): Boolean
```

Paging is verified live: `?limit=N&offset=M&hidebroken=true&order=clickcount&reverse=true` returns
the next page.

- [ ] **Step 1: Write the failing tests**

```kotlin
    // "без навантаження" means waiting for a moment that costs the user nothing,
    // not taking smaller bites at an arbitrary one.
    @Test
    fun the_top_up_waits_for_wifi_and_charging() {
        assertTrue(topUpAllowed(unmetered = true, charging = true, held = 1000, ceiling = 20_000))
        assertFalse(topUpAllowed(unmetered = false, charging = true, held = 1000, ceiling = 20_000))
        assertFalse(topUpAllowed(unmetered = true, charging = false, held = 1000, ceiling = 20_000))
    }

    @Test
    fun the_top_up_stops_at_the_ceiling() {
        assertFalse(topUpAllowed(unmetered = true, charging = true, held = 20_000, ceiling = 20_000))
    }

    @Test
    fun a_page_asks_for_the_next_slice() {
        server.enqueue(MockResponse().setBody("[]"))
        catalog.fetchPage(offset = 1000, limit = 200)
        val asked = server.takeRequest().path.orEmpty()
        assertTrue(asked, asked.contains("offset=1000"))
        assertTrue(asked, asked.contains("limit=200"))
        assertTrue(asked, asked.contains("order=clickcount"))
    }
```

- [ ] **Step 2:** run red, implement, run green.

- [ ] **Step 3: Wire it**, one page per opportunity, on the service's existing scope. Stop when a page adds nothing new. Read the two conditions from `ConnectivityManager` (`NET_CAPABILITY_NOT_METERED`) and `BatteryManager` — read them at the moment of the attempt, never cached.

- [ ] **Step 4:** `./gradlew test` green, commit `the station list keeps growing quietly in the background`.

**Real-path proof required:** on the emulator with unmetered network and charging simulated (`adb shell dumpsys battery set ac 1`), report the catalogue count growing past 1000 across attempts; and report that it does **not** grow with charging off.

---

### Task 4: Show what is held

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/HomeState.kt` (the label)
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/MainActivity.kt` + `res/layout/activity_main.xml` (a pill beside the existing ones)
- Test: `HomeState`'s test file

**Interfaces:**
- Consumes: the catalogue count the service already publishes in its session extras (`PlaybackService` — grep `catalogLoaded`/`playableCount` for how counts reach the screen today).

The catalogue is no longer either "the top-1000" or "everything" — it is somewhere in between, and the user asked to be able to see where.

- [ ] **Step 1: Write the failing tests** — exact strings:

```kotlin
    @Test
    fun the_pill_names_how_many_stations_are_held() {
        assertEquals("1 240 STATIONS", catalogueLabel(1240, growing = false))
    }

    @Test
    fun a_growing_catalogue_says_so() {
        assertEquals("1 240 STATIONS +", catalogueLabel(1240, growing = true))
    }

    // before the first fetch resolves there is no number worth showing.
    @Test
    fun an_unknown_count_shows_nothing() {
        assertEquals("", catalogueLabel(0, growing = false))
    }
```

- [ ] **Step 2:** red, implement, green, wire the pill following `overlay_pill`'s shape in `activity_main.xml` and `MainActivity.applyOverlayPill`.

- [ ] **Step 3:** `./gradlew test` green, commit `see how many stations you have`.

**Real-path proof required:** an emulator screenshot with the pill showing a real count.

---

## Verification after all tasks (controller, not subagents)

1. Whole-branch review before anything is pushed.
2. On the emulator, against the real API: with a UA filter, the catalogue holds **substantially more than 7** UA stations and ten shuffles land on more than a handful of distinct ones; with no filter, the catalogue grows past 1000 only under unmetered+charging.
3. A station arriving mid-session is subject to the active filter immediately.
4. Release notes: the catalogue now grows on its own on wi-fi while charging, and setting a country filter pulls that country in full.
