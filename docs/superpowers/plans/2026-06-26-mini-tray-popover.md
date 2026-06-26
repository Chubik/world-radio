# Mini Tray-Popover Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development or executing-plans. Steps use checkbox (`- [ ]`).

**Goal:** Turn the mini window into a real macOS menu-bar companion: no Dock icon, borderless popup, tray-icon click toggles the popup, focus-loss hides it.

**Architecture:** Keep eframe/egui (owns the event loop). Configure the viewport as a borderless, no-taskbar, always-on-top, initially-hidden window. Hide the Dock icon via a winit `ActivationPolicy::Accessory` hook on `NativeOptions.event_loop_builder`. Drive show/hide from tray-icon **left-click** events (`TrayIconEvent::Click`) via `ViewportCommand::Visible`, and hide on `ViewportEvent::Focused(false)`.

**Tech Stack:** eframe/egui 0.35, tray-icon 0.24 (`TrayIconEvent`), winit 0.30 (`ActivationPolicy`).

## Global Constraints

- No code comments; logs English + lowercase; no `else if`.
- mini is a `[[bin]]` — tests via `cargo test -p radio-mini --bin world-radio-mini`.
- `cargo fmt -p radio-mini` + `cargo clippy -p radio-mini --all-targets -- -D warnings` clean.
- Commit to `dev`; messages English, concise, no AI/personal mentions.
- **macOS GUI behaviour cannot be unit-tested** — each GUI task ends with a manual smoke test the human runs. The implementer does code+build+clippy and SKIPS the manual run.
- This is the **riskiest** area (tray+eframe loop on macOS previously aborted). Keep each step independently revertable; if hide-Dock or popover positioning destabilizes startup, fall back to the prior working window.

---

### Task 1: Borderless, hidden-on-start viewport

**Files:**
- Modify: `crates/radio-mini/src/main.rs`

**Interfaces:**
- Produces: the window starts borderless, no taskbar entry, always-on-top, **not visible** until the tray shows it. Tray click handling lands in Task 3 — after this task the window is hidden with no way to show it yet, which is expected mid-plan.

- [ ] **Step 1: Update the viewport builder** — in `crates/radio-mini/src/main.rs`, replace the `viewport` line of `NativeOptions` with:

```rust
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([300.0, 200.0])
            .with_decorations(false)
            .with_taskbar(false)
            .with_always_on_top()
            .with_visible(false),
```

- [ ] **Step 2: Build**

Run: `cargo build -p radio-mini`
Expected: compiles.

- [ ] **Step 3: fmt + clippy + test**

Run: `cargo fmt -p radio-mini && cargo clippy -p radio-mini --all-targets -- -D warnings && cargo test -p radio-mini --bin world-radio-mini`
Expected: clean; 21 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/radio-mini/src/main.rs
git commit -m "feat: mini window is borderless and hidden on start"
```

---

### Task 2: Hide the Dock icon (macOS accessory policy)

**Files:**
- Modify: `crates/radio-mini/src/main.rs`

**Interfaces:**
- Consumes: `NativeOptions.event_loop_builder` (eframe 0.35, type `Option<EventLoopBuilderHook>`).
- Produces: on macOS the process runs as an accessory (no Dock icon, no app menu). Non-macOS unaffected (the hook is `#[cfg(target_os = "macos")]`).

- [ ] **Step 1: Add the activation-policy hook** — in `crates/radio-mini/src/main.rs`, after constructing `options` (the `NativeOptions`), set the hook. Add this block (gated to macOS):

```rust
    #[cfg(target_os = "macos")]
    {
        use eframe::egui::ViewportBuilder as _;
        options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
            builder.with_activation_policy(ActivationPolicy::Accessory);
        }));
    }
```

Note: `options` must be `let mut options`. If `winit` is not a direct dependency, add it to `crates/radio-mini/Cargo.toml` `[dependencies]`: `winit = "0.30"` (matches the version eframe resolves — verify with `cargo tree -p radio-mini | grep '^.*winit v'` and pin to that). Drop the unused `ViewportBuilder as _` import if it is not needed for the cfg block to compile.

- [ ] **Step 2: Verify the winit version matches eframe's**

Run: `cargo tree -p radio-mini 2>/dev/null | grep -E 'winit v' | head -1`
Expected: a single `winit v0.30.x`. Pin the `Cargo.toml` entry to that minor (e.g. `winit = "0.30"`). If two winit versions appear, STOP and report — a second winit would re-introduce the objc2 conflict.

