# World Radio — Android MVP (Design Spec)

**Date:** 2026-06-29
**Status:** approved for planning
**Deadline:** tomorrow (testable on a phone, in the car).

## Why

The user wants to listen in the car, all day, on battery, controlling play/stop/shuffle from
the steering wheel — like Spotify. There is no Android app yet. `radio-core`/`radio-audio`
(Rust) are desktop-only (cpal is not an Android audio backend), so Android is a separate
native app, as the desktop spec always anticipated ("Android is a separate future spec —
different paradigm").

## Goal

A native Kotlin Android app that streams a radio station, plays in the background with the
screen off, and exposes play / stop / next(shuffle) to the lock screen, notification, and
**steering-wheel / Android Auto / Bluetooth controls** — with minimal battery use. Delivered to
the user's phone via Firebase App Distribution (not Google Play yet).

## Decisions (from brainstorm)

1. **Stack:** native **Kotlin** + **Media3 (ExoPlayer + MediaSession)**. Chosen for battery —
   the user's #1 priority for an all-day in-car player: background audio runs as a
   `MediaSessionService` with no UI engine resident while the screen is off. MediaSession gives
   wheel / lock-screen / headset controls for free (standard Android media transport).
   (`preferans` is only the reference for *how Firebase App Distribution is wired*, not the
   language — preferans is Flutter; this app is Kotlin.)
2. **Catalog:** **radio-browser API** directly from Kotlin (same source as desktop). Shuffle =
   random station from an API query. Favorites sync with desktop is **deferred** (no sync
   server yet) — "redo later if time."
3. **Delivery:** **Firebase App Distribution** — a new Firebase project for the radio app;
   build an APK and distribute to the user's phone as a tester. Google Play is later.
4. **No Rust reuse** in this MVP — the Android app is self-contained Kotlin.

## Architecture

- **`MediaSessionService` (Media3):** owns the ExoPlayer instance and a `MediaSession`. This is
  the battery-critical core — it runs in the background, holds a foreground-service
  notification with transport controls, and answers wheel/Bluetooth/Android-Auto commands
  (play, pause, next) through standard MediaSession callbacks. `next` is wired to "shuffle a
  new station".
- **Catalog client:** Kotlin (Retrofit/OkHttp or plain `HttpURLConnection`) hitting
  radio-browser (`https://<mirror>/json/stations/...`). Provides a list to shuffle from; picks
  a random playable station (non-empty resolved URL).
- **Player controller:** thin layer mapping app actions (shuffle / play / stop) to ExoPlayer +
  MediaSession state, and reflecting playback state back to the UI.
- **UI (single screen):** one Activity (Jetpack Compose or Views) rendering the amber-CRT
  design from `docs/design/mini/android.jsx`: station name, now-playing line, spectrum
  placeholder, ⇄ SHUFFLE primary + ▶/⏸. Minimal — the real control surface is the
  notification / wheel.

## Data flow

```
launch ─→ MediaSessionService starts ─→ catalog client fetches stations (radio-browser)
shuffle (UI button OR wheel "next") ─→ pick random station ─→ ExoPlayer.setMediaItem(url) + play
play/stop (UI OR wheel/lock-screen) ─→ MediaSession callback ─→ ExoPlayer play/pause
playback state ─→ MediaSession metadata ─→ lock screen + Android Auto + wheel display
```

## Steering-wheel / car controls

This is the key feature and it comes from doing media right, not from custom code:
- A `MediaSessionService` + `MediaSession` with a `Player` automatically surfaces transport
  controls to: lock screen, notification, Bluetooth AVRCP (steering-wheel buttons over BT),
  and Android Auto.
- Map the standard commands: **play/pause** → ExoPlayer play/pause; **next** → shuffle a new
  station; **previous** → previous station in history (or also shuffle if no history).
- Declare the foreground-service type `mediaPlayback` and the media button receiver so the OS
  routes hardware/wheel media keys to the session.

## Battery

- Background playback is the only long-running work; the `MediaSessionService` is a foreground
  service with a media notification — the lightest standard way to keep audio alive.
- No polling loops, no always-on UI. The Activity is destroyed when backgrounded; the service
  keeps playing. Spectrum/visualizer is **off by default** (a visualizer would force the screen
  on and drain battery) — static or absent in MVP.
- ExoPlayer handles buffering efficiently; use a modest buffer to avoid excess network wakeups.

## Project layout

A new Gradle/Kotlin project at `android/` in the repo root (outside the Cargo workspace). It
does not touch the Rust crates. Package id e.g. `net.r4dio.android`.

## Firebase

- Create a new Firebase project for the radio app (the user already uses Firebase for other
  projects; CLI access confirmed).
- Register the Android app (package id) → download `google-services.json` into `android/app/`.
- **App Distribution** is the only Firebase product wired in MVP: build a release/debug APK and
  `firebase appdistribution:distribute` it to the user's phone (tester group). Crashlytics /
  Analytics / Auth are out of scope for tomorrow.

## Testing

- **Catalog client / shuffle pick:** unit-testable Kotlin (JVM tests) — random pick skips
  stations with empty URLs; returns null on empty list. These mirror the desktop `pick_random`
  contract.
- **Player/service:** instrumented behaviour is manual on-device (audio + wheel cannot be unit
  tested) — manual smoke: app plays on launch, survives screen-off, lock-screen controls work,
  Bluetooth/wheel play-pause-next works in the car.

## Out of scope (MVP / tomorrow)

- Favorites + sync with desktop (no sync server yet).
- Offline cache, station search UI, filters.
- Google Play release (App Distribution only for now).
- Crashlytics / Analytics.
- iOS.
- Reusing Rust `radio-core` on Android (separate future effort).
- A live audio-reactive spectrum (battery cost).

## Visual target

The amber-CRT look from `docs/design/mini/android.jsx`: rounded panel, `r4dio` wordmark,
station name + now-playing line, ⇄ SHUFFLE primary, play/stop, and a media-style notification
with ★ / ⇄ / ▶⏸ controls. First iteration targets a clean minimal version of this; polish later.
