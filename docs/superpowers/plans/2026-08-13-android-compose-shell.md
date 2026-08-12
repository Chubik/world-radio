# Android Compose Shell + Themes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Phase 1 of the Compose rewrite — the app runs on Compose with four bottom tabs, Home ported so it looks and behaves exactly as it does today, and all 14 themes live from the value that already syncs from the desktop and is currently discarded.

**Architecture:** One `MainActivity` hosting a Compose tree. A `R4dioTheme` composable supplies a 9-slot palette through a `CompositionLocal`, so every screen reads tokens rather than colours. A single `PlayerState` holder owns the `MediaController` and exposes state as a `StateFlow`, replacing the per-Activity fields and session-extras decoding that today live inside `MainActivity` — the other three tabs consume the same holder. Catalog/Library/Settings ship as placeholder screens in this phase; they get their content in phases 2–5.

**Tech Stack:** Kotlin, Jetpack Compose (BOM), Media3, DataStore, kotlinx.serialization. New deps: `androidx.compose.*`, `androidx.activity:activity-compose`, `androidx.lifecycle:lifecycle-runtime-compose`.

**Spec:** `docs/superpowers/specs/2026-08-13-android-compose-rewrite-design.md`

## Global Constraints

- Repo `radio`, branch `dev`, everything under `android/`. Do not push — the controller pushes after review.
- **Home is ported verbatim, not redesigned.** Any visual difference from the current release is a defect. The giant shuffle target, the pill row, the four bottom buttons and the sync bar all keep their present behaviour and proportions.
- **The widget (`widget_radio.xml`, `widget_radio_small.xml`) and the media notification are not touched.** RemoteViews cannot be Compose.
- **Every colour comes from a token.** No `Color(0xFF...)` literals in screen code. A screen that hardcodes one colour breaks 14 themes at once.
- **The palette is 9 slots**: `bg, fg, accent, hot, dim, ok, err, info, peak`. Values are copied verbatim from `crates/radio-tui/src/tui/theme.rs:112-269` — Task 1 lists all 14 in full. `amber-crt` is exactly today's `colors.xml`, so a correct port is visually identical to the current release.
- **`hifi-paper` is the only light theme** (bg `#EFE6CC`). Nothing may assume a dark background.
- **The theme slug arrives from sync and must be honoured, not defaulted.** An unknown slug (from a newer client) keeps the current theme rather than resetting to amber — mirrors `Theme::try_from_slug` returning `None`.
- **The RU/BY ban holds on every path** ([[exclude-russian-stations]]).
- All code, comments, strings and commit messages English, lowercase-first. Comments only where they state a constraint. Commit subjects are the public changelog.
- Gate before every commit: `cd android && ./gradlew test` green, real output pasted in the report.
- **Never launch playback to prove something** — read the rendered screen, `catalog.json` and logcat. The emulator plays audio out loud on the user's machine.
- Verified on the emulator, not by reading code. Screenshots prove layout, **not colour** — check pairings against the palette table.

---

### Task 1: The palette, as data

**Files:**
- Create: `android/app/src/main/kotlin/net/vchub/r4dio/ui/Palette.kt`
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/ui/PaletteTest.kt`

**Interfaces:**
- Produces (every later task and phase depends on these):

```kotlin
data class Palette(
    val bg: Long, val fg: Long, val accent: Long, val hot: Long, val dim: Long,
    val ok: Long, val err: Long, val info: Long, val peak: Long,
)
fun paletteFor(slug: String): Palette?   // null for an unknown slug
val THEME_SLUGS: List<String>            // the 14, in cycle order
```

Colours are `Long` (0xAARRGGBB) rather than Compose `Color` so this file has no Compose dependency and stays unit-testable on the JVM.

**Read first:** nothing in Android. The source of truth is `crates/radio-tui/src/tui/theme.rs:112-269`. The values below were extracted from it programmatically — do not retype them from the Rust by hand.

- [ ] **Step 1: Write the failing test**

```kotlin
package net.vchub.r4dio.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class PaletteTest {
    // amber-crt is the current release's colours.xml. if this drifts, every
    // existing screenshot and the widget stop matching the app.
    @Test
    fun amber_crt_is_todays_palette() {
        val p = paletteFor("amber-crt")!!
        assertEquals(0xFF15100B, p.bg)
        assertEquals(0xFFD49A3A, p.fg)
        assertEquals(0xFFFFC457, p.accent)
        assertEquals(0xFFFF8A3D, p.hot)
        assertEquals(0xFF6E5430, p.dim)
        assertEquals(0xFF9EC074, p.ok)
        assertEquals(0xFFD96A5A, p.err)
        assertEquals(0xFF6FB0C8, p.info)
        assertEquals(0xFFFFF0C0, p.peak)
    }

    @Test
    fun all_fourteen_themes_resolve() {
        assertEquals(14, THEME_SLUGS.size)
        THEME_SLUGS.forEach { assertNotNull("no palette for $it", paletteFor(it)) }
    }

    // an unknown slug means a newer client chose a theme this build does not
    // have. the caller keeps its current theme; it must not be handed a default
    // that would silently overwrite the user's choice.
    @Test
    fun an_unknown_slug_resolves_to_nothing() {
        assertNull(paletteFor("solarized-light-extra"))
        assertNull(paletteFor(""))
    }

    // hifi-paper is the one light theme. a build that assumes a dark background
    // renders dark-on-dark here, and this is the test that says so.
    @Test
    fun hifi_paper_is_light() {
        val p = paletteFor("hifi-paper")!!
        assertEquals(0xFFEFE6CC, p.bg)
        assertEquals(0xFF0F0A04, p.peak)
    }

    @Test
    fun every_palette_is_fully_opaque() {
        THEME_SLUGS.mapNotNull { paletteFor(it) }.forEach { p ->
            listOf(p.bg, p.fg, p.accent, p.hot, p.dim, p.ok, p.err, p.info, p.peak)
                .forEach { assertEquals(0xFF000000, it and 0xFF000000) }
        }
    }
}
```

Add `import org.junit.Assert.assertNotNull` alongside the other imports.

- [ ] **Step 2: Run to verify it fails**

Run: `cd android && ./gradlew testDebugUnitTest --tests '*PaletteTest*'`
Expected: compile failure — `Palette` does not exist.
NOTE: `./gradlew test --tests ...` does NOT work; the aggregate task rejects `--tests`. Always use `testDebugUnitTest`.

- [ ] **Step 3: Implement**

```kotlin
package net.vchub.r4dio.ui

