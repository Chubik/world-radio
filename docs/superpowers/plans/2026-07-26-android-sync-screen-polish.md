# Android Sync Screen Brand Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Android sync screen typographically identical to the v1.6.0 home screen and close the remaining redline deviations, without touching sync behaviour.

**Architecture:** Presentation-only change to three resource files. `activity_sync.xml` swaps the system `monospace` family for the bundled IBM Plex families, moves every user-facing literal into `strings.xml`, and drops the Unicode glyphs from button labels. `pill_synced.xml` turns olive. `SyncActivity.kt` is not touched — it binds by id, and no id changes.

**Tech Stack:** Android XML Views (no Compose, no Material Components), bundled IBM Plex Mono/Sans font families in `res/font/`, shape drawables.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-26-android-sync-screen-polish-design.md`. The visual authority is `r4dio - Sync Screen Handoff.html` in the Claude Design project "radio" (`a5023d91-924d-4f7b-9561-12f6cc7477a5`).
- Comments and code in English. Comments only where the logic is non-obvious.
- No AI / Claude / Anthropic mention anywhere. No personal data.
- No `else if` chains (not applicable here — no Kotlin changes).
- Never blind-overwrite an existing file: read it first, then `Edit`.
- Do NOT modify `SyncActivity.kt`, the service, the widget, or the home-screen layouts.
- Do NOT add a Material Components dependency, do NOT change button heights, do NOT add icons — all three are explicit non-goals in the spec.
- minSdk is 26, so `android:fontWeight` is supported and is the correct way to select a weight. Do not use `android:textStyle="bold"` for the new weights.
- Toast and dialog strings inside `SyncActivity.kt` are out of scope and stay as they are.

---

### Task 1: Sync screen typography, strings, and pill colour

**Files:**
- Modify: `android/app/src/main/res/values/strings.xml` (append a `sync_*` block before `</resources>`)
- Modify: `android/app/src/main/res/layout/activity_sync.xml` (whole file: 17 `fontFamily="monospace"` occurrences, 18 hardcoded literals, glyph removal)
- Modify: `android/app/src/main/res/drawable/pill_synced.xml` (stroke amber → olive)

**Interfaces:**
- Consumes: `@font/ibm_plex_mono` and `@font/ibm_plex_sans` (already bundled in `res/font/`, mapping weights 400/500/600/700 to the eight TTFs); `@color/olive` (`#9EC074`, already in `colors.xml`).
- Produces: nothing other tasks depend on. This is the only task.

- [ ] **Step 1: Add the string resources**

Read `android/app/src/main/res/values/strings.xml` first. Append this block immediately before the closing `</resources>` tag, after the existing `home_*` entries:

```xml
    <string name="sync_title">r4dio sync</string>
    <string name="sync_done">✕ DONE</string>
    <string name="sync_lede">Sync your favourites, blocked and hidden countries across devices.</string>
    <string name="sync_key_label">SYNC KEY</string>
    <string name="sync_key_hint">paste key r4-…</string>
    <string name="sync_use_key">USE KEY</string>
    <string name="sync_or">OR</string>
    <string name="sync_create">CREATE NEW</string>
    <string name="sync_scan">SCAN QR</string>
    <string name="sync_synced">⊙ SYNCED</string>
    <string name="sync_your_key">YOUR SYNC KEY</string>
    <string name="sync_copy">COPY</string>
    <string name="sync_copy_hint">tap to select · copy to clipboard</string>
    <string name="sync_qr_caption">SCAN ON ANOTHER DEVICE</string>
    <string name="sync_logout">LOG OUT</string>
    <string name="sync_delete">DELETE</string>
    <string name="sync_hide_countries">HIDE COUNTRIES</string>
    <string name="sync_feedback">ideas or bugs? support@r4dio.net</string>
```

Note the glyphs: `✕ DONE` and `⊙ SYNCED` KEEP theirs (both appear in the handoff mockup and neither has an emoji presentation). Every other label drops its glyph — that is why `sync_use_key` is `USE KEY`, not `✓  USE KEY`, and `sync_feedback` has no `✉`.

- [ ] **Step 2: Verify the strings resolve**

