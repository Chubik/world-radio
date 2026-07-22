# Android Widget Buttons Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the home-screen widget's shuffle and play/pause buttons work even when nothing is playing, by delivering taps through a MediaController instead of a background `startService`.

**Architecture:** The widget currently fires `PendingIntent.getService(PlaybackService, ACTION_WIDGET_*)`; Android 8+ blocks a background service start, so taps are dropped when the service is not foreground. Switch the widget to `PendingIntent.getBroadcast` → `RadioWidgetProvider.onReceive`, which connects a `MediaController` to the session (the same Media3 path `MainActivity` already uses, allowed from the background) and sends a custom `SessionCommand`. Reuse the existing `CMD_SHUFFLE` handler; add a new `CMD_TOGGLE` for play/pause-or-shuffle.

**Tech Stack:** Kotlin, androidx.media3 1.6.1 (session + common), Android AppWidgetProvider / BroadcastReceiver `goAsync()`.

## Global Constraints

- No comments in code unless a step shows one.
- Code output (logs) English, lowercase.
- No AI/Claude/personal mentions anywhere.
- Commit to `dev`; commit subjects are the public changelog — write them for users.
- Version is CI-owned; never hand-edit `build.gradle.kts` versionName/versionCode.
- No `else if` (project rule); use guard `if` or `when`.
- Test on the Android emulator before claiming done; never touch the user's data dir.
- Android unit-test surface is thin here; verification is emulator-driven (broadcast the widget actions to the provider component; the real widget cannot be placed via adb).

## File Structure

- `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt` — add `CMD_TOGGLE` const + `toggleCommand`; grant it in `onConnect`; add a `CMD_TOGGLE` branch in `onCustomCommand`; remove the two `ACTION_WIDGET_*` branches from `onStartCommand`.
- `android/app/src/main/kotlin/net/vchub/r4dio/RadioWidgetProvider.kt` — swap `getService` for `getBroadcast`; implement `onReceive` that connects a `MediaController` and sends the matching command.

No new files. Both files stay well under the size limit.

---

## Task 1: Add CMD_TOGGLE to the session (const, grant, handler)

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt`
  - constants block (~line 29-34)
  - command fields (~line 79-83)
  - `Callback.onConnect` sessionCommands (~line 357-364)
  - `Callback.onCustomCommand` `when` (~line 381-419)

**Interfaces:**
- Produces:
  - `const val CMD_TOGGLE = "net.vchub.r4dio.TOGGLE"` — the custom action string the widget sends.
  - Session accepts `SessionCommand(CMD_TOGGLE, Bundle.EMPTY)` from any connected controller; on receipt it does: playing → `exo.pause()`; paused with a media item → `exo.play()`; nothing loaded (`exo == null` or `mediaItemCount == 0`) → `shuffle()`.

- [ ] **Step 1: Add the CMD_TOGGLE constant**

In the top-level const block (next to `CMD_SHUFFLE`/`CMD_STOP`, ~line 29-34), add:

```kotlin
const val CMD_TOGGLE = "net.vchub.r4dio.TOGGLE"
```

- [ ] **Step 2: Add the toggleCommand field**

Next to `shuffleCommand` etc. (~line 79-83):

```kotlin
    private val toggleCommand = SessionCommand(CMD_TOGGLE, android.os.Bundle.EMPTY)
```

- [ ] **Step 3: Grant CMD_TOGGLE in onConnect**

In `Callback.onConnect`, the `sessionCommands` builder (~line 357-364) already adds `shuffleCommand`. Add `toggleCommand` to the same chain:

```kotlin
            val sessionCommands =
                MediaSession.ConnectionResult.DEFAULT_SESSION_AND_LIBRARY_COMMANDS.buildUpon()
                    .add(shuffleCommand)
                    .add(toggleCommand)
                    .add(starCommand)
                    .add(scopeCommand)
                    .add(stopCommand)
                    .add(syncUiCommand)
                    .build()
