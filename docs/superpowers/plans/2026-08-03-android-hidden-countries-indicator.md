# Android Hidden-Countries Indicator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show the user, on the home screen, that their own country filters are active — and tell them plainly when those filters have hidden everything, instead of leaving a silently dead app.

**Architecture:** Two pieces of state flow from `PlaybackService` to `MainActivity` over the existing MediaSession-extras channel (the established source-of-truth pattern): the number of countries the user has hidden, and the number of stations that survive filtering. The screen renders the first as a new pill in the context row (visible only when non-zero, hidden in FAVS scope where the filter does not apply), and the second as a warn state on the hero, reusing the exact recipe that already backs `NO FAVOURITES YET`.

**Tech Stack:** Kotlin, Android Views (XML layouts, no Compose), androidx.media3 MediaSession, DataStore Preferences, JUnit 4 (JVM unit tests under `android/app/src/test/`).

## Global Constraints

- **The RU/BY ban must stay invisible.** It lives in two private vals in `Catalog.kt` (`EXCLUDED_COUNTRYCODES`, `EXCLUDED_NAME_SUBSTRINGS`) and must never be surfaced, counted, or hinted at in any user-facing string, log, or number. The indicator reads **only** `FavStore.currentExcluded()`, which is guaranteed to contain only the user's own choices — no code path writes RU/BY into it. **Never derive a count from a filtered-vs-unfiltered station difference**, because that difference includes the ban.
- No Room / SQLite / WorkManager. The catalogue must not go into DataStore.
- The Rust CLI under `crates/` must not be touched.
- Code, comments and logs in English, lowercase. Comments only where the "why" is non-obvious — no trivial comments.
- No `else if` chains — use `when`.
- No AI/assistant references anywhere in code, comments, or commit messages. No personal data (emails, names) in code or comments.
- Commit only to the `dev` branch. Commit subjects are the public changelog: write them for users, not developers (`feat(android): ...` / `fix(android): ...`).
- User-facing copy conventions, taken from the existing screen: SCREAMING CAPS for pills, kickers, buttons and warn lines; `·` (U+00B7) as separator; `—` for em-dash phrases; British spelling. The established user-facing term for this feature is **"hidden"** (see `sync_hide_countries` = `HIDE COUNTRIES`), not "excluded" or "filtered".
- Both `res/layout/activity_main.xml` (portrait) and `res/layout-land/activity_main.xml` (landscape) must be kept in sync — they duplicate the same context row, and there is no `styles.xml` indirection, so every attribute is inline and must be written twice.
- Colour tokens are fixed (`res/values/colors.xml`): `olive` is reserved for LIVE only and must not be used here; `amber`/`amber_hi` mean "actionable/active"; `danger` means warn; `dim`/`mute` mean quiet.

## Background the implementer needs

**Where the user's hidden countries come from.** `FavStore.keyExcludedCountries` (`FavStore.kt:59`) is a `stringSetPreferencesKey("excluded_countries")` holding uppercase ISO-2 codes. It is read via `suspend fun currentExcluded(): Set<String>` (`FavStore.kt:123-124`). It is written from exactly two places: the `HIDE COUNTRIES` dialog in `SyncActivity.kt:162` (`setExcluded`), and the sync merge path `applyMerged` (`FavStore.kt:147-153`).

**Why a naive refresh is not enough.** `SyncActivity` saves exclusions then calls `triggerSync()`, which fires `ACTION_SYNC_NOW` → `PlaybackService.syncNow()`. But `syncNow()` early-returns at `PlaybackService.kt:272` when the user has no sync key (`favStore.syncKey() ?: return@launch`), so `refreshCustomLayout()` at `:280` never runs for an unsynced user. Task 3 fixes this; without it the new pill would show a stale count after the user changes their filters.

