# World Radio Mini (Desktop) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A macOS menu-bar companion that shuffles and plays World Radio stations from a small retro-CRT window, reusing the existing catalog and audio engine.

**Architecture:** Extract the AudioEngine from `radio-tui` into a new shared `radio-audio` crate, then build a new `radio-mini` crate (tray-icon + egui) on top of `radio-core` + `radio-audio`. The TUI is refactored to depend on `radio-audio` (no behaviour change). Mini reads the shared `data/` directory through `radio-core`.

**Tech Stack:** Rust workspace; `tray-icon`, `egui` + `eframe`; existing `cpal`/`symphonia`/`ringbuf`/`rustfft`; `fastrand`.

**Reference design:** `docs/design/mini/` (window, themes, states, icons).

---

## Phase A — extract `radio-audio` (safe refactor, no behaviour change)

The whole phase is gated on **existing tests staying green**. Nothing new is built; the
AudioEngine just moves to its own crate so both TUI and Mini can use it.

### Task A1: Create the empty `radio-audio` crate

**Files:**
- Create: `crates/radio-audio/Cargo.toml`
- Create: `crates/radio-audio/src/lib.rs`
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: Add the crate to the workspace**

In `Cargo.toml`, change the members line to:
```toml
members = ["crates/radio-core", "crates/radio-audio", "crates/radio-tui"]
```

