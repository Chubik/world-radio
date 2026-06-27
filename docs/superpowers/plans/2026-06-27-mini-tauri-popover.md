# radio-mini Tauri Popover Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild `radio-mini` as a Tauri v2 macOS menu-bar app — a frameless amber-CRT popover anchored under the tray icon (like Proton VPN), playing/shuffling/favoriting stations via a Rust backend that reuses `radio-core` + `radio-audio`.

**Architecture:** `radio-mini` becomes a Tauri app. A thin Rust command layer wraps the existing audio engine and catalog (held in Tauri managed state behind a `Mutex`). A vanilla HTML/CSS/JS frontend renders the amber-CRT panel and calls the backend via `invoke`. A frameless transparent window + `tauri-plugin-positioner` give the native popover look; tray left-click toggles it; focus loss hides it.

**Tech Stack:** Tauri 2.11, tauri-plugin-positioner 2.3, vanilla HTML/CSS/JS, existing `radio-core` / `radio-audio` (cpal/symphonia).

## Global Constraints

- No code comments in Rust unless requested. Logs/strings English, lowercase.
- No `else if` in Rust; use `match`.
- Rust files ≤ 600–800 lines; split by responsibility.
- mini binary stays named `world-radio-mini`.
- Audio stays in the Rust backend (`radio-audio::AudioEngine`); the webview never plays audio.
- Reuse `radio-core` (catalog, favorites/history, `paths`, `single_instance`) and `radio-audio` unchanged.
- Favorites/history use the shared `favorites.json` / `history.json` so mini and TUI share state.
- Commit to `dev`; messages English, concise, no AI/personal mentions.
- macOS GUI behaviour is not unit-testable — GUI tasks end with a manual smoke test the human runs; the implementer does code + `cargo build` + `cargo clippy` and skips the manual run.
- Tauri versions: `tauri = "2"`, `tauri-build = "2"`, `tauri-plugin-positioner = "2"`.

---

### Task 1: Scaffold the Tauri crate skeleton (no behaviour yet)

**Files:**
- Modify: `crates/radio-mini/Cargo.toml`
- Create: `crates/radio-mini/build.rs`
- Create: `crates/radio-mini/tauri.conf.json`
- Create: `crates/radio-mini/ui/index.html`
- Replace: `crates/radio-mini/src/main.rs`
- Delete: `crates/radio-mini/src/app.rs`, `crates/radio-mini/src/theme.rs`, `crates/radio-mini/src/tray.rs`

**Interfaces:**
- Produces: a Tauri app that opens one frameless window loading `ui/index.html`. No tray, no audio yet (added later). `state.rs` and `catalog_src.rs` are kept (used by later tasks).

- [ ] **Step 1: Rewrite `crates/radio-mini/Cargo.toml`**

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

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
radio-core = { path = "../radio-core" }
radio-audio = { path = "../radio-audio" }
anyhow = "1"
fastrand = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-positioner = { version = "2", features = ["tray-icon"] }

[dev-dependencies]
```

- [ ] **Step 2: Create `crates/radio-mini/build.rs`**

```rust
fn main() {
    tauri_build::build();
}
```

- [ ] **Step 3: Create `crates/radio-mini/tauri.conf.json`**

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "World Radio Mini",
  "version": "1.2.0",
  "identifier": "net.r4dio.mini",
  "build": {
    "frontendDist": "ui"
  },
  "app": {
    "windows": [
      {
        "label": "popover",
        "url": "index.html",
        "width": 300,
        "height": 220,
        "decorations": false,
        "transparent": true,
        "alwaysOnTop": true,
        "skipTaskbar": true,
        "visible": false,
        "resizable": false
      }
    ],
    "macOSPrivateApi": true,
    "security": { "csp": null }
  },
  "bundle": {
    "active": true,
    "targets": "app",
    "macOS": {}
  }
}
```

- [ ] **Step 4: Create a placeholder `crates/radio-mini/ui/index.html`**

```html
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>r4dio mini</title>
  </head>
  <body>
    <div id="app">r4dio mini</div>
  </body>
</html>
```

- [ ] **Step 5: Replace `crates/radio-mini/src/main.rs`** with the minimal Tauri entrypoint

