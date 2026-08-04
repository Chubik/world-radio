# Android Widget Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild the home-screen widget to the Amber CRT design — a 4×1 layout with the logo tile, station name, LIVE indicator, metadata line and three controls, plus a compact 2×1 layout — replacing today's single unstyled row.

**Architecture:** The widget's state model grows from two values (`station`, `isPlaying`) to five, still carried in the existing `shared_prefs/widget.xml` and still pushed from `PlaybackService` — no new state store, no session-extras rewiring. Two layouts share one `render()` path; the layout is chosen per widget instance from the size the launcher reports. The decision of *which* layout and *what* each field shows is extracted into pure functions so it is unit-testable without an Android harness.

**Tech Stack:** Kotlin, `AppWidgetProvider` + `RemoteViews`, XML layouts (no Compose, no Glance), SharedPreferences, JUnit 4 under `android/app/src/test/`.

## Global Constraints

- **minSdk is 26.** `RemoteViews(Map<SizeF, RemoteViews>)`, `setColorStateList`, `setViewLayoutMargin`, `previewLayout` and `targetCellWidth`/`targetCellHeight` are **API 31+**. Anything API 31+ must either be behind a `Build.VERSION.SDK_INT` guard or be an attribute older launchers safely ignore. `android:fontFamily="@font/…"` in a RemoteViews layout **is** supported from API 26 — use it.
- **Do not simplify `onReceive`.** Its `MediaController` + `Player.Listener` + 15-second timeout dance (`RadioWidgetProvider.kt:30-80`) is load-bearing: it exists because releasing the controller immediately kills the service before playback buffers (commits `d4c5634`, `0bc8f23`). Adding new actions to the `when` is fine; changing the controller lifecycle is not.
- **The widget is push-driven.** `updatePeriodMillis="0"` means nothing ever corrects a stale widget on its own. Every new field must have a matching `refresh()` call in `PlaybackService`, or it will show stale data indefinitely.
- **The private RU/BY station ban must never be surfaced, counted, or hinted at.** It lives in two private vals in `Catalog.kt`. The widget shows only the current station's own fields.
- Colour tokens are fixed (`res/values/colors.xml`): `olive` **only** for LIVE; `amber_hi` for the primary action; `on_amber` (`#241A08`) for text/glyphs *on* an amber fill; `panel` for surfaces; `rule` for 1dp borders; `bright` for the station name; `mute` for the metadata line; `dim` for quiet labels.
- Typography: `@font/ibm_plex_mono` on every text view, matching the home screen. The station name uses `fontWeight` 600–700; the metadata line 400.
- Copy: SCREAMING CAPS for button labels and the LIVE chip, `·` (U+00B7) as separator, British spelling. Metadata line is the existing `"US · MP3 · 128k"` shape.
- Code, comments and logs in English, lowercase. Comments only where the "why" is non-obvious. No `else if` — use `when`. No AI/assistant references anywhere. Commit subjects are the public changelog, written for users.
- Do not touch `crates/` (the Rust CLI), `Catalog.kt`, `shuffle()`, or `playPick()`'s playback logic (adding a `refresh()` call is fine).

## Background the implementer needs

**What exists today.** `RadioWidgetProvider.kt` is 108 lines and is the entire widget. `onUpdate` reads two keys from `getSharedPreferences("widget", MODE_PRIVATE)`; `render()` sets one text view, swaps one icon and wires two PendingIntents; `refresh(context, station, isPlaying)` writes those two keys and re-renders. The single layout `res/layout/widget_radio.xml` is a vertical `LinearLayout` with a station `TextView` and a row of two square, unrounded buttons. Provider metadata `res/xml/widget_radio_info.xml` declares `minWidth="180dp" minHeight="72dp" resizeMode="horizontal"`.

**What the design asks for** (`r4dio - Android Assets Handoff.html`, Priority 3):
- **4×1** — logo tile on the left, station name + `●LIVE` chip on the first line, metadata line below, three controls on the right: star (ghost), shuffle (ghost), play/pause (**amber, primary**).
- **2×1** — small logo tile, station name, `●LIVE` under it, then a row: `SHUFFLE` (**amber, primary**, icon + label) and a square play/pause ghost button.
- Widget background: translucent panel, 20dp corners. Ghost buttons: 9dp corners, 1dp `rule` border. Primary button: `amber_hi` fill, `on_amber` glyph.

**Assets: nothing to import.** `ic_shuffle.xml`, `ic_star.xml`, `ic_star_outline.xml`, `ic_play.xml`, `ic_pause.xml` already exist in `res/drawable/` and their path data is byte-identical to the handoff SVGs. The logo tile reuses `res/drawable/ic_launcher_foreground.xml`, which is the same `▌4` mark the design uses. **Do not add new vector assets.**

**Deliberately out of scope: the track title.** The design mock shows "Chuck Wayne — What A Difference…", but the app does not read ICY metadata at all (no `IcyInfo`, no `onMetadata` anywhere in the codebase). The second line shows the existing metadata string instead — `pick.country · pick.codec · bitrate`, the same value `PlaybackService.kt:478-480` already builds for the notification. Do not add ICY support in this plan.