- [ ] **Step 2: Create `crates/radio-audio/Cargo.toml`** (deps copied from radio-tui's audio needs)

```toml
[package]
name = "radio-audio"
version = "1.2.0"
edition = "2021"
license = "MIT"
description = "Native audio engine for World Radio: streaming, decode, crossfade, output."
repository = "https://github.com/Chubik/world-radio"

[dependencies]
radio-core = { path = "../radio-core" }
anyhow = "1"
reqwest = { version = "0.12", default-features = false, features = ["blocking", "rustls-tls"] }
symphonia = { version = "0.6", features = ["mp3", "aac", "isomp4", "ogg", "vorbis", "flac", "pcm"] }
cpal = "0.17"
ringbuf = "0.5"

[dev-dependencies]
```

- [ ] **Step 3: Create a placeholder `crates/radio-audio/src/lib.rs`**

```rust
```
(empty file for now — modules are added in A2)

- [ ] **Step 4: Verify the workspace still builds**

Run: `cargo build`
Expected: compiles (radio-audio is empty but valid).

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/radio-audio
git commit -m "chore: add empty radio-audio crate to workspace"
```

### Task A2: Move the audio module files into radio-audio

**Files:**
- Move: `crates/radio-tui/src/audio/{mod.rs,slot.rs,stream.rs,output.rs,ring.rs}` → `crates/radio-audio/src/`
- Modify: `crates/radio-audio/src/lib.rs`

- [ ] **Step 1: Move the files with git**

```bash
mkdir -p crates/radio-audio/src
git mv crates/radio-tui/src/audio/mod.rs   crates/radio-audio/src/engine.rs
git mv crates/radio-tui/src/audio/slot.rs  crates/radio-audio/src/slot.rs
git mv crates/radio-tui/src/audio/stream.rs crates/radio-audio/src/stream.rs
git mv crates/radio-tui/src/audio/output.rs crates/radio-audio/src/output.rs
git mv crates/radio-tui/src/audio/ring.rs  crates/radio-audio/src/ring.rs
rmdir crates/radio-tui/src/audio
```

- [ ] **Step 2: Write `crates/radio-audio/src/lib.rs`** to declare the modules and re-export the public API

The old `audio/mod.rs` (now `engine.rs`) declared `pub mod output; pub mod ring; pub mod slot; pub mod stream;` and held the engine types. Set `lib.rs` to:

```rust
mod engine;
pub mod output;
pub mod ring;
pub mod slot;
pub mod stream;

pub use engine::{AudioEngine, SharedGain, SharedVolume};
pub use radio_core::audio::command::{Command, Status};
```

- [ ] **Step 3: Fix the module-internal paths in the moved files**

These files reference `crate::audio::X` (the old path). Inside radio-audio the crate root is now this crate, so replace `crate::audio::` with `crate::` in all moved files:
- `engine.rs`: line ~54 `use crate::audio::ring::SampleCons;` → `use crate::ring::SampleCons;`; line ~70 `use crate::audio::output::mix_output;` → `use crate::output::mix_output;`. Also remove the now-duplicated `pub mod ...;` lines at the top of `engine.rs` (they live in `lib.rs` now) and the `pub use radio_core::...` line (also in lib.rs).
- `slot.rs`: line 1 `use crate::audio::ring::{...}` → `use crate::ring::{...}`; line 2 `use crate::audio::{Command, SharedGain, Status}` → `use crate::{Command, SharedGain, Status}`; line ~299 `use crate::audio::stream;` → `use crate::stream;`.
- `output.rs`: line 1 `use crate::audio::ring::SampleCons;` → `use crate::ring::SampleCons;`; line ~28 `use crate::audio::ring::make_ring;` → `use crate::ring::make_ring;`.

Run `grep -rn "crate::audio" crates/radio-audio/src/` and fix any remaining occurrence to `crate::`.

- [ ] **Step 4: Build radio-audio alone**

Run: `cargo build -p radio-audio`
Expected: FAILS first if any `crate::audio` path remains — fix until it compiles.

- [ ] **Step 5: Run radio-audio's tests (moved from radio-tui)**

Run: `cargo test -p radio-audio`
Expected: the AudioEngine tests that moved with the files pass (e.g. `abort_is_non_blocking`).

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor: move audio engine into radio-audio crate"
```

### Task A3: Point radio-tui at radio-audio

**Files:**
- Modify: `crates/radio-tui/Cargo.toml`
- Modify: `crates/radio-tui/src/main.rs:1,4`
- Modify: `crates/radio-tui/src/tui/mod.rs:13`
- Modify: `crates/radio-tui/src/tui/update.rs:1`

- [ ] **Step 1: Add radio-audio as a dependency** and drop the now-unused direct audio deps from radio-tui

In `crates/radio-tui/Cargo.toml` `[dependencies]`, add:
```toml
radio-audio = { path = "../radio-audio" }
```
Remove these lines (now owned by radio-audio): `symphonia = ...`, `cpal = "0.17"`, `ringbuf = "0.5"`. Keep `reqwest` (the TUI worker still uses it) and `rustfft` (the spectrum FFT stays in the TUI).

- [ ] **Step 2: Fix `main.rs`**

Delete line 1 `mod audio;`. Change line 4 `use audio::AudioEngine;` to:
```rust
use radio_audio::AudioEngine;
```

- [ ] **Step 3: Fix `tui/mod.rs:13`**

Change `use crate::audio::AudioEngine;` to:
```rust
use radio_audio::AudioEngine;
```
Also check this file for `WorkerReq`/`Status` audio uses — change any `crate::audio::X` to `radio_audio::X` (grep below catches them).

- [ ] **Step 4: Fix `tui/update.rs:1`**

Change `use crate::audio::Status;` to:
```rust
use radio_audio::Status;
```

- [ ] **Step 5: Catch any remaining references**

Run: `grep -rn "crate::audio\|mod audio" crates/radio-tui/src/`
Expected: no matches. Fix any that remain (replace `crate::audio::` with `radio_audio::`).

- [ ] **Step 6: Build the whole workspace**

Run: `cargo build`
Expected: compiles.

- [ ] **Step 7: Run the FULL test suite — the refactor safety net**

Run: `cargo test --workspace`
Expected: all green — radio-tui's 244 tests + radio-audio's moved tests + radio-core. This proves the move changed nothing.

- [ ] **Step 8: Clippy + fmt**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "refactor: radio-tui depends on radio-audio crate"
```

---

## Phase B — scaffold `radio-mini` + pure state logic (tested, no GUI yet)

### Task B1: Create the radio-mini crate skeleton

**Files:**
- Create: `crates/radio-mini/Cargo.toml`
- Create: `crates/radio-mini/src/main.rs`
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: Add to workspace members**

In `Cargo.toml`:
```toml
members = ["crates/radio-core", "crates/radio-audio", "crates/radio-tui", "crates/radio-mini"]
```

- [ ] **Step 2: Create `crates/radio-mini/Cargo.toml`**

```toml
[package]
name = "radio-mini"
version = "1.2.0"
edition = "2021"
license = "MIT"
description = "World Radio Mini: a tray companion that shuffles and plays stations."
repository = "https://github.com/Chubik/world-radio"

[[bin]]
name = "world-radio-mini"
path = "src/main.rs"

[dependencies]
radio-core = { path = "../radio-core" }
radio-audio = { path = "../radio-audio" }
anyhow = "1"
fastrand = "2"
eframe = "0.28"
egui = "0.28"
tray-icon = "0.19"

[dev-dependencies]
```

- [ ] **Step 3: Create a minimal `crates/radio-mini/src/main.rs`**

```rust
mod state;

fn main() {
    println!("world-radio-mini");
}
```

- [ ] **Step 4: Build**

Run: `cargo build -p radio-mini`
Expected: FAILS — `state` module missing. That is fixed in B2; for this step temporarily remove `mod state;` to confirm the deps resolve, then re-add it before B2. To confirm deps: `cargo build -p radio-mini` after removing `mod state;` → compiles (downloads egui/eframe/tray-icon).

- [ ] **Step 5: Commit** (with `mod state;` present, even though it won't build yet — B2 lands immediately after)

Re-add `mod state;`, then:
```bash
git add Cargo.toml crates/radio-mini
git commit -m "chore: scaffold radio-mini crate"
```

### Task B2: Shuffle scope + station selection (pure, tested)

**Files:**
- Create: `crates/radio-mini/src/state.rs`

This is the core logic: given a list of stations and a scope, pick a random playable one.
Mirror the TUI's shuffle (skip stations the user can't play). Uses `fastrand`.

- [ ] **Step 1: Write the failing test** — add to the bottom of `src/state.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn st(uuid: &str, url: &str) -> StationPick {
        StationPick { uuid: uuid.into(), name: uuid.into(), url: url.into() }
    }

    #[test]
    fn pick_returns_none_for_empty() {
        assert!(pick_random(&[]).is_none());
    }

    #[test]
    fn pick_skips_stations_without_url() {
        let list = vec![st("a", ""), st("b", "http://x")];
        let p = pick_random(&list).unwrap();
        assert_eq!(p.uuid, "b");
    }

    #[test]
    fn pick_returns_a_playable_one() {
        let list = vec![st("a", "http://a"), st("b", "http://b")];
        let p = pick_random(&list).unwrap();
        assert!(p.uuid == "a" || p.uuid == "b");
        assert!(!p.url.is_empty());
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p radio-mini state`
Expected: FAIL — `StationPick` / `pick_random` not found.

- [ ] **Step 3: Implement at the top of `src/state.rs`**

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct StationPick {
    pub uuid: String,
    pub name: String,
    pub url: String,
}

pub fn pick_random(stations: &[StationPick]) -> Option<StationPick> {
    let playable: Vec<&StationPick> = stations.iter().filter(|s| !s.url.is_empty()).collect();
    if playable.is_empty() {
        return None;
    }
    let idx = fastrand::usize(..playable.len());
    Some(playable[idx].clone())
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p radio-mini state`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: random station pick for mini shuffle"
```

### Task B3: Playback state machine (pure, tested)

**Files:**
- Modify: `crates/radio-mini/src/state.rs`

Models the four UI states and volume, decoupled from the engine. The engine's `Status`
maps into these; volume clamps to 0..=1.

- [ ] **Step 1: Write the failing test** — add to the `tests` module in `src/state.rs`:

```rust
    #[test]
    fn starts_idle() {
        let m = MiniState::new();
        assert_eq!(m.phase, Phase::Idle);
        assert!(m.now.is_none());
    }

    #[test]
    fn shuffle_sets_buffering_and_now() {
        let mut m = MiniState::new();
        m.begin_play(st("a", "http://a"));
        assert_eq!(m.phase, Phase::Buffering);
        assert_eq!(m.now.as_ref().unwrap().uuid, "a");
    }

    #[test]
    fn stop_clears_to_idle() {
        let mut m = MiniState::new();
        m.begin_play(st("a", "http://a"));
        m.stop();
        assert_eq!(m.phase, Phase::Idle);
        assert!(m.now.is_none());
    }

    #[test]
    fn volume_clamps() {
        let mut m = MiniState::new();
        m.set_volume(1.5);
        assert_eq!(m.volume, 1.0);
        m.set_volume(-0.2);
        assert_eq!(m.volume, 0.0);
    }

    #[test]
    fn scope_toggles() {
        let mut m = MiniState::new();
        assert_eq!(m.scope, Scope::All);
        m.set_scope(Scope::Favorites);
        assert_eq!(m.scope, Scope::Favorites);
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p radio-mini state`
Expected: FAIL — `MiniState` / `Phase` / `Scope` not found.

- [ ] **Step 3: Implement — add above the `tests` module in `src/state.rs`**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Buffering,
    Playing,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    All,
    Favorites,
}

#[derive(Debug, Clone)]
pub struct MiniState {
    pub phase: Phase,
    pub now: Option<StationPick>,
    pub volume: f32,
    pub scope: Scope,
}

impl MiniState {
    pub fn new() -> Self {
        Self {
            phase: Phase::Idle,
            now: None,
            volume: 0.8,
            scope: Scope::All,
        }
    }

    pub fn begin_play(&mut self, pick: StationPick) {
        self.now = Some(pick);
        self.phase = Phase::Buffering;
    }

    pub fn stop(&mut self) {
        self.now = None;
        self.phase = Phase::Idle;
    }

    pub fn set_volume(&mut self, v: f32) {
        self.volume = v.clamp(0.0, 1.0);
    }

    pub fn set_scope(&mut self, scope: Scope) {
        self.scope = scope;
    }
}

impl Default for MiniState {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p radio-mini state`
Expected: 8 passed (3 from B2 + 5 here).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: mini playback state machine"
```

### Task B4: Map engine Status into Phase (pure, tested)

**Files:**
- Modify: `crates/radio-mini/src/state.rs`

The engine reports `radio_audio::Status`. Translate it into the UI `Phase` so the window
reflects buffering/playing/error. Check the actual Status variants first.

- [ ] **Step 1: Read the Status enum**

Run: `grep -n "pub enum Status" -A8 crates/radio-core/src/audio/command.rs`
Note the exact variant names (e.g. `Buffering`, `Playing`, `Stopped`, `Error`). Use those
exact names in the match below; if a variant differs, adjust the arm names to match.

- [ ] **Step 2: Write the failing test** — add to the `tests` module:

```rust
    #[test]
    fn status_playing_maps_to_playing() {
        let mut m = MiniState::new();
        m.begin_play(st("a", "http://a"));
        m.apply_status(radio_audio::Status::Playing);
        assert_eq!(m.phase, Phase::Playing);
    }

    #[test]
    fn status_error_maps_to_error() {
        let mut m = MiniState::new();
        m.begin_play(st("a", "http://a"));
        m.apply_status(radio_audio::Status::Error);
        assert_eq!(m.phase, Phase::Error);
    }
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p radio-mini status`
Expected: FAIL — `apply_status` not found (and possibly a Status variant name mismatch —
fix the test/impl to the real variant names from Step 1).

- [ ] **Step 4: Implement — add the method inside `impl MiniState`**

```rust
    pub fn apply_status(&mut self, status: radio_audio::Status) {
        use radio_audio::Status;
        self.phase = match status {
            Status::Playing => Phase::Playing,
            Status::Buffering => Phase::Buffering,
            Status::Error => Phase::Error,
            Status::Stopped => Phase::Idle,
        };
    }
```
(If the real Status enum has different/extra variants, cover them all — the match must be
exhaustive. Map any "connecting"/"opening" variant to `Phase::Buffering`, any stopped/idle to
`Phase::Idle`.)

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p radio-mini status`
Expected: 2 passed.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: map audio engine status to mini phase"
```

### Task B5: Catalog access — load stations for shuffle (integration-ish, tested with in-memory cache)

**Files:**
- Create: `crates/radio-mini/src/catalog_src.rs`
- Modify: `crates/radio-mini/src/main.rs` (add `mod catalog_src;`)

Provides the station lists Mini shuffles over, from the shared `data/` via radio-core.
`all_stations` reads the offline cache; `favorite_stations` resolves favorite ids. Both
return `Vec<StationPick>`. Tested against an in-memory `Catalog` so no disk is needed.

- [ ] **Step 1: Confirm the Catalog test constructors**

Run: `grep -n "pub fn new\|open_in_memory\|pub fn ingest\|pub fn toggle_favorite\|search_offline\b" crates/radio-core/src/catalog/catalog.rs crates/radio-core/src/catalog/cache.rs`
Use `Cache::open_in_memory()`, `Catalog::new(cache, health)`, `catalog.ingest(&[Station])`,
`catalog.toggle_favorite(uuid)`, `catalog.search_offline("")`, `catalog.favorite_ids()`,
`catalog.station_by_uuid(uuid)` — these exist (seen in catalog.rs).

- [ ] **Step 2: Write the failing test** — add to the bottom of `src/catalog_src.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use radio_core::catalog::{Cache, Catalog, Health, Station};

    fn station(uuid: &str, url: &str) -> Station {
        Station {
            stationuuid: uuid.into(),
            name: uuid.into(),
            url_resolved: url.into(),
            countrycode: String::new(),
            language: String::new(),
            tags: String::new(),
            codec: String::new(),
            bitrate: 0,
            geo_lat: None,
            geo_long: None,
        }
    }

    fn catalog() -> Catalog {
        let cache = Cache::open_in_memory().unwrap();
        let cat = Catalog::new(cache, Health::new());
        cat.ingest(&[station("u1", "http://one"), station("u2", "http://two")]).unwrap();
        cat
    }

    #[test]
    fn all_stations_lists_cached() {
        let cat = catalog();
        let picks = all_stations(&cat).unwrap();
        assert_eq!(picks.len(), 2);
        assert!(picks.iter().all(|p| !p.url.is_empty()));
    }

    #[test]
    fn favorite_stations_resolves_marked() {
        let mut cat = catalog();
        cat.toggle_favorite("u2");
        let picks = favorite_stations(&cat).unwrap();
        assert_eq!(picks.len(), 1);
        assert_eq!(picks[0].uuid, "u2");
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p radio-mini catalog_src`
Expected: FAIL — `all_stations` / `favorite_stations` not found.

- [ ] **Step 4: Implement at the top of `src/catalog_src.rs`**

```rust
use crate::state::StationPick;
use radio_core::catalog::Catalog;

fn to_pick(s: &radio_core::catalog::Station) -> StationPick {
    StationPick {
        uuid: s.stationuuid.clone(),
        name: s.name.clone(),
        url: s.url_resolved.clone(),
    }
}

pub fn all_stations(catalog: &Catalog) -> anyhow::Result<Vec<StationPick>> {
    let stations = catalog.search_offline("")?;
    Ok(stations.iter().map(to_pick).collect())
}

pub fn favorite_stations(catalog: &Catalog) -> anyhow::Result<Vec<StationPick>> {
    let mut out = Vec::new();
    for uuid in catalog.favorite_ids() {
        if let Some(s) = catalog.station_by_uuid(uuid)? {
            out.push(to_pick(&s));
        }
    }
    Ok(out)
}
```

- [ ] **Step 5: Wire the module** — in `src/main.rs` add `mod catalog_src;` (keep `mod state;`).

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p radio-mini catalog_src`
Expected: 2 passed.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: load all/favorite stations for mini shuffle"
```

---

## Phase C — GUI: tray + egui window (manual smoke test)

These tasks wire native UI; they can't be unit-tested, so each ends with a manual run on
macOS. Keep the visuals minimal first (correct behaviour), polish the CRT look last.

### Task C1: Theme tokens

**Files:**
- Create: `crates/radio-mini/src/theme.rs`
- Modify: `crates/radio-mini/src/main.rs` (add `mod theme;`)

Port the amber-crt palette (and one alternate) from `docs/design/mini/themes.js` into egui
`Color32` values.

- [ ] **Step 1: Write the failing test** — add to the bottom of `src/theme.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amber_is_default_and_dark() {
        let t = Theme::amber();
        assert_eq!(t.name, "amber-crt");
        // near-black background
        assert!(t.bg.r() < 40 && t.bg.g() < 40 && t.bg.b() < 40);
    }

    #[test]
    fn alternate_differs_from_amber() {
        assert_ne!(Theme::amber().hi, Theme::nord().hi);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p radio-mini theme`
Expected: FAIL — `Theme` not found.

- [ ] **Step 3: Implement at the top of `src/theme.rs`** (hex values from the design's themes.js)

```rust
use egui::Color32;

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub name: &'static str,
    pub bg: Color32,
    pub fg: Color32,
    pub hi: Color32,
    pub dim: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub err: Color32,
    pub accent: Color32,
}

impl Theme {
    pub fn amber() -> Self {
        Self {
            name: "amber-crt",
            bg: Color32::from_rgb(0x15, 0x10, 0x0b),
            fg: Color32::from_rgb(0xd4, 0x9a, 0x3a),
            hi: Color32::from_rgb(0xff, 0xc4, 0x57),
            dim: Color32::from_rgb(0x6e, 0x54, 0x30),
            ok: Color32::from_rgb(0x9e, 0xc0, 0x74),
            warn: Color32::from_rgb(0xff, 0xc4, 0x57),
            err: Color32::from_rgb(0xd9, 0x6a, 0x5a),
            accent: Color32::from_rgb(0xff, 0x8a, 0x3d),
        }
    }

    pub fn nord() -> Self {
        Self {
            name: "nord",
            bg: Color32::from_rgb(0x2e, 0x34, 0x40),
            fg: Color32::from_rgb(0xd8, 0xde, 0xe9),
            hi: Color32::from_rgb(0x88, 0xc0, 0xd0),
            dim: Color32::from_rgb(0x4c, 0x56, 0x6a),
            ok: Color32::from_rgb(0xa3, 0xbe, 0x8c),
            warn: Color32::from_rgb(0xeb, 0xcb, 0x8b),
            err: Color32::from_rgb(0xbf, 0x61, 0x6a),
            accent: Color32::from_rgb(0x81, 0xa1, 0xc1),
        }
    }
}
```

- [ ] **Step 4: Wire the module** — in `src/main.rs` add `mod theme;`.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p radio-mini theme`
Expected: 2 passed.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: mini theme palettes"
```

### Task C2: The egui app shell (window renders, no audio yet)

**Files:**
- Create: `crates/radio-mini/src/app.rs`
- Modify: `crates/radio-mini/src/main.rs`

Build the eframe app holding `MiniState` + `Theme`, rendering a small window with the
now-playing line, a Shuffle button, Play/Stop, a volume slider, and a placeholder spectrum
row. No engine wired yet — buttons just mutate state.

- [ ] **Step 1: Create `crates/radio-mini/src/app.rs`**

```rust
use crate::state::{MiniState, Phase, Scope};
use crate::theme::Theme;
use eframe::egui;

pub struct MiniApp {
    state: MiniState,
    theme: Theme,
}

impl MiniApp {
    pub fn new() -> Self {
        Self {
            state: MiniState::new(),
            theme: Theme::amber(),
        }
    }
}

impl eframe::App for MiniApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let t = self.theme;
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = t.bg;
        visuals.override_text_color = Some(t.fg);
        ctx.set_visuals(visuals);

        egui::CentralPanel::default().show(ctx, |ui| {
            let title = self
                .state
                .now
                .as_ref()
                .map(|n| n.name.clone())
                .unwrap_or_else(|| "Nothing playing".into());
            ui.colored_label(t.hi, title);

            let status = match self.state.phase {
                Phase::Idle => "idle",
                Phase::Buffering => "connecting…",
                Phase::Playing => "live",
                Phase::Error => "offline",
            };
            ui.colored_label(t.dim, status);

            ui.horizontal(|ui| {
                if ui.button("⤮ Shuffle").clicked() {
                    self.state.begin_play(crate::state::StationPick {
                        uuid: "demo".into(),
                        name: "Demo Station".into(),
                        url: "http://demo".into(),
                    });
                }
                let playing = self.state.phase == Phase::Playing
                    || self.state.phase == Phase::Buffering;
                if ui.button(if playing { "⏸ Stop" } else { "▶ Play" }).clicked() {
                    self.state.stop();
                }
            });

            ui.horizontal(|ui| {
                ui.label("vol");
                let mut v = self.state.volume;
                if ui.add(egui::Slider::new(&mut v, 0.0..=1.0).show_value(false)).changed() {
                    self.state.set_volume(v);
                }
            });

            let scope_all = self.state.scope == Scope::All;
            ui.horizontal(|ui| {
                if ui.selectable_label(scope_all, "all").clicked() {
                    self.state.set_scope(Scope::All);
                }
                if ui.selectable_label(!scope_all, "favorites").clicked() {
                    self.state.set_scope(Scope::Favorites);
                }
            });
        });
    }
}
```

- [ ] **Step 2: Update `main.rs` to launch the eframe app**

```rust
mod app;
mod catalog_src;
mod state;
mod theme;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([300.0, 180.0]),
        ..Default::default()
    };
    eframe::run_native(
        "World Radio Mini",
        options,
        Box::new(|_cc| Ok(Box::new(app::MiniApp::new()))),
    )
}
```
Add `use eframe::egui;` at the top if needed for `ViewportBuilder`.

- [ ] **Step 3: Build**

Run: `cargo build -p radio-mini`
Expected: compiles.

- [ ] **Step 4: Run tests (state/theme unaffected)**

Run: `cargo test -p radio-mini`
Expected: all green (15 tests).

- [ ] **Step 5: MANUAL smoke test (macOS)**

Run: `cargo run -p radio-mini`
Expected: a small window opens; clicking Shuffle shows "Demo Station / connecting…"; Stop
returns to "Nothing playing / idle"; the volume slider and all/favorites toggle work.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: mini egui window shell"
```