**Why the warn state cannot be `excludedCount > 0`.** `pickForScope` (`Catalog.kt:54-66`) falls back to the full catalogue in FAVS scope when there are no favourites, and `FavLogic.pickFav` (`FavStore.kt:28`) deliberately ignores `userExcluded` entirely. So "nothing to play" is a property of the actual playable set, not of the filter settings. The warn state must therefore be driven by a real count of playable stations, computed service-side where the catalogue lives.

**Why favourites ignore the country filter (do not "fix" this).** A starred station from a hidden country still plays in FAVS scope — that is intentional: if you starred it, you want it. The consequence for this plan is that the pill must be **hidden in FAVS scope**, because there the filter genuinely does not apply and showing it would be a lie.

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt` | Modify | Declare two new extras; publish hidden-country count and playable-station count; refresh extras after a filter change even without a sync key |
| `android/app/src/main/kotlin/net/vchub/r4dio/MainActivity.kt` | Modify | Read the new extras; render the new pill; extend the warn state |
| `android/app/src/main/kotlin/net/vchub/r4dio/HomeState.kt` | **Create** | Pure, testable decision functions for pill visibility/text and warn state — extracted so the logic has real unit coverage without a Service/Activity harness |
| `android/app/src/main/res/layout/activity_main.xml` | Modify | Add the pill view to the portrait context row |
| `android/app/src/main/res/layout-land/activity_main.xml` | Modify | Add the same pill to the landscape context row |
| `android/app/src/main/res/values/strings.xml` | Modify | New `home_hidden_n` and `home_warn_all_hidden` strings |
| `android/app/src/test/kotlin/net/vchub/r4dio/HomeStateTest.kt` | **Create** | Unit tests for the decision functions |

**Why a new `HomeState.kt` file:** the review of the previous branch flagged (M6) that all the orchestration logic lives in `PlaybackService`/`MainActivity`, which have no unit-test harness, so seam bugs reach final review undetected. Extracting the decisions as pure functions — the same shape as the existing `catalogIsStale` and `parseArtist` — makes this feature's logic genuinely testable. Keep the file small; it holds only these functions.

---

### Task 1: Pure decision functions for the pill and warn state

**Files:**
- Create: `android/app/src/main/kotlin/net/vchub/r4dio/HomeState.kt`
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/HomeStateTest.kt`

**Interfaces:**
- Consumes: nothing (pure Kotlin, no Android imports)
- Produces, relied on by Tasks 2 and 4:
  - `fun showsHiddenPill(hiddenCount: Int, scope: String): Boolean`
  - `fun isAllHiddenWarn(playableCount: Int, hiddenCount: Int, scope: String): Boolean`

- [ ] **Step 1: Write the failing test**

Create `android/app/src/test/kotlin/net/vchub/r4dio/HomeStateTest.kt`:

```kotlin
package net.vchub.r4dio

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class HomeStateTest {
    @Test
    fun pill_is_hidden_when_the_user_has_hidden_no_countries() {
        assertFalse(showsHiddenPill(hiddenCount = 0, scope = "all"))
    }

    @Test
    fun pill_shows_in_all_scope_when_countries_are_hidden() {
        assertTrue(showsHiddenPill(hiddenCount = 1, scope = "all"))
        assertTrue(showsHiddenPill(hiddenCount = 40, scope = "all"))
    }

    // favourites deliberately ignore the country filter, so the pill would lie there
    @Test
    fun pill_is_hidden_in_favs_scope_even_when_countries_are_hidden() {
        assertFalse(showsHiddenPill(hiddenCount = 3, scope = "favs"))
    }

    @Test
    fun warn_when_filters_leave_nothing_playable() {
        assertTrue(isAllHiddenWarn(playableCount = 0, hiddenCount = 3, scope = "all"))
    }

    // an empty catalogue with no filters set is a network problem, not a filter problem
    @Test
    fun no_warn_when_nothing_is_playable_but_no_country_is_hidden() {
        assertFalse(isAllHiddenWarn(playableCount = 0, hiddenCount = 0, scope = "all"))
    }

    @Test
    fun no_warn_while_stations_remain_playable() {
        assertFalse(isAllHiddenWarn(playableCount = 1, hiddenCount = 3, scope = "all"))
        assertFalse(isAllHiddenWarn(playableCount = 1000, hiddenCount = 40, scope = "all"))
    }

    // in favs scope pickForScope falls back to the catalogue, so the filter is not
    // what is stopping playback; the existing no-favourites warn owns that case
    @Test
    fun no_warn_in_favs_scope() {
        assertFalse(isAllHiddenWarn(playableCount = 0, hiddenCount = 3, scope = "favs"))
    }

    @Test
    fun unknown_scope_string_is_treated_as_all() {
        assertTrue(showsHiddenPill(hiddenCount = 2, scope = "something-else"))
        assertTrue(isAllHiddenWarn(playableCount = 0, hiddenCount = 2, scope = "something-else"))
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd android && ./gradlew testDebugUnitTest --tests "net.vchub.r4dio.HomeStateTest"`

