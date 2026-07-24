# Android Home Screen Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the headless `MainActivity` into the real home screen from the design — a giant eyes-free shuffle target, now-playing, secondary controls, and a reachable SYNC bar.

**Architecture:** Classic Android XML Views (no Compose). `MainActivity` connects a `MediaController` (already does) and now stays up as the screen; it reflects state from `Player.Listener` (playing, metadata) and `MediaController.Listener.onExtrasChanged` (fav/scope, newly published by the service via `setSessionExtras`), and fires the existing custom session commands on taps.

**Tech Stack:** Kotlin, androidx.media3 session, Android Views, ConstraintLayout/LinearLayout, vector drawables, `res/font` (IBM Plex).

## Global Constraints

- No comments in code unless a step shows one; lowercase logs; no AI/personal mentions.
- Commit to `dev`; commit subjects are the public changelog — write for users.
- Version is CI-owned; never hand-edit build.gradle.kts version fields.
- No `else if`; use `when` or guard `if`.
- Test on the Android emulator before claiming done; never touch the user's data dir.
- Design authority: `docs/design/android/home-screen-redline.md` (exact dp/sp/colours/states). The visual must match it.
- Reuse existing session commands (`CMD_SHUFFLE/TOGGLE/STAR/SCOPE/STOP`) — no new playback logic.

## File Structure

- `PlaybackService.kt` — publish fav/scope via `session.setSessionExtras` in `refreshCustomLayout`; add extras keys.
- `res/drawable/ic_stop.xml` — new (rounded square).
- `res/values/colors.xml` — add `olive`, `mute` if missing.
- `res/values/strings.xml` — screen strings.
- `res/font/` — IBM Plex Mono/Sans families.
- `res/layout/activity_main.xml` — portrait home screen.
- `res/layout-land/activity_main.xml` — landscape home screen.
- `MainActivity.kt` — rewrite: persistent screen, state binding, tap handlers.
- `res/values/EXTRA_*` constants live in Kotlin (PlaybackService companion / top-level consts).

---

## Task 1: Service publishes fav + scope as session extras

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt` (top-level consts ~line 29-35; `refreshCustomLayout` ~line 119-124)

**Interfaces:**
- Produces: top-level `const val EXTRA_FAV = "net.vchub.r4dio.EXTRA_FAV"` (boolean) and `const val EXTRA_SCOPE = "net.vchub.r4dio.EXTRA_SCOPE"` (string "all"/"favs"). After every `refreshCustomLayout()`, `session.setSessionExtras(Bundle)` carries the current `isFav` and `scope`, so a connected `MediaController` receives them via `onExtrasChanged`.

- [ ] **Step 1: Add the extras keys**

In the top-level const block (~line 29-35), add:

```kotlin
const val EXTRA_FAV = "net.vchub.r4dio.EXTRA_FAV"
const val EXTRA_SCOPE = "net.vchub.r4dio.EXTRA_SCOPE"
```

- [ ] **Step 2: Publish extras in refreshCustomLayout**

`refreshCustomLayout` already computes `isFav` and `sc`. Add the extras publish after the custom-layout call:

```kotlin
    private suspend fun refreshCustomLayout() {
        val favs = favStore.currentFavUuids()
        val isFav = current?.uuid?.let { favs.contains(it) } ?: false
        val sc = favStore.currentScope()
        session?.setCustomLayout(listOf(shuffleButton, starButton(isFav), syncButton, stopButton))
        val extras = android.os.Bundle().apply {
            putBoolean(EXTRA_FAV, isFav)
            putString(EXTRA_SCOPE, if (sc == Scope.FAVS) "favs" else "all")
        }
        session?.setSessionExtras(extras)
    }
```

- [ ] **Step 3: Build**

Run: `cd /Users/vchub/dev/projects/world-radio/radio/android && ./gradlew compileDebugKotlin 2>&1 | tail -12`
Expected: BUILD SUCCESSFUL.

- [ ] **Step 4: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt
git commit -m "feat(android): publish favourite + scope state to the app so the home screen can show it"
```

---

## Task 2: Resource assets — stop icon, colours, strings, fonts