### Task C3: Wire the AudioEngine + catalog (real playback)

**Files:**
- Modify: `crates/radio-mini/src/app.rs`
- Modify: `crates/radio-mini/src/main.rs`

Replace the demo shuffle with a real one: load the shared catalog at startup, hold an
`AudioEngine`, and on Shuffle pick a real station and play it. Poll status each frame to
update the phase.

- [ ] **Step 1: Load catalog + engine in `MiniApp::new`**

Change `app.rs` to hold the engine and catalog:

```rust
use crate::state::{MiniState, Phase, Scope, StationPick};
use crate::theme::Theme;
use crate::{catalog_src, state};
use eframe::egui;
use radio_audio::AudioEngine;
use radio_core::catalog::{Cache, Catalog, Health};

pub struct MiniApp {
    state: MiniState,
    theme: Theme,
    engine: AudioEngine,
    catalog: Catalog,
}

impl MiniApp {
    pub fn new() -> anyhow::Result<Self> {
        let data = radio_core::paths::ensure_data_dir()?;
        let cache = Cache::open(&data.join("stations.db"))?;
        let health = Health::load(&data.join("station_health.json"));
        let catalog = Catalog::load(
            cache,
            health,
            &data.join("favorites.json"),
            &data.join("history.json"),
            &data.join("blocklist.json"),
        );
        let engine = AudioEngine::spawn()?;
        Ok(Self {
            state: MiniState::new(),
            theme: Theme::amber(),
            engine,
            catalog,
        })
    }

    fn shuffle(&mut self) {
        let list = match self.state.scope {
            Scope::All => catalog_src::all_stations(&self.catalog),
            Scope::Favorites => catalog_src::favorite_stations(&self.catalog),
        }
        .unwrap_or_default();
        if let Some(pick) = state::pick_random(&list) {
            self.engine.set_volume(self.state.volume);
            self.engine.play(&pick.url);
            self.state.begin_play(pick);
        }
    }
}
```

