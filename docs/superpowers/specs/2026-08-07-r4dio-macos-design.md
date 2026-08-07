# r4dio macOS — Design

**Status:** approved 2026-08-07

## What this is

A menubar surface for macOS: a template icon in the status bar, left-click drops a 272px CRT
panel, right-click opens a short native menu. Same product as the CLI and the Android app, same
shuffle-first idea — one action in the middle, no station list and no search.

It is **not** a second product. The relationship to the CLI is the one Android already has.

## Where the code stands

`crates/radio-mini/` has a **complete Rust backend, 719 lines**, depending on `radio-core` and
`radio-audio`, so it reuses the real catalogue, audio engine and sync. Twelve Tauri IPC commands
exist: `shuffle`, `play_last`, `resume`, `stop`, `set_volume`, `set_scope`, `toggle_favorite`,
`now_state`, `spectrum`, `sync`, `set_sync_key`, `clear_sync_key`.

**Two things are missing, and the second is easy to overlook:**

1. **The UI does not exist.** `ui/index.html` is 11 lines containing one `<div>`. No JS, no CSS.
2. **There is no tray at all.** `main.rs` builds Tauri with the twelve commands but never creates a
   `TrayIcon`, and the `popover` window is configured `visible: false`. Built and run today, the
   app starts, begins playing the last station, and shows nothing — with no way to open or quit it.

So the work is three pieces, not one: the tray (Rust, absent), the panel (HTML/CSS/JS, absent), and
renaming plus shipping (partial).

## Naming

| What | Now | Becomes |
|---|---|---|
| Bundle identifier | `net.r4dio.mini` | **`net.r4dio.macos`** |
| productName (user-visible) | World Radio Mini | **r4dio** |
| Binary | `world-radio-mini` | **`r4dio-macos`** |
| Cargo package | `radio-mini` | **`r4dio-macos`** |
| Crate directory | `crates/radio-mini` | **`crates/r4dio-macos`** |
| version | `1.2.0`, hardcoded and stale | CI-stamped like the others |

The identifier follows the domain the user controls (`r4dio.net`) rather than a nickname, and it is
permanent: macOS ties TCC permissions and Keychain entries to it. Note Android's `net.vchub.r4dio`
stays as it is — it cannot be changed for existing installs, and the mismatch is harmless because
Android is not signed or published through a store.

`tauri.conf.json` currently hardcodes `version 1.2.0` while the workspace is far ahead; the version
must be stamped in CI like `Cargo.toml` and `build.gradle.kts` already are.

## The tray

Left-click toggles the panel, positioned under the icon (`tauri-plugin-positioner` is already a
dependency). Right-click opens a native menu: `Shuffle`, `Play/Stop`, `Open r4dio`, `Quit`.

Two behaviours a menubar app is broken without:

- **Close on focus loss.** Without it the panel hangs over everything (`alwaysOnTop: true`).
- **No dock icon** — `ActivationPolicy::Accessory`. Otherwise an app with no windows sits in the dock.

**The icon is static**, deliberately departing from the design's three animated states (idle /
playing / buffering). macOS template images are flattened to a solid alpha mask, and the idle
variant's 1.8 stroke at 0.62 opacity would smear into grey at 16pt — the same failure mode that
kept `ic_stat_r4dio_waves.svg` out of the Android build. One solid `▌` plus waves, no animation:
state is visible in the panel, and a blinking menubar icon is an irritant.

Out of scope: global hotkeys (a separate macOS permission and an Accessibility rabbit hole),
launch-at-login, and a theme menu.

## The panel

Ported from `mini/mini-window.jsx` in the August design pack — note this is **newer** than the copy
in `_private/design-docs/`, and differs in two ways that matter: the wordmark is already rebranded
to `▌r4dio` (amber `▌` and `4`, the rest in `fg`), and there is a **favourite star** that the older
version lacked.

```
▌r4dio  ● LIVE                    MX · AAC 48k
Smooth Jazz Café                            ★
▮▮▮▮▮▮▮▯▯▯▯▯▯▯▯▯          VOL ▮▮▮▮▯▯
┌────────────────────────────┐  ┌────┐
│        ⇄ SHUFFLE           │  │ ⏸  │
└────────────────────────────┘  └────┘
[ ALL │ ★ FAVS ]                shuffle scope
```

