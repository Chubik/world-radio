# Android Widget — Keep Controller Until Playback — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop the widget's cold-start tap from killing the service — keep the MediaController connected until playback starts (or 15 s), then release.

**Architecture:** `RadioWidgetProvider.onReceive` currently releases the controller immediately after `sendCustomCommand`. On a cold start playback hasn't begun, so the MediaSessionService stops itself and cancels the shuffle coroutine. Fix: hold the controller, release it (and finish the async broadcast) when `isPlaying` turns true or after a 15 s fallback — exactly like `MainActivity.waitForPlayback`. A single-shot guard makes release run once.

**Tech Stack:** Kotlin, androidx.media3 session/common, Android BroadcastReceiver `goAsync()`, `Handler`/`Looper`.

## Global Constraints

- No comments in code unless a step shows one.
- Code output (logs) English, lowercase.
- No AI/Claude/personal mentions anywhere.
- Commit to `dev`; commit subjects are the public changelog — write them for users.
- Version is CI-owned; never hand-edit build.gradle.kts version fields.
- No `else if` (project rule); guard `if` or `when`.
- Test on the Android emulator before claiming done; never touch the user's data dir. The real widget can't be adb-placed, so verification uses a TEMPORARY `exported=true` test build; that edit MUST be reverted before the commit.

## File Structure

- `android/app/src/main/kotlin/net/vchub/r4dio/RadioWidgetProvider.kt` — rewrite `onReceive` to hold the controller until playback; add imports.

Single file. No new files. No service change.

---

## Task 1: Hold the controller until playback starts in onReceive

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/RadioWidgetProvider.kt` (imports; `onReceive`)

**Interfaces:**
- Consumes: `CMD_SHUFFLE`, `CMD_TOGGLE`, `PlaybackService`, `ACTION_WIDGET_*`; media3 `MediaController`, `SessionToken`, `SessionCommand`; `androidx.media3.common.Player`.
- Produces: `onReceive` that keeps the controller connected until `isPlaying` (or 15 s) before releasing, so the service survives a cold-start start.

- [ ] **Step 1: Add imports**

At the top of the file, add:

```kotlin
import android.os.Handler
import android.os.Looper
import androidx.media3.common.Player
```

(`android.app.PendingIntent`, `ComponentName`, `Context`, `Intent`, `MediaController`, `SessionCommand`, `SessionToken`, `MoreExecutors` are already imported.)

- [ ] **Step 2: Rewrite onReceive**

The current `onReceive` is:

```kotlin
    override fun onReceive(context: Context, intent: Intent) {
        super.onReceive(context, intent)
        val cmd = when (intent.action) {
            ACTION_WIDGET_SHUFFLE -> CMD_SHUFFLE
            ACTION_WIDGET_TOGGLE -> CMD_TOGGLE
            else -> null
        }
        cmd ?: return
        val pending = goAsync()
        val token = SessionToken(
            context.applicationContext,
            ComponentName(context.applicationContext, PlaybackService::class.java),
        )
        val future = MediaController.Builder(context.applicationContext, token).buildAsync()
        future.addListener({
            val controller = runCatching { future.get() }.getOrNull()
            controller?.sendCustomCommand(
                SessionCommand(cmd, android.os.Bundle.EMPTY),
                android.os.Bundle.EMPTY,
            )
            controller?.release()
            pending.finish()
        }, MoreExecutors.directExecutor())
    }
```

Replace it with (hold the controller until `isPlaying` or 15 s, release once):

```kotlin
    override fun onReceive(context: Context, intent: Intent) {
        super.onReceive(context, intent)
        val cmd = when (intent.action) {
            ACTION_WIDGET_SHUFFLE -> CMD_SHUFFLE
            ACTION_WIDGET_TOGGLE -> CMD_TOGGLE
            else -> null
        }
        cmd ?: return
        val pending = goAsync()
        val token = SessionToken(
            context.applicationContext,
            ComponentName(context.applicationContext, PlaybackService::class.java),
        )
        val future = MediaController.Builder(context.applicationContext, token).buildAsync()
        future.addListener({
            val controller = runCatching { future.get() }.getOrNull()
            if (controller == null) {
                pending.finish()
                return@addListener
            }
            val handler = Handler(Looper.getMainLooper())
            var released = false
            var listener: Player.Listener? = null
            val releaseOnce = {
                if (!released) {
                    released = true
                    handler.removeCallbacksAndMessages(null)
                    listener?.let { controller.removeListener(it) }
                    controller.release()
                    pending.finish()
                }
            }
            controller.sendCustomCommand(
                SessionCommand(cmd, android.os.Bundle.EMPTY),
                android.os.Bundle.EMPTY,
            )
            val l = object : Player.Listener {
                override fun onIsPlayingChanged(isPlaying: Boolean) {
                    if (isPlaying) {
                        releaseOnce()
                    }
                }
            }
            listener = l
            controller.addListener(l)
            if (controller.isPlaying) {
                releaseOnce()
            }
            handler.postDelayed({ releaseOnce() }, 15000)
        }, MoreExecutors.directExecutor())
    }