- [ ] **Step 2: Use real shuffle/stop/status in `update`**

In the `update` method: replace the demo Shuffle body with `self.shuffle();`; on Stop call
`self.engine.stop(); self.state.stop();`; when the volume slider changes also call
`self.engine.set_volume(v);`. At the top of `update`, poll status and request repaint:

```rust
        while let Some(status) = self.engine.poll_status() {
            self.state.apply_status(status);
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
```

- [ ] **Step 3: Make `main.rs` handle the fallible `new`**

```rust
    eframe::run_native(
        "World Radio Mini",
        options,
        Box::new(|_cc| match app::MiniApp::new() {
            Ok(a) => Ok(Box::new(a) as Box<dyn eframe::App>),
            Err(e) => {
                eprintln!("startup failed: {e}");
                std::process::exit(1);
            }
        }),
    )
```

- [ ] **Step 4: Build + test**

Run: `cargo build -p radio-mini && cargo test -p radio-mini`
Expected: compiles; 15 tests green.

- [ ] **Step 5: MANUAL smoke test (macOS)** — requires a populated cache

If `data/stations.db` is empty (Mini run before the TUI ever searched), shuffle finds
nothing. First populate it: `cargo run -p radio-tui -- --name jazz` (writes the cache), then:
Run: `cargo run -p radio-mini`
Expected: clicking Shuffle plays a real station; status moves connecting… → live; Stop
silences it; volume changes loudness; favorites scope plays a favorited station (favorite one
in the TUI first).

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: mini plays real stations via audio engine and catalog"
```

### Task C4: Tray / menu-bar icon + menu

**Files:**
- Create: `crates/radio-mini/src/tray.rs`
- Modify: `crates/radio-mini/src/main.rs`

Add a menu-bar icon with a context menu (Shuffle all, Shuffle favorites, Play/Stop, Quit).
On macOS `tray-icon` integrates with the winit/eframe event loop. Because eframe owns the
loop, the simplest robust MVP is: keep the eframe window as the primary surface and add the
tray icon + menu that sends actions into the app via a shared channel.

- [ ] **Step 1: Create `crates/radio-mini/src/tray.rs`**

```rust
use tray_icon::menu::{Menu, MenuId, MenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

pub struct Tray {
    pub _icon: TrayIcon,
    pub shuffle_all: MenuId,
    pub shuffle_fav: MenuId,
    pub toggle: MenuId,
    pub quit: MenuId,
}

pub fn build() -> anyhow::Result<Tray> {
    let menu = Menu::new();
    let shuffle_all = MenuItem::new("Shuffle", true, None);
    let shuffle_fav = MenuItem::new("Shuffle favorites", true, None);
    let toggle = MenuItem::new("Play / Stop", true, None);
    let quit = MenuItem::new("Quit", true, None);
    menu.append(&shuffle_all)?;
    menu.append(&shuffle_fav)?;
    menu.append(&toggle)?;
    menu.append(&quit)?;

    let icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("World Radio Mini")
        .build()?;

    Ok(Tray {
        _icon: icon,
        shuffle_all: shuffle_all.id().clone(),
        shuffle_fav: shuffle_fav.id().clone(),
        toggle: toggle.id().clone(),
        quit: quit.id().clone(),
    })
}
```

- [ ] **Step 2: Build the tray at startup and poll its menu events in the app**

In `main.rs` add `mod tray;` and build the tray before `run_native`, storing it so it lives
for the program's lifetime. In `app.rs`'s `update`, drain tray menu events each frame:

```rust
        while let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
            match event.id {
                id if id == self.tray.shuffle_all => {
                    self.state.set_scope(Scope::All);
                    self.shuffle();
                }
                id if id == self.tray.shuffle_fav => {
                    self.state.set_scope(Scope::Favorites);
                    self.shuffle();
                }
                id if id == self.tray.toggle => {
                    match self.state.phase {
                        Phase::Idle | Phase::Error => self.shuffle(),
                        _ => {
                            self.engine.stop();
                            self.state.stop();
                        }
                    }
                }
                id if id == self.tray.quit => std::process::exit(0),
                _ => {}
            }
        }