Run: `cd /Users/vchub/dev/projects/world-radio/radio/android && ./gradlew assembleDebug 2>&1 | tail -5`
Expected: `BUILD SUCCESSFUL`. The strings are not referenced yet; this only proves the XML is well-formed (a stray `&` or unescaped apostrophe fails here).

- [ ] **Step 3: Rewrite the layout's text and typography**

Read `android/app/src/main/res/layout/activity_sync.xml` first, then edit it. Apply all of the following. Every `android:text`/`android:hint` literal becomes a `@string/sync_*` reference, and every `android:fontFamily="monospace"` becomes a bundled family with an explicit weight.

The per-element target state (id or role → text ref, family, weight, size — sizes are already correct in the file, do not change them):

| Element | text | fontFamily | fontWeight |
|---|---|---|---|
| app-bar title | `@string/sync_title` | `@font/ibm_plex_mono` | 700 |
| `done` | `@string/sync_done` | `@font/ibm_plex_mono` | 600 |
| lede paragraph (state A, first TextView) | `@string/sync_lede` | `@font/ibm_plex_sans` | 400 |
| SYNC KEY label | `@string/sync_key_label` | `@font/ibm_plex_mono` | 400 |
| `key_input` (hint) | `@string/sync_key_hint` | `@font/ibm_plex_mono` | 400 |
| `use_key` | `@string/sync_use_key` | `@font/ibm_plex_mono` | 600 |
| OR divider | `@string/sync_or` | `@font/ibm_plex_mono` | 400 |
| `create` | `@string/sync_create` | `@font/ibm_plex_mono` | 600 |
| `scan` | `@string/sync_scan` | `@font/ibm_plex_mono` | 600 |
| SYNCED pill | `@string/sync_synced` | `@font/ibm_plex_mono` | 600 |
| YOUR SYNC KEY label | `@string/sync_your_key` | `@font/ibm_plex_mono` | 400 |
| `key_shown` | (no text attr — set at runtime) | `@font/ibm_plex_mono` | 400 |
| `copy` | `@string/sync_copy` | `@font/ibm_plex_mono` | 600 |
| copy hint | `@string/sync_copy_hint` | `@font/ibm_plex_sans` | 400 |
| QR caption | `@string/sync_qr_caption` | `@font/ibm_plex_mono` | 400 |
| `logout` | `@string/sync_logout` | `@font/ibm_plex_mono` | 600 |
| `delete` | `@string/sync_delete` | `@font/ibm_plex_mono` | 600 |
| `excluded_countries` | `@string/sync_hide_countries` | `@font/ibm_plex_mono` | 600 |
| `feedback` | `@string/sync_feedback` | `@font/ibm_plex_sans` | 400 |

Three rules while doing this:

1. **Remove every `android:textStyle="bold"` on these elements.** It resolves to weight 700 and would defeat the 600 the redline asks for on button labels. Replace it with the `android:fontWeight` from the table. The app-bar title genuinely wants 700, but express it as `android:fontWeight="700"`, not `textStyle`.
2. **Do not change any `android:textSize`, `android:layout_height`, `android:padding`, `android:layout_margin`, `android:letterSpacing`, or `android:background`.** Those already match the redline. Only text, fontFamily, fontWeight, and the removed textStyle change.
3. **Do not change any `android:id`.** `SyncActivity.kt` binds them by name and is not being modified.

Also change the SYNCED pill's `android:textColor` from `@color/amber` to `@color/olive` (the pill is the `TextView` with `android:background="@drawable/pill_synced"` in `state_b`).

- [ ] **Step 4: Turn the pill's stroke olive**

Read `android/app/src/main/res/drawable/pill_synced.xml`, then change only the stroke colour:

```xml
<?xml version="1.0" encoding="utf-8"?>
<shape xmlns:android="http://schemas.android.com/apk/res/android" android:shape="rectangle">
    <solid android:color="@android:color/transparent" />
    <stroke android:width="1dp" android:color="@color/olive" />
    <corners android:radius="100dp" />
</shape>
```

- [ ] **Step 5: Verify no literals or system monospace remain**