```

- [ ] **Step 4: Handle CMD_TOGGLE in onCustomCommand**

In `Callback.onCustomCommand`'s `when (customCommand.customAction)` (~line 381), add a branch after the `CMD_SHUFFLE` branch. Cold start (no player yet, or player with no media item) shuffles; otherwise it is a plain play/pause:

```kotlin
                CMD_TOGGLE -> {
                    val player = exo
                    when {
                        player == null -> shuffle()
                        player.mediaItemCount == 0 -> shuffle()
                        player.isPlaying -> player.pause()
                        else -> player.play()
                    }
                    return Futures.immediateFuture(SessionResult(SessionResult.RESULT_SUCCESS))
                }
```

(`when { … }` with boolean branches is allowed — no `else if`.)

- [ ] **Step 5: Build to confirm it compiles**

Run: `cd android && ./gradlew compileDebugKotlin 2>&1 | tail -12`
Expected: BUILD SUCCESSFUL.

- [ ] **Step 6: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt
git commit -m "feat(android): session command to toggle play/pause, or shuffle on a cold start"
```

---

## Task 2: Route the widget through a MediaController broadcast

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/RadioWidgetProvider.kt`

**Interfaces:**
- Consumes: `CMD_SHUFFLE` (existing), `CMD_TOGGLE` (Task 1), `PlaybackService`, `ACTION_WIDGET_SHUFFLE`, `ACTION_WIDGET_TOGGLE` (existing consts in this file).
- Produces: taps on `R.id.widget_shuffle` / `R.id.widget_toggle` deliver as broadcasts to `RadioWidgetProvider.onReceive`, which connects a `MediaController` and sends the matching `SessionCommand`, then releases.

- [ ] **Step 1: Swap getService for getBroadcast**

Replace the private `servicePending` with a broadcast builder targeting the provider itself. The intent must be explicit (set the component) so an `exported=false` receiver still receives it:

```kotlin
        private fun broadcastPending(context: Context, action: String): PendingIntent {
            val intent = Intent(context, RadioWidgetProvider::class.java).setAction(action)
            return PendingIntent.getBroadcast(
                context, action.hashCode(), intent,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
        }
```

Update the two `setOnClickPendingIntent` calls in `render` to call `broadcastPending` instead of `servicePending`:

```kotlin
            views.setOnClickPendingIntent(R.id.widget_shuffle, broadcastPending(context, ACTION_WIDGET_SHUFFLE))
            views.setOnClickPendingIntent(R.id.widget_toggle, broadcastPending(context, ACTION_WIDGET_TOGGLE))
```

- [ ] **Step 2: Handle the broadcast in onReceive**

`AppWidgetProvider.onReceive` already dispatches `APPWIDGET_UPDATE` etc. to `onUpdate`. Override it to also catch the two widget actions and drive the session. Use `goAsync()` so the process stays alive while the controller connects; the returned `PendingResult` is a per-broadcast local (never a static field) threaded into the connect callback, which releases the controller and calls `finish()`. Add this method to the class body (not the companion) and call `super.onReceive` so the base dispatch still runs:

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

Remove the old `servicePending` method (nothing calls it after Step 1).

- [ ] **Step 3: Add the imports**

Add at the top of the file:

```kotlin
import androidx.media3.session.MediaController
import androidx.media3.session.SessionCommand
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.MoreExecutors
```

- [ ] **Step 4: Build**

Run: `cd android && ./gradlew compileDebugKotlin 2>&1 | tail -12`
Expected: BUILD SUCCESSFUL. Fix any unresolved reference (import) the compiler names.

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/RadioWidgetProvider.kt
git commit -m "fix(android): widget buttons work even when nothing is playing, by driving the session directly"
```

---

## Task 3: Remove the dead startService path

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt` (`onStartCommand` ~line 151-158)

**Interfaces:**
- Consumes: nothing new.
- Produces: `onStartCommand` no longer handles `ACTION_WIDGET_SHUFFLE` / `ACTION_WIDGET_TOGGLE` (they can no longer arrive there); `ACTION_SYNC_NOW` stays.

- [ ] **Step 1: Drop the two widget branches**

`onStartCommand` currently is:

```kotlin
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_WIDGET_SHUFFLE -> shuffle()
            ACTION_WIDGET_TOGGLE -> exo?.let { if (it.isPlaying) it.pause() else it.play() }
            ACTION_SYNC_NOW -> syncNow()
        }
        return super.onStartCommand(intent, flags, startId)
    }