**Deliberately out of scope: the EQ bars.** `RemoteViews` cannot animate, and a frozen equaliser reads as a hung app.

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `android/app/src/main/kotlin/net/vchub/r4dio/WidgetState.kt` | **Create** | Pure, testable decisions: which layout for a given size, and what each field renders. No Android imports beyond none at all. |
| `android/app/src/test/kotlin/net/vchub/r4dio/WidgetStateTest.kt` | **Create** | Unit tests for those decisions. |
| `android/app/src/main/res/layout/widget_radio.xml` | Rewrite | The 4×1 layout. |
| `android/app/src/main/res/layout/widget_radio_small.xml` | **Create** | The 2×1 layout. |
| `android/app/src/main/res/drawable/widget_bg.xml` | Modify | Panel fill + 20dp corners (currently `bg` fill + 22dp). |
| `android/app/src/main/res/drawable/widget_btn_ghost.xml` | **Create** | 9dp corners, `panel` fill, 1dp `rule` stroke. |
| `android/app/src/main/res/drawable/widget_btn_primary.xml` | **Create** | 9dp corners, `amber_hi` fill. |
| `android/app/src/main/res/drawable/widget_tile_bg.xml` | **Create** | The logo tile's recessed background: `bg` fill, 9dp corners, 1dp `rule` stroke. |
| `android/app/src/main/res/values/strings.xml` | Modify | `widget_live`, `widget_shuffle`, `widget_idle` and content descriptions. |
| `android/app/src/main/res/xml/widget_radio_info.xml` | Modify | Sizing metadata, `resizeMode`, `description`, API-31 target cells. |
| `android/app/src/main/kotlin/net/vchub/r4dio/RadioWidgetProvider.kt` | Modify | Widened state, per-size layout choice, new star action, tap-to-open. |
| `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt` | Modify | Call `refresh()` with the widened state, including from `CMD_STAR` and `CMD_SCOPE`. |

**Why a new `WidgetState.kt`:** the codebase's recurring defect is correct logic that is never reached, and `PlaybackService`/`RadioWidgetProvider` have no test harness. Extracting the decisions as pure functions — the same shape as the existing `HomeState.kt`, `catalogIsStale`, `parseArtist` — makes them genuinely testable. Keep the file small.

---

### Task 1: Pure widget-state decisions