```
Store the `Tray` in `MiniApp` (add a `tray: crate::tray::Tray` field, build it in `new`).

- [ ] **Step 3: Build + test**

Run: `cargo build -p radio-mini && cargo test -p radio-mini`
Expected: compiles; 15 tests green.

- [ ] **Step 4: MANUAL smoke test (macOS)**

Run: `cargo run -p radio-mini`
Expected: a tray/menu-bar icon appears; its menu has Shuffle / Shuffle favorites / Play /
Stop / Quit; Shuffle from the menu plays a station; Quit exits.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: mini tray icon and menu"
```

### Task C5: Spectrum bars + CRT polish

**Files:**
- Modify: `crates/radio-mini/src/app.rs`
- Create: `crates/radio-mini/src/spectrum.rs`

Draw a small FFT spectrum from the engine's sample tap, and apply the CRT look (scanline
overlay, amber glow) to match the design.

- [ ] **Step 1: Add rustfft to the crate**

In `crates/radio-mini/Cargo.toml` `[dependencies]` add:
```toml
rustfft = "6.4"
```

- [ ] **Step 2: Create `crates/radio-mini/src/spectrum.rs`** — a tested pure helper that turns samples into N bar magnitudes

```rust
use rustfft::{num_complex::Complex, FftPlanner};

pub fn bars(samples: &[f32], n_bars: usize) -> Vec<f32> {
    if samples.is_empty() || n_bars == 0 {
        return vec![0.0; n_bars];
    }
    let size = samples.len().next_power_of_two().min(1024);
    let mut buf: Vec<Complex<f32>> = samples
        .iter()
        .take(size)
        .map(|&s| Complex { re: s, im: 0.0 })
        .collect();
    buf.resize(size, Complex { re: 0.0, im: 0.0 });
    FftPlanner::new().plan_fft_forward(size).process(&mut buf);
    let half = size / 2;
    let per = (half / n_bars).max(1);
    (0..n_bars)
        .map(|i| {
            let start = i * per;
            let end = (start + per).min(half);
            let sum: f32 = buf[start..end].iter().map(|c| c.norm()).sum();
            (sum / per as f32).min(1.0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_is_flat_zero() {
        let b = bars(&[0.0; 256], 8);
        assert_eq!(b.len(), 8);
        assert!(b.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn empty_input_returns_zeroed_bars() {
        assert_eq!(bars(&[], 5), vec![0.0; 5]);
    }
}
```