```rust
mod catalog_src;
mod state;

fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 6: Delete the egui files**

```bash
git rm crates/radio-mini/src/app.rs crates/radio-mini/src/theme.rs crates/radio-mini/src/tray.rs
```

- [ ] **Step 7: Fix `state.rs` for the slimmer module set**

`state.rs` may reference things only `app.rs` used. Run `cargo build -p radio-mini` and remove any now-unused items it flags (e.g. egui-only helpers). Keep `StationPick`, `pick_random`, `Phase`, `Scope`, `MiniState`, `pick_shuffle`, `state_labels`, `spectrum_bars`, `apply_status`, `set_favorites`, `begin_play`, `stop`, `set_volume`, `set_scope`, `load_stations`, `active_stations` and their tests. `catalog_src.rs` stays as-is.

- [ ] **Step 8: Build**

Run: `cargo build -p radio-mini`
Expected: compiles (downloads Tauri). If `tauri::generate_context!` complains about a missing icon, add `"icon": []` is not valid — instead the default config needs no icon for `targets: "app"` in dev; if it errors, create an empty `crates/radio-mini/icons/` is unnecessary — report the exact error.

- [ ] **Step 9: Run tests + clippy**

Run: `cargo test -p radio-mini --bin world-radio-mini && cargo clippy -p radio-mini --all-targets -- -D warnings`
Expected: the preserved `state.rs` / `catalog_src.rs` tests pass; clippy clean.

- [ ] **Step 10: Commit**

```bash
git add -A crates/radio-mini
git commit -m "chore: scaffold radio-mini as a tauri app"
```

---

### Task 2: Backend app state (audio engine + catalog + selection)

**Files:**
- Create: `crates/radio-mini/src/backend.rs`
- Modify: `crates/radio-mini/src/main.rs`

**Interfaces:**
- Consumes: `radio_audio::AudioEngine`, `radio_core::catalog::{Cache, Catalog, Health}`, `catalog_src`, `state::{MiniState, StationPick, Phase, Scope}`.
- Produces: `pub struct Backend { state: MiniState, engine: Option<AudioEngine>, catalog: Catalog, fav_path, hist_path, blacklist_path }` with `Backend::new() -> anyhow::Result<Backend>` and methods `play_pick(&mut self, StationPick)`, `shuffle(&mut self)`, `play_last(&mut self)`, `resume(&mut self)`, `stop(&mut self)`, `set_volume(&mut self, f32)`, `set_scope(&mut self, Scope)`, `toggle_favorite(&mut self)`, `now_is_favorite(&self) -> bool`. These are ports of the current `app.rs` logic with the egui parts removed.

- [ ] **Step 1: Create `crates/radio-mini/src/backend.rs`** with the state struct and constructor

```rust
use crate::catalog_src;
use crate::state::{MiniState, Phase, Scope, StationPick};
use radio_audio::AudioEngine;
use radio_core::catalog::{Cache, Catalog, Health};
use std::path::PathBuf;

pub struct Backend {
    pub state: MiniState,
    engine: Option<AudioEngine>,
    catalog: Catalog,
    fav_path: PathBuf,
    hist_path: PathBuf,
    blacklist_path: PathBuf,
}

impl Backend {
    pub fn new() -> anyhow::Result<Backend> {
        let data = radio_core::paths::ensure_data_dir()?;
        let cache = Cache::open(&data.join("stations.db"))?;
        let health = Health::load(&data.join("station_health.json"));
        let fav_path = data.join("favorites.json");
        let hist_path = data.join("history.json");
        let blacklist_path = data.join("blacklist.json");
        let catalog = Catalog::load(cache, health, &fav_path, &hist_path, &blacklist_path);

        let all = catalog_src::all_stations(&catalog)?;
        let favorites = catalog_src::favorite_stations(&catalog)?;

        let mut state = MiniState::new();
        state.load_stations(all, favorites);

        let engine = AudioEngine::spawn().ok();
        if let Some(engine) = &engine {
            engine.set_volume(state.volume);
        }

        Ok(Backend {
            state,
            engine,
            catalog,
            fav_path,
            hist_path,
            blacklist_path,
        })
    }