- [ ] **Step 3: Build**

Run: `cargo build -p radio-mini`
Expected: compiles. If `with_activation_policy`/`EventLoopBuilderExtMacOS` is not found, the winit feature/path differs on the resolved version — report the exact compile error.

- [ ] **Step 4: fmt + clippy + test**

Run: `cargo fmt -p radio-mini && cargo clippy -p radio-mini --all-targets -- -D warnings && cargo test -p radio-mini --bin world-radio-mini`
Expected: clean; 21 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/radio-mini/src/main.rs crates/radio-mini/Cargo.toml Cargo.lock
git commit -m "feat: hide mini dock icon via accessory policy on macos"
```

---

### Task 3: Tray click toggles the popup; focus-loss hides it

**Files:**
- Modify: `crates/radio-mini/src/app.rs`

**Interfaces:**
- Consumes: `tray_icon::TrayIconEvent` (`Click { button, button_state, .. }`), `egui::ViewportCommand::Visible(bool)`, the existing `handle_tray_events` + `logic()`.
- Produces: left-click on the tray icon toggles window visibility; clicking away (focus lost) hides it. The app tracks `visible: bool`.

- [ ] **Step 1: Add a visibility field** — in `crates/radio-mini/src/app.rs`, add `visible: bool` to `MiniApp` and initialize it `false` in `new()` (the window starts hidden, matching Task 1).

- [ ] **Step 2: Drain tray-icon click events** — add a method on `MiniApp` that toggles visibility on a left button-press:

```rust
    fn handle_tray_clicks(&mut self, ctx: &egui::Context) {
        use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            let toggle = matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Down,
                    ..
                }
            );
            if toggle {
                self.visible = !self.visible;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(self.visible));
                if self.visible {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
            }
        }
    }
```

- [ ] **Step 3: Hide on focus loss** — in `logic()`, after the existing `handle_tray_events()` call, add the click handler and a focus-loss check:

```rust
        self.handle_tray_clicks(ctx);
        let focused = ctx.input(|i| i.focused);
        if self.visible && !focused {
            self.visible = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
```

(Insert this right after `self.handle_tray_events();` — `handle_tray_events` stays for the menu items; the new code adds left-click toggle + focus-hide.)

- [ ] **Step 4: Build**

Run: `cargo build -p radio-mini`
Expected: compiles. If `MouseButton`/`MouseButtonState` import path differs, check `tray_icon::` exports and adjust (they live at the crate root in 0.24).

- [ ] **Step 5: fmt + clippy + test**

Run: `cargo fmt -p radio-mini && cargo clippy -p radio-mini --all-targets -- -D warnings && cargo test -p radio-mini --bin world-radio-mini`
Expected: clean; 21 tests pass.

- [ ] **Step 6: MANUAL smoke test (macOS)** — human runs:

Run: `cargo run -p radio-mini`
Expected:
- No Dock icon appears; a tray icon shows in the menu bar; no window on launch.
- Left-click the tray icon → the amber-CRT popup appears (borderless).
- Click elsewhere (popup loses focus) → it hides.
- Left-click the tray icon again → it reappears.
- The tray right-click menu (Shuffle / Quit) still works.

- [ ] **Step 7: Commit**

```bash
git add crates/radio-mini/src/app.rs
git commit -m "feat: tray click toggles mini popup, focus loss hides it"
```

---

## Self-Review Notes

- **Spec coverage:** borderless+hidden viewport (Task 1) · hide Dock via accessory policy (Task 2) · tray-click toggle + focus-loss hide (Task 3). Manual macOS smoke test at Task 3 covers the full popover behaviour.
- **Risk (called out in Global Constraints):** macOS tray+eframe loop previously aborted from objc2 version conflict — Task 2 Step 2 explicitly guards against a second winit version. If startup destabilizes, revert the offending task; each is independent.
- **Out of scope:** popover positioning exactly under the tray icon (MVP centers/uses OS default placement); per-platform tray variants (Windows/Linux); the popover "arrow"/native NSPopover look.
- **Type consistency:** `visible: bool`, `handle_tray_clicks(ctx)`, `TrayIconEvent::Click`, `ViewportCommand::Visible`/`Focus` — used consistently. `handle_tray_events` (menu) untouched and coexists with the new click handler.
