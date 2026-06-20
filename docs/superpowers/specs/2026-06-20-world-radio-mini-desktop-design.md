# World Radio Mini (Desktop) — design

## Purpose

A tiny tray / menu-bar companion to World Radio that lets you start listening and
shuffle stations without opening the full TUI. The headline action is **Shuffle** —
one click and a station plays. It carries the app's retro-CRT identity (amber on
near-black, scanlines, a small FFT spectrum). Design is already delivered (see
`docs/design/mini/`); this spec covers the desktop implementation.

Scope of this spec: **desktop only** (macOS first, then Linux/Windows with the same
code). Android is a separate future spec — different paradigm, not covered here.

## Architecture

Pure Rust, reusing the existing workspace. Two new crates plus a refactor:

```
radio-core   (exists)  portable: catalog, favorites, health, pure audio logic. No UI, no cpal.
radio-audio  (NEW)     the AudioEngine moved out of radio-tui (cpal + symphonia + ringbuf).
                       Exposes play/stop/set_volume/poll_status/read_tap. Depends on radio-core.
radio-tui    (exists)  now depends on radio-audio instead of owning the engine (refactor only).
radio-mini   (NEW)     desktop tray app: tray-icon + egui. Depends on radio-core + radio-audio.
```

**GUI stack:** `tray-icon` for the menu-bar/tray icon and context menu; **egui** for the
mini window. egui is chosen over iced because the window is a small custom-drawn panel
(amber text, scanline overlay, FFT bars drawn directly with egui's painter) and egui keeps
the binary small with an immediate-mode painter that suits per-frame spectrum redraw.

**Why two new crates:** the architecture rule is portable core + native front-end crates
(see the multiplatform-architecture memory). The AudioEngine currently lives in
`radio-tui/src/audio/` (1026 lines, 5 files: mod/slot/stream/output/ring) and pulls
symphonia/cpal/ringbuf. Both TUI and Mini need playback, so it must be shared, not
duplicated.

## The AudioEngine boundary (what radio-audio exposes)

The engine is already a clean message-driven API; the move is mechanical:
- `AudioEngine::spawn() -> Result<Self>`
- `play(&self, url: &str)`, `stop(&self)`, `set_volume(&self, f32)`, `set_crossfade(&self, bool)`
- `poll_status(&self) -> Option<Status>` (Status from `radio_core::audio::command`)
- `read_tap(&self, out: &mut [f32]) -> usize` — raw post-mix samples for visualization.

The FFT itself is NOT in the engine — `radio-tui/src/tui/spectrum.rs` runs rustfft on the
tap. Mini does the same: reads the tap, runs its own small FFT for the bars. (A future
cleanup could share a spectrum helper, but it is out of scope here.)

## radio-mini components

Focused files, one responsibility each:

- `main.rs` — startup: build the tray icon, create `MiniState`, spawn the AudioEngine, run
  the event loop, wire repaint ticks.
- `tray.rs` — the menu-bar/tray icon (idle vs playing state) and click handling: a click
  toggles the mini window; on macOS the window is a popover anchored under the icon.
- `window.rs` — the egui CRT mini window (~260×120): now-playing line (station + ICY track,
  marquee if long), Shuffle button, Play/Stop, volume, FFT spectrum bars.
- `menu.rs` — the context menu: Shuffle (all), Shuffle favorites, Play/Stop, Open World
  Radio, Quit.
- `state.rs` — `MiniState` (current station, playback status, volume, shuffle scope) and the
  bridge to AudioEngine; pure transition logic lives here.
- `theme.rs` — palettes (amber-crt default + one or two alternates) lifted from the design
  tokens in `docs/design/mini/themes.js`.

## States (from the design)

Playing · Stopped/idle · Buffering · Error/offline. Each renders in the window and is
reflected in the tray icon (idle vs a playing equalizer). The status comes from
`poll_status()`; buffering/error map from the engine's `Status`.

## Data flow

```
tray click / menu  ->  MiniState (mutated on the UI thread)
Shuffle            ->  radio-core: pick a random station (scope all|favorites)  ->  url
                   ->  radio-audio: AudioEngine.play(url)
                   <-  poll_status()  ->  Status (Buffering/Playing/Error)
                   <-  read_tap()     ->  FFT  ->  spectrum bars
data/ (SQLite + favorites.json)  <-  read via radio-core (read-only in Mini)
```

State flows one way: actions become engine commands; status and spectrum are polled back —
the same pattern the TUI uses. egui repaints on a tick (≈20–30 fps) for the live spectrum
and marquee.

**Shared data dir:** Mini reads favorites and the station cache from the same `data/`
directory the TUI uses, through radio-core. Mini is read-only on that data (no sync yet).
Writing favorites stays with the main app for now; this avoids two processes contending for
the SQLite cache.

## Error handling

- Stream failed / no network → Error/offline state in the window, idle tray icon. Retry uses
  radio-audio's existing retry logic.
- `data/` absent (Mini launched without the main app ever run) → empty favorites; shuffle-all
  still works (pulls from radio-browser via core).
- Another process holds the SQLite cache → Mini degrades to shuffle-all, never panics.

## Testing

Unit tests (no audio hardware, no display):
- **radio-audio:** the AudioEngine tests move with it from radio-tui and must stay green —
  this is the safety net proving the refactor changed nothing.
- **radio-mini `state.rs`:** shuffle selection (scope all/favorites, skipping dead stations),
  state transitions (idle→buffering→playing→error), volume clamp.
- **radio-mini `theme.rs`:** palette parse/select.
- **radio-tui:** its existing 244 tests stay green after it switches to depending on
  radio-audio.

Not unit-tested (needs hardware/display): real cpal output, egui rendering, the tray icon —
these are a manual smoke test on macOS.

MVP gate: `cargo test --workspace` green + a manual macOS smoke test (menu-bar icon appears,
click opens the popover, Shuffle plays a station, Play/Stop works, volume works).

## MVP scope

In: macOS menu-bar icon + popover, the CRT mini window, Shuffle (all + favorites), Play/Stop,
volume, now-playing, a live FFT spectrum, amber-crt + one alternate theme.

Out (later): Linux/Windows packaging, the full 7-theme set, the complete icon set, Mini
writing favorites, cross-device sync, Android.

## Open questions

- macOS popover vs a plain always-on-top window for MVP — popover is nicer but more work;
  start with whichever `tray-icon` + egui support most cleanly on macOS, note the choice.
- Does Mini share the TUI's running AudioEngine or own its own? For MVP, Mini owns its own
  engine (the TUI and Mini are separate processes); a shared audio daemon is a future option.