**Files:**
- Create: `android/app/src/main/kotlin/net/vchub/r4dio/WidgetState.kt`
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/WidgetStateTest.kt`

**Interfaces:**
- Consumes: nothing.
- Produces, relied on by Tasks 4 and 5:
  - `const val WIDGET_SMALL_MAX_WIDTH_DP = 250`
  - `fun usesCompactLayout(widthDp: Int): Boolean`
  - `fun widgetStationLabel(station: String, idle: String): String`
  - `fun widgetMetaLabel(country: String, codec: String, bitrate: Int): String`

- [ ] **Step 1: Write the failing test**

Create `android/app/src/test/kotlin/net/vchub/r4dio/WidgetStateTest.kt`:

```kotlin
package net.vchub.r4dio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class WidgetStateTest {
    @Test
    fun narrow_widgets_use_the_compact_layout() {
        assertTrue(usesCompactLayout(110))
        assertTrue(usesCompactLayout(WIDGET_SMALL_MAX_WIDTH_DP))
    }

    @Test
    fun wide_widgets_use_the_full_layout() {
        assertFalse(usesCompactLayout(WIDGET_SMALL_MAX_WIDTH_DP + 1))
        assertFalse(usesCompactLayout(320))
    }

    // a launcher that reports nothing must not collapse to the cramped layout
    @Test
    fun a_zero_width_report_falls_back_to_the_full_layout() {
        assertFalse(usesCompactLayout(0))
    }

    @Test
    fun station_label_shows_the_station_when_there_is_one() {
        assertEquals("Radio Paradise", widgetStationLabel("Radio Paradise", "— idle —"))
    }

    @Test
    fun station_label_falls_back_to_idle_when_blank() {
        assertEquals("— idle —", widgetStationLabel("", "— idle —"))
        assertEquals("— idle —", widgetStationLabel("   ", "— idle —"))
    }

    @Test
    fun meta_label_joins_the_parts_it_has() {
        assertEquals("US · MP3 · 128k", widgetMetaLabel("US", "MP3", 128))
    }

    // the api leaves any of these empty on plenty of stations
    @Test
    fun meta_label_skips_missing_parts_without_a_dangling_separator() {
        assertEquals("MP3 · 128k", widgetMetaLabel("", "MP3", 128))
        assertEquals("US · 128k", widgetMetaLabel("US", "", 128))
        assertEquals("US · MP3", widgetMetaLabel("US", "MP3", 0))
        assertEquals("US", widgetMetaLabel("US", "", 0))
        assertEquals("", widgetMetaLabel("", "", 0))
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd android && ./gradlew testDebugUnitTest --tests "net.vchub.r4dio.WidgetStateTest"`

Expected: **compilation failure** — `unresolved reference: usesCompactLayout`. That is the correct failure.

- [ ] **Step 3: Write the minimal implementation**

Create `android/app/src/main/kotlin/net/vchub/r4dio/WidgetState.kt`:

```kotlin
package net.vchub.r4dio

/**
 * widget decisions kept free of android types so they can be unit tested —
 * RemoteViews and AppWidgetManager have no test harness in this project.
 */

/** widths at or below this get the stacked 2x1 layout; above it, the 4x1 row. */
const val WIDGET_SMALL_MAX_WIDTH_DP = 250

/**
 * a launcher that has not measured the widget yet reports 0. treat that as wide:
 * the full layout degrades legibly when squeezed, the compact one wastes a wide cell.
 */
fun usesCompactLayout(widthDp: Int): Boolean = widthDp in 1..WIDGET_SMALL_MAX_WIDTH_DP

fun widgetStationLabel(station: String, idle: String): String =
    station.ifBlank { idle }

fun widgetMetaLabel(country: String, codec: String, bitrate: Int): String =
    listOf(country, codec, if (bitrate > 0) "${bitrate}k" else "")
        .filter { it.isNotBlank() }
        .joinToString(" · ")
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd android && ./gradlew testDebugUnitTest --tests "net.vchub.r4dio.WidgetStateTest"`

Expected: PASS, 7 tests. **Verify from the JUnit XML, not the console:** read `android/app/build/test-results/testDebugUnitTest/TEST-net.vchub.r4dio.WidgetStateTest.xml` and confirm `tests="7" failures="0" errors="0" skipped="0"`. If gradle reports `UP-TO-DATE` and the XML is missing or stale, re-run with `--rerun-tasks`.

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/WidgetState.kt android/app/src/test/kotlin/net/vchub/r4dio/WidgetStateTest.kt
git commit -m "feat(android): decide what the widget shows at each size"
```

---

### Task 2: Drawables and strings

**Files:**
- Modify: `android/app/src/main/res/drawable/widget_bg.xml`
- Create: `android/app/src/main/res/drawable/widget_btn_ghost.xml`
- Create: `android/app/src/main/res/drawable/widget_btn_primary.xml`
- Create: `android/app/src/main/res/drawable/widget_tile_bg.xml`
- Modify: `android/app/src/main/res/values/strings.xml`

**Interfaces:**
- Consumes: nothing.
- Produces, used by Task 3's layouts: `@drawable/widget_bg`, `@drawable/widget_btn_ghost`, `@drawable/widget_btn_primary`, `@drawable/widget_tile_bg`, and the strings `widget_live`, `widget_shuffle`, `widget_idle`, `widget_cd_star`, `widget_cd_shuffle`, `widget_cd_toggle`.

- [ ] **Step 1: Update the widget background**

Replace the whole of `android/app/src/main/res/drawable/widget_bg.xml` with:

```xml
<?xml version="1.0" encoding="utf-8"?>
<shape xmlns:android="http://schemas.android.com/apk/res/android" android:shape="rectangle">
    <solid android:color="@color/panel" />
    <corners android:radius="20dp" />
    <stroke android:width="1dp" android:color="@color/rule" />
</shape>
```

Two deliberate changes from the current file: the fill moves from `bg` to `panel` (every other surface in the app is `panel`; `bg` is the screen behind them), and the radius goes 22dp → 20dp to match the design's widget corner.

- [ ] **Step 2: Create the three new shapes**

`android/app/src/main/res/drawable/widget_btn_ghost.xml`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<shape xmlns:android="http://schemas.android.com/apk/res/android" android:shape="rectangle">
    <solid android:color="@android:color/transparent" />
    <corners android:radius="9dp" />
    <stroke android:width="1dp" android:color="@color/rule" />
</shape>
```

`android/app/src/main/res/drawable/widget_btn_primary.xml`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<shape xmlns:android="http://schemas.android.com/apk/res/android" android:shape="rectangle">
    <solid android:color="@color/amber_hi" />
    <corners android:radius="9dp" />
</shape>
```

`android/app/src/main/res/drawable/widget_tile_bg.xml`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<shape xmlns:android="http://schemas.android.com/apk/res/android" android:shape="rectangle">
    <solid android:color="@color/bg" />
    <corners android:radius="9dp" />
    <stroke android:width="1dp" android:color="@color/rule" />
</shape>
```

The tile is the one place `bg` is correct: the design shows the logo recessed into a darker well inside the panel.

- [ ] **Step 3: Add the strings**

In `android/app/src/main/res/values/strings.xml`, add alongside the existing `home_*` entries:

```xml
    <string name="widget_live">LIVE</string>
    <string name="widget_shuffle">SHUFFLE</string>
    <string name="widget_idle">— idle —</string>
    <string name="widget_cd_star">star this station</string>
    <string name="widget_cd_shuffle">shuffle</string>
    <string name="widget_cd_toggle">play or pause</string>
</resources>
```

(Add the five `<string>` lines before the closing `</resources>`; do not duplicate that tag.) The `widget_idle` value matches the home screen's existing `home_idle`, so an idle widget and an idle screen read the same. The three `widget_cd_*` strings are content descriptions — the current widget has one hardcoded English literal (`android:contentDescription="play"`), which this replaces.

- [ ] **Step 4: Verify resources compile**

Run: `cd android && ./gradlew assembleDebug`
Expected: BUILD SUCCESSFUL. A malformed shape or a duplicated `</resources>` fails here, before any Kotlin depends on it.

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/res/drawable/widget_bg.xml android/app/src/main/res/drawable/widget_btn_ghost.xml android/app/src/main/res/drawable/widget_btn_primary.xml android/app/src/main/res/drawable/widget_tile_bg.xml android/app/src/main/res/values/strings.xml
git commit -m "feat(android): widget surfaces and labels in the app's own style"
```

---

### Task 3: The two layouts

**Files:**
- Modify (rewrite): `android/app/src/main/res/layout/widget_radio.xml`
- Create: `android/app/src/main/res/layout/widget_radio_small.xml`
- Modify: `android/app/src/main/res/xml/widget_radio_info.xml`

**Interfaces:**
- Consumes: the drawables and strings from Task 2.
- Produces, relied on by Tasks 4 and 5 — **these ids must exist in BOTH layouts with these exact names**, because one `render()` path writes to both:
  - `@+id/widget_root` — the tappable body
  - `@+id/widget_station` — station name `TextView`
  - `@+id/widget_live` — the `●LIVE` chip `TextView`
  - `@+id/widget_meta` — the `US · MP3 · 128k` line `TextView`
  - `@+id/widget_star` — star `ImageView`
  - `@+id/widget_shuffle` — shuffle control (an `ImageView` in 4×1, a `LinearLayout` in 2×1)
  - `@+id/widget_toggle` — play/pause `ImageView`

- [ ] **Step 1: Rewrite the 4×1 layout**

Replace the whole of `android/app/src/main/res/layout/widget_radio.xml`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<LinearLayout xmlns:android="http://schemas.android.com/apk/res/android"
    android:id="@+id/widget_root"
    android:layout_width="match_parent"
    android:layout_height="match_parent"
    android:orientation="horizontal"
    android:gravity="center_vertical"
    android:background="@drawable/widget_bg"
    android:padding="14dp">

    <FrameLayout
        android:layout_width="52dp"
        android:layout_height="52dp"
        android:background="@drawable/widget_tile_bg">
        <ImageView
            android:layout_width="40dp"
            android:layout_height="40dp"
            android:layout_gravity="center"
            android:src="@drawable/ic_launcher_foreground"
            android:contentDescription="@null" />
    </FrameLayout>

    <LinearLayout
        android:layout_width="0dp"
        android:layout_height="wrap_content"
        android:layout_weight="1"
        android:layout_marginStart="14dp"
        android:orientation="vertical">

        <LinearLayout
            android:layout_width="match_parent"
            android:layout_height="wrap_content"
            android:orientation="horizontal"
            android:gravity="center_vertical">
            <TextView
                android:id="@+id/widget_station"
                android:layout_width="0dp"
                android:layout_height="wrap_content"
                android:layout_weight="1"
                android:text="@string/widget_idle"
                android:textColor="@color/bright"
                android:textSize="14sp"
                android:fontFamily="@font/ibm_plex_mono"
                android:textFontWeight="600"
                android:maxLines="1"
                android:ellipsize="end" />
            <TextView
                android:id="@+id/widget_live"
                android:layout_width="wrap_content"
                android:layout_height="wrap_content"
                android:layout_marginStart="9dp"
                android:text="@string/widget_live"
                android:textColor="@color/olive"
                android:textSize="10sp"
                android:fontFamily="@font/ibm_plex_mono"
                android:textFontWeight="600"
                android:letterSpacing="0.08"
                android:visibility="gone" />
        </LinearLayout>

        <TextView
            android:id="@+id/widget_meta"
            android:layout_width="match_parent"
            android:layout_height="wrap_content"
            android:layout_marginTop="3dp"
            android:textColor="@color/mute"
            android:textSize="11sp"
            android:fontFamily="@font/ibm_plex_mono"
            android:maxLines="1"
            android:ellipsize="end" />
    </LinearLayout>

    <ImageView
        android:id="@+id/widget_star"
        android:layout_width="38dp"
        android:layout_height="38dp"
        android:layout_marginStart="10dp"
        android:padding="9dp"
        android:background="@drawable/widget_btn_ghost"
        android:src="@drawable/ic_star_outline"
        android:contentDescription="@string/widget_cd_star" />

    <ImageView
        android:id="@+id/widget_shuffle"
        android:layout_width="38dp"
        android:layout_height="38dp"
        android:layout_marginStart="8dp"
        android:padding="9dp"
        android:background="@drawable/widget_btn_ghost"
        android:src="@drawable/ic_shuffle"
        android:contentDescription="@string/widget_cd_shuffle" />

    <ImageView
        android:id="@+id/widget_toggle"
        android:layout_width="44dp"
        android:layout_height="44dp"
        android:layout_marginStart="8dp"
        android:padding="11dp"
        android:background="@drawable/widget_btn_primary"
        android:src="@drawable/ic_play"
        android:contentDescription="@string/widget_cd_toggle" />
</LinearLayout>
```

Note `android:textFontWeight` is API 28+; on 26–27 it is ignored and the font renders at its regular weight, which is an acceptable degradation (the family is still IBM Plex Mono). Do **not** use `android:textStyle="bold"` instead — it synthesises a fake bold over the bundled family.

The tile's `ImageView` uses `contentDescription="@null"` because it is decorative; TalkBack should not announce it.

- [ ] **Step 2: Create the 2×1 layout**

Create `android/app/src/main/res/layout/widget_radio_small.xml`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<LinearLayout xmlns:android="http://schemas.android.com/apk/res/android"
    android:id="@+id/widget_root"
    android:layout_width="match_parent"
    android:layout_height="match_parent"
    android:orientation="vertical"
    android:background="@drawable/widget_bg"
    android:padding="13dp">

    <LinearLayout
        android:layout_width="match_parent"
        android:layout_height="wrap_content"
        android:orientation="horizontal"
        android:gravity="center_vertical">

        <FrameLayout
            android:layout_width="34dp"
            android:layout_height="34dp"
            android:background="@drawable/widget_tile_bg">
            <ImageView
                android:layout_width="26dp"
                android:layout_height="26dp"
                android:layout_gravity="center"
                android:src="@drawable/ic_launcher_foreground"
                android:contentDescription="@null" />
        </FrameLayout>

        <LinearLayout
            android:layout_width="0dp"
            android:layout_height="wrap_content"
            android:layout_weight="1"
            android:layout_marginStart="9dp"
            android:orientation="vertical">
            <TextView
                android:id="@+id/widget_station"
                android:layout_width="match_parent"
                android:layout_height="wrap_content"
                android:text="@string/widget_idle"
                android:textColor="@color/bright"
                android:textSize="12.5sp"
                android:fontFamily="@font/ibm_plex_mono"
                android:textFontWeight="600"
                android:maxLines="1"
                android:ellipsize="end" />
            <TextView
                android:id="@+id/widget_live"
                android:layout_width="wrap_content"
                android:layout_height="wrap_content"
                android:text="@string/widget_live"
                android:textColor="@color/olive"
                android:textSize="9sp"
                android:fontFamily="@font/ibm_plex_mono"
                android:textFontWeight="600"
                android:letterSpacing="0.08"
                android:visibility="gone" />
        </LinearLayout>
    </LinearLayout>

    <TextView
        android:id="@+id/widget_meta"
        android:layout_width="match_parent"
        android:layout_height="wrap_content"
        android:layout_marginTop="2dp"
        android:textColor="@color/mute"
        android:textSize="10sp"
        android:fontFamily="@font/ibm_plex_mono"
        android:maxLines="1"
        android:ellipsize="end"
        android:visibility="gone" />

    <LinearLayout
        android:layout_width="match_parent"
        android:layout_height="wrap_content"
        android:layout_marginTop="10dp"
        android:orientation="horizontal">

        <LinearLayout
            android:id="@+id/widget_shuffle"
            android:layout_width="0dp"
            android:layout_height="36dp"
            android:layout_weight="1"
            android:orientation="horizontal"
            android:gravity="center"
            android:background="@drawable/widget_btn_primary">
            <ImageView
                android:layout_width="17dp"
                android:layout_height="17dp"
                android:src="@drawable/ic_shuffle"
                android:tint="@color/on_amber"
                android:contentDescription="@null" />
            <TextView
                android:layout_width="wrap_content"
                android:layout_height="wrap_content"
                android:layout_marginStart="7dp"
                android:text="@string/widget_shuffle"
                android:textColor="@color/on_amber"
                android:textSize="11sp"
                android:fontFamily="@font/ibm_plex_mono"
                android:textFontWeight="700"
                android:letterSpacing="0.06" />
        </LinearLayout>

        <ImageView
            android:id="@+id/widget_toggle"
            android:layout_width="36dp"
            android:layout_height="36dp"
            android:layout_marginStart="8dp"
            android:padding="9dp"
            android:background="@drawable/widget_btn_ghost"
            android:src="@drawable/ic_play"
            android:contentDescription="@string/widget_cd_toggle" />

        <ImageView
            android:id="@+id/widget_star"
            android:layout_width="36dp"
            android:layout_height="36dp"
            android:layout_marginStart="8dp"
            android:padding="9dp"
            android:background="@drawable/widget_btn_ghost"
            android:src="@drawable/ic_star_outline"
            android:contentDescription="@string/widget_cd_star"
            android:visibility="gone" />
    </LinearLayout>
</LinearLayout>
```

`widget_star` and `widget_meta` exist in this layout but start `gone` — the compact design has no room for them. They are present so one `render()` path can address the same ids in both layouts without null checks. Do not delete them.

In this layout the primary action is SHUFFLE (amber), per the design; in 4×1 it is play/pause. That inversion is deliberate.

- [ ] **Step 3: Update the provider metadata**

Replace the whole of `android/app/src/main/res/xml/widget_radio_info.xml`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<appwidget-provider xmlns:android="http://schemas.android.com/apk/res/android"
    android:minWidth="180dp"
    android:minHeight="72dp"
    android:minResizeWidth="110dp"
    android:minResizeHeight="72dp"
    android:targetCellWidth="4"
    android:targetCellHeight="1"
    android:description="@string/widget_description"
    android:updatePeriodMillis="0"
    android:initialLayout="@layout/widget_radio"
    android:previewLayout="@layout/widget_radio"
    android:resizeMode="horizontal"
    android:widgetCategory="home_screen" />
```

`targetCellWidth`/`targetCellHeight`/`previewLayout`/`description` are API 31+; older launchers ignore them, so no version guard is needed. `minResizeWidth="110dp"` is what lets the user shrink the widget down into 2×1 territory — without it the compact layout is unreachable.

Add the description string to `android/app/src/main/res/values/strings.xml`:

```xml
    <string name="widget_description">Shuffle a random station from the home screen</string>
```

- [ ] **Step 4: Verify it builds**

Run: `cd android && ./gradlew assembleDebug`
Expected: BUILD SUCCESSFUL.

Then confirm the shared ids really are in both layouts:

```bash
for id in widget_root widget_station widget_live widget_meta widget_star widget_shuffle widget_toggle; do
  echo "$id: $(grep -c "@+id/$id" android/app/src/main/res/layout/widget_radio.xml) / $(grep -c "@+id/$id" android/app/src/main/res/layout/widget_radio_small.xml)"
done
```

Expected: `1 / 1` for every id. Anything else means Task 4's `render()` will crash or silently no-op on one layout.

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/res/layout/widget_radio.xml android/app/src/main/res/layout/widget_radio_small.xml android/app/src/main/res/xml/widget_radio_info.xml android/app/src/main/res/values/strings.xml
git commit -m "feat(android): a widget that looks like the rest of the app"
```

---

### Task 4: Render both layouts from the widened state

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/RadioWidgetProvider.kt`

**Interfaces:**
- Consumes: `usesCompactLayout`, `widgetStationLabel`, `widgetMetaLabel`, `WIDGET_SMALL_MAX_WIDTH_DP` (Task 1); the layouts and ids (Task 3); `CMD_SHUFFLE`, `CMD_TOGGLE`, `CMD_STAR` from `PlaybackService.kt:30-32`.
- Produces, called by Task 5:
  - `fun refresh(context: Context, station: String, meta: String, isPlaying: Boolean, isFav: Boolean)`
  - `const val ACTION_WIDGET_STAR = "net.vchub.r4dio.WIDGET_STAR"`

- [ ] **Step 1: Widen the stored state and the actions**

At the top of `RadioWidgetProvider.kt`, next to the two existing action constants, add the star action and delete the dead one:

```kotlin
const val ACTION_WIDGET_SHUFFLE = "net.vchub.r4dio.WIDGET_SHUFFLE"
const val ACTION_WIDGET_TOGGLE = "net.vchub.r4dio.WIDGET_TOGGLE"
const val ACTION_WIDGET_STAR = "net.vchub.r4dio.WIDGET_STAR"
```

`EXTRA_WIDGET_STATION` is dead — grep confirms no other reference in the codebase. Delete that line.

- [ ] **Step 2: Rewrite `onUpdate` to read all five values**

```kotlin
    override fun onUpdate(context: Context, mgr: AppWidgetManager, ids: IntArray) {
        val prefs = context.getSharedPreferences("widget", Context.MODE_PRIVATE)
        val station = prefs.getString("station", "") ?: ""
        val meta = prefs.getString("meta", "") ?: ""
        val isPlaying = prefs.getBoolean("is_playing", false)
        val isFav = prefs.getBoolean("is_fav", false)
        ids.forEach { render(context, mgr, it, station, meta, isPlaying, isFav) }
    }
```

The default for `station` changes from `"r4dio"` to `""`. That is deliberate: `widgetStationLabel` turns blank into `— idle —`, so a never-played widget now says it is idle instead of impersonating a station called "r4dio".

- [ ] **Step 3: Route the new action**

In `onReceive`, extend the existing `when` — do not touch anything else in that function:

```kotlin
        val cmd = when (intent.action) {
            ACTION_WIDGET_SHUFFLE -> CMD_SHUFFLE
            ACTION_WIDGET_TOGGLE -> CMD_TOGGLE
            ACTION_WIDGET_STAR -> CMD_STAR
            else -> null
        }
```

Leave the `MediaController` + `Player.Listener` + 15-second timeout below it exactly as it is. It is load-bearing for cold-start playback.

- [ ] **Step 4: Rewrite `refresh` and `render`**

Replace the `companion object`'s `refresh` and `render` with:

```kotlin
        fun refresh(context: Context, station: String, meta: String, isPlaying: Boolean, isFav: Boolean) {
            context.getSharedPreferences("widget", Context.MODE_PRIVATE).edit()
                .putString("station", station)
                .putString("meta", meta)
                .putBoolean("is_playing", isPlaying)
                .putBoolean("is_fav", isFav)
                .apply()
            val mgr = AppWidgetManager.getInstance(context)
            val ids = mgr.getAppWidgetIds(ComponentName(context, RadioWidgetProvider::class.java))
            ids.forEach { render(context, mgr, it, station, meta, isPlaying, isFav) }
        }

        private fun render(
            context: Context,
            mgr: AppWidgetManager,
            id: Int,
            station: String,
            meta: String,
            isPlaying: Boolean,
            isFav: Boolean,
        ) {
            // the launcher reports the current cell size per widget instance, so a user
            // with both a wide and a narrow copy gets the right layout for each.
            val widthDp = mgr.getAppWidgetOptions(id)
                .getInt(AppWidgetManager.OPTION_APPWIDGET_MIN_WIDTH, 0)
            val layout = when (usesCompactLayout(widthDp)) {
                true -> R.layout.widget_radio_small
                false -> R.layout.widget_radio
            }
            val views = RemoteViews(context.packageName, layout)
            views.setTextViewText(
                R.id.widget_station,
                widgetStationLabel(station, context.getString(R.string.widget_idle)),
            )
            views.setTextViewText(R.id.widget_meta, meta)
            views.setViewVisibility(R.id.widget_meta, if (meta.isBlank()) View.GONE else View.VISIBLE)
            views.setViewVisibility(R.id.widget_live, if (isPlaying) View.VISIBLE else View.GONE)
            views.setImageViewResource(
                R.id.widget_toggle,
                if (isPlaying) R.drawable.ic_pause else R.drawable.ic_play,
            )
            views.setImageViewResource(
                R.id.widget_star,
                if (isFav) R.drawable.ic_star else R.drawable.ic_star_outline,
            )
            views.setOnClickPendingIntent(R.id.widget_shuffle, broadcastPending(context, ACTION_WIDGET_SHUFFLE))
            views.setOnClickPendingIntent(R.id.widget_toggle, broadcastPending(context, ACTION_WIDGET_TOGGLE))
            views.setOnClickPendingIntent(R.id.widget_star, broadcastPending(context, ACTION_WIDGET_STAR))
            views.setOnClickPendingIntent(R.id.widget_root, openAppPending(context))
            mgr.updateAppWidget(id, views)
        }

        private fun openAppPending(context: Context): PendingIntent {
            val intent = Intent(context, MainActivity::class.java)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
            return PendingIntent.getActivity(
                context, 0, intent,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
        }
```

`setViewVisibility` on `widget_meta` and `widget_live` is what makes the compact layout's `gone` defaults correct: in 2×1 the meta line stays hidden because the layout starts it `gone` **and** render only shows it when non-blank — but note render *will* show it in 2×1 if meta is non-blank. That is intended; the compact layout has room for one short line under the name.

Add `import android.view.View` to the imports.

- [ ] **Step 5: Handle resize so the layout switches live**

Add this override to the class (not the companion), after `onUpdate`:

```kotlin
    // without this, resizing a widget keeps rendering the layout chosen at add time.
    override fun onAppWidgetOptionsChanged(
        context: Context,
        mgr: AppWidgetManager,
        id: Int,
        newOptions: android.os.Bundle,
    ) {
        onUpdate(context, mgr, intArrayOf(id))
    }
```

- [ ] **Step 6: Verify it builds and the suite is green**

Run: `cd android && ./gradlew assembleDebug`
Expected: BUILD SUCCESSFUL.

Run: `cd android && ./gradlew testDebugUnitTest`
Expected: BUILD SUCCESSFUL. Report exact `tests=` / `failures=` / `errors=` aggregated from the JUnit XML under `android/app/build/test-results/testDebugUnitTest/`. Baseline before this plan is 85 tests; Task 1 adds 7, so expect 92.

- [ ] **Step 7: Verify no caller was left behind**

Run: `grep -rn "RadioWidgetProvider.refresh" android/app/src/main/kotlin/`

Expected: three hits in `PlaybackService.kt`, **all currently failing to compile** because `refresh` now takes five arguments. That is the correct state to hand to Task 5 — do not fix them here, and do not commit a broken build. Instead, complete Task 5 and commit both together, or add the temporary call-site updates as part of this task's commit. **Choose one and say which in your report.**

- [ ] **Step 8: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/RadioWidgetProvider.kt
git commit -m "feat(android): widget shows the station, live state and your star"
```

---

### Task 5: Keep the widget in step with playback

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt` — the three existing `RadioWidgetProvider.refresh` call sites (around `:165`, `:426`, `:476`), plus the `CMD_STAR` and `CMD_SCOPE` handlers (around `:558` and `:570`).

**Interfaces:**
- Consumes: `RadioWidgetProvider.refresh(context, station, meta, isPlaying, isFav)` (Task 4); `widgetMetaLabel(country, codec, bitrate)` (Task 1).
- Produces: nothing consumed later.

**Why this task exists:** the widget is push-only (`updatePeriodMillis="0"`). A field with no `refresh()` behind it shows stale data forever. The star icon added in Task 4 is exactly such a field: `CMD_STAR` currently calls `refreshCustomLayout()` — which updates the *notification* — and never touches the widget.

- [ ] **Step 1: Read the current call sites**

Read `PlaybackService.kt` around lines 160-170, 420-430, 470-480, and 550-585 before editing. You need the exact shape of each `refresh` call and of the `CMD_STAR` / `CMD_SCOPE` handlers.

- [ ] **Step 2: Add a single helper that builds the widget's view of the world**

Add this private method to `PlaybackService`, next to `refreshCustomLayout`:

```kotlin
    // one place that knows what the widget needs, so the five call sites cannot drift.
    private fun refreshWidget(station: Station?, isPlaying: Boolean) {
        val favs = runBlocking { favStore.currentFavUuids() }
        RadioWidgetProvider.refresh(
            context = this,
            station = station?.name.orEmpty(),
            meta = station?.let { widgetMetaLabel(it.country, it.codec, it.bitrate) }.orEmpty(),
            isPlaying = isPlaying,
            isFav = station?.uuid?.let { favs.contains(it) } ?: false,
        )
    }
```

If `Station` has no `bitrate` field, or it is named differently, check `Station.kt` and use the real field — `PlaybackService.kt:478-480` already builds this same string for the notification, so copy the field names from there.

- [ ] **Step 3: Replace the three existing call sites**

Each currently looks like `RadioWidgetProvider.refresh(this, <name>, <playing>)`. Replace with the helper:

- around `:165` (ExoPlayer `onIsPlayingChanged`): `refreshWidget(current, isPlaying)`
- around `:426` (mirror event applied while not playing): `refreshWidget(evt.toStationOrNull(), false)` — if no such conversion exists, pass the station object the surrounding code already has; read the enclosing function to see what is in scope. If only a name is available there, pass `null` for the station and accept that the meta line clears on that path — but say so in your report rather than inventing a conversion.
- around `:476` (`playPick`): `refreshWidget(pick, true)`

- [ ] **Step 4: Add the two missing call sites**

In the `CMD_STAR` handler (around `:558`), after `favStore.toggleFav(st)` and alongside the existing `refreshCustomLayout()`:

```kotlin
                            refreshWidget(current, exo?.isPlaying == true)
```

In the `CMD_SCOPE` handler (around `:570`), after `favStore.setScope(next)` and alongside `refreshCustomLayout()`, add the same line. Scope does not change what the widget renders today, but `shuffle()` immediately follows it and can change the station — and the widget must not be the one surface that misses it.

Both handlers already run inside `scope.launch`, so `runBlocking` inside `refreshWidget` is on a coroutine thread, matching how `refreshCustomLayout` already reads DataStore there.

- [ ] **Step 5: Verify it builds and the suite is green**

Run: `cd android && ./gradlew assembleDebug`
Expected: BUILD SUCCESSFUL — this is the task that makes the five-argument `refresh` compile.

Run: `cd android && ./gradlew testDebugUnitTest`
Expected: BUILD SUCCESSFUL, 92 tests / 0 failures. Report the exact totals from the JUnit XML.

- [ ] **Step 6: Verify every refresh path is wired**

Run: `grep -n "refreshWidget\|RadioWidgetProvider.refresh" android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt`

Expected: one definition plus **five** call sites, and zero remaining direct `RadioWidgetProvider.refresh(` calls outside the helper. State the list in your report.

- [ ] **Step 7: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt
git commit -m "fix(android): keep the widget in step when you star a station"
```

---

### Task 6: Emulator verification (controller-run, no code change)

Run by the coordinating session, not an implementation subagent. Produces no commit unless it uncovers a defect.

**Standing rules:** never touch the user's real data dir; boot the emulator with `-no-audio`; shut it down and uninstall the app when finished.

- [ ] **Step 1: Clean install and add the widget**

Boot Pixel_7 with `-no-audio`, install a fresh debug APK, then add the widget from the launcher's widget picker. Confirm the picker shows the **description** text and a real **preview** (not the generic app icon).

- [ ] **Step 2: Verify the idle state**

Before playing anything, confirm the widget reads `— idle —`, shows **no** LIVE chip, no meta line, a play glyph, and an outline star. Screenshot.

- [ ] **Step 3: Verify the playing state**

Tap SHUFFLE on the widget. Confirm: station name appears, `LIVE` chip appears in **olive**, the meta line reads `XX · CODEC · NNNk`, and the toggle glyph becomes pause. Screenshot.

- [ ] **Step 4: Verify the star round-trip — this is the Task 5 path**

Tap the star on the widget. Confirm the glyph fills. Then open the app and confirm the home screen agrees (`★ FAVOURITE`). Tap star in the app and confirm the **widget** updates without being touched. This is the path that had no refresh call before this plan; if it fails, the fix belongs in Task 5, not the layout.

- [ ] **Step 5: Verify the compact layout**

Resize the widget down to roughly 2 cells wide. Confirm it switches to the stacked layout with the amber `SHUFFLE` button, and that resizing back restores the wide layout **without** re-adding the widget. Screenshot both.

- [ ] **Step 6: Verify tap-to-open**

Tap the widget body (not a button). Confirm `MainActivity` opens.

- [ ] **Step 7: Verify a long station name**

Shuffle until a station with a very long name appears (the catalogue has several). Confirm the name ellipsises on one line and does not push the buttons off the widget in either layout.

- [ ] **Step 8: Check logcat**

Confirm no new warnings or exceptions from `r4dio`, and specifically no `RemoteViews` / `BadParcelableException` / `Bad notification` errors, which is how a layout-vs-render id mismatch surfaces.

- [ ] **Step 9: Shut down**

Uninstall the app and shut the emulator down.

---

## Out of scope — do not do these

- **ICY track metadata.** The design's "Chuck Wayne — What A Difference…" line needs `IcyInfo` support the app does not have. The second line shows `country · codec · bitrate` instead. Do not add ICY in this plan.
- **EQ bars.** `RemoteViews` cannot animate; a frozen equaliser reads as a hung app.
- **Launcher icon and status-bar icon** (Priority 1 and 2 of the handoff). Separate branch — they touch different surfaces and need their own device checks.
- **New vector assets.** `ic_shuffle`, `ic_star`, `ic_star_outline`, `ic_play`, `ic_pause` already exist and match the handoff byte-for-byte; the tile reuses `ic_launcher_foreground`.
- **A widget configuration activity**, dynamic colour (`@android:color/system_accent*`, API 31+), and a keyguard variant.
- Do not change `onReceive`'s controller lifecycle, `crates/`, `Catalog.kt`, `shuffle()`, or `playPick()`'s playback logic.

## Self-review notes

- **Spec coverage:** 4×1 and 2×1 layouts (Task 3), logo tile (Task 3, reusing `ic_launcher_foreground`), station name + `●LIVE` olive chip (Tasks 3-4), metadata line (Tasks 1, 3-5), star/shuffle/play controls with the primary-action inversion between sizes (Task 3), IBM Plex Mono and the rounded surfaces (Tasks 2-3), widget picker description and preview (Task 3).
- **Type consistency:** `usesCompactLayout(widthDp: Int)`, `widgetStationLabel(station, idle)`, `widgetMetaLabel(country, codec, bitrate)` are defined in Task 1 and used with those exact names and argument orders in Tasks 4-5. `refresh(context, station, meta, isPlaying, isFav)` is defined in Task 4 and called in Task 5. The seven shared view ids are listed in Task 3's Interfaces block and used in Task 4's `render`.
- **The seam that needs watching in the final review:** Task 4 renders both layouts through one code path that addresses seven ids. If any id is missing from one layout, `RemoteViews` fails at runtime — not at compile time — and only on the size that uses that layout. Task 3 Step 4 greps for this, and Task 6 Step 5 exercises both sizes on device, but the final whole-branch review should verify the id sets independently.
- **The second seam:** the widget is push-only, so any field without a `refresh()` behind it is permanently stale. Task 5 Step 6 enumerates the call sites; the final review should confirm each of the five corresponds to a real state change and that no path changing station, playback or fav state is missing one.