    fn play_pick(&mut self, pick: StationPick) {
        if let Some(engine) = &self.engine {
            engine.play(&pick.url);
        }
        self.catalog.record_history(&pick.uuid);
        if let Err(e) =
            self.catalog
                .save_state(&self.fav_path, &self.hist_path, &self.blacklist_path)
        {
            eprintln!("save history failed: {e}");
        }
        self.state.begin_play(pick);
    }

    pub fn shuffle(&mut self) {
        if let Some(pick) = self.state.pick_shuffle() {
            self.play_pick(pick);
        }
    }

    pub fn play_last(&mut self) {
        match catalog_src::last_played(&self.catalog) {
            Ok(Some(pick)) => self.play_pick(pick),
            Ok(None) => self.shuffle(),
            Err(e) => {
                eprintln!("load last station failed: {e}");
                self.shuffle();
            }
        }
    }

    pub fn resume(&mut self) {
        match self.state.now.clone() {
            Some(pick) => self.play_pick(pick),
            None => self.shuffle(),
        }
    }

    pub fn stop(&mut self) {
        self.state.stop();
        if let Some(engine) = &self.engine {
            engine.stop();
        }
    }

    pub fn set_volume(&mut self, v: f32) {
        self.state.set_volume(v);
        if let Some(engine) = &self.engine {
            engine.set_volume(self.state.volume);
        }
    }

    pub fn set_scope(&mut self, scope: Scope) {
        self.state.set_scope(scope);
    }

    pub fn now_is_favorite(&self) -> bool {
        match &self.state.now {
            Some(pick) => self.catalog.is_favorite(&pick.uuid),
            None => false,
        }
    }

    pub fn toggle_favorite(&mut self) {
        let Some(pick) = self.state.now.clone() else {
            return;
        };
        match catalog_src::toggle_and_reload(&mut self.catalog, &pick.uuid) {
            Ok(favorites) => self.state.set_favorites(favorites),
            Err(e) => eprintln!("toggle favorite failed: {e}"),
        }
        if let Err(e) =
            self.catalog
                .save_state(&self.fav_path, &self.hist_path, &self.blacklist_path)
        {
            eprintln!("save favorites failed: {e}");
        }
    }

    pub fn poll_engine(&mut self) {
        if let Some(engine) = &self.engine {
            while let Some(status) = engine.poll_status() {
                self.state.apply_status(status);
            }
        }
    }

    pub fn read_spectrum(&self, bars: usize) -> Vec<f32> {
        let _ = bars;
        crate::state::spectrum_bars(bars)
    }

    pub fn phase(&self) -> Phase {
        self.state.phase
    }
}
```

(Note: `read_spectrum` returns the static `spectrum_bars` for now; a later task wires the real FFT tap. This keeps Task 2 self-contained and compiling.)

- [ ] **Step 2: Register the module** — in `crates/radio-mini/src/main.rs` add `mod backend;` to the module list.

- [ ] **Step 3: Build + test + clippy**

Run: `cargo build -p radio-mini && cargo test -p radio-mini --bin world-radio-mini && cargo clippy -p radio-mini --all-targets -- -D warnings`
Expected: compiles; preserved tests pass; clippy clean. `Backend` is not yet used by `main` (added in Task 3) — if clippy flags it dead, add `#[allow(dead_code)]` on `struct Backend` and remove it in Task 3.

- [ ] **Step 4: Commit**

```bash
git add crates/radio-mini/src/backend.rs crates/radio-mini/src/main.rs
git commit -m "feat: mini tauri backend state and playback methods"
```

---

### Task 3: Tauri commands + managed state + now_state serialization

**Files:**
- Create: `crates/radio-mini/src/commands.rs`
- Modify: `crates/radio-mini/src/main.rs`

**Interfaces:**
- Consumes: `Backend` (Task 2).
- Produces: a serializable `NowState` and these `#[tauri::command]`s registered with the app: `shuffle`, `play_last`, `resume`, `stop`, `set_volume(v: f32)`, `set_scope(scope: String)`, `toggle_favorite`, `now_state() -> NowState`, `spectrum() -> Vec<f32>`. App holds `tauri::State<Mutex<Backend>>`.

- [ ] **Step 1: Write a failing test for scope parsing** — create `crates/radio-mini/src/commands.rs` with a pure helper and its test:

```rust
use crate::backend::Backend;
use crate::state::{Phase, Scope};
use serde::Serialize;
use std::sync::Mutex;

fn parse_scope(s: &str) -> Scope {
    match s {
        "favorites" => Scope::Favorites,
        _ => Scope::All,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_scope_maps_known_and_defaults_to_all() {
        assert_eq!(parse_scope("favorites"), Scope::Favorites);
        assert_eq!(parse_scope("all"), Scope::All);
        assert_eq!(parse_scope("garbage"), Scope::All);
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p radio-mini --bin world-radio-mini parse_scope`
Expected: FAIL — `parse_scope` not found (module not registered yet) — register it in Step 4 first if needed, or the test compiles once `mod commands;` is added. To see the red, add `mod commands;` to `main.rs` now, then run; expect FAIL only if `Scope` lacks `PartialEq` (it derives it) — otherwise it passes immediately, which is fine for a pure mapping.

- [ ] **Step 3: Add `NowState` + the phase-to-string helper** — append to `commands.rs`:

```rust
#[derive(Serialize)]
pub struct NowState {
    pub station: Option<String>,
    pub track: String,
    pub phase: String,
    pub volume: f32,
    pub scope: String,
    pub is_favorite: bool,
    pub meta: String,
}

fn phase_str(phase: Phase) -> &'static str {
    match phase {
        Phase::Idle => "idle",
        Phase::Buffering => "buffering",
        Phase::Playing => "playing",
        Phase::Error => "error",
    }
}

fn scope_str(scope: Scope) -> &'static str {
    match scope {
        Scope::All => "all",
        Scope::Favorites => "favorites",
    }
}
```

- [ ] **Step 4: Add the commands** — append to `commands.rs`:

```rust
pub type Shared = Mutex<Backend>;

#[tauri::command]
pub fn shuffle(state: tauri::State<Shared>) {
    state.lock().unwrap().shuffle();
}

#[tauri::command]
pub fn play_last(state: tauri::State<Shared>) {
    state.lock().unwrap().play_last();
}

#[tauri::command]
pub fn resume(state: tauri::State<Shared>) {
    state.lock().unwrap().resume();
}

#[tauri::command]
pub fn stop(state: tauri::State<Shared>) {
    state.lock().unwrap().stop();
}

#[tauri::command]
pub fn set_volume(state: tauri::State<Shared>, v: f32) {
    state.lock().unwrap().set_volume(v);
}

#[tauri::command]
pub fn set_scope(state: tauri::State<Shared>, scope: String) {
    state.lock().unwrap().set_scope(parse_scope(&scope));
}

#[tauri::command]
pub fn toggle_favorite(state: tauri::State<Shared>) {
    state.lock().unwrap().toggle_favorite();
}

#[tauri::command]
pub fn now_state(state: tauri::State<Shared>) -> NowState {
    let mut b = state.lock().unwrap();
    b.poll_engine();
    let now = b.state.now.clone();
    NowState {
        station: now.as_ref().map(|n| n.name.clone()),
        track: String::new(),
        phase: phase_str(b.phase()).to_string(),
        volume: b.state.volume,
        scope: scope_str(b.state.scope).to_string(),
        is_favorite: b.now_is_favorite(),
        meta: now.as_ref().map(|_| "live".to_string()).unwrap_or_default(),
    }
}

#[tauri::command]
pub fn spectrum(state: tauri::State<Shared>) -> Vec<f32> {
    state.lock().unwrap().read_spectrum(16)
}
```

(`b.state` and `b.phase()` are accessed here, so `Backend.state` must be `pub` — it already is in Task 2.)

- [ ] **Step 5: Wire commands + managed state into `main.rs`**

```rust
mod backend;
mod catalog_src;
mod commands;
mod state;

use std::sync::Mutex;

fn main() {
    radio_core::single_instance::take_over();

    let mut backend = backend::Backend::new().expect("failed to init backend");
    backend.play_last();
    run(backend);
}

fn run(backend: backend::Backend) {
    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .manage(Mutex::new(backend))
        .invoke_handler(tauri::generate_handler![
            commands::shuffle,
            commands::play_last,
            commands::resume,
            commands::stop,
            commands::set_volume,
            commands::set_scope,
            commands::toggle_favorite,
            commands::now_state,
            commands::spectrum,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Remove the `#[allow(dead_code)]` from `struct Backend` if Task 2 added it.