**Files:**
- Create: `android/app/src/main/res/drawable/ic_stop.xml`
- Modify: `android/app/src/main/res/values/colors.xml`
- Modify: `android/app/src/main/res/values/strings.xml`
- Create: `android/app/src/main/res/font/…` (IBM Plex Mono/Sans) + family XMLs

**Interfaces:**
- Produces: `@drawable/ic_stop`, `@color/olive`, `@color/mute`, string resources, `@font/ibm_plex_mono` / `@font/ibm_plex_sans`.

- [ ] **Step 1: Add ic_stop vector**

Create `res/drawable/ic_stop.xml`:

```xml
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="24dp" android:height="24dp"
    android:viewportWidth="24" android:viewportHeight="24">
    <path android:fillColor="#FFFFFFFF"
        android:pathData="M8,6 h8 a2,2 0 0 1 2,2 v8 a2,2 0 0 1 -2,2 h-8 a2,2 0 0 1 -2,-2 v-8 a2,2 0 0 1 2,-2 z" />
</vector>
```

(fillColor is overridden by tint at use-site; a plain fill is fine.)

- [ ] **Step 2: Add missing colours**

In `res/values/colors.xml`, add any not already present:

```xml
<color name="olive">#9EC074</color>
<color name="mute">#8A7F64</color>
```

(bg, panel, amber, amber_hi, accent, bright, dim, rule, danger already exist — do not duplicate. Verify names against the file; the redline uses `amber_hi` etc.)

- [ ] **Step 3: Add strings**

In `res/values/strings.xml`, add (English, since UI copy is English per the redline):

```xml
<string name="home_now_playing">NOW PLAYING</string>
<string name="home_paused">PAUSED</string>
<string name="home_live">LIVE</string>
<string name="home_off_air">OFF AIR</string>
<string name="home_shuffle_label">TAP ANYWHERE — SHUFFLE</string>
<string name="home_shuffle_sub_all">random station · eyes-free</string>
<string name="home_shuffle_sub_favs">random favourite · eyes-free</string>
<string name="home_warn_no_favs">NO FAVOURITES YET — STAR ONE FIRST</string>
<string name="home_scope_all">ALL STATIONS</string>
<string name="home_scope_favs">FAVOURITES ONLY</string>
<string name="home_fav_yes">★ FAVOURITE</string>
<string name="home_fav_no">☆ not saved</string>
<string name="home_play">PLAY</string>
<string name="home_pause">PAUSE</string>
<string name="home_star">STAR</string>
<string name="home_starred">STARRED</string>
<string name="home_scope">scope</string>
<string name="home_stop">STOP</string>
<string name="home_sync">SYNC</string>
<string name="home_sync_sub">link desktop ↔ phone</string>
```

- [ ] **Step 4: Bundle IBM Plex fonts**

Download the static TTFs for IBM Plex Mono (400/500/600/700) and IBM Plex Sans
(400/500/600/700) from the official IBM Plex release (SIL Open Font License —
redistribution allowed; include the license). Place under `res/font/` with
lowercase-underscore names (e.g. `ibm_plex_mono_regular.ttf`,
`ibm_plex_mono_semibold.ttf`, …). Create family XMLs:

`res/font/ibm_plex_mono.xml`:
```xml
<font-family xmlns:android="http://schemas.android.com/apk/res/android">
    <font android:fontStyle="normal" android:fontWeight="400" android:font="@font/ibm_plex_mono_regular"/>
    <font android:fontStyle="normal" android:fontWeight="500" android:font="@font/ibm_plex_mono_medium"/>
    <font android:fontStyle="normal" android:fontWeight="600" android:font="@font/ibm_plex_mono_semibold"/>
    <font android:fontStyle="normal" android:fontWeight="700" android:font="@font/ibm_plex_mono_bold"/>
</font-family>
```
and `res/font/ibm_plex_sans.xml` analogously.

If the TTFs cannot be fetched in this environment, STOP and report (status
NEEDS_CONTEXT) — do not silently fall back; the human will supply the fonts or
approve the platform-monospace fallback.

- [ ] **Step 5: Build (resources compile)**

