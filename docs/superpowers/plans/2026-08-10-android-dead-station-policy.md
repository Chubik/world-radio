# Android Dead-Station Policy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Android remembers dead stations the way the CLI does — hide on the first genuine stream failure, un-hide on success, blame budget of 5 — so shuffle stops returning to corpses.

**Architecture:** A new pure-logic file (`StationHealth.kt`) holds the Media3 error classification and the blame-budget tracker; `FavStore` gets a locally-persisted `hidden_dead` DataStore set following the exact pattern of `blocked`; `PlaybackService` wires the two into its existing error/success callbacks and unions `hiddenDead` with `blocked` at the three pick sites. Fetching, caching, and fav resolution are untouched.

**Tech Stack:** Kotlin, Media3 `PlaybackException`, DataStore preferences, JUnit4 (plain JVM tests).

**Spec:** `docs/superpowers/specs/2026-08-10-android-dead-station-policy.md`

## Global Constraints

- No code comments unless they state a constraint the code cannot show; when present, lowercase first letter (project rule).
- All strings/logs in English, lowercase-first.
- No `else if`; the codebase prefers `when`.
- Commit subjects are the public changelog — write them for users.
- `hidden_dead` is local-only: it must NOT appear in `SyncData`, `SyncMerge`, `applyMerged`, `Backup`, or `restore`.
- Run tests from `android/`: `./gradlew test`.

---

### Task 1: Classification and tracker (`StationHealth.kt`)