- [ ] **Step 6: Build + test + clippy**

Run: `cargo build -p radio-mini && cargo test -p radio-mini --bin world-radio-mini && cargo clippy -p radio-mini --all-targets -- -D warnings`
Expected: compiles; tests (incl. `parse_scope`) pass; clippy clean.

- [ ] **Step 7: Commit**

```bash
git add crates/radio-mini/src/commands.rs crates/radio-mini/src/main.rs
git commit -m "feat: mini tauri commands and now_state"
```

---

### Task 4: The amber-CRT popover UI (HTML/CSS/JS)

**Files:**
- Replace: `crates/radio-mini/ui/index.html`
- Create: `crates/radio-mini/ui/style.css`
- Create: `crates/radio-mini/ui/app.js`

**Interfaces:**
- Consumes: the Tauri commands from Task 3 via `window.__TAURI__.core.invoke`.
- Produces: the rendered amber-CRT panel. No new Rust.

Reference the design tokens from `docs/design/mini/themes.js` (amber): bg `#15100b`, panel `#1b1510`, fg `#d49a3a`, hi `#ffc457`, dim `#6e5430`, rule `#3a2c17`, ok `#9ec074`, warn `#ffc457`, err `#d96a5a`, accent `#ff8a3d`, bright `#fff0c0`.

- [ ] **Step 1: Write `crates/radio-mini/ui/style.css`** — amber-CRT panel styling

```css
:root {
  --bg: #15100b; --panel: #1b1510; --fg: #d49a3a; --hi: #ffc457;
  --dim: #6e5430; --rule: #3a2c17; --ok: #9ec074; --warn: #ffc457;
  --err: #d96a5a; --accent: #ff8a3d; --bright: #fff0c0;
}
* { margin: 0; padding: 0; box-sizing: border-box; }
html, body { background: transparent; }
body {
  font-family: "IBM Plex Mono", ui-monospace, Menlo, monospace;
  color: var(--fg); user-select: none;
}
#app {
  margin: 8px; padding: 12px 14px; border-radius: 12px;
  background: var(--bg); box-shadow: inset 0 0 0 1px var(--rule), 0 12px 40px -8px rgba(0,0,0,0.7);
  position: relative; overflow: hidden;
}
#app::after {
  content: ""; position: absolute; inset: 0; pointer-events: none;
  background: repeating-linear-gradient(to bottom, rgba(0,0,0,0) 0 2px, rgba(0,0,0,0.16) 2px 3px);
  mix-blend-mode: overlay;
}
.row { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
.wordmark { font-weight: 700; color: var(--fg); }
.wordmark b { color: var(--hi); }
.dot { font-size: 11px; letter-spacing: .1em; }
.dot.idle { color: var(--dim); } .dot.buffering { color: var(--warn); }
.dot.playing { color: var(--ok); } .dot.error { color: var(--err); }
.meta { margin-left: auto; font-size: 10px; color: var(--dim); }
.station { font-family: "IBM Plex Sans", system-ui, sans-serif; font-weight: 600;
  font-size: 14px; color: var(--bright); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.track { font-size: 11px; color: var(--accent); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; height: 14px; }
.star { margin-left: auto; cursor: pointer; width: 26px; height: 26px; border-radius: 6px;
  display: grid; place-items: center; box-shadow: inset 0 0 0 1px var(--rule); color: var(--dim); }
.star.on { color: var(--bg); background: var(--hi); box-shadow: 0 0 9px -2px var(--hi); }
#spectrum { display: block; }
.controls { display: flex; gap: 8px; }
.btn { cursor: pointer; border: none; font-family: inherit; font-weight: 700; font-size: 12px;
  letter-spacing: .06em; padding: 9px 12px; border-radius: 6px; color: var(--fg);
  background: transparent; box-shadow: inset 0 0 0 1px var(--rule); }
.btn.primary { flex: 1; color: var(--bg); background: var(--hi); box-shadow: 0 0 10px -2px var(--hi); }
.scope { display: flex; gap: 4px; align-items: center; }
.seg { cursor: pointer; padding: 3px 8px; font-size: 10px; border-radius: 4px; color: var(--dim); box-shadow: inset 0 0 0 1px var(--rule); }
.seg.on { color: var(--bg); background: var(--hi); }
.quit { margin-left: auto; cursor: pointer; font-size: 10px; color: var(--dim); }
.vol { display: flex; align-items: center; gap: 6px; }
.vol input { width: 90px; }
```

