# Mini Popup Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild the `radio-mini` popup as the designed amber-CRT panel (4 states) and make the FavStar toggle write favorites to disk.

**Architecture:** A view-only redesign of `app.rs`'s `ui()` plus a theme-token extension, with one structural change — `MiniApp` now owns the `Catalog` (and data-dir paths) so the favorite toggle can persist via `catalog.save_state(...)`, the same path the TUI uses. Pure helpers (state→label mapping, static spectrum bars) are unit-tested; the egui panel is manually smoke-tested.

**Tech Stack:** Rust, eframe/egui 0.35, radio-core (`Catalog`/`Favorites`), radio-audio (`AudioEngine`).

## Global Constraints

- No code comments unless explicitly requested.
- All in-code strings/logs in English, lowercase (logs).
- No `else if`; prefer `match`.
- Files stay 600–800 lines max; split by responsibility if exceeded.
- Commit to `dev` only; messages English, concise, no AI/personal mentions.
- `cargo fmt` + `cargo clippy -p radio-mini --all-targets -- -D warnings` must be clean.
- mini is a `[[bin]]` (no lib target) — run tests with `cargo test -p radio-mini --bin world-radio-mini`.

---

### Task 1: Extend `Theme` with the full design token set

**Files:**
- Modify: `crates/radio-mini/src/theme.rs`

**Interfaces:**
- Produces: `Theme` struct gains fields `panel: Color32`, `rule: Color32`, `bright: Color32`, `scan: f32`, `glow: Color32`, `light: bool`. `Theme::amber()` and `Theme::nord()` populate them.

- [ ] **Step 1: Write the failing test** — add to the `tests` module in `crates/radio-mini/src/theme.rs`:

```rust
    #[test]
    fn amber_has_full_design_tokens() {
        let t = Theme::amber();
        // panel is a dark inset surface
        assert!(t.panel.r() < 60 && t.panel.g() < 60 && t.panel.b() < 60);
        // bright is the max-emphasis near-white-warm
        assert!(t.bright.r() > 200 && t.bright.g() > 200);
        // scanline strength is positive but subtle
        assert!(t.scan > 0.0 && t.scan < 0.5);
        assert!(!t.light);
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p radio-mini --bin world-radio-mini amber_has_full_design_tokens`
Expected: FAIL — no fields `panel`/`bright`/`scan`/`light` on `Theme`.

- [ ] **Step 3: Add the fields to the struct**

In `crates/radio-mini/src/theme.rs`, change the struct to:

```rust
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct Theme {
    pub name: &'static str,
    pub bg: Color32,
    pub panel: Color32,
    pub fg: Color32,
    pub hi: Color32,
    pub dim: Color32,
    pub rule: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub err: Color32,
    pub accent: Color32,
    pub bright: Color32,
    pub scan: f32,
    pub glow: Color32,
    pub light: bool,
}
```

- [ ] **Step 4: Populate `amber()`** — replace the body of `Theme::amber()` with:

```rust
    pub fn amber() -> Self {
        Self {
            name: "amber-crt",
            bg: Color32::from_rgb(0x15, 0x10, 0x0b),
            panel: Color32::from_rgb(0x1b, 0x15, 0x10),
            fg: Color32::from_rgb(0xd4, 0x9a, 0x3a),
            hi: Color32::from_rgb(0xff, 0xc4, 0x57),
            dim: Color32::from_rgb(0x6e, 0x54, 0x30),
            rule: Color32::from_rgb(0x3a, 0x2c, 0x17),
            ok: Color32::from_rgb(0x9e, 0xc0, 0x74),
            warn: Color32::from_rgb(0xff, 0xc4, 0x57),
            err: Color32::from_rgb(0xd9, 0x6a, 0x5a),
            accent: Color32::from_rgb(0xff, 0x8a, 0x3d),
            bright: Color32::from_rgb(0xff, 0xf0, 0xc0),
            scan: 0.16,
            glow: Color32::from_rgb(0xd4, 0x9a, 0x3a),
            light: false,
        }
    }
```

- [ ] **Step 5: Populate `nord()`** — replace the body of `Theme::nord()` with the same fields (keep its existing hues, add the new tokens):