Expected: **compilation failure** — `unresolved reference: showsHiddenPill`. That is the correct failure; the functions do not exist yet.

- [ ] **Step 3: Write the minimal implementation**

Create `android/app/src/main/kotlin/net/vchub/r4dio/HomeState.kt`:

```kotlin
package net.vchub.r4dio

/**
 * home-screen decisions kept free of android types so they can be unit tested.
 * `scope` is the wire value carried in the session extras: "favs" or "all".
 */

/**
 * favourites bypass the country filter entirely (FavLogic.pickFav ignores
 * userExcluded), so advertising a filter in that scope would be false.
 */
fun showsHiddenPill(hiddenCount: Int, scope: String): Boolean =
    hiddenCount > 0 && scope != "favs"

/**
 * only blame the user's filters when they are actually set — an empty playable
 * set with no hidden countries is a network or catalogue problem, and in favs
 * scope pickForScope falls back to the catalogue, so the filter is not the cause.
 */
fun isAllHiddenWarn(playableCount: Int, hiddenCount: Int, scope: String): Boolean =
    playableCount == 0 && hiddenCount > 0 && scope != "favs"
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd android && ./gradlew testDebugUnitTest --tests "net.vchub.r4dio.HomeStateTest"`

Expected: PASS, 8 tests. **Verify from the JUnit XML, not the console summary:** read `android/app/build/test-results/testDebugUnitTest/TEST-net.vchub.r4dio.HomeStateTest.xml` and confirm `tests="8" failures="0" errors="0" skipped="0"`.

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/HomeState.kt android/app/src/test/kotlin/net/vchub/r4dio/HomeStateTest.kt
git commit -m "feat(android): decide when to show the hidden-countries indicator"
```

---

### Task 2: Publish the two new counts from the service

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt:37-39` (extras constants), `:123-134` (`refreshCustomLayout`)

**Interfaces:**
- Consumes: `showsHiddenPill` / `isAllHiddenWarn` are not used here — this task only publishes the raw counts.
- Produces, relied on by Task 4:
  - `const val EXTRA_HIDDEN_COUNT = "net.vchub.r4dio.EXTRA_HIDDEN_COUNT"` — `Int`, the size of the user's own hidden-country set
  - `const val EXTRA_PLAYABLE_COUNT = "net.vchub.r4dio.EXTRA_PLAYABLE_COUNT"` — `Int`, how many catalogue stations survive both the ban and the user's filters

- [ ] **Step 1: Add the two extras constants**

In `PlaybackService.kt`, immediately after the existing `EXTRA_FAV_COUNT` declaration at line 39, add:

```kotlin
        const val EXTRA_HIDDEN_COUNT = "net.vchub.r4dio.EXTRA_HIDDEN_COUNT"
        const val EXTRA_PLAYABLE_COUNT = "net.vchub.r4dio.EXTRA_PLAYABLE_COUNT"
```

