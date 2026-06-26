# World Radio Mini — Popup Redesign (Design Spec)

**Date:** 2026-06-26
**Status:** approved for planning
**Scope:** this spec covers the **visual redesign of the mini popup** (layout + amber-CRT
theme tokens) **plus a working favorite toggle** (FavStar writes to `favorites.json`).
Tray-popover behaviour (hide Dock, borderless window, click-to-toggle) is a **follow-up**
spec, intentionally out of scope here.

## Goal

Replace the current bare egui widgets in `radio-mini` with the designed amber-CRT panel from
`docs/design/mini/` (`mini-window.jsx` + `themes.js`). The panel reflects all four playback
states with a consistent retro look, so the idle "Nothing playing" state reads as a finished
screen, not raw text.

## Reference

- `mini-window.jsx` — the ~260px CRT panel: header, station/now-playing, spectrum+volume,
  controls, scope. Components: `WRMark`, `StateDot`, `FavStar`, `Spectrum`, `VolBar`,
  `ShuffleScope`, `CtrlBtn`.
- `themes.js` — `amber` palette tokens.
- Screenshots: `mini-mac.png`, `mini-rework.png` (4 states · Amber CRT).

## Decisions (from brainstorm)

1. **Depth this iteration:** layout + theme tokens. Static spectrum bars (no live FFT
   animation, no scanline overlay, no glow, no marquee yet) — those are a later polish pass.
2. **Themes:** extend `Theme` with the full token set but implement **amber-crt only** now.
3. **Order:** layout first; tray-popover behaviour is the next spec.

## Theme token extension

The current `Theme` has `bg, fg, hi, dim, ok, warn, err, accent`. The design needs more.
Extend `Theme` (in `crates/radio-mini/src/theme.rs`) with:

| token | amber value | use |
|---|---|---|
| `panel` | `#1b1510` | inset surfaces (volume segs, scope box) |
| `rule` | `#3a2c17` | hairline borders / insets |
| `bright` | `#fff0c0` | station name, max-emphasis text |
| `scan` | `0.16` (f32) | scanline strength — **stored, not rendered yet** |
| `glow` | `#d49a3a` | text glow — **stored, not rendered yet** |
| `light` | `false` (bool) | light-theme flag for future paper theme |

`name`/existing tokens stay. `nord()` stays as the alternate (already present). New tokens get
amber values; the `#[allow(dead_code)]` on the struct already covers unused-for-now fields.

## Layout (top to bottom, ~260px wide panel)

A single `egui` panel, vertical stack, ~7px gaps. Built in `app.rs` `ui()`:

1. **Header row:** `▌r4dio` wordmark (hi-colored ▌ and 4) · state dot + label
   (`● LIVE` / `IDLE` / `··· buffering` / `OFFLINE`, colored by state) · right-aligned meta
   (`🇲🇽 MX · AAC 48k` or `—` when idle).