```rust
    #[allow(dead_code)]
    pub fn nord() -> Self {
        Self {
            name: "nord",
            bg: Color32::from_rgb(0x2e, 0x34, 0x40),
            panel: Color32::from_rgb(0x3b, 0x42, 0x52),
            fg: Color32::from_rgb(0xd8, 0xde, 0xe9),
            hi: Color32::from_rgb(0x88, 0xc0, 0xd0),
            dim: Color32::from_rgb(0x4c, 0x56, 0x6a),
            rule: Color32::from_rgb(0x43, 0x4c, 0x5e),
            ok: Color32::from_rgb(0xa3, 0xbe, 0x8c),
            warn: Color32::from_rgb(0xeb, 0xcb, 0x8b),
            err: Color32::from_rgb(0xbf, 0x61, 0x6a),
            accent: Color32::from_rgb(0x81, 0xa1, 0xc1),
            bright: Color32::from_rgb(0xec, 0xef, 0xf4),
            scan: 0.10,
            glow: Color32::from_rgb(0x88, 0xc0, 0xd0),
            light: false,
        }
    }
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p radio-mini --bin world-radio-mini theme`
Expected: PASS — all theme tests green (existing `amber_is_default_and_dark`, `alternate_differs_from_amber`, new `amber_has_full_design_tokens`).

- [ ] **Step 7: Build, fmt, clippy**