Match the surrounding indentation exactly — read lines 37-39 first and copy their leading whitespace.

- [ ] **Step 2: Publish both counts in `refreshCustomLayout`**

`refreshCustomLayout` (`PlaybackService.kt:123-134`) is already a `suspend fun` reading DataStore, so the new read needs no plumbing. Change its body so the extras bundle carries the two new values. The existing code is:

```kotlin
    private suspend fun refreshCustomLayout() {
        val favs = favStore.currentFavUuids()
        val isFav = current?.uuid?.let { favs.contains(it) } ?: false
        val sc = favStore.currentScope()
        session?.setCustomLayout(listOf(shuffleButton, starButton(isFav), syncButton, stopButton))
        val extras = android.os.Bundle().apply {
            putBoolean(EXTRA_FAV, isFav)
            putString(EXTRA_SCOPE, if (sc == Scope.FAVS) "favs" else "all")
            putInt(EXTRA_FAV_COUNT, favs.size)
        }
        session?.setSessionExtras(extras)
    }
```

Replace it with:

```kotlin
    private suspend fun refreshCustomLayout() {
        val favs = favStore.currentFavUuids()
        val isFav = current?.uuid?.let { favs.contains(it) } ?: false
        val sc = favStore.currentScope()
        val hidden = favStore.currentExcluded()
        // count what the user could actually reach, so the screen can tell
        // "your filters hid everything" apart from "the catalogue is empty"
        val playable = stations.count { allowedStation(it, hidden) }
        session?.setCustomLayout(listOf(shuffleButton, starButton(isFav), syncButton, stopButton))
        val extras = android.os.Bundle().apply {
            putBoolean(EXTRA_FAV, isFav)
            putString(EXTRA_SCOPE, if (sc == Scope.FAVS) "favs" else "all")
            putInt(EXTRA_FAV_COUNT, favs.size)
            putInt(EXTRA_HIDDEN_COUNT, hidden.size)
            putInt(EXTRA_PLAYABLE_COUNT, playable)
        }
        session?.setSessionExtras(extras)
    }
```

Two things to be careful about:
- `stations` is the service's `@Volatile var stations: List<Station>` — read it once into `playable` as shown, do not iterate it twice.
- `allowedStation` is already imported/available in this file (it is used elsewhere in the service); if the compiler disagrees, add `import net.vchub.r4dio.allowedStation` — but check first, a duplicate import is a compile error.

- [ ] **Step 3: Verify it compiles and nothing regressed**

Run: `cd android && ./gradlew assembleDebug`
Expected: BUILD SUCCESSFUL.

Run: `cd android && ./gradlew testDebugUnitTest`
Expected: BUILD SUCCESSFUL, all pre-existing suites still green. Read the aggregate from the JUnit XML files under `android/app/build/test-results/testDebugUnitTest/` and report the exact `tests=` / `failures=` / `errors=` totals.

- [ ] **Step 4: Verify the privacy constraint by inspection**

Run: `grep -rn "EXCLUDED_COUNTRYCODES\|EXCLUDED_NAME_SUBSTRINGS" android/app/src/main/kotlin/`

Expected: hits **only** inside `Catalog.kt`. Confirm that neither new extra is derived from those vals, and that `hidden.size` comes from `favStore.currentExcluded()` alone. State this explicitly in your report — the ban must never become a user-visible number.

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt
git commit -m "feat(android): tell the screen how many countries are hidden"
```

---

### Task 3: Refresh the screen after a filter change with no sync key

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt` (the `ACTION_SYNC_NOW` handling around `:166` and `syncNow()` around `:270-281`)

**Interfaces:**
- Consumes: `refreshCustomLayout()` from Task 2 (now publishing the two new counts)
- Produces: nothing new — this task only guarantees the extras are republished after the user edits their hidden countries

