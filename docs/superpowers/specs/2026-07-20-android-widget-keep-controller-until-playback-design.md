# Android Widget — Keep Controller Until Playback — Design

## Problem

On a cold start (process freshly created, e.g. widget tapped right after boot),
tapping a widget button connects a `MediaController`, sends the command, and
immediately `release()`s the controller. Lifecycle instrumentation (reverted)
showed the service is then destroyed within milliseconds:

```
onCreate → shuffle coroutine start → onDestroy(!) → loaded 0 stations → playPick: exo null skip
```

Root cause: at the moment of `release()`, playback has not started yet (the
shuffle coroutine only just began). `MediaSessionService` sees no active playback
and no connected controller, so it stops itself. `onDestroy` runs `scope.cancel()`,
which kills the shuffle coroutine before it can play; `exo` is nulled.

Media3 keeps a `MediaSessionService` alive only while there is active playback or a
connected controller (foreground held during playback, plus ~10 min after pause
since 1.6.0). Releasing the controller before playback begins tears the service
down. The launcher path does NOT hit this: `MainActivity` keeps its controller
connected across playback start (`waitForPlayback`).

## Decision

The widget's `onReceive` must keep the `MediaController` connected until playback
actually starts, then release — mirroring `MainActivity.waitForPlayback`. Release
(and finish the async broadcast) when `isPlaying` becomes true, or after a 15 s
fallback if playback never starts (e.g. no network), whichever comes first.

Scope: `RadioWidgetProvider.onReceive` only. The service, the next/previous=shuffle
change, the widget-broadcast routing, and the cold-start DataStore fix all stay as
already committed.

## Component: RadioWidgetProvider.onReceive

Replace the "send command then release immediately" tail with a
release-when-playing / release-on-timeout scheme:

1. `goAsync()` → connect `MediaController` (unchanged).
2. On connect: `sendCustomCommand(cmd)` (unchanged).
3. Instead of releasing now:
   - Add a `Player.Listener` whose `onIsPlayingChanged(true)` releases the
     controller, cancels the fallback, and calls `pending.finish()`.
   - If already `controller.isPlaying` at connect time (warm tap that resumes
     instantly), release immediately.
   - Post a 15 s fallback on a main-thread `Handler` that releases + finishes if
     playback never starts.
4. A guard (single-shot flag) ensures release + `finish()` run exactly once,
   whichever of playing / timeout fires first, and never double-release.

All controller callbacks run on the main thread; `goAsync()` keeps the process
alive while the controller is held. While the controller stays connected, the
service is not torn down, so the shuffle coroutine has time to load the catalogue
and start playback — at which point the service becomes foreground and sustains
itself.

## Data flow (after fix)

```
onReceive → goAsync() → MediaController.buildAsync() → onConnected
  → sendCustomCommand(cmd)
  → if controller.isPlaying: releaseOnce()
    else: controller.addListener{ onIsPlayingChanged(true) -> releaseOnce() }
         + handler.postDelayed(releaseOnce, 15000)
  → releaseOnce(): remove listener, cancel timeout, controller.release(), pending.finish()  [runs once]
```

## Error handling

- Connect failure (`future.get()` null): release-path still runs once — finish the
  broadcast, no crash (existing `runCatching`).
- Playback never starts (no network): the 15 s fallback releases gracefully; no
  leak, no ANR.
- Double trigger (playing races timeout): the single-shot guard makes the second a
  no-op.

## Testing / verification (emulator)

Temporary `exported=true` test build to drive the real widget→session path via an
adb broadcast (reverted before commit).

1. COLD: force-stop, broadcast WIDGET_SHUFFLE → a station PLAYS (before this fix the
   service was destroyed and nothing played). Confirm a `playing <station>` log and
   `state=PLAYING`.
2. COLD: broadcast WIDGET_TOGGLE → plays (cold toggle = shuffle).
3. WARM: broadcast WIDGET_SHUFFLE → different station; WIDGET_TOGGLE pauses ↔ resumes.
4. No crash / ANR; process alive; service not destroyed mid-start.
5. Manifest reverted to `exported="false"`; only the fix commit remains.

## Out of scope

- The service's own stop/destroy logic (`onTaskRemoved`, `onDestroy`).
- Widget UI / resize.
- The already-committed next/previous, widget-broadcast, and DataStore fixes.