Run: `cd /Users/vchub/dev/projects/world-radio/radio/android && ./gradlew assembleDebug 2>&1 | tail -8`
Expected: BUILD SUCCESSFUL. Font/drawable/colour/string resources resolve.

- [ ] **Step 6: Commit**

```bash
git add android/app/src/main/res
git commit -m "feat(android): stop icon, home-screen colours, strings, and IBM Plex fonts"
```

---

## Task 3: Portrait home-screen layout

**Files:**
- Create: `android/app/src/main/res/layout/activity_main.xml`

**Interfaces:**
- Consumes: `@drawable/ic_*`, `@color/*`, `@string/home_*`, `@font/ibm_plex_*` from Task 2.
- Produces: a layout with these IDs the Activity binds (Task 4): `stage` (the giant
  tap target), `kicker`, `eq` (equaliser container), `station_name`, `ctx_country`,
  `ctx_codec`, `ctx_fav`, `scope_pill`, `hero_ring`, `hero_glyph`, `hero_label`,
  `hero_sub`, `btn_play`, `btn_star`, `btn_scope`, `btn_stop`, `btn_sync`.

- [ ] **Step 1: Build the layout**

Create `res/layout/activity_main.xml` following `docs/design/android/home-screen-redline.md`.
Root: vertical LinearLayout, `@color/bg`, padding 16dp sides / 8dp top / 16dp bottom.