**Why this task exists:** `SyncActivity.kt:163` saves the new exclusion set and calls `triggerSync()`. That fires `ACTION_SYNC_NOW`, which calls `syncNow()`, which early-returns at `PlaybackService.kt:272` (`favStore.syncKey() ?: return@launch`) for a user who has never linked a device. For that user — the majority — `refreshCustomLayout()` never runs, so the pill would keep showing the old count until something else happened to refresh it. Without this, Task 4's pill is visibly wrong.

- [ ] **Step 1: Read the current code**

Read `PlaybackService.kt` lines 160-175 and 265-285 before editing. You need to see the exact shape of the `ACTION_SYNC_NOW` branch and of `syncNow()`'s coroutine launch and its early return.

- [ ] **Step 2: Make the refresh unconditional**

The fix must guarantee `refreshCustomLayout()` runs after a sync trigger **regardless of whether a sync key exists**, without changing what `syncNow()` does when a key *is* present (it must still merge, then refresh — do not double-refresh in a way that races).

Restructure `syncNow()` so the early return no longer skips the refresh. The shape to aim for:

```kotlin
    private fun syncNow() {
        scope.launch {
            val key = favStore.syncKey()
            when (key) {
                // no linked device: nothing to merge, but local settings may have
                // changed, so the screen still needs the fresh counts
                null -> refreshCustomLayout()
                else -> {
                    // ... existing merge body, unchanged, ending in refreshCustomLayout()
                }
            }
        }
    }
```

Preserve the existing merge body byte-for-byte inside the `else ->` arm — do not rewrite or "improve" it. If the existing body is long, keeping it in place and only changing the control flow around it is preferable to moving code.

Note the constraint: **no `else if` chains** — use `when` as shown.

- [ ] **Step 3: Verify it compiles**

Run: `cd android && ./gradlew assembleDebug`
Expected: BUILD SUCCESSFUL.

Run: `cd android && ./gradlew testDebugUnitTest`
Expected: all suites still green; report exact counts from the JUnit XML.

- [ ] **Step 4: Verify the sync path did not regress**

Run: `git diff` and confirm by inspection that:
- the merge body inside the `else ->` arm is unchanged from before,
- `refreshCustomLayout()` is still called exactly once per `syncNow()` invocation on each path (once in the `null` arm, once at the end of the merge arm — never twice on the same path),
- nothing outside `syncNow()` and the `ACTION_SYNC_NOW` branch was touched.

State each of these three in your report.

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt
git commit -m "fix(android): update the screen after changing hidden countries without a linked device"
```

---

### Task 4: The pill and the warn state on screen

**Files:**
- Modify: `android/app/src/main/res/values/strings.xml`
- Modify: `android/app/src/main/res/layout/activity_main.xml:44-66` (portrait context row)
- Modify: `android/app/src/main/res/layout-land/activity_main.xml:71-93` (landscape context row — read it first, the line numbers differ from portrait)
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/MainActivity.kt:36-38` (fields), `:112-116` (`readExtras`), `:119` (`isWarn`), `:121-131` (`render`), `:168-185` (`renderScope`), `:204-225` (`renderHero`)

**Interfaces:**
- Consumes: `EXTRA_HIDDEN_COUNT` and `EXTRA_PLAYABLE_COUNT` (Task 2); `showsHiddenPill(hiddenCount, scope)` and `isAllHiddenWarn(playableCount, hiddenCount, scope)` (Task 1)
- Produces: nothing consumed by later tasks

- [ ] **Step 1: Add the strings**

In `android/app/src/main/res/values/strings.xml`, add next to the existing `home_warn_no_favs` and `home_scope_*` entries:

```xml
    <string name="home_hidden_n">%1$d HIDDEN</string>
    <string name="home_warn_all_hidden">NO STATIONS — ALL COUNTRIES HIDDEN</string>
```

Copy conventions to respect: SCREAMING CAPS, the em-dash `—` (U+2014) with a space either side, British spelling. Do **not** write "countries you excluded" or "filtered" — the established term is "hidden".