/**
 * the nine colour roles every r4dio client shares, mirroring
 * crates/radio-tui/src/tui/theme.rs. kept as 0xAARRGGBB longs rather than
 * compose Colors so this file stays a plain jvm unit and the values can be
 * compared against the rust source without a device.
 */
data class Palette(
    val bg: Long,
    val fg: Long,
    val accent: Long,
    val hot: Long,
    val dim: Long,
    val ok: Long,
    val err: Long,
    val info: Long,
    val peak: Long,
)

/** the 14 themes in the cli's cycle order. */
val THEME_SLUGS: List<String> = listOf(
    "amber-crt", "tube-glow", "hifi-paper", "shortwave-green", "cyber-neon",
    "atomic-terminal", "mainframe-blue", "nord", "gruvbox", "dracula",
    "solarized", "catppuccin", "rose-pine", "monokai",
)

private fun p(
    bg: Long, fg: Long, accent: Long, hot: Long, dim: Long,
    ok: Long, err: Long, info: Long, peak: Long,
) = Palette(
    bg or OPAQUE, fg or OPAQUE, accent or OPAQUE, hot or OPAQUE, dim or OPAQUE,
    ok or OPAQUE, err or OPAQUE, info or OPAQUE, peak or OPAQUE,
)

private const val OPAQUE = 0xFF000000

private val PALETTES: Map<String, Palette> = mapOf(
    "amber-crt" to p(0x15100B, 0xD49A3A, 0xFFC457, 0xFF8A3D, 0x6E5430, 0x9EC074, 0xD96A5A, 0x6FB0C8, 0xFFF0C0),
    "tube-glow" to p(0x0B1220, 0xE5D7B8, 0xFFE3A8, 0xFF8A4D, 0x6A6855, 0x7FD9A8, 0xFF6A6A, 0x5CC7D8, 0xFFF2CC),
    "hifi-paper" to p(0xEFE6CC, 0x2E2517, 0xC5872A, 0xA13E2D, 0x8A7A5A, 0x5A7A3A, 0xB14D2D, 0x2F6680, 0x0F0A04),
    "shortwave-green" to p(0x061008, 0x7FDA7F, 0xB5FF8A, 0xFF9D3D, 0x2D6633, 0x5FFF9C, 0xFF5C5C, 0x66C5FF, 0xD6FFC8),
    "cyber-neon" to p(0x07041A, 0xC7C0E8, 0x00FFE1, 0xFF2BD5, 0x463860, 0x6DFF7F, 0xFF5050, 0x5AD8FF, 0xFFFFFF),
    "atomic-terminal" to p(0x0A1A0C, 0x4CDC60, 0x9CFF66, 0xFFC232, 0x1F5E2A, 0x66FF5C, 0xFF5040, 0x5CFFAA, 0xD2FF8C),
    "mainframe-blue" to p(0x081A3A, 0xD8E8FF, 0x66C0FF, 0xFFD54A, 0x3A5A8A, 0x66E8A0, 0xFF7070, 0xFFB84D, 0xFFFFFF),
    "nord" to p(0x2E3440, 0xD8DEE9, 0x88C0D0, 0xD08770, 0x4C566A, 0xA3BE8C, 0xBF616A, 0x81A1C1, 0xECEFF4),
    "gruvbox" to p(0x282828, 0xEBDBB2, 0xFABD2F, 0xFE8019, 0x665C54, 0xB8BB26, 0xFB4934, 0x83A598, 0xFBF1C7),
    "dracula" to p(0x282A36, 0xF8F8F2, 0xBD93F9, 0xFF79C6, 0x6272A4, 0x50FA7B, 0xFF5555, 0x8BE9FD, 0xF1FA8C),
    "solarized" to p(0x002B36, 0x93A1A1, 0x268BD2, 0xCB4B16, 0x586E75, 0x859900, 0xDC322F, 0x2AA198, 0xFDF6E3),
    "catppuccin" to p(0x1E1E2E, 0xCDD6F4, 0xCBA6F7, 0xF5C2E7, 0x6C7086, 0xA6E3A1, 0xF38BA8, 0x89DCEB, 0xF9E2AF),
    "rose-pine" to p(0x191724, 0xE0DEF4, 0xC4A7E7, 0xEBBCBA, 0x6E6A86, 0x9CCFD8, 0xEB6F92, 0x31748F, 0xF6C177),
    "monokai" to p(0x272822, 0xF8F8F2, 0xA6E22E, 0xF92672, 0x75715E, 0xA6E22E, 0xF92672, 0x66D9EF, 0xE6DB74),
)

/**
 * null rather than a default: an unknown slug means a newer client picked a
 * theme this build does not have, and the caller must keep what it has rather
 * than silently resetting the user's choice.
 */
fun paletteFor(slug: String): Palette? = PALETTES[slug]
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd android && ./gradlew testDebugUnitTest --tests '*PaletteTest*'`
Expected: PASS, 5 tests.

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/ui/Palette.kt android/app/src/test/kotlin/net/vchub/r4dio/ui/PaletteTest.kt
git commit -m "every r4dio colour, as data"
```

---

### Task 2: Compose builds and the theme reaches a screen

**Files:**
- Modify: `android/app/build.gradle.kts` (Compose deps + `buildFeatures`)
- Create: `android/app/src/main/kotlin/net/vchub/r4dio/ui/Theme.kt`
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/ui/ThemeResolveTest.kt`

**Interfaces:**
- Consumes: `Palette`, `paletteFor`, `THEME_SLUGS` (Task 1).
- Produces:

```kotlin
@Composable fun R4dioTheme(slug: String, content: @Composable () -> Unit)
object R4dioTokens { val colors: Palette @Composable get }   // read as R4dioTokens.colors.accent
fun resolveTheme(synced: String, current: String): String     // pure, testable
```

**Read first:** `android/app/build.gradle.kts:1-30` for the existing android block, and `app/build.gradle.kts:56-68` for the dependency list. Kotlin is 2.4.0, so the Compose compiler ships with the Kotlin plugin — add `org.jetbrains.kotlin.plugin.compose`, do NOT set `composeOptions.kotlinCompilerExtensionVersion` (that is the pre-2.0 idiom and will fail).

- [ ] **Step 1: Write the failing test**

`resolveTheme` is the whole rule the sync path depends on, and it is pure, so it is tested without a device.

```kotlin
package net.vchub.r4dio.ui

