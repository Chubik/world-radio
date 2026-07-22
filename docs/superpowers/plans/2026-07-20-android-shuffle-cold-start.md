# Android shuffle() Cold-Start Hang Fix — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop `shuffle()` from hanging on a cold start by reading favStore off the Main dispatcher with bounded timeouts and safe fallbacks.

**Architecture:** `shuffle()` currently runs on `Dispatchers.Main` and its first suspend line (`favStore.currentScope()`, the first DataStore emission) stalls under cold-start contention. Move the three favStore reads into a single `withContext(Dispatchers.IO)` block, each wrapped in `runCatching { withTimeout(3000) { … } }.getOrDefault(<default>)`. Fallbacks: scope=ALL, favs=empty, excluded=empty — a scope-less full-catalogue shuffle, the correct degrade.

**Tech Stack:** Kotlin, kotlinx.coroutines (`withContext`, `withTimeout`), androidx media3.

## Global Constraints

- No comments in code unless a step shows one.
- Code output (logs) English, lowercase.
- No AI/Claude/personal mentions anywhere.
- Commit to `dev`; commit subjects are the public changelog — write them for users.
- Version is CI-owned; never hand-edit build.gradle.kts version fields.
- No `else if` (project rule); guard `if` or `when`.
- Test on the Android emulator before claiming done; never touch the user's data dir. The real widget can't be adb-placed, so verification uses a TEMPORARY `exported=true` test build to broadcast the widget action; that edit and any diagnostic logs MUST be reverted before the commit.

## File Structure

- `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt` — add the `withTimeout` import; rewrite the favStore reads inside `shuffle()`.

Single file. No new files.

---

## Task 1: Read favStore off Main with bounded fallbacks in shuffle()

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt`
  - imports block (~line 18-27, add one)
  - `shuffle()` (~line 292-306)

**Interfaces:**
- Consumes: existing `favStore.currentScope(): Scope`, `favStore.currentCachedFavs(): List<Station>`, `favStore.currentExcluded(): Set<String>`; `withReadyCatalog()`; `pickForScope(...)`; `playPick(...)`; `Scope.ALL`.
- Produces: `shuffle()` that never blocks the Main coroutine on a DataStore read and always makes progress (plays, or logs "nothing to play").

- [ ] **Step 1: Add the withTimeout import**

In the kotlinx.coroutines import group (~line 18-27), add (keep the existing alphabetical-ish grouping; placement need not be exact):

```kotlin
import kotlinx.coroutines.withTimeout
```

- [ ] **Step 2: Rewrite the favStore reads in shuffle()**

The current `shuffle()` body is:

```kotlin
    private fun shuffle() {
        scope.launch {
            val sc = favStore.currentScope()
            val favs = favStore.currentCachedFavs()
            val cat = withReadyCatalog()
            val userExcluded = favStore.currentExcluded()
            val pick = pickForScope(sc, cat, favs, userExcluded)
            when (pick) {
                null -> Log.i("r4dio", "shuffle: nothing to play for scope $sc")
                else -> playPick(pick)
            }
        }
    }
```

Replace it with (reads moved to IO, each bounded + fallback; catalogue read unchanged):

```kotlin
    private fun shuffle() {
        scope.launch {
            val sc = withContext(Dispatchers.IO) {
                runCatching { withTimeout(3000) { favStore.currentScope() } }.getOrDefault(Scope.ALL)
            }
            val favs = withContext(Dispatchers.IO) {
                runCatching { withTimeout(3000) { favStore.currentCachedFavs() } }.getOrDefault(emptyList())
            }
            val userExcluded = withContext(Dispatchers.IO) {
                runCatching { withTimeout(3000) { favStore.currentExcluded() } }.getOrDefault(emptySet())
            }
            val cat = withReadyCatalog()
            val pick = pickForScope(sc, cat, favs, userExcluded)
            when (pick) {
                null -> Log.i("r4dio", "shuffle: nothing to play for scope $sc")
                else -> playPick(pick)
            }
        }
    }