- [ ] **Step 2: Add the pill to the portrait layout**

In `android/app/src/main/res/layout/activity_main.xml`, the context row is at lines 44-66: country, codec, fav marker, a weighted spacer, then `@+id/scope_pill`. Insert the new pill **between the spacer and `scope_pill`** so it sits to the left of the scope pill, matching the agreed design. Add, immediately before the `<TextView android:id="@+id/scope_pill"` element:

```xml
                <TextView
                    android:id="@+id/hidden_pill"
                    android:layout_width="wrap_content"
                    android:layout_height="wrap_content"
                    android:layout_marginEnd="6dp"
                    android:visibility="gone"
                    android:textColor="@color/amber_hi"
                    android:textSize="9.5sp"
                    android:fontFamily="@font/ibm_plex_mono"
                    android:letterSpacing="0.1"
                    android:paddingStart="9dp"
                    android:paddingEnd="9dp"
                    android:paddingTop="3dp"
                    android:paddingBottom="3dp"
                    android:background="@drawable/bg_pill_on" />
```

Match the surrounding indentation — read the neighbouring elements and copy their leading whitespace exactly. Note there is no `android:text` attribute: the pill has no meaningful default and starts `gone`; `MainActivity` sets the text. The type spec (9.5sp, letterSpacing 0.1, padding 9/9/3/3) is copied from `scope_pill` so the two read as one family.

- [ ] **Step 3: Add the same pill to the landscape layout**

Read `android/app/src/main/res/layout-land/activity_main.xml` and find its context row (the `scope_pill` is around line 88). Insert the **identical** `hidden_pill` block immediately before its `scope_pill`, with that file's indentation. The two layouts must stay in sync — a control that exists only in portrait is a bug that only shows up in a car mount.

- [ ] **Step 4: Wire the state into MainActivity**

Add the fields next to the existing `favCount` at `MainActivity.kt:36-38`:

```kotlin
    private var hiddenCount = 0
    private var playableCount = 0
```

Extend `readExtras` (`MainActivity.kt:112-116`):

```kotlin
    private fun readExtras(extras: Bundle) {
        fav = extras.getBoolean(EXTRA_FAV, false)
        scope = extras.getString(EXTRA_SCOPE, "all") ?: "all"
        favCount = extras.getInt(EXTRA_FAV_COUNT, 0)
        hiddenCount = extras.getInt(EXTRA_HIDDEN_COUNT, 0)
        playableCount = extras.getInt(EXTRA_PLAYABLE_COUNT, 0)
    }
```

- [ ] **Step 5: Render the pill**

Add a `renderHidden()` function next to `renderScope()`, and call it from `render()` (`MainActivity.kt:121-131`) — put the call immediately after `renderScope()`:

```kotlin
    private fun renderHidden() {
        val pill = findViewById<TextView>(R.id.hidden_pill)
        when (showsHiddenPill(hiddenCount, scope)) {
            true -> {
                pill.text = getString(R.string.home_hidden_n, hiddenCount)
                pill.visibility = View.VISIBLE
            }
            false -> pill.visibility = View.GONE
        }
    }
```

`View.GONE` rather than `INVISIBLE`: the pill must not reserve space in the context row when the user has hidden nothing, which is the common case.

- [ ] **Step 6: Extend the warn state**

The existing warn is `isWarn()` at `MainActivity.kt:119`:

```kotlin
    /** favs scope with nothing starred: shuffle has nothing to pick, so warn instead. */
    private fun isWarn(): Boolean = scope == "favs" && favCount == 0
```

There are now two distinct warn causes that need **different copy** but share the same red treatment. Restructure so the hero asks for the reason once. Replace `isWarn()` with:

```kotlin
    /** the two reasons shuffle has nothing to pick, each with its own message. */
    private fun warnMessage(): Int? = when {
        scope == "favs" && favCount == 0 -> R.string.home_warn_no_favs
        isAllHiddenWarn(playableCount, hiddenCount, scope) -> R.string.home_warn_all_hidden
        else -> null
    }
```