- [ ] **Step 2: Write `crates/radio-mini/ui/index.html`**

```html
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <link rel="stylesheet" href="style.css" />
    <title>r4dio mini</title>
  </head>
  <body>
    <div id="app">
      <div class="row">
        <span class="wordmark">▌r<b>4</b>dio</span>
        <span id="dot" class="dot idle">IDLE</span>
        <span id="meta" class="meta">—</span>
      </div>
      <div class="row">
        <div style="min-width:0;flex:1">
          <div id="station" class="station">Nothing playing</div>
          <div id="track" class="track"></div>
        </div>
        <div id="star" class="star">☆</div>
      </div>
      <div class="row">
        <canvas id="spectrum" width="150" height="16"></canvas>
        <div class="vol">
          <span style="font-size:10px;color:var(--dim)">VOL</span>
          <input id="vol" type="range" min="0" max="1" step="0.01" />
        </div>
      </div>
      <div class="controls">
        <button id="shuffle" class="btn primary">⇄ SHUFFLE</button>
        <button id="playstop" class="btn">▶</button>
      </div>
      <div class="row" style="margin-top:8px;margin-bottom:0">
        <div class="scope">
          <span id="scope-all" class="seg on">ALL</span>
          <span id="scope-fav" class="seg">★ FAVS</span>
        </div>
        <span id="quit" class="quit">✕ quit</span>
      </div>
    </div>
    <script src="app.js"></script>
  </body>
</html>
```

- [ ] **Step 3: Write `crates/radio-mini/ui/app.js`**

```js
const invoke = window.__TAURI__.core.invoke;

const el = (id) => document.getElementById(id);
const dot = el("dot"), meta = el("meta"), station = el("station"), track = el("track");
const star = el("star"), playstop = el("playstop"), shuffle = el("shuffle");
const scopeAll = el("scope-all"), scopeFav = el("scope-fav"), vol = el("vol"), quit = el("quit");
const canvas = el("spectrum"), ctx = canvas.getContext("2d");

const DOT_LABEL = { idle: "IDLE", buffering: "···", playing: "LIVE", error: "OFFLINE" };

function render(s) {
  dot.className = "dot " + s.phase;
  dot.textContent = DOT_LABEL[s.phase] || "IDLE";
  meta.textContent = s.meta || "—";
  station.textContent = s.station || "Nothing playing";
  const txt = { idle: "press Shuffle to start listening", buffering: "connecting to stream…",
    playing: "now playing", error: "stream offline — couldn't connect" };
  track.textContent = txt[s.phase] || "";
  star.className = "star" + (s.is_favorite ? " on" : "");
  star.textContent = s.is_favorite ? "★" : "☆";
  playstop.textContent = (s.phase === "playing" || s.phase === "buffering") ? "⏸" : "▶";
  shuffle.textContent = s.phase === "error" ? "⇄ RETRY" : "⇄ SHUFFLE";
  scopeAll.className = "seg" + (s.scope === "all" ? " on" : "");
  scopeFav.className = "seg" + (s.scope === "favorites" ? " on" : "");
  if (document.activeElement !== vol) vol.value = s.volume;
}

function drawSpectrum(bars) {
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  const c = getComputedStyle(document.documentElement);
  ctx.fillStyle = c.getPropertyValue("--hi").trim();
  const bw = canvas.width / bars.length;
  bars.forEach((h, i) => {
    const bh = Math.max(2, h * canvas.height);
    ctx.fillRect(i * bw + 1, canvas.height - bh, bw - 2, bh);
  });
}

async function tick() {
  const s = await invoke("now_state");
  render(s);
  const bars = await invoke("spectrum");
  drawSpectrum(bars);
}

shuffle.onclick = () => invoke("shuffle");
playstop.onclick = async () => {
  const s = await invoke("now_state");
  const playing = s.phase === "playing" || s.phase === "buffering";
  await invoke(playing ? "stop" : "resume");
};
star.onclick = () => invoke("toggle_favorite");
scopeAll.onclick = () => invoke("set_scope", { scope: "all" });
scopeFav.onclick = () => invoke("set_scope", { scope: "favorites" });
vol.oninput = () => invoke("set_volume", { v: parseFloat(vol.value) });
quit.onclick = () => invoke("stop").then(() => window.__TAURI__.process.exit(0));

setInterval(tick, 250);
tick();
```