**Files:**
- Create: `android/app/src/main/kotlin/net/vchub/r4dio/StationHealth.kt`
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/StationHealthTest.kt`

**Interfaces:**
- Consumes: nothing — pure Kotlin, no Android imports (the Media3 codes are written as literal ints so the test module needs no Media3 dependency).
- Produces: `fun shouldBlame(errorCode: Int): Boolean` and `class HealthTracker(private val budget: Int = 5)` with `fun onError(blame: Boolean): Boolean` (true = hide) and `fun onSuccess()`. Task 3 calls all three.

- [ ] **Step 1: Write the failing tests**

Create `android/app/src/test/kotlin/net/vchub/r4dio/StationHealthTest.kt`:

```kotlin
package net.vchub.r4dio

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class StationHealthTest {
    @Test
    fun blames_the_station_for_stream_side_errors() {
        // invalid content type, bad http status, file not found
        assertTrue(shouldBlame(2003))
        assertTrue(shouldBlame(2004))
        assertTrue(shouldBlame(2005))
        // parsing errors (3xxx)
        assertTrue(shouldBlame(3001))
        assertTrue(shouldBlame(3004))
        // decoding errors (4xxx)
        assertTrue(shouldBlame(4001))
        assertTrue(shouldBlame(4003))
    }

    @Test
    fun never_blames_the_device_network() {
        assertFalse(shouldBlame(2000)) // io unspecified
        assertFalse(shouldBlame(2001)) // network connection failed
        assertFalse(shouldBlame(2002)) // network connection timeout
    }

    @Test
    fun unknown_codes_do_not_blame() {
        assertFalse(shouldBlame(0))
        assertFalse(shouldBlame(1000)) // generic unspecified
        assertFalse(shouldBlame(2006)) // io no permission — device side
        assertFalse(shouldBlame(5001))
        assertFalse(shouldBlame(-1))
    }

    @Test
    fun genuine_failure_hides_on_first_strike() {
        val t = HealthTracker()
        assertTrue(t.onError(blame = true))
    }

    @Test
    fun network_failure_never_hides() {
        val t = HealthTracker()
        assertFalse(t.onError(blame = false))
    }

    @Test
    fun budget_stops_hiding_after_five_strikes_without_success() {
        val t = HealthTracker(budget = 5)
        repeat(5) { assertTrue(t.onError(blame = true)) }
        assertFalse(t.onError(blame = true))
        assertFalse(t.onError(blame = true))
    }

    @Test
    fun success_resets_the_budget() {
        val t = HealthTracker(budget = 5)
        repeat(5) { t.onError(blame = true) }
        assertFalse(t.onError(blame = true))
        t.onSuccess()
        assertTrue(t.onError(blame = true))
    }

    @Test
    fun network_errors_do_not_consume_the_budget() {
        val t = HealthTracker(budget = 2)
        repeat(10) { assertFalse(t.onError(blame = false)) }
        assertTrue(t.onError(blame = true))
        assertTrue(t.onError(blame = true))
        assertFalse(t.onError(blame = true))
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd android && ./gradlew test --tests "net.vchub.r4dio.StationHealthTest" 2>&1 | tail -20`
Expected: compilation FAILS — `shouldBlame` and `HealthTracker` do not exist.

- [ ] **Step 3: Implement**

Create `android/app/src/main/kotlin/net/vchub/r4dio/StationHealth.kt`:

```kotlin
package net.vchub.r4dio

// media3 PlaybackException codes, written as literals so this file stays pure
// jvm and the classification is testable without the android framework.
// station-side: 2003 invalid content type, 2004 bad http status, 2005 file not
// found, 3xxx parsing, 4xxx decoding. everything else — including 2001/2002
// network failures and any unknown code — is treated as the device's problem:
// wrongly hiding a live station is worse than meeting a dead one again.
fun shouldBlame(errorCode: Int): Boolean =
    when {
        errorCode in setOf(2003, 2004, 2005) -> true
        errorCode in 3000..3999 -> true
        errorCode in 4000..4999 -> true
        else -> false
    }

class HealthTracker(private val budget: Int = 5) {
    private var blamesSinceSuccess = 0

    fun onError(blame: Boolean): Boolean {
        if (!blame) return false
        if (blamesSinceSuccess >= budget) return false
        blamesSinceSuccess += 1
        return true
    }

    fun onSuccess() {
        blamesSinceSuccess = 0
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd android && ./gradlew test --tests "net.vchub.r4dio.StationHealthTest" 2>&1 | tail -5`
Expected: BUILD SUCCESSFUL, all tests pass.

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/StationHealth.kt \
        android/app/src/test/kotlin/net/vchub/r4dio/StationHealthTest.kt
git commit -m "android: tell a dead stream apart from a dead network"
```

---

### Task 2: Persistence in `FavStore`

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/FavStore.kt` (add one key next to `keyBlocked` at :81, and four methods next to `currentBlocked()` at :174)
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/StationHealthTest.kt` (prune logic only — the DataStore accessors are thin wrappers, same as `blocked`'s, which have no store-level tests either)

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces (Task 3 calls these):

```kotlin
suspend fun currentHiddenDead(): Set<String>
suspend fun hideDead(uuid: String)
suspend fun unhideDead(uuid: String)
suspend fun pruneHiddenDead(keep: Set<String>)
```

Plus a pure helper in `StationHealth.kt` that carries the prune rule:

```kotlin
fun pruneHidden(hidden: Set<String>, keep: Set<String>): Set<String>
```

- [ ] **Step 1: Write the failing prune test**

Add to `StationHealthTest.kt`:

```kotlin
    @Test
    fun prune_keeps_only_uuids_still_reachable() {
        val hidden = setOf("dead1", "gone2", "fav3")
        val keep = setOf("dead1", "fav3", "alive4")
        assertEquals(setOf("dead1", "fav3"), pruneHidden(hidden, keep))
    }
```

(and add `import org.junit.Assert.assertEquals` to the imports.)

- [ ] **Step 2: Run it to verify it fails**

Run: `cd android && ./gradlew test --tests "net.vchub.r4dio.StationHealthTest" 2>&1 | tail -20`
Expected: compilation FAILS — `pruneHidden` does not exist.

- [ ] **Step 3: Implement the helper and the store methods**

Add to `StationHealth.kt`:

```kotlin
fun pruneHidden(hidden: Set<String>, keep: Set<String>): Set<String> = hidden intersect keep
```

In `FavStore.kt`, add the key after `keyBlocked` (line 81):

```kotlin
    private val keyHiddenDead = stringSetPreferencesKey("hidden_dead")
```

Add the methods after `currentBlocked()` (line 174):

```kotlin
    // stream health is local by design: a station dead on this device's network
    // may be fine elsewhere, so hidden_dead is never synced and never backed up.
    suspend fun currentHiddenDead(): Set<String> = store.data.first()[keyHiddenDead] ?: emptySet()

    suspend fun hideDead(uuid: String) {
        store.edit { prefs ->
            prefs[keyHiddenDead] = (prefs[keyHiddenDead] ?: emptySet()) + uuid
        }
    }

    suspend fun unhideDead(uuid: String) {
        store.edit { prefs ->
            prefs[keyHiddenDead] = (prefs[keyHiddenDead] ?: emptySet()) - uuid
        }
    }

    suspend fun pruneHiddenDead(keep: Set<String>) {
        store.edit { prefs ->
            prefs[keyHiddenDead] = pruneHidden(prefs[keyHiddenDead] ?: emptySet(), keep)
        }
    }
```

- [ ] **Step 4: Run the test suite**

Run: `cd android && ./gradlew test 2>&1 | tail -5`
Expected: BUILD SUCCESSFUL, everything green.

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/FavStore.kt \
        android/app/src/main/kotlin/net/vchub/r4dio/StationHealth.kt \
        android/app/src/test/kotlin/net/vchub/r4dio/StationHealthTest.kt
git commit -m "android: remember hidden dead stations across restarts"
```

---

### Task 3: Wire the policy into `PlaybackService`

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt` — the listener (:183-191), `loadStations` (:233-266), `startFrom` (:268-278), `fetchAndStore` (:280-296), `shuffle` (:488-518)
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/StationHealthTest.kt` (the union-excludes-hidden pick check)

**Interfaces:**
- Consumes: `shouldBlame`, `HealthTracker` (Task 1); `currentHiddenDead`, `hideDead`, `unhideDead`, `pruneHiddenDead` (Task 2); existing `pickForScopeDetailed`, `pickRandom`, `favStore`, `current`.
- Produces: nothing new for later tasks (this is the last task).

- [ ] **Step 1: Write the failing pick-union test**

Add to `StationHealthTest.kt`:

```kotlin
    @Test
    fun hidden_uuids_unioned_into_blocked_never_get_picked() {
        val cat = listOf(
            Station("dead1", "Dead FM", "http://x/1", "UA", "MP3", 128),
            Station("alive2", "Alive FM", "http://x/2", "UA", "MP3", 128),
        )
        val blocked = emptySet<String>()
        val hidden = setOf("dead1")
        repeat(20) {
            val pick = pickForScope(Scope.ALL, cat, emptyList(), emptySet(), blocked + hidden)
            assertEquals("alive2", pick?.uuid)
        }
    }
```

- [ ] **Step 2: Run it to verify it fails or passes for the right reason**

Run: `cd android && ./gradlew test --tests "net.vchub.r4dio.StationHealthTest" 2>&1 | tail -5`
Expected: PASS — this documents the union contract the service relies on (the pick machinery
already honours `blocked`; the service change is passing the union in). It guards the contract
against a future signature change.

- [ ] **Step 3: Wire the service**

In `PlaybackService.kt`:

a. Add a field near `current`:

```kotlin
    private val health = HealthTracker()
```

b. Replace the listener body (lines 183-191):

```kotlin
            override fun onIsPlayingChanged(isPlaying: Boolean) {
                if (isPlaying) {
                    health.onSuccess()
                    current?.let { st -> scope.launch { favStore.unhideDead(st.uuid) } }
                }
                refreshWidget(current, isPlaying)
            }

            override fun onPlayerError(error: androidx.media3.common.PlaybackException) {
                val blame = shouldBlame(error.errorCode)
                Log.w("r4dio", "playback error: ${error.errorCodeName}, blame=$blame, skipping station")
                if (health.onError(blame)) {
                    current?.let { st -> scope.launch { favStore.hideDead(st.uuid) } }
                }
                shuffle()
            }
```

c. In `loadStations` (line 239), read the hidden set next to `blocked` and pass the union to
`startFrom` and `fetchAndStore`'s pick path — replace:

```kotlin
            val blocked = runBlocking { favStore.currentBlocked() }
```

with:

```kotlin
            val blocked = runBlocking { favStore.currentBlocked() }
            val hidden = runBlocking { favStore.currentHiddenDead() }
```

and change both `startFrom(fetched, userExcluded, blocked)` and
`startFrom(cached, userExcluded, blocked)` to
`startFrom(fetched, userExcluded, blocked + hidden)` /
`startFrom(cached, userExcluded, blocked + hidden)`.
Leave `fetchAndStore(userExcluded, blocked)` and `refreshIfStale(userExcluded, blocked)` as they
are — health must not filter the fetch or the cache.

d. In `shuffle()` (line 499), read the hidden set alongside the other four reads:

```kotlin
            val hidden = withContext(Dispatchers.IO) {
                runCatching { withTimeout(3000) { favStore.currentHiddenDead() } }.getOrDefault(emptySet<String>())
            }
```

and change the pick call to:

```kotlin
            val picked = pickForScopeDetailed(sc, cat, favs, userExcluded, blocked + hidden)
```

e. In `fetchAndStore`, prune after the successful disk write (right after the
`favStore.setCatalogSyncedAt(nowSecs())` line):

```kotlin
        runBlocking {
            favStore.pruneHiddenDead(
                fetched.map { it.uuid }.toSet() + favStore.currentFavUuids(),
            )
        }
```

- [ ] **Step 4: Full test suite + build**

Run: `cd android && ./gradlew test assembleDebug 2>&1 | tail -5`
Expected: BUILD SUCCESSFUL.

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt \
        android/app/src/test/kotlin/net/vchub/r4dio/StationHealthTest.kt
git commit -m "android: stop returning to dead stations in shuffle"
```

---

## Verification after all tasks

- `cd android && ./gradlew test assembleDebug` green.
- Emulator (never against real user data): install the debug apk, play; force an error by
  playing a station whose URL returns 404 (or block its host in the emulator); logcat shows
  `blame=true`; subsequent shuffles never return that uuid; restart the app — still hidden.
  With airplane-mode-style network loss, logcat shows `blame=false` and nothing is hidden.
- Push `dev` when done; rides with the next release.
