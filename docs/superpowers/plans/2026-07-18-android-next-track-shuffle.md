# Android Next/Previous = Shuffle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the user switch station eyes-free — the standard Next and Previous media gestures (headphones, steering wheel, lock screen, Android Auto) trigger `shuffle()`.

**Architecture:** Wrap the ExoPlayer in a `ForwardingPlayer` that advertises the seek-to-next/previous commands and routes those seeks to `shuffle()`, and stop the MediaSession callback from removing those commands. No new deps (ForwardingPlayer is in media3-common, already transitively present via media3 1.6.1).

**Tech Stack:** Kotlin, androidx.media3 1.6.1 (exoplayer + session + common), Android media session.

## Global Constraints

- No comments in code unless a step shows one.
- Code output (logs) English, lowercase.
- No AI/Claude/personal mentions anywhere.
- Commit to `dev`; commit subjects are the public changelog — write them for users.
- Version is CI-owned; never hand-edit `build.gradle.kts` versionName/versionCode.
- Test on the Android emulator before claiming done; never touch the user's data dir.

---

## File Structure

- `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt` — add a `ForwardingPlayer` subclass wired into the MediaSession; adjust `Callback.onConnect` to keep the seek commands.

If `PlaybackService.kt` would grow past the size limit, extract the forwarding player into its own file `ShufflePlayer.kt` in the same package. Prefer keeping it inline if the file stays reasonable.

---

## Task 1: ForwardingPlayer that maps next/previous seeks to shuffle

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt`

**Interfaces:**
- Produces: a player wrapper whose `seekToNext`/`seekToNextMediaItem`/`seekToPrevious`/`seekToPreviousMediaItem` call a supplied `onShuffle: () -> Unit`, and whose available-commands include the four seek commands so the OS surfaces Next/Previous.

- [ ] **Step 1: Add the forwarding player**

In `PlaybackService.kt`, add a class wrapping the ExoPlayer. Use `androidx.media3.common.ForwardingPlayer` and `androidx.media3.common.Player.Commands`. Media3's own docs require overriding `getAvailableCommands()` + `isCommandAvailable()` AND the seek methods when you change command availability, so do all of them:

```kotlin
private class ShufflePlayer(
    delegate: androidx.media3.common.Player,
    private val onShuffle: () -> Unit,
) : androidx.media3.common.ForwardingPlayer(delegate) {

    private val extraCommands = intArrayOf(
        androidx.media3.common.Player.COMMAND_SEEK_TO_NEXT,
        androidx.media3.common.Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM,
        androidx.media3.common.Player.COMMAND_SEEK_TO_PREVIOUS,
        androidx.media3.common.Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM,
    )

    override fun getAvailableCommands(): androidx.media3.common.Player.Commands {
        val builder = super.getAvailableCommands().buildUpon()
        extraCommands.forEach { builder.add(it) }
        return builder.build()
    }

    override fun isCommandAvailable(command: Int): Boolean =
        command in extraCommands || super.isCommandAvailable(command)

    override fun seekToNext() = onShuffle()
    override fun seekToNextMediaItem() = onShuffle()
    override fun seekToPrevious() = onShuffle()
    override fun seekToPreviousMediaItem() = onShuffle()
}
```

Verify the exact method names/signatures against media3 1.6.1's `ForwardingPlayer` (Android Studio will flag a bad override signature). `Player.Commands.Builder.add(Int)` and `buildUpon()` are the documented API.

- [ ] **Step 2: Build to confirm the overrides compile**

Run: `cd android && ./gradlew compileDebugKotlin 2>&1 | tail -15`
Expected: BUILD SUCCESSFUL. If an override signature is wrong (e.g. media3 1.6.1 marks a method final or uses a different name), the compiler names it — fix to match.

- [ ] **Step 3: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt
git commit -m "feat(android): player wrapper that turns next/previous into shuffle"
```

---