Four interactive elements: shuffle, play/pause, the star, the scope segmented control.

**Geometry, verbatim from the design:** panel `w = 272`, `radius = 14`, padding `11px 13px 12px`,
7px between rows. Primary button `8px 12px` at radius 5; secondary `7px 9px`. Scope segments
`2px 6px` at radius 3. The Tauri window is 300×220 — the extra 28px of width is the popover's
shadow gutter, not a mistake.

**Four states**, differing in content, not just colour:

| State | Indicator | Title | Meta | Primary button | Star |
|---|---|---|---|---|---|
| playing | olive `LIVE`, pulsing | station | `MX · AAC 48k` | SHUFFLE | shown |
| idle | grey `IDLE` | "Nothing playing" | `—` | SHUFFLE | **hidden** |
| buffering | amber `···`, pulsing | station | country · codec | SHUFFLE | shown |
| error | red `OFFLINE` | station | country | **RETRY** | shown |

The star is hidden in idle because there is nothing to star (`{!isStopped && <FavStar/>}` in the
design).

**The second line of the mockup is dropped.** It shows a live ICY track with a marquee, and the
project has **no ICY support anywhere** — verified on the Android work, and `NowState.track` is
always empty. Metadata (`country · codec · bitrate`) goes in the top right, where the design
already puts it. This matches the decision taken for the Android widget.

The design renders metadata with a flag emoji (`🇲🇽 MX · AAC 48k`). Emit the country code only;
mapping codes to flags is presentation sugar that can be added later without touching the backend.

**The spectrum is decorative and honest about it.** `spectrum()` currently returns a hardcoded
array — it is not audio. The bars animate in CSS while playing and freeze when paused; `spectrum()`
is not called at all. A real FFT would mean plumbing samples through `radio-audio` and polling over
IPC at 60fps, which is a separate feature.

**Volume** is six segments, an indicator rather than a slider. Clicking segment N sets volume N/6 —
simpler than drag, and easier to hit in a 272px window.

## Implementation shape

Three static files — `ui/index.html`, `ui/app.css`, `ui/app.js`. No npm, no bundler: Tauri serves
`frontendDist: "ui"` as plain files, and adding a JS toolchain to a Rust repo for ~300 lines of
markup is not worth the weight.

**Backend change, the only one:** `StationPick` carries just `uuid`, `name`, `url`, and
`NowState.meta` is the literal `"live"`. Country, codec and bitrate must be threaded from the
catalogue into both. The star needs nothing — `toggle_favorite` and `NowState.is_favorite` already
exist.

**Polling:** the panel calls `now_state()` once a second while visible and stops the timer when
hidden. A closed popover must not wake the CPU.

**Testability:** the decisions — `stateLabels(phase)`, `metaLabel(station)`, `volumeSegments(v)` —
live in a separate module as pure functions with tests, the same pattern as `HomeState.kt` and
`WidgetState.kt` on Android. This repo's dominant defect class is "correct code never reached on
the path that matters", seven instances so far, and pure functions are what made those catchable.

## Shipping

CI compiles the crate today — that is why it installs gtk/webkit deps at `ci.yml:31-35` and
`:229-230` — but publishes no artefact. It needs a bundle step and a release asset, alongside the
existing CLI tarballs and APK.

Checked while writing this: **CI never names `radio-mini` anywhere**; the crate is built only via
the workspace. So the rename does not break CI and needs no workflow edits — only
`Cargo.toml`'s `members` list and the directory itself.

Signing and notarisation are **out of scope for v1**: without an Apple Developer ID Gatekeeper will
warn, and the app is distributed the same way the APK is — as a file from r4dio.net for someone who
already trusts the source. Worth revisiting, not worth blocking on.

## Out of scope for v1

Station list, search, history, a favourites list screen, themes (amber only, though seven exist in
`themes.js`), ICY track titles, a real FFT, Linux and Windows trays, the account sheet in
`mini/account.jsx` (sync works through the existing commands), signing and notarisation.