- [ ] **Step 4: Build**

Run: `cargo build -p radio-mini`
Expected: compiles (the UI is static assets bundled by Tauri).

- [ ] **Step 5: Commit**

```bash
git add crates/radio-mini/ui
git commit -m "feat: amber-crt popover ui for tauri mini"
```

---

### Task 5: Tray icon + popover show/hide (native menu-bar behaviour)

**Files:**
- Modify: `crates/radio-mini/src/main.rs`

**Interfaces:**
- Consumes: the positioner plugin, the popover window (`label: "popover"`).
- Produces: a tray icon; left-click toggles the popover anchored under the icon; focus loss hides it; no Dock icon.

- [ ] **Step 1: Build the tray + accessory policy + window events in `run()`** — replace the `run` fn in `crates/radio-mini/src/main.rs`:

```rust
fn run(backend: backend::Backend) {
    use tauri::tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState};
    use tauri::{Manager, WindowEvent};
    use tauri_plugin_positioner::{Position, WindowExt};

    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .manage(Mutex::new(backend))
        .invoke_handler(tauri::generate_handler![
            commands::shuffle,
            commands::play_last,
            commands::resume,
            commands::stop,
            commands::set_volume,
            commands::set_scope,
            commands::toggle_favorite,
            commands::now_state,
            commands::spectrum,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle().clone();
            TrayIconBuilder::with_id("wr-tray")
                .title("WR")
                .tooltip("World Radio Mini")
                .on_tray_icon_event(move |_tray, event| {
                    tauri_plugin_positioner::on_tray_event(&handle, &event);
                    let clicked = matches!(
                        event,
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        }
                    );
                    if clicked {
                        if let Some(win) = handle.get_webview_window("popover") {
                            let visible = win.is_visible().unwrap_or(false);
                            match visible {
                                true => {
                                    let _ = win.hide();
                                }
                                false => {
                                    let _ = win.move_window(Position::TrayBottomCenter);
                                    let _ = win.show();
                                    let _ = win.set_focus();
                                }
                            }
                        }
                    }
                })
                .build(app)?;

            if let Some(win) = app.get_webview_window("popover") {
                let w = win.clone();
                win.on_window_event(move |event| {
                    if let WindowEvent::Focused(false) = event {
                        let _ = w.hide();
                    }
                });
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 2: Build**

Run: `cargo build -p radio-mini`
Expected: compiles. If `move_window`/`on_tray_event`/`WindowExt` paths differ on the resolved positioner version, check `tauri_plugin_positioner`'s exports and adjust imports; report the exact error if it does not resolve.

- [ ] **Step 3: Test + clippy**

Run: `cargo test -p radio-mini --bin world-radio-mini && cargo clippy -p radio-mini --all-targets -- -D warnings`
Expected: tests pass; clippy clean.

- [ ] **Step 4: MANUAL smoke test (macOS)** — human runs:

Run: `cargo run -p radio-mini`
Expected:
- No Dock icon; a `WR` tray icon appears in the menu bar; no window on launch; audio starts on the last-played station.
- Left-click the tray icon → the amber-CRT popover drops **anchored under the icon** (like Proton VPN).
- Click elsewhere (focus lost) → the popover hides.
- Left-click again → it reappears; Shuffle / play-stop / ★ / ALL-FAVS / volume / quit all work.
- Quit from the popover exits the app.

- [ ] **Step 5: Commit**

```bash
git add crates/radio-mini/src/main.rs
git commit -m "feat: tray popover anchored under icon, hides on focus loss"
```

---

### Task 6: Real FFT spectrum from the audio tap

**Files:**
- Modify: `crates/radio-mini/src/backend.rs`
- Modify: `crates/radio-mini/Cargo.toml`

**Interfaces:**
- Consumes: `engine.read_tap(&mut [f32]) -> usize` (existing).
- Produces: `Backend::read_spectrum(&mut self, bars: usize) -> Vec<f32>` returns FFT magnitudes from the live tap while playing, else zeros. `commands::spectrum` already calls `read_spectrum(16)`.

- [ ] **Step 1: Add `rustfft` to `crates/radio-mini/Cargo.toml`** `[dependencies]`:

```toml
rustfft = "6"
```

- [ ] **Step 2: Replace `read_spectrum`** in `backend.rs` (it currently returns static bars). Add a tap buffer field to `Backend` and compute the FFT:

Add to the struct: `tap: Vec<f32>` initialized in `new()` to `vec![0.0; 1024]`. Then:

```rust
    pub fn read_spectrum(&mut self, bars: usize) -> Vec<f32> {
        if self.phase() != Phase::Playing {
            return vec![0.0; bars];
        }
        let n = match &self.engine {
            Some(engine) => engine.read_tap(&mut self.tap),
            None => 0,
        };
        if n < 2 {
            return vec![0.0; bars];
        }
        use rustfft::{num_complex::Complex, FftPlanner};
        let size = n.min(1024);
        let mut buf: Vec<Complex<f32>> =
            self.tap[..size].iter().map(|&s| Complex { re: s, im: 0.0 }).collect();
        FftPlanner::new().plan_fft_forward(size).process(&mut buf);
        let half = size / 2;
        let per = (half / bars).max(1);
        (0..bars)
            .map(|i| {
                let start = i * per;
                let end = (start + per).min(half);
                let sum: f32 = buf[start..end].iter().map(|c| c.norm()).sum();
                (sum / per as f32 / 8.0).min(1.0)
            })
            .collect()
    }