- [ ] **Step 3: Run the spectrum test**

Run: `cargo test -p radio-mini spectrum`
Expected: 2 passed.

- [ ] **Step 4: Draw the bars in `app.rs`** — add `mod spectrum;` in main.rs; in the window,
read the tap and paint bars:

```rust
        let mut tap = [0.0f32; 1024];
        let got = self.engine.read_tap(&mut tap);
        let bars = crate::spectrum::bars(&tap[..got], 14);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 24.0), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        let bw = rect.width() / bars.len() as f32;
        for (i, &h) in bars.iter().enumerate() {
            let x = rect.left() + i as f32 * bw;
            let bar_h = (h * rect.height()).clamp(1.0, rect.height());
            painter.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(x + 1.0, rect.bottom() - bar_h),
                    egui::pos2(x + bw - 1.0, rect.bottom()),
                ),
                0.0,
                self.theme.accent,
            );
        }
```

- [ ] **Step 5: Build + test**

Run: `cargo build -p radio-mini && cargo test -p radio-mini`
Expected: compiles; 17 tests green.

- [ ] **Step 6: MANUAL smoke test (macOS)**

Run: `cargo run -p radio-mini` (with a populated cache)
Expected: while a station plays, the spectrum bars move; the window reads as an amber CRT
panel.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: mini fft spectrum bars"
```

---

## Self-review notes

- **Spec coverage:** radio-audio extraction (A1–A3) · radio-mini scaffold (B1) · shuffle
  scope + pick (B2, B5) · state machine + status mapping (B3–B4) · egui CRT window (C2, C5) ·
  tray + menu (C4) · real playback via engine + shared data/ (C3) · themes (C1) · spectrum
  (C5). Testing: radio-audio tests stay green (A3 step 7), state/theme/spectrum/catalog
  unit-tested, manual macOS smoke tests at C2–C5. MVP scope (macOS, shuffle all+fav,
  play/stop, volume, now-playing, spectrum, amber+nord) all covered.
- **Out of scope (per spec):** Linux/Windows packaging, full 7 themes, full icon set, Mini
  writing favorites, sync, Android. macOS popover vs window: MVP uses a plain eframe window +
  tray menu (the open question's stated default); a true menu-bar popover is a follow-up.
- **Type consistency:** `StationPick{uuid,name,url}`, `MiniState`, `Phase`, `Scope`,
  `pick_random`, `all_stations`/`favorite_stations`, `Theme`, `spectrum::bars`,
  `MiniApp` are used consistently across tasks. `apply_status` arm names depend on the real
  `Status` enum (B4 step 1 verifies them before coding).
- **Known fragilities to watch during execution:** egui/eframe/tray-icon versions (0.28 /
  0.19) — if the API differs on the resolved version, adapt the few call sites (ViewportBuilder,
  MenuEvent::receiver, run_native return type). The tray + eframe event-loop integration on
  macOS is the riskiest part (C4) — if menu events don't arrive inside eframe's loop, fall
  back to driving the app from a winit event loop with the egui-winit integration, noted as the
  follow-up.
```
