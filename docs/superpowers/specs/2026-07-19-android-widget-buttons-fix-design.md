# Android Widget Buttons Fix — Design

## Problem

The home-screen widget's buttons (shuffle, play/pause) silently do nothing when
music is not already playing.

Root cause (reproduced on the emulator): the widget fires
`PendingIntent.getService(PlaybackService, ACTION_WIDGET_*)`. On Android 8+ a
background `startService` is blocked, so when the service is not already in the
foreground the OS refuses the start and `onStartCommand` is never called
(`am start-service` when the app is backgrounded returns `app is in background`).
When music IS playing, the service is foreground and the same tap works.

Evidence:
- service not playing → tap → `Error: app is in background uid null`, no `onStartCommand`.
- service playing → tap → `onStartCommand action=WIDGET_SHUFFLE exo=true`, shuffle fires.

## Decision

Move the widget's action delivery off `getService` and onto the standard Media3
path that already works from `MainActivity`: a `MediaController` connecting to the
`MediaSessionService`. `MediaController` legally starts the session service even
from the background, and the session's existing custom-command handler already
knows how to shuffle/stop/etc.

## Scope (this release)

Fix the buttons only. Vertical resize + extra controls are a separate later step
(the widget still stretches horizontally only for now — untouched here).

## Components

### RadioWidgetProvider
- `servicePending()` → `broadcastPending()`: build the tap intent with
  `PendingIntent.getBroadcast(...)` targeting the provider itself (the receiver is
  already declared in the manifest, `exported=false`). Keep the two action
  constants but they now travel as broadcasts to `onReceive`, not to the service.
- `onReceive`: on `ACTION_WIDGET_SHUFFLE` / `ACTION_WIDGET_TOGGLE`, call
  `goAsync()` to keep the broadcast alive, build
  `MediaController.Builder(SessionToken(ctx, PlaybackService))` and `buildAsync()`.
  On connect, send the matching custom `SessionCommand`, then `controller.release()`
  and `pendingResult.finish()`.

### PlaybackService
- Add a new `CMD_TOGGLE` session command with play/pause-or-shuffle semantics:
  - playing → `pause()`
  - paused with a current station → `play()`
  - nothing selected yet (cold start) → `shuffle()`
- Reuse the existing `CMD_SHUFFLE` branch in `onCustomCommand` for the shuffle
  button — no new shuffle logic.
- `onConnect`: the current `onConnect` builds `availableSessionCommands` for its
  controllers. Ensure `CMD_SHUFFLE` and `CMD_TOGGLE` are both in that set for every
  connecting controller (the existing custom layout already adds `CMD_SHUFFLE`;
  `CMD_TOGGLE` is new and MUST be added, otherwise the widget's `MediaController`
  is rejected when it sends it). A plain `MediaController` reaches the same
  `onConnect`, so granting there covers the widget path.
- Remove `ACTION_WIDGET_SHUFFLE` / `ACTION_WIDGET_TOGGLE` handling from
  `onStartCommand` (no longer reached via startService). `ACTION_SYNC_NOW` stays.

## Data flow

```
tap → getBroadcast → RadioWidgetProvider.onReceive → goAsync()
    → MediaController.buildAsync() → onConnected
    → sendCustomCommand(CMD_SHUFFLE | CMD_TOGGLE) → session Callback.onCustomCommand
    → shuffle() / play-pause-or-shuffle
    → controller.release() → pendingResult.finish()
```

## Cold-start play semantics

Widget play/pause when nothing has ever played (no station chosen) → `shuffle()`
(one tap always produces sound, eyes-free). If a station exists, it is normal
play/pause.

## Error handling

- `buildAsync` connect failure → release the future, finish the broadcast, no crash.
- `goAsync()` + `finish()` bound the async work so the broadcast receiver does not
  leak or ANR.

## Testing / verification (emulator)

The widget cannot be placed on the launcher via adb, but the exact tap path can be
driven by broadcasting the widget actions to the provider component and by using a
`MediaController` connect. Verify in BOTH states:
1. Service NOT playing (cold): trigger widget SHUFFLE → a station starts playing.
2. Service NOT playing (cold): trigger widget TOGGLE → a station starts playing (cold = shuffle).
3. Service playing: SHUFFLE → different station; TOGGLE → pauses; TOGGLE again → resumes.
4. No crash / ANR in any state; app process stays alive.

## Out of scope

- Widget vertical resize and additional buttons (favs-scope etc.) — later step.
- Any change to the notification / lock-screen controls.