Structure (use the redline's dp/sp/radii):
1. `stage` — a clickable container (`android:id="@+id/stage"`,
   `android:background="@drawable/bg_stage"` a 22dp-radius rounded rect stroke `@color/rule` fill `@color/panel`), `layout_weight="1"`, padding 22×20dp, holding:
   - now-playing block: `kicker` (LinearLayout with a TextView + the `eq` container of thin bars + a LIVE TextView), `station_name` (mono 700 30sp `@color/bright`), a context row (`ctx_country`, `ctx_codec`, `ctx_fav`, `scope_pill`).
   - hero block (centered, weight 1): `hero_ring` (200dp, `@drawable/bg_hero_ring`), `hero_glyph` (ImageView `@drawable/ic_shuffle` 128dp, tint `@color/amber_hi`), `hero_label`, `hero_sub`.
2. controls row: horizontal LinearLayout, 4 equal-weight buttons `btn_play`/`btn_star`/`btn_scope`/`btn_stop`, each a vertical ImageView+label using `@drawable/bg_sec_btn` (14dp radius, stroke rule, panel), min-height 66dp, gap 10dp (use `layout_marginStart` between).
3. `btn_sync` — full-width, 56dp, `@drawable/bg_sync_bar` (14dp radius, transparent, 1dp amber stroke), horizontal: icon + "SYNC" + sub aligned end.

Also create the supporting shape drawables: `res/drawable/bg_stage.xml`,
`bg_hero_ring.xml` (oval, 2dp amber_hi stroke, subtle fill), `bg_sec_btn.xml`,
`bg_sec_btn_on.xml` (amber stroke + tinted fill for the active state),
`bg_sync_bar.xml`. Give the mono/sans via `android:fontFamily="@font/ibm_plex_mono"`
etc. Equaliser bars: a horizontal LinearLayout of thin `View`s (2.5dp wide,
`@color/amber`) — static height in XML; animation is optional (Task 4 may animate).

- [ ] **Step 2: Preview-compile the layout**

Run: `cd /Users/vchub/dev/projects/world-radio/radio/android && ./gradlew assembleDebug 2>&1 | tail -8`
Expected: BUILD SUCCESSFUL (layout + drawables inflate at build time; runtime binding is Task 4).

- [ ] **Step 3: Commit**

```bash
git add android/app/src/main/res/layout/activity_main.xml android/app/src/main/res/drawable
git commit -m "feat(android): home screen layout — giant shuffle target, now-playing, controls, sync bar"
```

---

## Task 4: MainActivity — bind state and actions

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/MainActivity.kt` (full rewrite of the screen behaviour)

**Interfaces:**
- Consumes: `activity_main.xml` IDs (Task 3); `EXTRA_FAV`/`EXTRA_SCOPE` (Task 1);
  session commands `CMD_SHUFFLE/TOGGLE/STAR/SCOPE/STOP`; `SyncActivity`.
- Produces: a persistent home screen that renders playback + metadata + fav/scope
  and sends commands on tap.

- [ ] **Step 1: Write the metadata-parse test first (TDD)**

The context line needs country + codec out of the packed artist string
("SA · MP3 · 128k"). Add a pure helper `parseArtist(artist: String?): Pair<String?, String?>`
(country, codec) and a JVM unit test. Create
`android/app/src/test/kotlin/net/vchub/r4dio/ArtistParseTest.kt`:

```kotlin
package net.vchub.r4dio

import org.junit.Assert.assertEquals
import org.junit.Test

class ArtistParseTest {
    @Test fun parses_country_and_codec() {
        assertEquals("SA" to "MP3", parseArtist("SA · MP3 · 128k"))
    }
    @Test fun handles_null_and_blank() {
        assertEquals(null to null, parseArtist(null))
        assertEquals(null to null, parseArtist(""))
    }
    @Test fun handles_missing_tokens() {
        assertEquals("US" to null, parseArtist("US"))
    }
}
```

Run: `cd /Users/vchub/dev/projects/world-radio/radio/android && ./gradlew testDebugUnitTest --tests "*ArtistParseTest*" 2>&1 | tail -15`
Expected: FAIL (parseArtist not defined).

- [ ] **Step 2: Implement parseArtist**

Add to MainActivity.kt (top-level or companion):

```kotlin
fun parseArtist(artist: String?): Pair<String?, String?> {
    val parts = artist?.split("·")?.map { it.trim() }?.filter { it.isNotEmpty() } ?: emptyList()
    val country = parts.getOrNull(0)
    val codec = parts.getOrNull(1)
    return country to codec
}
```

Run the test again → PASS.

- [ ] **Step 3: Rewrite MainActivity onto the layout**

Replace the headless controller behaviour. Key changes:
- `onCreate` → after the notification-permission gate, `setContentView(R.layout.activity_main)`, wire tap listeners, then `connect()`.
- `connect()` unchanged (MediaController.buildAsync). On connect, DO NOT `finish()`.
  Instead: keep the controller, add a `Player.Listener` (for `onIsPlayingChanged`,
  `onMediaMetadataChanged`) and a `MediaController.Listener` with
  `onExtrasChanged(controller, extras)`; call an initial `render()`; and if
  `controller.mediaItemCount == 0`, send `CMD_SHUFFLE` (existing autoplay behaviour).
- `render()` reads `controller.isPlaying`, `controller.mediaMetadata` (station via
  `.station` — actually media3 exposes it as `mediaMetadata.station`; if unavailable
  use `.title`), parses `.artist` via `parseArtist`, reads last-known fav/scope from
  the stored extras, and updates every view: kicker text + eq/LIVE visibility,
  station name, context row, scope pill text/style, hero label + warn state,
  secondary button states (play↔pause icon/label, star filled/outline, scope
  ALL/FAVS). Keep a `private var fav = false; private var scope = "all"`, updated in
  `onExtrasChanged`, then `render()`.
- Tap handlers send commands:
  - `stage.setOnClickListener { send(CMD_SHUFFLE) }`
  - `btn_play` → `send(CMD_TOGGLE)`, `btn_star` → `send(CMD_STAR)`,
    `btn_scope` → `send(CMD_SCOPE)`, `btn_stop` → `send(CMD_STOP)`
  - `btn_sync` → `startActivity(Intent(this, SyncActivity::class.java))`
  - `send(action)`: `controller?.sendCustomCommand(SessionCommand(action, Bundle.EMPTY), Bundle.EMPTY)`
- `onDestroy` releases the controller/listener as before (keep the existing cleanup;
  drop the 15s close-guard and the finish-on-playing logic — the screen persists).

Do not invent metadata the service doesn't send. If `mediaMetadata.station` is
empty, show `.title`; if both empty, show a neutral idle label ("— idle —").

- [ ] **Step 4: Build + unit tests**

Run: `cd /Users/vchub/dev/projects/world-radio/radio/android && ./gradlew compileDebugKotlin testDebugUnitTest 2>&1 | tail -15`
Expected: BUILD SUCCESSFUL, ArtistParseTest passes.

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/MainActivity.kt android/app/src/test/kotlin/net/vchub/r4dio/ArtistParseTest.kt
git commit -m "feat(android): home screen shows what's playing and drives shuffle, favourites, scope, and sync"
```

---

## Task 5: Landscape layout

**Files:**
- Create: `android/app/src/main/res/layout-land/activity_main.xml`

**Interfaces:**
- Same view IDs as portrait (Task 3) so MainActivity binds unchanged.

- [ ] **Step 1: Build the landscape layout**

Create `res/layout-land/activity_main.xml`: a horizontal root splitting the stage
(left, weight 1.35) from a right column (weight 1) holding now-playing context +
the controls grid (2 columns in landscape) + the sync bar. Reuse the same IDs and
drawables; sizes per the redline's landscape values (hero ring 168dp, name 26sp,
sync 50dp). Every ID from Task 3's interface list MUST be present so the shared
MainActivity binding does not NPE on rotation.

