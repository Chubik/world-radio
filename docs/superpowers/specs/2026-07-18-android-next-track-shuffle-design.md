# Android next-track = shuffle — design

## Problem

The Android shuffle control is a custom button in the media notification — hard
to hit without looking (driving; phone in pocket with headphones). The user needs
to switch station **eyes-free**.

## Idea

Map the standard **Next** and **Previous** media gestures — headphone
double/triple-tap, steering-wheel buttons, lock-screen, Android Auto, Bluetooth —
to `shuffle()`. These gestures are already muscle memory and work with the screen
off. Both Next and Previous trigger shuffle (no left/right distinction needed
eyes-free).

## Current state (verified in code)

`PlaybackService.onCreate` builds an `ExoPlayer` with a single `setMediaItem`
(the current stream) and a `MediaSession`. `Callback.onConnect`
(`PlaybackService.kt` ~339-342) **removes** `COMMAND_SEEK_TO_NEXT`,
`COMMAND_SEEK_TO_NEXT_MEDIA_ITEM`, `COMMAND_SEEK_TO_PREVIOUS`,
`COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM` from the player commands — so Next/Previous
are currently dead. `shuffle()` is a private service method that launches a
coroutine, honours the scope (ALL / favs-only) and excluded countries, and plays
a random pick.

## Solution

Wrap the `ExoPlayer` in a `ForwardingPlayer` whose seek-to-next/previous methods
call `shuffle()` instead of moving within a (non-existent) playlist, and stop
removing the Next/Previous commands so the system exposes them.

1. **`ShuffleForwardingPlayer`** — a `ForwardingPlayer(exo)` overriding:
   - `seekToNext()` / `seekToNextMediaItem()` → `onShuffle()` (a callback into the service), not `super`.
   - `seekToPrevious()` / `seekToPreviousMediaItem()` → `onShuffle()`.
   - `getAvailableCommands()` (or `isCommandAvailable`) to advertise
     `COMMAND_SEEK_TO_NEXT` / `..._MEDIA_ITEM` / `COMMAND_SEEK_TO_PREVIOUS` /
     `..._MEDIA_ITEM` as available, so the OS shows/accepts the gestures.
   Verify the exact Media3 override surface against the installed media3 version;
   `ForwardingPlayer` + `getAvailableCommands` is the documented seam.
2. Pass the forwarding player (not the raw `exo`) into `MediaSession.Builder`.
   Keep `exo` for direct playback control inside the service.
3. `Callback.onConnect`: stop removing the four seek commands (or add them back),
   so the session's player commands include Next/Previous.
4. `onShuffle` calls the existing `shuffle()` — scope + excluded still honoured,
   so Next in favs-scope walks favourites.

## Components

- `crates`/Android: `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt`
  - add the `ForwardingPlayer` subclass (inner class or top-level with a lambda),
  - use it in `MediaSession.Builder`,
  - adjust `onConnect` player commands.
- No new files strictly required; a small top-level class is fine if it keeps
  `PlaybackService` under the size limit.
- No new dependencies (`ForwardingPlayer` is in media3-common, already a dep).

## Error handling

- If `shuffle()` finds nothing to play for the scope, it already logs and no-ops
  (existing behaviour) — Next/Previous then simply does nothing, no crash.
- Rapid Next presses: each calls `shuffle()`, which launches a coroutine and
  plays the newest pick; last-write-wins on the player is fine (same as the
  notification button today).

## Testing

- The mapping is thin Kotlin glue; the real proof is **live on the emulator**:
  - Play a station, then send a media "next" (via `adb shell input keyevent 87`
    KEYCODE_MEDIA_NEXT, and `86`/`88` for stop/previous) → station changes
    (shuffle), scope respected.
  - Lock screen shows Next/Previous and they shuffle.
  - Optionally a headphone/AVRCP path if the emulator supports it; otherwise the
    keyevent path is the canonical eyes-free proxy.
- Add a unit test only if a pure seam exists (e.g. the forwarding player's
  command set) — do not fabricate one around the Android framework.

## Out of scope (tracked separately — see android-eyes-free-controls memory)

- Fixing the broken home-screen widget (buttons don't react) + growing it
  vertically with more controls.
- A big full-screen shuffle button in the app.
- Shake-to-shuffle.

## Verification

Emulator: `adb shell input keyevent 87` while a station plays → a different
station starts, honouring the current scope; lock-screen Next/Previous both
shuffle; no crash when the scope is empty.