2. **Station block:** station name (bright, bold, ellipsized) + now-playing line under it
   (track text when playing; "press Shuffle to start listening" when idle; "connecting to
   stream…" when buffering; "stream offline — couldn't connect" when error). A `FavStar`
   (☆/★) sits to the right when a station is loaded — **functional this iteration**: clicking
   toggles the now-playing station's favorite (see "Favorite toggle" below).
3. **Spectrum + volume row:** static spectrum bars (16 bars, dim when idle/error, hi when
   playing) on the left; segmented `VOL` meter (6 segments) on the right.
4. **Controls row:** primary `⇄ SHUFFLE` button (wide, hi background) + secondary play/stop
   button (`▶`/`⏸`). On error the primary reads `⇄ RETRY`.
5. **Scope row:** `ALL` / `★ FAVS` segmented toggle on the left, "shuffle scope" label right.

## State → content mapping

Reuse the existing `Phase` enum (`Idle`/`Buffering`/`Playing`/`Error`) — already mapped from
the audio engine. Per state:

| Phase | dot | station | now-playing line | primary btn |
|---|---|---|---|---|
| Idle | `IDLE` (dim) | "Nothing playing" (dim) | press Shuffle to start listening | SHUFFLE |
| Buffering | `···` (warn) | station name | connecting to stream… | SHUFFLE |
| Playing | `LIVE` (ok) | station name (bright) | now-playing / track | SHUFFLE |
| Error | `OFFLINE` (err) | last station | stream offline — couldn't connect | RETRY |

Real metadata (country/codec/bitrate) comes from the playing `StationPick`; when absent show
`—`. Track/now-playing text uses the engine `Status::Playing { title }` when present, else the
station name.

## Components → egui

Each `*.jsx` component maps to a small egui helper in `app.rs` (or a new `widgets.rs` if
`app.rs` grows past ~300 lines — split by the 600-800 line rule):

- `WRMark` → colored `RichText` spans.
- `StateDot` → small filled circle (`ui.painter`) + label.
- `Spectrum` → row of `rect_filled` bars from a fixed seed array `[5,7,4,8,...]` (static).
- `VolBar` → 6 segment rects, filled up to `volume * 6`.
- `ShuffleScope` → two `selectable_label`s styled as a segmented box.
- `CtrlBtn` → styled `Button` (primary = hi bg + bg-colored text).
- `FavStar` → ☆/★ toggle button reflecting `catalog.is_favorite(now.uuid)`; click toggles.

## Favorite toggle (functional)

Mirror the TUI's persistence path. Today `MiniApp::new()` builds a `Catalog`, reads the
station lists, and drops it. To write favorites the app must **keep the `Catalog`** plus the
data-dir paths.

`MiniApp` gains:
- `catalog: Catalog` (owned, mutable)
- `fav_path` / `hist_path` / `blacklist_path: PathBuf` (for `save_state`)

`fn toggle_favorite(&mut self)`:
1. take `now.uuid` (do nothing if nothing is loaded)
2. `self.catalog.toggle_favorite(&uuid)`
3. `self.catalog.save_state(&fav_path, &hist_path, &blacklist_path)` — persist; log on error
4. reload the favorites list into `MiniState` (`catalog_src::favorite_stations`) so the
   `★ FAVS` scope stays current

`FavStar` shows ★ when `catalog.is_favorite(now.uuid)`, ☆ otherwise. This writes the same
`favorites.json` the TUI reads, so favorites made in mini show up in the TUI and vice-versa —
and it is the local-write half that a future sync feature pushes to the server.

## Architecture & data flow

The existing `MiniState` (`phase`, `now`, `volume`, `scope`, `all`/`favorites`) holds the view
state. The redesign is mostly a view change + theme tokens, with one structural addition:
`MiniApp` now **owns the `Catalog`** and the data-dir paths so the favorite toggle can persist
(previously the catalog was dropped after `new()`). The `logic()`/`ui()` split from eframe
0.35 stays: `logic()` polls status + sets visuals; `ui()` renders the panel and handles the
FavStar click.

```
MiniState ─────────► ui(): build panel from phase + now + volume + scope
Theme (extended) ──┘        FavStar click ─► toggle_favorite():
Catalog (owned) ───────────► toggle + save_state + reload favorites scope
```

## Testing

The panel is egui-rendered (not unit-testable directly), so tests cover the pure pieces:

- **Theme tokens:** `amber()` returns the new tokens with expected values (e.g. `panel` dark,
  `bright` light, `scan > 0`). Extends the existing theme tests.
- **State→label mapping:** a pure `fn state_labels(phase) -> (dot_label, primary_label)`
  helper, tested for all four phases (e.g. Error → ("OFFLINE", "RETRY")).
- **Spectrum bar count:** static `spectrum_bars(n)` returns `n` values in range.
- **Favorite toggle (pure-ish):** against an in-memory `Catalog` (as `catalog_src` tests
  already do), toggling a uuid then reloading `favorite_stations` reflects the change; a
  second toggle removes it. Persistence round-trip (`save_state` → `Favorites::load`) is
  already covered in `radio-core`, so the mini test focuses on the toggle+reload wiring.
- Existing 17 mini tests stay green.

Manual smoke test (macOS): `cargo run -p radio-mini` — window shows the amber-CRT panel in all
four states (idle on launch; shuffle → buffering → live; kill network → error/retry); volume
meter and ALL/FAVS toggle reflect state.

## Out of scope (explicit)

- Tray-popover behaviour (hide Dock, borderless, click-to-toggle) — **next spec**.
- Live FFT spectrum animation, scanline overlay, glow, marquee — later polish.
- The other 4 themes (blue/neon/green/paper) and a theme switcher.
- **Sync** (pushing favorites to a server) — separate feature track; this iteration only does
  the local `favorites.json` write.
- Android / Windows / Linux tray variants.