Run: `cargo fmt -p radio-mini && cargo clippy -p radio-mini --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 8: Commit**

```bash
git add crates/radio-mini/src/theme.rs
git commit -m "feat: extend mini theme with full crt token set"
```

---

### Task 2: Pure helpers — state labels + static spectrum bars

**Files:**
- Modify: `crates/radio-mini/src/state.rs`

**Interfaces:**
- Consumes: `Phase` (existing enum in `state.rs`).
- Produces:
  - `pub fn state_labels(phase: Phase) -> (&'static str, &'static str)` — returns `(dot_label, primary_label)`.
  - `pub fn spectrum_bars(n: usize) -> Vec<f32>` — `n` deterministic bar heights in `0.0..=1.0`.

- [ ] **Step 1: Write the failing tests** — add to the `tests` module in `crates/radio-mini/src/state.rs`:

```rust
    #[test]
    fn state_labels_cover_all_phases() {
        assert_eq!(state_labels(Phase::Idle), ("IDLE", "SHUFFLE"));
        assert_eq!(state_labels(Phase::Buffering), ("···", "SHUFFLE"));
        assert_eq!(state_labels(Phase::Playing), ("LIVE", "SHUFFLE"));
        assert_eq!(state_labels(Phase::Error), ("OFFLINE", "RETRY"));
    }

    #[test]
    fn spectrum_bars_returns_n_values_in_range() {
        let b = spectrum_bars(16);
        assert_eq!(b.len(), 16);
        assert!(b.iter().all(|&v| (0.0..=1.0).contains(&v)));
        assert_eq!(spectrum_bars(0).len(), 0);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p radio-mini --bin world-radio-mini state_labels spectrum_bars`
Expected: FAIL — `state_labels` / `spectrum_bars` not found.

- [ ] **Step 3: Implement the helpers** — add to `crates/radio-mini/src/state.rs` (top level, after the `Phase` enum):

```rust
pub fn state_labels(phase: Phase) -> (&'static str, &'static str) {
    match phase {
        Phase::Idle => ("IDLE", "SHUFFLE"),
        Phase::Buffering => ("···", "SHUFFLE"),
        Phase::Playing => ("LIVE", "SHUFFLE"),
        Phase::Error => ("OFFLINE", "RETRY"),
    }
}

pub fn spectrum_bars(n: usize) -> Vec<f32> {
    const SEED: [f32; 14] = [5.0, 7.0, 4.0, 8.0, 6.0, 3.0, 7.0, 5.0, 8.0, 4.0, 6.0, 7.0, 3.0, 5.0];
    (0..n).map(|i| SEED[i % SEED.len()] / 8.0).collect()
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p radio-mini --bin world-radio-mini state_labels spectrum_bars`
Expected: PASS — 2 new tests green.

- [ ] **Step 5: Build, fmt, clippy**

Run: `cargo fmt -p radio-mini && cargo clippy -p radio-mini --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/radio-mini/src/state.rs
git commit -m "feat: mini state labels and static spectrum bars"
```

---

### Task 3: `MiniApp` owns the `Catalog` + data paths

**Files:**
- Modify: `crates/radio-mini/src/app.rs`

**Interfaces:**
- Consumes: `Catalog::load`, `catalog_src::all_stations`, `catalog_src::favorite_stations`, `AudioEngine::spawn` (all existing).
- Produces: `MiniApp` gains fields `catalog: Catalog`, `fav_path: PathBuf`, `hist_path: PathBuf`, `blacklist_path: PathBuf`. No behaviour change yet (Task 4 uses them).

- [ ] **Step 1: Add imports + fields** — in `crates/radio-mini/src/app.rs`, add `use std::path::PathBuf;` near the top imports, and change the struct to:

```rust
pub struct MiniApp {
    state: MiniState,
    theme: Theme,
    engine: Option<AudioEngine>,
    tray: Option<Tray>,
    tray_ready: bool,
    catalog: Catalog,
    fav_path: PathBuf,
    hist_path: PathBuf,
    blacklist_path: PathBuf,
}
```

- [ ] **Step 2: Keep the catalog + paths in `new()`** — replace the body of `MiniApp::new()` with:

```rust
    pub fn new() -> anyhow::Result<Self> {
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

        Ok(Self {
            state,
            theme: Theme::amber(),
            engine,
            tray: None,
            tray_ready: false,
            catalog,
            fav_path,
            hist_path,
            blacklist_path,
        })
    }
```

- [ ] **Step 3: Silence the not-yet-used fields** — `catalog`/paths are written but not read until Task 4. Add `#[allow(dead_code)]` on the four new fields (remove in Task 4):

In the struct from Step 1, prefix the four new fields:
```rust
    #[allow(dead_code)]
    catalog: Catalog,
    #[allow(dead_code)]
    fav_path: PathBuf,
    #[allow(dead_code)]
    hist_path: PathBuf,
    #[allow(dead_code)]
    blacklist_path: PathBuf,
```

- [ ] **Step 4: Build + test**

Run: `cargo build -p radio-mini && cargo test -p radio-mini --bin world-radio-mini`
Expected: compiles; 19 tests pass (17 existing + 2 from Task 2).

- [ ] **Step 5: fmt + clippy**

Run: `cargo fmt -p radio-mini && cargo clippy -p radio-mini --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/radio-mini/src/app.rs
git commit -m "refactor: mini keeps catalog and data paths"
```

---

### Task 4: Functional favorite toggle

**Files:**
- Modify: `crates/radio-mini/src/app.rs`
- Modify: `crates/radio-mini/src/catalog_src.rs` (add a tested toggle+reload helper)

**Interfaces:**
- Consumes: `Catalog::toggle_favorite(&mut self, &str) -> bool`, `Catalog::save_state(&self, &Path, &Path, &Path)`, `Catalog::is_favorite(&self, &str) -> bool`, `catalog_src::favorite_stations(&Catalog)` (all existing).
- Produces:
  - `catalog_src::toggle_and_reload(catalog: &mut Catalog, uuid: &str) -> anyhow::Result<Vec<StationPick>>` — toggles favorite, returns the refreshed favorites list (no disk I/O; persistence stays in `app.rs`).
  - `MiniApp::toggle_favorite(&mut self)` and `MiniApp::now_is_favorite(&self) -> bool`.

- [ ] **Step 1: Write the failing test for the reload helper** — add to the `tests` module in `crates/radio-mini/src/catalog_src.rs`:

```rust
    #[test]
    fn toggle_and_reload_reflects_change() {
        let mut cat = catalog();
        let favs = toggle_and_reload(&mut cat, "u1").unwrap();
        assert_eq!(favs.len(), 1);
        assert_eq!(favs[0].uuid, "u1");
        // toggling again removes it
        let favs = toggle_and_reload(&mut cat, "u1").unwrap();
        assert!(favs.is_empty());
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p radio-mini --bin world-radio-mini toggle_and_reload`
Expected: FAIL — `toggle_and_reload` not found.

- [ ] **Step 3: Implement the helper** — add to `crates/radio-mini/src/catalog_src.rs` (top level):

```rust
pub fn toggle_and_reload(
    catalog: &mut Catalog,
    uuid: &str,
) -> anyhow::Result<Vec<StationPick>> {
    catalog.toggle_favorite(uuid);
    favorite_stations(catalog)
}
```

Ensure the `use radio_core::catalog::Catalog;` import at the top is in scope (it already is — `favorite_stations` uses it).

- [ ] **Step 4: Run it to verify it passes**

Run: `cargo test -p radio-mini --bin world-radio-mini toggle_and_reload`
Expected: PASS.

- [ ] **Step 5: Wire it into `MiniApp`** — in `crates/radio-mini/src/app.rs`, remove the four `#[allow(dead_code)]` attributes added in Task 3 (the fields are now read), and add these methods inside `impl MiniApp` (next to `shuffle`/`stop`):

```rust
    fn now_is_favorite(&self) -> bool {
        match &self.state.now {
            Some(pick) => self.catalog.is_favorite(&pick.uuid),
            None => false,
        }
    }

    fn toggle_favorite(&mut self) {
        let Some(pick) = self.state.now.clone() else {
            return;
        };
        match catalog_src::toggle_and_reload(&mut self.catalog, &pick.uuid) {
            Ok(favorites) => self.state.set_favorites(favorites),
            Err(e) => eprintln!("toggle favorite failed: {e}"),
        }
        if let Err(e) = self.catalog.save_state(
            &self.fav_path,
            &self.hist_path,
            &self.blacklist_path,
        ) {
            eprintln!("save favorites failed: {e}");
        }
    }
```

- [ ] **Step 6: Add `set_favorites` to `MiniState`** — in `crates/radio-mini/src/state.rs`, add inside `impl MiniState` (next to `load_stations`):

```rust
    pub fn set_favorites(&mut self, favorites: Vec<StationPick>) {
        self.favorites = favorites;
    }
```

- [ ] **Step 7: Build + full test**

Run: `cargo build -p radio-mini && cargo test -p radio-mini --bin world-radio-mini`
Expected: compiles; all tests pass (20 now). `toggle_favorite`/`now_is_favorite` are not yet called from `ui()` (Task 5 wires the button) — if clippy flags them as unused, that is resolved in Task 5; to keep this task green, add `#[allow(dead_code)]` on both methods now and remove them in Task 5.

- [ ] **Step 8: fmt + clippy**

Run: `cargo fmt -p radio-mini && cargo clippy -p radio-mini --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 9: Commit**

```bash
git add crates/radio-mini/src/app.rs crates/radio-mini/src/catalog_src.rs crates/radio-mini/src/state.rs
git commit -m "feat: mini favorite toggle writes favorites.json"
```

---

### Task 5: Render the amber-CRT panel (5 sections + FavStar wiring)

**Files:**
- Modify: `crates/radio-mini/src/app.rs`

**Interfaces:**
- Consumes: `state_labels`, `spectrum_bars` (Task 2); `Theme` tokens (Task 1); `MiniApp::toggle_favorite`/`now_is_favorite` (Task 4); existing `shuffle`/`stop`/`set_volume`/`set_scope`.
- Produces: the redesigned `ui()` body. Remove the `#[allow(dead_code)]` from `toggle_favorite`/`now_is_favorite` (now called).

- [ ] **Step 1: Replace the `ui()` body** — in `crates/radio-mini/src/app.rs`, replace the whole `fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame)` body with:

```rust
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let t = self.theme;
        let phase = self.state.phase;
        let (dot_label, primary_label) = crate::state::state_labels(phase);

        let dot_color = match phase {
            Phase::Idle => t.dim,
            Phase::Buffering => t.warn,
            Phase::Playing => t.ok,
            Phase::Error => t.err,
        };

        // header: wordmark · state dot · meta
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("▌r4dio").color(t.hi).strong());
            ui.label(egui::RichText::new(dot_label).color(dot_color).small());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let meta = self
                    .state
                    .now
                    .as_ref()
                    .map(|_| "live")
                    .unwrap_or("—");
                ui.label(egui::RichText::new(meta).color(t.dim).small());
            });
        });

        // station + now-playing + fav star
        let station = self
            .state
            .now
            .as_ref()
            .map(|n| n.name.clone())
            .unwrap_or_else(|| "Nothing playing".into());
        let now_text = match phase {
            Phase::Idle => "press Shuffle to start listening",
            Phase::Buffering => "connecting to stream…",
            Phase::Playing => "now playing",
            Phase::Error => "stream offline — couldn't connect",
        };
        let now_color = match phase {
            Phase::Error => t.err,
            Phase::Buffering => t.warn,
            Phase::Idle => t.dim,
            Phase::Playing => t.accent,
        };
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                let name_color = match phase {
                    Phase::Idle => t.dim,
                    _ => t.bright,
                };
                ui.label(egui::RichText::new(station).color(name_color).strong());
                ui.label(egui::RichText::new(now_text).color(now_color).small());
            });
            if self.state.now.is_some() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    let star = match self.now_is_favorite() {
                        true => "★",
                        false => "☆",
                    };
                    if ui.button(egui::RichText::new(star).color(t.hi)).clicked() {
                        self.toggle_favorite();
                    }
                });
            }
        });

        // spectrum + volume
        ui.horizontal(|ui| {
            let bars = crate::state::spectrum_bars(16);
            let active = phase == Phase::Playing;
            let bar_color = match active {
                true => t.hi,
                false => t.dim,
            };
            let (rect, _) = ui.allocate_exact_size(egui::vec2(120.0, 16.0), egui::Sense::hover());
            let painter = ui.painter_at(rect);
            let bw = rect.width() / bars.len() as f32;
            for (i, &h) in bars.iter().enumerate() {
                let x = rect.left() + i as f32 * bw;
                let bar_h = (h * rect.height()).max(2.0);
                painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(x + 1.0, rect.bottom() - bar_h),
                        egui::pos2(x + bw - 1.0, rect.bottom()),
                    ),
                    0.0,
                    bar_color,
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new("VOL").color(t.dim).small());
                let mut v = self.state.volume;
                if ui
                    .add(egui::Slider::new(&mut v, 0.0..=1.0).show_value(false))
                    .changed()
                {
                    self.set_volume(v);
                }
            });
        });

        // controls: shuffle (primary) + play/stop
        ui.horizontal(|ui| {
            if ui
                .button(egui::RichText::new(format!("⇄ {primary_label}")).color(t.bg).strong())
                .clicked()
            {
                self.shuffle();
            }
            let playing = phase == Phase::Playing || phase == Phase::Buffering;
            let glyph = match playing {
                true => "⏸",
                false => "▶",
            };
            if ui.button(egui::RichText::new(glyph).color(t.fg)).clicked() {
                match playing {
                    true => self.stop(),
                    false => self.shuffle(),
                }
            }
        });

        // scope
        ui.horizontal(|ui| {
            let scope_all = self.state.scope == Scope::All;
            if ui.selectable_label(scope_all, "ALL").clicked() {
                self.state.set_scope(Scope::All);
            }
            if ui.selectable_label(!scope_all, "★ FAVS").clicked() {
                self.state.set_scope(Scope::Favorites);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new("shuffle scope").color(t.dim).small());
            });
        });
    }
```

- [ ] **Step 2: Remove the dead-code allows** — in `crates/radio-mini/src/app.rs`, delete the `#[allow(dead_code)]` attributes on `toggle_favorite` and `now_is_favorite` added in Task 4 (they are called now).

- [ ] **Step 3: Confirm `logic()` still sets the panel fill** — verify `fn logic(...)` still contains the visuals block setting `panel_fill = t.bg` and `override_text_color = Some(t.fg)`. Leave it unchanged.

- [ ] **Step 4: Build + test**

Run: `cargo build -p radio-mini && cargo test -p radio-mini --bin world-radio-mini`
Expected: compiles; all tests pass (20).

- [ ] **Step 5: fmt + clippy**

Run: `cargo fmt -p radio-mini && cargo clippy -p radio-mini --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 6: MANUAL smoke test (macOS)**

Run: `cargo run -p radio-mini`
Expected: the window shows the amber-CRT panel:
- on launch: header `▌r4dio IDLE`, "Nothing playing / press Shuffle to start listening", dim spectrum, VOL slider, `⇄ SHUFFLE` + `▶`, `ALL` / `★ FAVS` scope.
- click Shuffle → buffering ("connecting to stream…") → live ("now playing"), spectrum brightens, `▶` becomes `⏸`, ☆ star appears next to the station.
- click ☆ → becomes ★; switch scope to `★ FAVS`, Shuffle → plays a favorited station.
- quit, relaunch → the favorited station persists (written to `favorites.json`).

- [ ] **Step 7: Commit**

```bash
git add crates/radio-mini/src/app.rs
git commit -m "feat: amber-crt mini popup layout with favorite toggle"
```

---

## Self-Review Notes

- **Spec coverage:** theme tokens (Task 1) · state→label + static spectrum (Task 2) · MiniApp owns Catalog (Task 3) · functional favorite toggle writing favorites.json (Task 4) · 5-section amber-CRT layout + FavStar wiring (Task 5). Manual macOS smoke test at Task 5 covers all four states + persistence.
- **Out of scope (per spec):** tray-popover behaviour (hide Dock, borderless, click-to-toggle) — next spec; live FFT spectrum animation / scanline / glow / marquee; the other 4 themes + switcher; sync.
- **Type consistency:** `state_labels`/`spectrum_bars` (Task 2) used verbatim in Task 5; `toggle_and_reload`/`set_favorites`/`toggle_favorite`/`now_is_favorite` defined in Task 4 and called in Task 5; `Catalog`/`StationPick`/`Phase`/`Scope` names match existing code.
- **Known fragility:** the `meta` line in the header is a placeholder ("live"/"—"); richer country/codec metadata from the playing `StationPick` is a trivial follow-up once `StationPick` carries it (it currently does not). The spectrum is static by design this iteration — idle bars render at full height in `dim` color rather than the design's shortened idle bars; shortening + live FFT animation are the later polish pass.