```

`commands::spectrum` already calls `state.lock().unwrap().read_spectrum(16)`; the lock guard is mutable, so the now-`&mut self` `read_spectrum` compiles unchanged. After this task `state::spectrum_bars` is no longer called from `backend.rs`. It still has its own unit test (`spectrum_bars_returns_n_values_in_range`), so keep the function — the test keeps it live and clippy stays clean. If clippy still flags it, the test reference suffices; do not delete the tested helper.

- [ ] **Step 3: Build + test + clippy**

Run: `cargo build -p radio-mini && cargo test -p radio-mini --bin world-radio-mini && cargo clippy -p radio-mini --all-targets -- -D warnings`
Expected: compiles; tests pass; clippy clean.

- [ ] **Step 4: MANUAL smoke test (macOS)** — human runs `cargo run -p radio-mini`, opens the popover while a station plays: the spectrum bars move with the audio; when stopped they flatten.

- [ ] **Step 5: Commit**

```bash
git add crates/radio-mini/src/backend.rs crates/radio-mini/Cargo.toml Cargo.lock
git commit -m "feat: live fft spectrum from audio tap in tauri mini"
```

---

## Self-Review Notes

- **Spec coverage:** Tauri scaffold replacing eframe (Task 1) · backend reusing radio-core/radio-audio (Task 2) · all 9 commands + now_state (Task 3) · amber-CRT HTML/CSS/JS popover (Task 4) · tray + positioner popover anchored under icon, focus-loss hide, accessory policy (Task 5) · live FFT spectrum (Task 6). Single-instance via `radio_core::single_instance::take_over()` (Task 3 main). Last-played auto-start via `play_last()` (Task 3 main). Shared favorites/history through `save_state` (Task 2). Audio in Rust backend (Task 2).
- **Out of scope (per spec):** Windows/Linux tray, launch-at-login, auto-update, sync, other themes — none planned. Spectrum starts by polling (Task 4 JS `setInterval`), matching the spec's "polling first, emit later" decision; switching to `emit` is a future optimization, not in this plan.
- **Type consistency:** `Backend` methods (Task 2) called by commands (Task 3); `NowState` fields (Task 3) consumed verbatim by `render()` (Task 4); `parse_scope`/`scope_str` strings (`"all"`/`"favorites"`) match the JS `set_scope` args and `render` checks; `read_spectrum(usize)` signature consistent Task 2 → Task 6.
- **Known risk:** Tauri/positioner API names (`move_window`, `on_tray_event`, `Position::TrayBottomCenter`, `set_activation_policy`, `get_webview_window`) are from Tauri 2.x docs; if the resolved 2.11 API differs slightly, the build error names the exact symbol and the implementer adjusts the import/path. This is called out in Task 5 Step 2.