- [ ] **Step 2: Build**

Run: `cd /Users/vchub/dev/projects/world-radio/radio/android && ./gradlew assembleDebug 2>&1 | tail -6`
Expected: BUILD SUCCESSFUL.

- [ ] **Step 3: Commit**

```bash
git add android/app/src/main/res/layout-land/activity_main.xml
git commit -m "feat(android): landscape home screen for car mounts"
```

---

## Task 6: Live verification on the emulator

**Files:** none.

- [ ] **Step 1: Build + install**

Run: `cd /Users/vchub/dev/projects/world-radio/radio/android && ./gradlew assembleDebug 2>&1 | tail -3`
Then `adb install -r app/build/outputs/apk/debug/app-debug.apk` (boot Pixel_7 if needed; wait for `sys.boot_completed`).

- [ ] **Step 2: Launch → home screen renders**

```bash
adb shell monkey -p net.vchub.r4dio -c android.intent.category.LAUNCHER 1
sleep 6
adb exec-out screencap -p > /tmp/r4dio_home.png
adb logcat -d -s r4dio | grep playing | tail -1
```

Read the screenshot: confirm the giant shuffle target, station name, context row,
scope pill, and secondary controls render in the amber-CRT style. A station is playing.

- [ ] **Step 3: Drive each control**

- Tap the stage centre (`adb shell input tap <x> <y>` at the hero) → a DIFFERENT
  station plays (`adb logcat -d -s r4dio | grep playing | tail -1`).
- Tap play/pause → kicker flips NOW PLAYING ↔ PAUSED, icon updates.
- Tap star → context flips ★ FAVOURITE ↔ ☆ not saved (extras drive it).
- Tap scope → pill flips ALL STATIONS ↔ FAVOURITES ONLY.
- Tap SYNC → SyncActivity opens (`adb shell dumpsys activity activities | grep -i syncactivity`).
Capture a screenshot after each to confirm the visual state.

- [ ] **Step 4: Rotate → landscape**

`adb shell settings put system accelerometer_rotation 0; adb shell settings put system user_rotation 1`
Relaunch/observe, screenshot: the side-by-side landscape layout renders, no crash,
all controls present.

- [ ] **Step 5: No crash**

Run: `adb logcat -d | grep -iE "FATAL|AndroidRuntime.*net.vchub.r4dio|ANR in net.vchub.r4dio" | tail -10`
Expected: empty. `adb shell pidof net.vchub.r4dio` non-empty.
Reset rotation: `adb shell settings put system user_rotation 0`.

- [ ] **Step 6: Report and STOP**

Confirm: home screen renders in the design's style; stage=shuffle; play/star/scope/
stop/sync all work and reflect state; landscape works; no crash. Screenshots
captured. STOP for the user to decide on release (this is a notable feature — likely
a minor bump, user's call).

---

## Deploy (manual, AFTER user approval — NOT part of task execution)

PR `dev`→`main`, admin-merge. The Sync screen redesign, asset refresh, and widget
layout are SEPARATE later specs — not in this release.