import org.junit.Assert.assertEquals
import org.junit.Test

class ThemeResolveTest {
    @Test
    fun a_synced_theme_wins() {
        assertEquals("nord", resolveTheme(synced = "nord", current = "amber-crt"))
    }

    // a slug this build does not know can only come from a newer client. the
    // user's current theme survives it; resetting to a default would be a
    // visible change nobody asked for.
    @Test
    fun an_unknown_synced_theme_keeps_what_we_have() {
        assertEquals("gruvbox", resolveTheme(synced = "plasma-9000", current = "gruvbox"))
    }

    // nothing synced yet: the account has never set a theme.
    @Test
    fun an_empty_synced_theme_keeps_what_we_have() {
        assertEquals("gruvbox", resolveTheme(synced = "", current = "gruvbox"))
    }

    // first run, nothing stored anywhere.
    @Test
    fun with_nothing_at_all_we_land_on_amber() {
        assertEquals("amber-crt", resolveTheme(synced = "", current = ""))
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd android && ./gradlew testDebugUnitTest --tests '*ThemeResolveTest*'`
Expected: compile failure — `resolveTheme` does not exist.

- [ ] **Step 3: Add Compose to the build**

In `android/app/build.gradle.kts`, add to the `plugins` block:

```kotlin
    id("org.jetbrains.kotlin.plugin.compose")
```

In the root `android/build.gradle.kts` `plugins` block, add:

```kotlin
    id("org.jetbrains.kotlin.plugin.compose") version "2.4.0" apply false
```

In `android/app/build.gradle.kts`, inside the `android { }` block:

```kotlin
    buildFeatures {
        compose = true
    }
```

And in `dependencies`, above the `testImplementation` lines:

```kotlin
    implementation(platform("androidx.compose:compose-bom:2025.09.00"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.activity:activity-compose:1.10.1")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.9.2")
    debugImplementation("androidx.compose.ui:ui-tooling")
```

- [ ] **Step 4: Implement the theme**

```kotlin
package net.vchub.r4dio.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ProvidableCompositionLocal
import androidx.compose.runtime.staticCompositionLocalOf

const val DEFAULT_THEME = "amber-crt"

/**
 * which theme to show, given what sync delivered and what we are showing now.
 * pure so the rule lives in one testable place: the synced value wins when this
 * build knows it, and is ignored otherwise rather than resetting the user.
 */
fun resolveTheme(synced: String, current: String): String {
    if (paletteFor(synced) != null) return synced
    if (paletteFor(current) != null) return current
    return DEFAULT_THEME
}

private val LocalPalette: ProvidableCompositionLocal<Palette> =
    staticCompositionLocalOf { paletteFor(DEFAULT_THEME)!! }

/**
 * the only place a colour enters the tree. screens read R4dioTokens.colors.x,
 * never a literal — a single hardcoded colour breaks all 14 themes at once.
 */
object R4dioTokens {
    val colors: Palette
        @Composable get() = LocalPalette.current
}

@Composable
fun R4dioTheme(slug: String, content: @Composable () -> Unit) {
    val palette = paletteFor(slug) ?: paletteFor(DEFAULT_THEME)!!
    CompositionLocalProvider(LocalPalette provides palette, content = content)
}
```

- [ ] **Step 5: Run to verify it passes and the project still builds**

Run: `cd android && ./gradlew testDebugUnitTest --tests '*ThemeResolveTest*' && ./gradlew assembleDebug`
Expected: 4 tests PASS, `BUILD SUCCESSFUL`. The APK still installs and the existing XML app still runs — nothing is wired to Compose yet.

- [ ] **Step 6: Report the APK size delta**

Run: `ls -la android/app/build/outputs/apk/debug/app-debug.apk`
Record the size in the task report and compare against 6.6 MB. The spec expects +2–3 MB; a much larger jump means a dependency was added that is not on the list.

- [ ] **Step 7: Commit**

```bash
git add android/build.gradle.kts android/app/build.gradle.kts android/app/src/main/kotlin/net/vchub/r4dio/ui/Theme.kt android/app/src/test/kotlin/net/vchub/r4dio/ui/ThemeResolveTest.kt
git commit -m "the theme you pick on the desktop now reaches the phone"
```

---

### Task 3: The theme is stored, and follows sync

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/FavStore.kt` (add a theme flow + local setter)
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/ProfileSyncTest.kt` (add cases)

**Interfaces:**
- Consumes: `resolveTheme` (Task 2); `SyncProfile.withTheme` follows the shape of `withScope`/`withCountries` already in `Profile.kt:61-90`.
- Produces:

```kotlin
// in FavStore
val theme: Flow<String>                       // "" until something sets it
suspend fun currentTheme(): String
suspend fun setTheme(slug: String, now: Long = System.currentTimeMillis() / 1000)
// in SyncProfile
fun withTheme(next: String, now: Long): SyncProfile
```

**Read first:** `FavStore.kt:163-193` (`setScope` and `setFilter` are the two models to copy — both stamp through a `SyncProfile.withX` helper so the "a same-value save must not move the stamp" rule lives in one place) and `Profile.kt:53-90`. `keyTheme`/`keyThemeAt` already exist at `FavStore.kt:94-95` and are already read/written by `profile()`/`applyProfile()` — this task only adds the *local* accessors.

- [ ] **Step 1: Write the failing tests**

Append to `ProfileSyncTest.kt`:

```kotlin
    @Test
    fun setting_the_theme_stamps_the_change() {
        val p = SyncProfile().withTheme("nord", 50)
        assertEquals("nord", p.theme)
        assertEquals(50L, p.themeAt)
    }

    // a same-value save must not move the stamp, or an idle device would win
    // every race against a device that actually changed something.
    @Test
    fun setting_the_same_theme_does_not_move_the_stamp() {
        val p = SyncProfile(theme = "nord", themeAt = 10).withTheme("nord", 99)
        assertEquals(10L, p.themeAt)
    }

    @Test
    fun a_locally_set_theme_is_sent() {
        val sent = SyncProfile().withTheme("dracula", 50)
            .outgoing(favs = emptyList(), blocked = emptyList(), excluded = emptyList(), plays = emptyList())
        assertEquals(50L, sent.theme?.at)
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd android && ./gradlew testDebugUnitTest --tests '*ProfileSyncTest*'`
Expected: compile failure — `withTheme` does not exist.

- [ ] **Step 3: Implement `withTheme`**

In `Profile.kt`, directly below `withCountries`:

```kotlin
    /**
     * the theme is shared across devices exactly as the scope and the filter
     * are. not normalised: a slug this build does not know is still a valid
     * value for a newer client, and must survive a round trip through here.
     */
    fun withTheme(next: String, now: Long): SyncProfile {
        if (next == theme) return this
        return copy(theme = next, themeAt = now)
    }
```

- [ ] **Step 4: Implement the store accessors**

In `FavStore.kt`, below `setFilter`:

```kotlin
    val theme: Flow<String> = store.data.map { it[keyTheme].orEmpty() }

    suspend fun currentTheme(): String = store.data.first()[keyTheme].orEmpty()

    /**
     * stamped through [SyncProfile.withTheme] so the "a same-value save must not
     * move the stamp" rule lives in one place, exactly as setScope does.
     */
    suspend fun setTheme(slug: String, now: Long = System.currentTimeMillis() / 1000) {
        store.edit { prefs ->
            val stamped = SyncProfile(
                theme = prefs[keyTheme].orEmpty(),
                themeAt = prefs[keyThemeAt] ?: 0L,
            ).withTheme(slug, now)
            prefs[keyTheme] = stamped.theme
            prefs[keyThemeAt] = stamped.themeAt
        }
    }
```

- [ ] **Step 5: Run to verify they pass**

Run: `cd android && ./gradlew test --rerun-tasks`
Expected: whole suite green. `--rerun-tasks` matters: a plain `./gradlew test` reports `UP-TO-DATE` and proves nothing.

- [ ] **Step 6: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/FavStore.kt android/app/src/main/kotlin/net/vchub/r4dio/Profile.kt android/app/src/test/kotlin/net/vchub/r4dio/ProfileSyncTest.kt
git commit -m "remember which theme you chose"
```

---

### Task 4: One state holder for every screen

**Files:**
- Create: `android/app/src/main/kotlin/net/vchub/r4dio/ui/PlayerState.kt`
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/ui/PlayerStateTest.kt`

**Interfaces:**
- Consumes: the session-extras keys already published by `PlaybackService.kt:38-46` and `:168-181`.
- Produces (phases 2–5 all read this):

```kotlin
data class UiState(
    val stationName: String = "", val country: String = "", val codec: String = "",
    val isPlaying: Boolean = false, val isFav: Boolean = false,
    val scope: String = "all", val favCount: Int = 0, val hiddenCount: Int = 0,
    val playableCount: Int = 0, val catalogueSize: Int = 0,
    val catalogueGrowing: Boolean = false, val catalogLoaded: Boolean = false,
    val filterCountries: List<String> = emptyList(),
)
fun uiStateFromExtras(extras: Bundle, previous: UiState): UiState
```

**Read first:** `MainActivity.kt:212-222` (`readExtras`) and `MainActivity.kt:32-37` (`parseArtist`). Today these decode into eleven loose private fields; this task turns the same decoding into one immutable value that four screens can share. `parseArtist` exists because the service packs country/codec/bitrate into `MediaMetadata.artist` joined with `" · "` (`PlaybackService.kt:754-756`) — keep that contract, do not change the service's packing in this phase.

- [ ] **Step 1: Write the failing test**

```kotlin
package net.vchub.r4dio.ui

import android.os.Bundle
import net.vchub.r4dio.EXTRA_CATALOG_GROWING
import net.vchub.r4dio.EXTRA_CATALOG_SIZE
import net.vchub.r4dio.EXTRA_FAV
import net.vchub.r4dio.EXTRA_FILTER_COUNTRIES
import net.vchub.r4dio.EXTRA_SCOPE
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.robolectric.RobolectricTestRunner
import org.junit.runner.RunWith

@RunWith(RobolectricTestRunner::class)
class PlayerStateTest {
    @Test
    fun extras_become_state() {
        val b = Bundle().apply {
            putBoolean(EXTRA_FAV, true)
            putString(EXTRA_SCOPE, "favs")
            putInt(EXTRA_CATALOG_SIZE, 1286)
            putBoolean(EXTRA_CATALOG_GROWING, true)
            putStringArray(EXTRA_FILTER_COUNTRIES, arrayOf("UA", "PL"))
        }
        val s = uiStateFromExtras(b, UiState())
        assertTrue(s.isFav)
        assertEquals("favs", s.scope)
        assertEquals(1286, s.catalogueSize)
        assertTrue(s.catalogueGrowing)
        assertEquals(listOf("UA", "PL"), s.filterCountries)
    }

    // the service publishes extras and player metadata on separate channels, so
    // a extras-only update must not wipe the station name the metadata channel
    // set — that would blank the screen every time a count changed.
    @Test
    fun an_extras_update_keeps_the_station_already_shown() {
        val previous = UiState(stationName = "Radio Trek", country = "UA", isPlaying = true)
        val s = uiStateFromExtras(Bundle(), previous)
        assertEquals("Radio Trek", s.stationName)
        assertEquals("UA", s.country)
        assertTrue(s.isPlaying)
    }

    @Test
    fun a_missing_scope_reads_as_all() {
        assertEquals("all", uiStateFromExtras(Bundle(), UiState()).scope)
        assertFalse(uiStateFromExtras(Bundle(), UiState()).isFav)
    }
}
```

Robolectric is required because `Bundle` is an Android type. Add to `dependencies` in `android/app/build.gradle.kts`:

```kotlin
    testImplementation("org.robolectric:robolectric:4.14.1")
```

and inside the `android { }` block:

```kotlin
    testOptions {
        unitTests {
            isIncludeAndroidResources = true
        }
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd android && ./gradlew testDebugUnitTest --tests '*PlayerStateTest*'`
Expected: compile failure — `UiState` does not exist.

- [ ] **Step 3: Implement**

```kotlin
package net.vchub.r4dio.ui

import android.os.Bundle
import net.vchub.r4dio.EXTRA_CATALOG_GROWING
import net.vchub.r4dio.EXTRA_CATALOG_LOADED
import net.vchub.r4dio.EXTRA_CATALOG_SIZE
import net.vchub.r4dio.EXTRA_FAV
import net.vchub.r4dio.EXTRA_FAV_COUNT
import net.vchub.r4dio.EXTRA_FILTER_COUNTRIES
import net.vchub.r4dio.EXTRA_HIDDEN_COUNT
import net.vchub.r4dio.EXTRA_PLAYABLE_COUNT
import net.vchub.r4dio.EXTRA_SCOPE

/**
 * everything the screens read, in one immutable value. replaces the eleven
 * private fields MainActivity used to hold: four tabs cannot each keep their
 * own copy and stay in agreement.
 */
data class UiState(
    val stationName: String = "",
    val country: String = "",
    val codec: String = "",
    val isPlaying: Boolean = false,
    val isFav: Boolean = false,
    val scope: String = "all",
    val favCount: Int = 0,
    val hiddenCount: Int = 0,
    val playableCount: Int = 0,
    val catalogueSize: Int = 0,
    val catalogueGrowing: Boolean = false,
    val catalogLoaded: Boolean = false,
    val filterCountries: List<String> = emptyList(),
)

/**
 * [previous] is carried, not defaulted: extras and player metadata arrive on
 * two separate callbacks, so folding extras over a fresh UiState would blank
 * the station name every time a count changed.
 */
fun uiStateFromExtras(extras: Bundle, previous: UiState): UiState = previous.copy(
    isFav = extras.getBoolean(EXTRA_FAV, previous.isFav),
    scope = extras.getString(EXTRA_SCOPE) ?: previous.scope,
    favCount = extras.getInt(EXTRA_FAV_COUNT, previous.favCount),
    hiddenCount = extras.getInt(EXTRA_HIDDEN_COUNT, previous.hiddenCount),
    playableCount = extras.getInt(EXTRA_PLAYABLE_COUNT, previous.playableCount),
    catalogueSize = extras.getInt(EXTRA_CATALOG_SIZE, previous.catalogueSize),
    catalogueGrowing = extras.getBoolean(EXTRA_CATALOG_GROWING, previous.catalogueGrowing),
    catalogLoaded = extras.getBoolean(EXTRA_CATALOG_LOADED, previous.catalogLoaded),
    filterCountries = extras.getStringArray(EXTRA_FILTER_COUNTRIES)?.toList()
        ?: previous.filterCountries,
)
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd android && ./gradlew testDebugUnitTest --tests '*PlayerStateTest*'`
Expected: 3 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add android/app/build.gradle.kts android/app/src/main/kotlin/net/vchub/r4dio/ui/PlayerState.kt android/app/src/test/kotlin/net/vchub/r4dio/ui/PlayerStateTest.kt
git commit -m "one place that knows what the screen is showing"
```

---

### Task 5: The controller, owned once

**Files:**
- Create: `android/app/src/main/kotlin/net/vchub/r4dio/ui/PlayerConnection.kt`

**Interfaces:**
- Consumes: `UiState`, `uiStateFromExtras` (Task 4); the `CMD_*` constants at `PlaybackService.kt:30-36`.
- Produces:

```kotlin
class PlayerConnection(context: Context) {
    val state: StateFlow<UiState>
    fun connect()
    fun release()
    fun send(command: String)
}
```

**Read first:** `MainActivity.kt:171-210` (connect + listeners) and `MainActivity.kt:420-428` (release). Port that lifecycle verbatim — including the `released` guard, which exists because the `ListenableFuture` can resolve after `onDestroy` and would otherwise leak a controller. `parseArtist` at `MainActivity.kt:32-37` moves here unchanged.

This task has no unit test: it is Android-framework lifecycle glue whose behaviour is only meaningful against a live `MediaSession`. It is proven on the emulator in Task 8. State *decoding* — the part worth testing — was tested in Task 4.

- [ ] **Step 1: Implement**

```kotlin
package net.vchub.r4dio.ui

import android.content.ComponentName
import android.content.Context
import android.os.Bundle
import androidx.media3.common.Player
import androidx.media3.session.MediaController
import androidx.media3.session.SessionCommand
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.MoreExecutors
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import net.vchub.r4dio.PlaybackService

/** the service packs country/codec/bitrate into one artist string. */
internal fun parseArtist(artist: String): Pair<String, String> {
    val parts = artist.split(" · ")
    val country = parts.getOrNull(0).orEmpty()
    val codec = parts.getOrNull(1).orEmpty()
    return country to codec
}

/**
 * the single MediaController for the whole app. every tab reads [state]; none
 * of them build a controller of their own, so there is one connect/release
 * lifecycle rather than one per screen.
 */
class PlayerConnection(private val context: Context) {
    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

    private var controller: MediaController? = null

    // the future can resolve after release(); without this the callback would
    // hand us a controller nobody will ever close.
    @Volatile private var released = false

    private val playerListener = object : Player.Listener {
        override fun onIsPlayingChanged(isPlaying: Boolean) {
            _state.value = _state.value.copy(isPlaying = isPlaying)
        }

        override fun onMediaMetadataChanged(metadata: androidx.media3.common.MediaMetadata) {
            val (country, codec) = parseArtist(metadata.artist?.toString().orEmpty())
            _state.value = _state.value.copy(
                stationName = (metadata.station ?: metadata.title)?.toString().orEmpty(),
                country = country,
                codec = codec,
            )
        }
    }

    private val controllerListener = object : MediaController.Listener {
        override fun onExtrasChanged(controller: MediaController, extras: Bundle) {
            _state.value = uiStateFromExtras(extras, _state.value)
        }
    }

    fun connect() {
        released = false
        val token = SessionToken(context, ComponentName(context, PlaybackService::class.java))
        val future = MediaController.Builder(context, token)
            .setListener(controllerListener)
            .buildAsync()
        future.addListener({
            val c = runCatching { future.get() }.getOrNull() ?: return@addListener
            if (released) {
                c.release()
                return@addListener
            }
            controller = c
            c.addListener(playerListener)
            _state.value = uiStateFromExtras(c.sessionExtras, _state.value).copy(
                isPlaying = c.isPlaying,
            )
        }, MoreExecutors.directExecutor())
    }

    fun release() {
        released = true
        controller?.release()
        controller = null
    }

    fun send(command: String) {
        controller?.sendCustomCommand(SessionCommand(command, Bundle.EMPTY), Bundle.EMPTY)
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd android && ./gradlew assembleDebug`
Expected: `BUILD SUCCESSFUL`. Nothing uses this class yet.

- [ ] **Step 3: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/ui/PlayerConnection.kt
git commit -m "the player connection lives in one place"
```

---

### Task 6: Home, in Compose, identical

**Files:**
- Create: `android/app/src/main/kotlin/net/vchub/r4dio/ui/HomeScreen.kt`
- Create: `android/app/src/main/kotlin/net/vchub/r4dio/ui/Pill.kt`

**Interfaces:**
- Consumes: `UiState` (Task 4), `R4dioTokens` (Task 2), the existing pure formatters in `HomeState.kt` (`filterPillLabel`, `catalogueLabel`, `filterIsInForce`, `showsHiddenPill`, `isAllHiddenWarn`, `keepAwakeLabel`) — **reuse them, do not rewrite them**; they carry tested rules.
- Produces:

```kotlin
@Composable fun HomeScreen(state: UiState, onShuffle: () -> Unit, onToggle: () -> Unit,
                           onStar: () -> Unit, onScope: () -> Unit, onStop: () -> Unit,
                           onSync: () -> Unit, onClearFilter: () -> Unit)
@Composable fun Pill(text: String, on: Boolean, modifier: Modifier = Modifier, onClick: (() -> Unit)? = null)
```

**Read first:** `android/app/src/main/res/layout/activity_main.xml` in full — this is the layout being reproduced — and `MainActivity.kt:231-380` for what each piece renders and when. The font is `res/font/ibm_plex_mono`; in Compose use `FontFamily(Font(R.font.ibm_plex_mono))`.

The pill row order in portrait is: `catalogue_pill`, spacer, `filter_pill`, `hidden_pill`, `scope_pill`, `awake_pill`, `overlay_pill`.

- [ ] **Step 1: Implement the pill**

```kotlin
package net.vchub.r4dio.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import net.vchub.r4dio.R

val MonoFamily = FontFamily(Font(R.font.ibm_plex_mono))

/**
 * the status pill from the current release, unchanged in proportion: 9.5sp
 * mono, 0.1 letter spacing, 9dp/3dp padding. [on] is the amber state, off is
 * dim — the difference is what tells the user a filter is or is not in force.
 */
@Composable
fun Pill(
    text: String,
    on: Boolean,
    modifier: Modifier = Modifier,
    onClick: (() -> Unit)? = null,
) {
    val c = R4dioTokens.colors
    val fg = if (on) Color(c.accent) else Color(c.dim)
    Text(
        text = text,
        color = fg,
        fontSize = 9.5.sp,
        fontFamily = MonoFamily,
        letterSpacing = 0.1.sp,
        maxLines = 1,
        modifier = modifier
            .border(1.dp, fg.copy(alpha = 0.55f), RoundedCornerShape(999.dp))
            .background(
                if (on) Color(c.accent).copy(alpha = 0.10f) else Color.Transparent,
                RoundedCornerShape(999.dp),
            )
            .let { if (onClick != null) it.clickable { onClick() } else it }
            .padding(horizontal = 9.dp, vertical = 3.dp),
    )
}
```

- [ ] **Step 2: Implement Home**

Reproduce `activity_main.xml`'s structure: a `stage` column that fills the screen and is tappable anywhere for shuffle, holding the kicker row (`NOW PLAYING` + `LIVE`), the station name, the context line (`country · codec · fav`), the pill row, then the hero ring with `TAP ANYWHERE — SHUFFLE`; below the stage a four-button row (`PAUSE`/`STAR`/`scope`/`STOP`) and the sync bar.

```kotlin
package net.vchub.r4dio.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.weight
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import net.vchub.r4dio.catalogueLabel
import net.vchub.r4dio.filterIsInForce
import net.vchub.r4dio.filterPillLabel
import net.vchub.r4dio.showsHiddenPill

@Composable
fun HomeScreen(
    state: UiState,
    onShuffle: () -> Unit,
    onToggle: () -> Unit,
    onStar: () -> Unit,
    onScope: () -> Unit,
    onStop: () -> Unit,
    onSync: () -> Unit,
    onClearFilter: () -> Unit,
) {
    val c = R4dioTokens.colors
    Column(modifier = Modifier.fillMaxSize().background(Color(c.bg)).padding(10.dp)) {
        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .border(1.dp, Color(c.rule()), RoundedCornerShape(14.dp))
                .clickable { onShuffle() }
                .padding(14.dp),
        ) {
            Row(modifier = Modifier.fillMaxWidth()) {
                Text(
                    "NOW PLAYING",
                    color = Color(c.dim), fontSize = 10.sp,
                    fontFamily = MonoFamily, letterSpacing = 0.2.sp,
                )
                Box(modifier = Modifier.weight(1f))
                Text(
                    if (state.isPlaying) "LIVE" else "IDLE",
                    color = Color(if (state.isPlaying) c.ok else c.dim),
                    fontSize = 10.sp, fontFamily = MonoFamily, letterSpacing = 0.2.sp,
                )
            }
            Text(
                state.stationName.ifBlank { "r4dio" },
                color = Color(c.peak), fontSize = 28.sp, fontWeight = FontWeight.Bold,
                fontFamily = MonoFamily, maxLines = 2,
                modifier = Modifier.padding(top = 8.dp),
            )
            Text(
                listOf(state.country, state.codec)
                    .filter { it.isNotBlank() }
                    .joinToString(" · ") + if (state.isFav) " · ★ favourite" else " · ☆ not saved",
                color = Color(c.mute()), fontSize = 11.sp, fontFamily = MonoFamily,
                modifier = Modifier.padding(top = 6.dp),
            )
            PillRow(state, onClearFilter)
            Box(modifier = Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    Box(
                        modifier = Modifier
                            .size(240.dp)
                            .border(2.dp, Color(c.accent), RoundedCornerShape(999.dp)),
                    )
                    Text(
                        "TAP ANYWHERE — SHUFFLE",
                        color = Color(c.accent), fontSize = 15.sp,
                        fontFamily = MonoFamily, letterSpacing = 0.15.sp,
                        modifier = Modifier.padding(top = 18.dp),
                    )
                }
            }
        }
        ButtonRow(state, onToggle, onStar, onScope, onStop)
        SyncBar(onSync)
    }
}

@Composable
private fun PillRow(state: UiState, onClearFilter: () -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(top = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        val count = catalogueLabel(state.catalogueSize, state.catalogueGrowing)
        if (count.isNotEmpty()) {
            Text(
                count,
                color = Color(R4dioTokens.colors.dim), fontSize = 9.5.sp,
                fontFamily = MonoFamily, letterSpacing = 0.1.sp, maxLines = 1,
            )
        }
        Box(modifier = Modifier.weight(1f))
        filterPillLabel(state.filterCountries, state.scope)?.let { label ->
            Pill(label, on = filterIsInForce(state.filterCountries, state.scope), onClick = onClearFilter)
        }
        if (showsHiddenPill(state.hiddenCount, state.scope)) {
            Pill("${state.hiddenCount} COUNTRIES HIDDEN", on = true)
        }
        Pill(
            if (state.scope == "favs") {
                if (state.favCount > 0) "FAVOURITES ONLY · ${state.favCount}" else "FAVOURITES ONLY"
            } else "ALL STATIONS",
            on = state.scope == "favs",
        )
    }
}
```

`ButtonRow` and `SyncBar` follow the same construction — four equal-weight bordered boxes with the glyph over a caption (`⏸`/`★`/`≡`/`■` matching today's icons), and a full-width bordered row reading `SYNC` with `link desktop ↔ phone` on the right. Use `R4dioTokens.colors.err` for `STOP`.

`c.rule()` and `c.mute()` above are placeholders for two colours today's XML has but the 9-slot palette does not (`rule`, `mute`). Add them as derived values in `Palette.kt` rather than new slots, so the 14 palettes stay a faithful copy of the CLI:

```kotlin
/** the hairline between panels — the CLI has no such role, so it is derived. */
fun Palette.rule(): Long = blend(bg, dim, 0.55f)

/** secondary text: dimmer than fg, brighter than dim. */
fun Palette.mute(): Long = blend(dim, fg, 0.45f)

private fun blend(a: Long, b: Long, t: Float): Long {
    fun ch(shift: Int): Long {
        val av = (a shr shift) and 0xFF
        val bv = (b shr shift) and 0xFF
        return (av + ((bv - av) * t)).toLong() and 0xFF
    }
    return (0xFFL shl 24) or (ch(16) shl 16) or (ch(8) shl 8) or ch(0)
}
```

- [ ] **Step 3: Test the derived colours**

Append to `PaletteTest.kt`:

```kotlin
    // rule and mute are derived, not copied from the cli — they must stay
    // opaque and must sit between the two colours they blend.
    @Test
    fun derived_colours_are_opaque_for_every_theme() {
        THEME_SLUGS.mapNotNull { paletteFor(it) }.forEach { p ->
            assertEquals(0xFF000000, p.rule() and 0xFF000000)
            assertEquals(0xFF000000, p.mute() and 0xFF000000)
        }
    }
```

Run: `cd android && ./gradlew testDebugUnitTest --tests '*PaletteTest*'`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/ui/HomeScreen.kt android/app/src/main/kotlin/net/vchub/r4dio/ui/Pill.kt android/app/src/main/kotlin/net/vchub/r4dio/ui/Palette.kt android/app/src/test/kotlin/net/vchub/r4dio/ui/PaletteTest.kt
git commit -m "home, rebuilt on the new foundation"
```

---

### Task 7: The four tabs and the mini-player

**Files:**
- Create: `android/app/src/main/kotlin/net/vchub/r4dio/ui/R4dioApp.kt`
- Create: `android/app/src/main/kotlin/net/vchub/r4dio/ui/Placeholder.kt`
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/ui/TabsTest.kt`

**Interfaces:**
- Consumes: `HomeScreen` (Task 6), `UiState` (Task 4), `R4dioTheme` (Task 2).
- Produces:

```kotlin
enum class Tab { HOME, CATALOG, LIBRARY, SETTINGS }
fun showsMiniPlayer(tab: Tab, stationName: String): Boolean
@Composable fun R4dioApp(state: UiState, themeSlug: String, send: (String) -> Unit, onOpenSync: () -> Unit)
```

**Read first:** the handoff's navigation section — four tabs `⇄ Home`, `⌕ Catalog`, `★ Library`, `⚙ Settings`; Now Playing is not a tab but a strip above the tabs, visible whenever something is playing.

- [ ] **Step 1: Write the failing test**

```kotlin
package net.vchub.r4dio.ui

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class TabsTest {
    // the strip is what carries now-playing on every tab except home, where
    // the station is already the biggest thing on the screen.
    @Test
    fun the_mini_player_shows_on_every_tab_but_home() {
        assertFalse(showsMiniPlayer(Tab.HOME, "Radio Trek"))
        assertTrue(showsMiniPlayer(Tab.CATALOG, "Radio Trek"))
        assertTrue(showsMiniPlayer(Tab.LIBRARY, "Radio Trek"))
        assertTrue(showsMiniPlayer(Tab.SETTINGS, "Radio Trek"))
    }

    // nothing playing yet: a strip naming no station is just a bar of noise.
    @Test
    fun the_mini_player_hides_when_nothing_is_playing() {
        assertFalse(showsMiniPlayer(Tab.CATALOG, ""))
    }

    @Test
    fun home_is_the_first_tab() {
        assertEquals(Tab.HOME, Tab.entries.first())
    }
}
```

Add `import org.junit.Assert.assertEquals`.

- [ ] **Step 2: Run to verify it fails**

Run: `cd android && ./gradlew testDebugUnitTest --tests '*TabsTest*'`
Expected: compile failure — `Tab` does not exist.

- [ ] **Step 3: Implement**

```kotlin
package net.vchub.r4dio.ui

enum class Tab { HOME, CATALOG, LIBRARY, SETTINGS }

/**
 * home already shows the station at full size, so the strip would be a second
 * copy of the same fact. everywhere else it is the only way back to it.
 */
fun showsMiniPlayer(tab: Tab, stationName: String): Boolean =
    tab != Tab.HOME && stationName.isNotBlank()
```

Then `R4dioApp`: a `Column` with the current tab's content in a weighted `Box`, the mini-player strip when `showsMiniPlayer`, and a bottom row of four equal-weight tab targets. Selected tab uses `accent`, unselected uses `dim`. Tab state is `rememberSaveable { mutableStateOf(Tab.HOME) }` so a rotation does not throw the user back to Home. Wrap the whole tree in `R4dioTheme(themeSlug)`.

Catalog/Library/Settings render `Placeholder("CATALOG", "search and filters land here")` etc. — a centred mono label in `dim` on `bg`. Settings' placeholder gets a tappable `OPEN SYNC` pill calling `onOpenSync`, so the existing `SyncActivity` stays reachable until phase 5 retires it.

- [ ] **Step 4: Run to verify it passes**

Run: `cd android && ./gradlew testDebugUnitTest --tests '*TabsTest*'`
Expected: 3 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/ui/R4dioApp.kt android/app/src/main/kotlin/net/vchub/r4dio/ui/Placeholder.kt android/app/src/test/kotlin/net/vchub/r4dio/ui/TabsTest.kt
git commit -m "four tabs, and the station always one tap away"
```

---

### Task 8: MainActivity becomes Compose

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/MainActivity.kt` (replaced wholesale)
- Delete: `android/app/src/main/res/layout/activity_main.xml`
- Delete: `android/app/src/main/res/layout-land/activity_main.xml`

**Interfaces:**
- Consumes: `PlayerConnection` (Task 5), `R4dioApp` (Task 7), `FavStore.theme` (Task 3).

**Read first:** the whole of `MainActivity.kt` — everything it does must still happen: the notification-permission request (`:61-64`), keep-awake (`:96-97`, `:107-116`), `StationToast.appIsInForeground` (`:146`, `:164`), the overlay-permission pill, the auto-shuffle on first connect (`:205-207`), and the clear-filter confirmation dialog (`:83-95`).

**The landscape layout disappears with the XML.** Compose has no `layout-land`; the same tree lays out for both orientations. This is deliberate — it also retires the crash fixed in `8dcf3be` and the missing-pill gap noted in `7c1e368`, because there is no longer a second layout to fall out of sync.

- [ ] **Step 1: Rewrite the activity**

```kotlin
class MainActivity : ComponentActivity() {
    private val connection by lazy { PlayerConnection(this) }
    private val favStore by lazy { FavStore(this) }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            val state by connection.state.collectAsStateWithLifecycle()
            val synced by favStore.theme.collectAsStateWithLifecycle(initialValue = "")
            R4dioApp(
                state = state,
                themeSlug = resolveTheme(synced, DEFAULT_THEME),
                send = { connection.send(it) },
                onOpenSync = { startActivity(Intent(this, SyncActivity::class.java)) },
            )
        }
        when (needsNotificationPermission()) {
            true -> requestPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
            false -> connection.connect()
        }
    }

    override fun onResume() {
        super.onResume()
        StationToast.appIsInForeground = true
        lifecycleScope.launch { applyKeepAwake(favStore.currentKeepAwake()) }
    }

    override fun onPause() {
        super.onPause()
        StationToast.appIsInForeground = false
    }

    override fun onDestroy() {
        connection.release()
        super.onDestroy()
    }
}
```

Keep `applyKeepAwake`, `needsNotificationPermission` and `requestPermission` exactly as they are today. The clear-filter dialog moves into Compose as an `AlertDialog` composable driven by a `remember { mutableStateOf(false) }`, using the same strings (`filter_clear_title`, `filter_clear_body`, `filter_clear_yes`, `filter_clear_no`) — do not reword them.

- [ ] **Step 2: Delete the layouts**

```bash
git rm android/app/src/main/res/layout/activity_main.xml android/app/src/main/res/layout-land/activity_main.xml
```

- [ ] **Step 3: Build, install, and compare against the current release**

```bash
cd android && ./gradlew test --rerun-tasks && ./gradlew assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
adb shell am start -n net.vchub.r4dio/.MainActivity
```

- [ ] **Step 4: Real-path proof required**

All four are mandatory in the task report:

1. **Home is unchanged.** Screenshot Home and compare against the current release side by side. Station name, context line, pill row, hero ring, four buttons, sync bar — same content, same order, same proportions. Any difference is a defect, not a refinement.
2. **Rotation.** Rotate to landscape (`adb shell settings put system accelerometer_rotation 0; adb shell settings put system user_rotation 1`), screenshot, and confirm **no crash** and that the pills are present. Then rotate back and confirm the selected tab survived.
3. **Themes.** Set the stored theme to `hifi-paper` (the light one) and screenshot Home; then `nord`; then back to `amber-crt`. Confirm amber-crt matches the current release exactly. **Read the colours in the screenshot against the palette table** — a screenshot proves layout, not contrast.
4. **Tabs.** Tap each of the four tabs, screenshot each, confirm the mini-player strip appears on Catalog/Library/Settings when a station is playing and is absent on Home.

Do **not** start playback to produce a station name; a station is already playing from the cache at launch, or the name can be read from `logcat -s r4dio`.

- [ ] **Step 5: Report the APK size**

Run: `ls -la android/app/build/outputs/apk/debug/app-debug.apk` and compare against the 6.6 MB baseline.

- [ ] **Step 6: Commit**

```bash
git add -A android/
git commit -m "the app you can grow into: tabs, themes, and the same shuffle"
```

---

## Verification after all tasks (controller, not subagents)

1. Whole-branch review before anything is pushed.
2. On the emulator: Home is visually identical to v1.18.4 in `amber-crt`; all 14 themes apply without a restart; `hifi-paper` is readable (dark text on light, no dark-on-dark).
3. Rotation does not crash and does not lose the selected tab.
4. The widget still updates and the media notification still shows the right station — neither was touched, and both must be confirmed untouched.
5. APK growth reported against 6.6 MB.
6. Release notes: the app has tabs now, and the theme you choose on the desktop finally reaches the phone.
