# radio-mini on Tauri v2 — macOS Menu-Bar Popover (Design Spec)

**Date:** 2026-06-27
**Status:** approved for planning
**Replaces:** the eframe/egui implementation of `radio-mini`.

## Why

The eframe/egui mini fights the framework: eframe owns the winit event loop and does **not**
process events while the window is hidden, so tray-click-to-show requires hacks (force-hide,
native winit visibility toggles, objc2 workarounds). Real macOS menu-bar apps (e.g. Proton
VPN) use `NSStatusItem` + an `NSPopover`-style panel anchored under the icon. Tauri v2 is built
for exactly this: it owns the event loop and ships first-class tray + window show/hide, so the
popover behaviour is native, not hacked.

## Goal

`radio-mini` becomes a Tauri v2 app: a menu-bar tray icon whose left-click drops a frameless,
amber-CRT popover anchored under it (like Proton VPN), hiding on focus loss. No Dock icon. The
popover plays/shuffles/favorites stations, resumes the last-played station on launch, and
shares favorites/history with the TUI.

## Decisions (from brainstorm)

1. **Frontend:** plain HTML/CSS/JS (no React/Vite, no npm build step). Port the design from
   `docs/design/mini/` (`mini-window.jsx` + `themes.js`) into one `index.html` + `style.css` +
   `app.js`.
2. **Crate:** replace `radio-mini` in-place (eframe → tauri). Same binary `world-radio-mini`,
   same role. Old egui code is deleted; pure logic is preserved.
3. **Audio:** stays in the Rust backend via `radio-audio::AudioEngine` (cpal/symphonia/
   crossfade/spectrum). The webview never touches audio — JS only sends commands.

## Architecture

- **Backend (Rust):** `radio-mini` is a Tauri app. It reuses, unchanged:
  - `radio-core` — catalog, favorites/history, `paths`, `single_instance`.
  - `radio-audio` — `AudioEngine` (play/stop/volume/poll_status/read_tap).
- **Frontend (web):** `index.html` + `style.css` + `app.js` (vanilla). Renders the amber-CRT
  panel; calls the backend via Tauri `invoke`; draws the spectrum on a `<canvas>`.
- **Tray + popover:** Tauri `TrayIconBuilder`; a frameless transparent window
  (`decorations:false`, `transparent:true`, `alwaysOnTop:true`, `skipTaskbar:true`,
  `visible:false`); `tauri-plugin-positioner` with `Position::TrayBottomCenter` anchors it
  under the icon. `activationPolicy: "accessory"` removes the Dock icon.

Why this removes the hacks: Tauri owns the event loop and provides supported window show/hide
and tray-event APIs — the loop does not sleep when the window is hidden, so tray clicks always
arrive.

## Backend — Tauri commands

App state (catalog + audio engine + current selection) lives in Tauri `State<Mutex<...>>`.
JS calls these via `invoke`:

| Command | Behaviour (reusing existing code) |
|---|---|
| `shuffle(scope)` | `state::pick_shuffle` (by scope) → `engine.play` → `catalog.record_history` → `save_state` |
| `play_last()` | `catalog_src::last_played` → play it (launch auto-start); falls back to shuffle if history empty |
| `resume()` | play the current `now` station (Play after Stop); shuffle if none |
| `stop()` | `engine.stop`, phase→idle, **keeps `now`** so resume works |
| `set_volume(v)` | `engine.set_volume` |
| `set_scope(scope)` | switch all/favorites |
| `toggle_favorite()` | `catalog_src::toggle_and_reload` → `save_state`; same `favorites.json` the TUI reads |
| `now_state()` | returns JSON `{ station, track, phase, volume, scope, is_favorite, meta }` for the UI to render |
| `spectrum_bars()` | `engine.read_tap` → FFT → N bar magnitudes (0..1) for the canvas |

`phase` is one of `idle | buffering | playing | error`, mapped from `radio_audio::Status` (the
existing mapping logic moves into the backend).

## Data flow

```
tray left-click ─(Tauri tray event)→ toggle frameless window visibility
                                      + positioner TrayBottomCenter
JS button ──invoke(cmd)──→ #[tauri::command] ──→ AudioEngine / Catalog ──→ now_state JSON
JS poll loop (~250 ms) ──invoke now_state + spectrum_bars──→ update DOM + <canvas>
window focus lost (WindowEvent::Focused(false)) ──→ hide window
```

Spectrum delivery: start with polling `spectrum_bars()` on the same ~250 ms loop while the
popover is **visible**; if that proves too coarse for a smooth spectrum, switch to Tauri
`emit` events pushing bars from Rust at ~15 fps. (Decide during planning; polling is the
simpler first cut and only runs while the popover is open.)

## Single instance

The repo already has `radio_core::single_instance::take_over()` (shared by TUI). The Tauri app
keeps calling it on startup so "last launch wins" still holds across mini + TUI. (Tauri's own
`tauri-plugin-single-instance` is per-app, so we keep the shared core lock instead.)

## What is reused / preserved / deleted

- **Reused unchanged:** `radio-core` (incl. `single_instance`), `radio-audio`.
- **Preserved (moved into the Tauri backend):** the pure logic — `pick_shuffle`, scope
  handling, `state_labels`, `catalog_src` (`all_stations`, `favorite_stations`, `last_played`,
  `toggle_and_reload`), and the `Status`→phase mapping. These keep their unit tests.
- **Deleted:** `app.rs` (egui ui/logic), `theme.rs` (egui `Color32`), `tray.rs` (eframe tray);
  the `eframe`/`egui`/`winit` dependencies. Theme colours move into CSS variables (from
  `themes.js`).

## Testing

- **Rust:** the pure functions stay unit-tested (they already are — they move, tests move with
  them). Tauri commands are thin wrappers over that tested logic.
- **Web UI:** not unit-tested; manual macOS smoke test.
- **Tray/popover:** manual — left-click shows the popover anchored under the icon; clicking
  away hides it; it reads as a native menu-bar utility, not a floating window; favorites and
  last-played persist and are shared with the TUI.

## Visual target

The popover matches the reference amber-CRT design (`mini-window.jsx`): rounded panel with
glow + scanline texture, `▌r4dio` wordmark + state dot, station name + marquee track line,
animated spectrum bars, segmented VOL, primary SHUFFLE (RETRY on error) + play/stop, ALL/★FAVS
scope, ☆/★ favorite toggle, and a quit affordance. HTML/CSS makes this polish straightforward
(unlike egui primitives), so the first iteration targets the full look, not a stripped-down
one.

## Risks

1. **Real-time spectrum over `invoke`** — polling FFT bars 15–30×/s may be inefficient.
   Mitigation: Tauri `emit` events instead of polling, or lower the rate; only runs while the
   popover is open.
2. **WKWebView transparency + frameless rounding/shadow** on macOS — minor; the positioner
   plugin and `transparent:true` cover the standard cases.
3. **First Tauri crate in the repo** — adds `tauri`/`tauri-build` and JS assets. CI (`ci.yml`)
   is `cargo`-only today; wiring a Tauri build into CI is a separate backlog item and does not
   block local development.

## Out of scope (first iteration)

- Windows / Linux tray variants.
- Launch-at-login (LaunchAgent plist).
- Auto-update.
- Sync (favorites to a server) — separate feature track.
- The other themes (blue/neon/green/paper) and a theme switcher.