Run:
```bash
cd /Users/vchub/dev/projects/world-radio/radio
grep -n 'fontFamily="monospace"' android/app/src/main/res/layout/activity_sync.xml
grep -nE 'android:(text|hint)="[^"@][^"]*"' android/app/src/main/res/layout/activity_sync.xml
```
Expected: both produce NO output. The first proves every family was swapped; the second proves every literal moved to a resource. If the second prints a line, that literal was missed — move it to a `sync_*` string.

- [ ] **Step 6: Build**

Run: `cd /Users/vchub/dev/projects/world-radio/radio/android && ./gradlew assembleDebug 2>&1 | tail -8`
Expected: `BUILD SUCCESSFUL`. A typo in a `@string/` or `@font/` reference fails here with "resource not found", so a green build proves every reference resolves.

- [ ] **Step 7: Commit**

```bash
cd /Users/vchub/dev/projects/world-radio/radio
git add android/app/src/main/res/layout/activity_sync.xml android/app/src/main/res/values/strings.xml android/app/src/main/res/drawable/pill_synced.xml
git commit -m "fix(android): sync screen now uses the same typeface as the rest of the app"
```

---

### Task 2: Emulator verification (controller-run, not a subagent)

**Files:** none — verification only.

**Interfaces:**
- Consumes: the debug APK built from Task 1.
- Produces: screenshots of both states for comparison against the handoff mockup.

This task is run by the controller, not delegated: it drives a real emulator and must never touch the user's real data directory.

- [ ] **Step 1: Boot the emulator and install**

```bash
export PATH="$PATH:$HOME/Library/Android/sdk/emulator:$HOME/Library/Android/sdk/platform-tools"
nohup emulator -avd Pixel_7 -no-snapshot-load > /tmp/emulator.log 2>&1 &
# wait for `adb devices` to list the device, then:
cd /Users/vchub/dev/projects/world-radio/radio/android && ./gradlew assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

- [ ] **Step 2: Verify state A (not linked)**

A fresh install has no sync key, so the sync screen opens in state A. Open the app, tap the SYNC bar on the home screen, and screenshot:

```bash
adb shell am start -n net.vchub.r4dio/.MainActivity
# tap the SYNC bar, then:
adb exec-out screencap -p > /tmp/sync-state-a.png
```
Expected: IBM Plex Mono throughout (visibly different from the previous system monospace — rounder, wider), the lede in IBM Plex Sans, buttons reading `USE KEY` / `CREATE NEW` / `SCAN QR` with no leading glyphs, `HIDE COUNTRIES` at the bottom, and `✕ DONE` in the app-bar.

- [ ] **Step 3: Verify state B (linked)**

Tap `CREATE NEW` to mint a sync account, which switches the screen to state B. Screenshot:

```bash
adb exec-out screencap -p > /tmp/sync-state-b.png
```
Expected: the `⊙ SYNCED` pill in OLIVE (`#9EC074`) with an olive stroke — visibly green, not amber; the key box with the key in IBM Plex Mono and a `COPY` button; the QR bitmap on the light card; `LOG OUT` / `DELETE` with no glyphs.

- [ ] **Step 4: Confirm nothing broke**

```bash
adb logcat -d -s AndroidRuntime:E | tail -10
```
Expected: empty (no crash). Also tap `✕ DONE` and confirm the screen closes back to the home screen, and tap `COPY` and confirm the "copied" toast appears — proving the ids still bind after the layout edit.

- [ ] **Step 5: Clean up**

```bash
adb uninstall net.vchub.r4dio
adb emu kill
```
The emulator must be shut down when the run finishes.

---

## Notes for the reviewer

The single highest-risk mistake in Task 1 is silently dropping or altering a
non-typography attribute while rewriting 17 elements — a changed `textSize`,
`layout_height`, or `background` would break the redline match that already
exists. Diff the layout attribute-by-attribute and confirm that ONLY `text`,
`hint`, `fontFamily`, `fontWeight`, the removed `textStyle`, and the pill's
`textColor` changed.

The second thing to check is that no `android:id` changed, since `SyncActivity.kt`
binds every control by id and was deliberately not modified.