```

Notes for the implementer:
- `releaseOnce` is a `val` lambda so both the listener and the timeout call the same single-shot release. `released` guards against double release/finish.
- The listener is added BEFORE the `controller.isPlaying` check so a race between "already playing" and "becomes playing" still resolves once (the guard).
- No new logging. No comments. No `else if` (uses `if`/guard only).
- Do NOT touch the companion `render`/`broadcastPending` — only `onReceive` changes.

- [ ] **Step 3: Build**

Run: `cd /Users/vchub/dev/projects/world-radio/radio/android && ./gradlew compileDebugKotlin 2>&1 | tail -12`
Expected: BUILD SUCCESSFUL. Fix any unresolved reference (import) the compiler names.
(Shell cwd does not persist between separate calls — use the absolute `cd` each time.)

- [ ] **Step 4: Unit tests**

Run: `cd /Users/vchub/dev/projects/world-radio/radio/android && ./gradlew testDebugUnitTest 2>&1 | tail -12`
Expected: PASS (no unit surface for this change; confirm nothing else broke).

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/RadioWidgetProvider.kt
git commit -m "fix(android): widget starts playback reliably from cold, keeping the session alive until it plays"
```

---

## Task 2: Live verification on the emulator

**Files:** none committed (temporary test edit only, reverted).

- [ ] **Step 1: Temporary exported=true test build**

In `android/app/src/main/AndroidManifest.xml` change:
```
<receiver android:name=".RadioWidgetProvider" android:exported="false">
```
to
```
<receiver android:name=".RadioWidgetProvider" android:exported="true">
```
TEMPORARY — reverted in Step 6, never committed.

- [ ] **Step 2: Build + install (boot emulator if needed)**

Run: `cd /Users/vchub/dev/projects/world-radio/radio/android && ./gradlew assembleDebug 2>&1 | tail -3`
Then: `adb install -r app/build/outputs/apk/debug/app-debug.apk`
(Boot Pixel_7 and wait for `sys.boot_completed` if no emulator is running.)

- [ ] **Step 3: COLD SHUFFLE — the case that used to kill the service**

```bash
adb shell am force-stop net.vchub.r4dio
adb logcat -c
adb shell am broadcast -a net.vchub.r4dio.WIDGET_SHUFFLE -n net.vchub.r4dio/.RadioWidgetProvider -f 0x00000020
sleep 12
adb logcat -d -s r4dio | grep -iE "playing|onDestroy" | tail -4
adb shell dumpsys media_session | grep -i "state=PLAYING" | head -1
adb shell pidof net.vchub.r4dio
```

Expected: a `playing <station>` line, `state=PLAYING`, pid non-empty, and NO `onDestroy` before playback. (Before the fix: onDestroy, no playing.)

- [ ] **Step 4: COLD TOGGLE**

```bash
adb shell am force-stop net.vchub.r4dio
adb logcat -c
adb shell am broadcast -a net.vchub.r4dio.WIDGET_TOGGLE -n net.vchub.r4dio/.RadioWidgetProvider -f 0x00000020
sleep 12
adb logcat -d -s r4dio | grep playing | tail -2
```

Expected: a `playing <station>` line (cold toggle = shuffle).

- [ ] **Step 5: WARM SHUFFLE + TOGGLE**

```bash
adb logcat -c
adb shell am broadcast -a net.vchub.r4dio.WIDGET_SHUFFLE -n net.vchub.r4dio/.RadioWidgetProvider
sleep 6
adb logcat -d -s r4dio | grep playing | tail -1
adb shell am broadcast -a net.vchub.r4dio.WIDGET_TOGGLE -n net.vchub.r4dio/.RadioWidgetProvider
sleep 3
adb shell dumpsys media_session | grep -iE "state=PAUSED|state=PLAYING" | head -1
```

Expected: SHUFFLE → a different station; TOGGLE → state flips to PAUSED (send again → PLAYING).

- [ ] **Step 6: No crash + revert test edit**

```bash
adb logcat -d | grep -iE "FATAL|AndroidRuntime.*net.vchub.r4dio|ANR in net.vchub.r4dio" | tail -10
cd /Users/vchub/dev/projects/world-radio/radio && git checkout -- android/app/src/main/AndroidManifest.xml
git status --short
```

Expected: crash grep empty; `git status` shows no manifest change (back to `exported="false"`).

- [ ] **Step 7: Report and STOP**

Confirm: cold shuffle & toggle PLAY (service survives), warm shuffle changes station, warm toggle pause↔resume, no crash, manifest reverted. Then STOP for the user to decide on release. Ships together with next/previous=shuffle + widget-broadcast + cold-start DataStore fix in ONE Android release.

---

## Deploy (manual, AFTER user approval — NOT part of task execution)

Bundled with the three earlier Android fixes on `dev`. PR `dev`→`main`, admin-merge; CI bumps version, tags, builds CLI+APK, auto-deploys.