```

Notes for the implementer:
- `getOrDefault(emptyList())` / `getOrDefault(emptySet())` must resolve to the same element types the originals return (`List<Station>`, `Set<String>`). If the compiler needs help inferring, annotate: `getOrDefault(emptyList<Station>())` and `getOrDefault(emptySet<String>())`.
- Do NOT wrap `withReadyCatalog()` in the timeout — it is out of scope and already works.
- No new logging in the committed version.

- [ ] **Step 3: Build**

Run: `cd /Users/vchub/dev/projects/world-radio/radio/android && ./gradlew compileDebugKotlin 2>&1 | tail -12`
Expected: BUILD SUCCESSFUL. If type inference fails on a `getOrDefault`, add the explicit type argument as noted, rebuild.
(Shell cwd does not persist between separate calls — use the absolute `cd` each time.)

- [ ] **Step 4: Run unit tests**

Run: `cd /Users/vchub/dev/projects/world-radio/radio/android && ./gradlew testDebugUnitTest 2>&1 | tail -12`
Expected: PASS (no unit surface for this change; confirm nothing else broke).

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt
git commit -m "fix(android): shuffle plays reliably right after launch instead of hanging"
```

---

## Task 2: Live verification on the emulator

**Files:** none for the committed tree (temporary test edits only, reverted).

The real widget can't be adb-placed; drive the exact widget→session→shuffle path via a temporary `exported=true` test build and an adb broadcast. The COLD case is the one that used to hang.

- [ ] **Step 1: Temporary test build (exported=true)**

In `android/app/src/main/AndroidManifest.xml`, change the widget receiver line:

```
<receiver android:name=".RadioWidgetProvider" android:exported="false">
```
to
```
<receiver android:name=".RadioWidgetProvider" android:exported="true">
```

This is TEMPORARY — it will be reverted in Step 5 and must NOT be committed.

- [ ] **Step 2: Build + install**

Run: `cd /Users/vchub/dev/projects/world-radio/radio/android && ./gradlew assembleDebug 2>&1 | tail -3`
Then: `adb install -r app/build/outputs/apk/debug/app-debug.apk`
(Boot the Pixel_7 emulator per the android-emulator workflow if none is running; wait for `sys.boot_completed`.)

- [ ] **Step 3: COLD — force-stop, then broadcast WIDGET_SHUFFLE**

```bash
adb shell am force-stop net.vchub.r4dio
adb logcat -c
adb shell am broadcast -a net.vchub.r4dio.WIDGET_SHUFFLE -n net.vchub.r4dio/.RadioWidgetProvider -f 0x00000020
sleep 10
adb logcat -d -s r4dio | grep playing | tail -3
adb shell pidof net.vchub.r4dio
```

Expected: a `playing <station>` line appears within a few seconds (before the fix it hung and none appeared); pid non-empty.

- [ ] **Step 4: WARM — broadcast WIDGET_SHUFFLE again, expect a different station**

```bash
adb logcat -c
adb shell am broadcast -a net.vchub.r4dio.WIDGET_SHUFFLE -n net.vchub.r4dio/.RadioWidgetProvider
sleep 7
adb logcat -d -s r4dio | grep playing | tail -1
```

Expected: a DIFFERENT `playing <station>` than Step 2.

- [ ] **Step 5: No crash**

Run: `adb logcat -d | grep -iE "FATAL|AndroidRuntime.*net.vchub.r4dio|ANR in net.vchub.r4dio" | tail -10`
Expected: empty.

- [ ] **Step 6: Revert the test-only manifest edit**

Run: `cd /Users/vchub/dev/projects/world-radio/radio && git checkout -- android/app/src/main/AndroidManifest.xml`
Then confirm clean: `git status --short` shows no manifest change. The receiver is back to `exported="false"`.

- [ ] **Step 7: Report and STOP**

Confirm: cold shuffle plays (hang fixed); warm shuffle changes station; no crash; manifest reverted to `exported="false"`; only the Task 1 shuffle commit remains. Ships in the same Android release as the widget fix and next/previous=shuffle. STOP for the user to decide on release.

---

## Deploy (manual, AFTER user approval — NOT part of task execution)

Bundled with the widget fix and next/previous=shuffle already on `dev`. PR `dev`→`main`, admin-merge; CI bumps version, tags, builds CLI+APK, auto-deploys.