## Task 2: Wire the wrapper into the session and stop removing the seek commands

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt` (`onCreate` MediaSession build; `Callback.onConnect` player commands)

**Interfaces:**
- Consumes: `ShufflePlayer` (Task 1), the existing private `shuffle()` method, existing `exo`/`session` fields.

- [ ] **Step 1: Pass the wrapper to the MediaSession**

In `onCreate` (~line 110), the session is currently built from `player`:

```kotlin
        session = MediaSession.Builder(this, player)
            .setCallback(Callback())
            .build()
```

Wrap it so the session controls the forwarding player (keep `exo = player` for direct in-service control):

```kotlin
        val sessionPlayer = ShufflePlayer(player) { shuffle() }
        session = MediaSession.Builder(this, sessionPlayer)
            .setCallback(Callback())
            .build()
```

`exo` must still point at the raw `player` (playback control inside the service uses `exo` directly). Confirm `exo = player` stays as-is.

- [ ] **Step 2: Stop removing Next/Previous in `onConnect`**

In `Callback.onConnect` (~line 337), the player-commands block removes the four seek commands. Delete those four `.remove(...)` lines so the session advertises Next/Previous:

```kotlin
            val playerCommands =
                MediaSession.ConnectionResult.DEFAULT_PLAYER_COMMANDS.buildUpon()
                    .build()
```

(If `DEFAULT_PLAYER_COMMANDS` already lacks the seek commands for a single-item player, the `ShufflePlayer.getAvailableCommands` override is what actually surfaces them — leaving this block as the default is correct either way. Do not re-add the removes.)

- [ ] **Step 3: Build**

Run: `cd android && ./gradlew compileDebugKotlin 2>&1 | tail -15`
Expected: BUILD SUCCESSFUL.

- [ ] **Step 4: Run existing Android unit tests**

Run: `cd android && ./gradlew testDebugUnitTest 2>&1 | tail -15`
Expected: PASS (this change has no unit-test surface; confirm nothing else broke).

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt
git commit -m "feat(android): next/previous media buttons now shuffle to a new station, so you can switch without looking"
```

---

## Task 3: Live verification on the emulator

**Files:** none (verification only).

- [ ] **Step 1: Build + install the debug APK**

Run: `cd android && ./gradlew assembleDebug 2>&1 | tail -5`
Then install to a running emulator: `adb install -r app/build/outputs/apk/debug/app-debug.apk`
(If no emulator is running, boot one per the android-emulator workflow.)

- [ ] **Step 2: Start playback, then send MEDIA_NEXT**

Launch the app, start a station (shuffle button or widget), confirm audio. Capture the current station name (logcat: `adb logcat -d | grep -i r4dio | tail -5`), then:

```bash
adb shell input keyevent 87   # KEYCODE_MEDIA_NEXT
sleep 3
adb logcat -d | grep -i r4dio | tail -5
```

Expected: a DIFFERENT station starts (shuffle fired). Repeat with `88` (KEYCODE_MEDIA_PREVIOUS) → also shuffles.

- [ ] **Step 3: Verify scope is honoured**

Switch scope to favs-only (the "favs only" button in the notification), then `adb shell input keyevent 87` → the new station must be a favourite (or, if favourites are empty, it falls back to the full catalogue per the existing shuffle-fallback behaviour). Confirm no crash.

- [ ] **Step 4: Lock-screen / notification Next**

With a station playing, confirm the media notification (and lock screen) now shows Next/Previous transport controls, and tapping Next shuffles. Capture a screenshot: `adb exec-out screencap -p > /tmp/r4dio_next.png`.

- [ ] **Step 5: Report**

Confirm: MEDIA_NEXT and MEDIA_PREVIOUS both shuffle; scope respected; Next appears on the notification/lock screen; no crash on empty scope. Note any friction (e.g. a visible transport that looks odd for radio). Then STOP for the user to decide on release.

---

## Deploy (manual, AFTER user approval — NOT part of task execution)

PR `dev`→`main`, admin-merge; CI bumps version, tags, builds CLI+Android APK, auto-deploys. Android-only change, but the release is unified (CLI + APK together).