```

Replace with (keep only `ACTION_SYNC_NOW`):

```kotlin
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_SYNC_NOW -> syncNow()
        }
        return super.onStartCommand(intent, flags, startId)
    }
```

Leave the `ACTION_WIDGET_SHUFFLE` / `ACTION_WIDGET_TOGGLE` consts in `RadioWidgetProvider.kt` — they are still the broadcast action strings.

- [ ] **Step 2: Build + unit tests**

Run: `cd android && ./gradlew compileDebugKotlin testDebugUnitTest 2>&1 | tail -12`
Expected: BUILD SUCCESSFUL, tests PASS.

- [ ] **Step 3: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt
git commit -m "refactor(android): drop the blocked widget startService path"
```

---

## Task 4: Live verification on the emulator

**Files:** none (verification only).

The real widget cannot be placed on the launcher via adb, so drive the exact code path by broadcasting the widget actions to the provider component. Because the fix targets the *background* case, do the cold tests FIRST, before ever launching the app.

- [ ] **Step 1: Build + install**

Run: `cd android && ./gradlew assembleDebug 2>&1 | tail -3`
Then: `adb install -r app/build/outputs/apk/debug/app-debug.apk`
(Boot the emulator per the android-emulator workflow if none is running.)

- [ ] **Step 2: COLD — force-stop, then widget SHUFFLE broadcast**

```bash
adb shell am force-stop net.vchub.r4dio
adb logcat -c
adb shell am broadcast -a net.vchub.r4dio.WIDGET_SHUFFLE -n net.vchub.r4dio/.RadioWidgetProvider
sleep 6
adb logcat -d -s r4dio | grep playing | tail -3
```

Expected: a `playing <station>` line appears — a station started from cold (the bug: before the fix, nothing happened). Confirm the process is alive: `adb shell pidof net.vchub.r4dio`.

- [ ] **Step 3: COLD — force-stop, then widget TOGGLE broadcast**

```bash
adb shell am force-stop net.vchub.r4dio
adb logcat -c
adb shell am broadcast -a net.vchub.r4dio.WIDGET_TOGGLE -n net.vchub.r4dio/.RadioWidgetProvider
sleep 6
adb logcat -d -s r4dio | grep playing | tail -3
```

Expected: a `playing <station>` line — cold TOGGLE shuffles (nothing was loaded).

- [ ] **Step 4: WARM — with a station playing, TOGGLE pauses then resumes**

With audio playing from Step 3:

```bash
adb logcat -c
adb shell am broadcast -a net.vchub.r4dio.WIDGET_TOGGLE -n net.vchub.r4dio/.RadioWidgetProvider
sleep 3
adb shell dumpsys media_session | grep -A2 "net.vchub.r4dio" | grep -i "state=" | tail -2
```

Expected: playback state goes to PAUSED. Send the TOGGLE broadcast again → state returns to PLAYING.

- [ ] **Step 5: WARM — SHUFFLE changes station**

```bash
adb logcat -c
adb shell am broadcast -a net.vchub.r4dio.WIDGET_SHUFFLE -n net.vchub.r4dio/.RadioWidgetProvider
sleep 6
adb logcat -d -s r4dio | grep playing | tail -1
```

Expected: a DIFFERENT `playing <station>` than before.

- [ ] **Step 6: Confirm no crash**

Run: `adb logcat -d | grep -iE "FATAL|AndroidRuntime.*net.vchub.r4dio|ANR in net.vchub.r4dio" | tail -10`
Expected: empty. `adb shell pidof net.vchub.r4dio` returns a pid.

- [ ] **Step 7: Report and STOP**

Confirm: cold SHUFFLE plays; cold TOGGLE plays; warm TOGGLE pause↔resume; warm SHUFFLE changes station; no crash. Then STOP for the user to decide on release (this ships together with the already-done next/previous=shuffle change in one Android release).

---

## Deploy (manual, AFTER user approval — NOT part of task execution)

Bundled with the next/previous=shuffle work already on `dev`. PR `dev`→`main`, admin-merge; CI bumps version, tags, builds CLI+APK, auto-deploys.