Order matters and is deliberate: the favourites case is checked first, and `isAllHiddenWarn` already returns false in favs scope, so the two can never both fire.

Then update `renderHero()` (`MainActivity.kt:204-225`) to use it. The existing body branches on `isWarn()`; change it to:

```kotlin
    private fun renderHero() {
        val ring = findViewById<View>(R.id.hero_ring)
        val sub = findViewById<TextView>(R.id.hero_sub)
        val glyph = findViewById<ImageView>(R.id.hero_glyph)
        val label = findViewById<TextView>(R.id.hero_label)
        val warn = warnMessage()
        val tone = if (warn != null) R.color.danger else R.color.amber_hi
        glyph.setColorFilter(getColor(tone))
        label.setTextColor(getColor(tone))
        when (warn) {
            null -> {
                ring.setBackgroundResource(R.drawable.bg_hero_ring)
                val sc = if (scope == "favs") R.string.home_shuffle_sub_favs else R.string.home_shuffle_sub_all
                sub.text = getString(sc)
                sub.setTextColor(getColor(R.color.dim))
            }
            else -> {
                ring.setBackgroundResource(R.drawable.bg_hero_ring_warn)
                sub.text = getString(warn)
                sub.setTextColor(getColor(R.color.danger))
            }
        }
    }
```

Check whether `isWarn()` is referenced anywhere else in the file (`grep -n "isWarn" MainActivity.kt`) before deleting it — if it is, update those call sites to `warnMessage() != null`.

- [ ] **Step 7: Verify it builds and all tests pass**

Run: `cd android && ./gradlew assembleDebug`
Expected: BUILD SUCCESSFUL.

Run: `cd android && ./gradlew testDebugUnitTest`
Expected: BUILD SUCCESSFUL. Report exact `tests=` / `failures=` / `errors=` totals read from the JUnit XML under `android/app/build/test-results/testDebugUnitTest/`, not from the console.

- [ ] **Step 8: Verify both layouts agree**

Run: `grep -n "hidden_pill" android/app/src/main/res/layout/activity_main.xml android/app/src/main/res/layout-land/activity_main.xml`

Expected: one hit in each file. Confirm the two blocks are attribute-for-attribute identical apart from indentation, and state this in your report.

- [ ] **Step 9: Commit**

```bash
git add android/app/src/main/res/values/strings.xml android/app/src/main/res/layout/activity_main.xml android/app/src/main/res/layout-land/activity_main.xml android/app/src/main/kotlin/net/vchub/r4dio/MainActivity.kt
git commit -m "feat(android): show when your country filters are on, and say so when they hide everything"
```

---

### Task 5: Emulator verification (controller-run, no code change)

This task is run by the coordinating session, not by an implementation subagent. It produces no commit unless it uncovers a defect.

**Constraints, from the project's standing rules:** never touch the user's real data dir; shut the emulator down when finished; uninstall the app afterwards.

- [ ] **Step 1: Build and install on a clean emulator**

Boot the Pixel_7 AVD, install a freshly built debug APK on a clean install (uninstall first if present).

- [ ] **Step 2: Verify the default state — no filters**

With no countries hidden, confirm on the home screen: **no `HIDDEN` pill is visible**, the scope pill reads `ALL STATIONS`, the hero is amber with `random station · eyes-free`. Screenshot.

- [ ] **Step 3: Verify the pill appears after hiding countries**

Home → SYNC → `HIDE COUNTRIES` → tick three countries → save. Return to the home screen. Confirm the pill reads `3 HIDDEN` in amber, sits to the left of the scope pill, and that the count is correct. **This is the Task 3 path** — do it on a device with **no sync key** (never linked), which is exactly the case that used to skip the refresh. Screenshot.

- [ ] **Step 4: Verify the pill hides in FAVS scope**

Star a station, toggle scope to favourites. Confirm the `HIDDEN` pill disappears while the scope pill reads `FAVOURITES ONLY · 1`. Toggle back to ALL and confirm it returns. Screenshot both.

- [ ] **Step 5: Verify the all-hidden warn state**

Hide enough countries that nothing remains playable. The `HIDE COUNTRIES` dialog offers a fixed 40-country list (`SyncActivity.kt:182-187`) — tick all 40. Then force a fresh pick (tap the stage). Confirm: red hero ring, red glyph and label, and the sub-line reading `NO STATIONS — ALL COUNTRIES HIDDEN`. Screenshot.

Note: whether all 40 is genuinely enough depends on the cached catalogue's country spread. If stations from unlisted countries survive, the warn correctly does **not** fire — verify the pill still shows `40 HIDDEN` and that playback still works, and record that outcome rather than forcing the warn artificially.

- [ ] **Step 6: Verify recovery**

Un-hide the countries. Confirm the warn clears, the hero returns to amber, the pill disappears, and shuffle plays again.

- [ ] **Step 7: Verify landscape**

Rotate to landscape in both the pill-visible and warn states. Confirm the pill is present and correctly placed in the car-mount layout, and that nothing is clipped. Screenshot.

- [ ] **Step 8: Check logcat for regressions**

Confirm no new warnings or exceptions from `r4dio`, and specifically that the catalogue-cache behaviour from the previous branch is unaffected (`loaded N stations from cache` on a second launch, no `.tmp`/`.bak` survivors in `files/`).

- [ ] **Step 9: Shut down**

Uninstall the app and shut the emulator down.

---

## Out of scope — do not do these

- **Do not** apply the country filter to favourites (`FavLogic.pickFav`). A starred station from a hidden country still plays, deliberately.
- **Do not** surface, count, or hint at the RU/BY ban anywhere.
- **Do not** add a country picker to the home screen — the pill is an indicator, not a control. `HIDE COUNTRIES` stays in SyncActivity.
- **Do not** make the pill tappable in this plan. (A future piece may route it to the picker; it needs its own design pass for the eyes-free/driving context.)
- **Do not** touch `crates/`, `Catalog.kt`'s filtering rules, `shuffle()`, or `playPick()`.
- Known pre-existing issues left alone: the 40-country picker silently drops codes synced from the CLI that are not in its list (`SyncActivity.kt:161`); `startFrom` returns silently when nothing is playable (`PlaybackService.kt:207`); `nowSecs()` uses wall-clock time.

## Self-review notes

- **Spec coverage:** the three decisions taken with the user are each implemented — indicator as a separate pill beside SCOPE (Task 4, Step 2/3/5), warn state reusing the favs recipe (Task 4, Step 6), favourites behaviour untouched with the pill hidden in FAVS scope (Task 1's `showsHiddenPill`, Task 4's `renderHidden`).
- **Privacy:** the ban is never counted; Task 2 Step 4 is an explicit check gate.
- **Type consistency:** `showsHiddenPill(hiddenCount: Int, scope: String)` and `isAllHiddenWarn(playableCount: Int, hiddenCount: Int, scope: String)` are defined in Task 1 and used with the same names and argument order in Task 4. `EXTRA_HIDDEN_COUNT` / `EXTRA_PLAYABLE_COUNT` are defined in Task 2 and read in Task 4. The `scope` wire values are `"favs"` / `"all"` throughout.
- **The seam that needs watching in final review:** Task 2 computes `playableCount` from the service's `stations` field at the moment `refreshCustomLayout()` runs. If `stations` is empty because the catalogue has not loaded yet, `playableCount` is 0 — and `isAllHiddenWarn` would fire purely because the user has some countries hidden, showing a false "all countries hidden" warning during a cold start before the catalogue arrives. **Whoever runs the final whole-branch review must check this specific interleaving**, and Task 5 Step 2 should be performed on a cold start with filters already set to try to catch it in practice.
