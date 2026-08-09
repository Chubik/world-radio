# Startup Update Visibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the existing startup update check fast to fail and visible in the TUI, without ever delaying the radio.

**Architecture:** Two independent point changes. The update-check HTTP client in `radio-core` gets tight timeouts (and the download client loses its 30 s default, which could kill a slow legitimate download); the TUI's existing `Msg::UpdateAvailable` handler additionally surfaces a notice line. No new modules, messages, keys, or flows.

**Tech Stack:** Rust, reqwest (blocking), the existing Elm-style TUI update loop.

**Spec:** `docs/superpowers/specs/2026-08-09-startup-update-visibility.md`

## Global Constraints

- No code comments unless they state a constraint the code cannot show (project rule).
- All strings/logs in English, lowercase-first (project rule).
- Commit subjects are the public changelog — write them for users.
- No `else if`.
- `cargo fmt` + `cargo clippy` clean before every commit.

---

### Task 1: Bound the update-check network, unbound the download

**Files:**
- Modify: `crates/radio-core/src/update.rs:60-63` (check client), `:129-132` (apply client)

**Interfaces:**
- Consumes: nothing new.
- Produces: no signature changes — `check_from`, `check_latest`, `apply` keep their exact signatures. Task 2 does not depend on this task.

- [ ] **Step 1: Add timeouts to the check client**

In `check_from`, replace the client construction:

```rust
let client = reqwest::blocking::Client::builder()
    .user_agent("world-radio-update/1")
    .connect_timeout(std::time::Duration::from_secs(2))
    .timeout(std::time::Duration::from_secs(5))
    .build()?;
```

Rationale from the spec: reqwest's blocking default is a 30 s total timeout — far too slow to fail for an interactive `r4dio update`, and pointlessly long for the silent startup thread.

- [ ] **Step 2: Free the download client from the 30 s default**

In `apply`, replace the client construction:

```rust
let client = reqwest::blocking::Client::builder()
    .user_agent("world-radio-update/1")
    .connect_timeout(std::time::Duration::from_secs(5))
    .timeout(None)
    .build()?;
```

The tarball is ~5 MB; on a slow link the default total timeout can abort a legitimate download mid-way. Connect stays bounded; transfer does not.

- [ ] **Step 3: Run the update tests**

Run: `cargo test -p radio-core update`
Expected: all existing tests PASS (mockito servers are local, well inside 5 s).

- [ ] **Step 4: fmt, clippy, commit**

```bash
cargo fmt
cargo clippy --all-targets -p radio-core
git add crates/radio-core/src/update.rs
git commit -m "fail the update check fast on a bad network"
```

---

### Task 2: Announce a new version in the TUI header

**Files:**
- Modify: `crates/radio-tui/src/tui/update.rs:213-216` (the `Msg::UpdateAvailable` arm)
- Test: same file, `mod tests` at the bottom (uses the existing `model()` helper)

**Interfaces:**
- Consumes: `Msg::UpdateAvailable(radio_core::update::Release)` (exists), `model.notice: Option<String>` (exists), `model.pending_update: Option<Release>` (exists).
- Produces: nothing new — the notice renders through the existing third-header-line path in `view/header.rs:97`.

- [ ] **Step 1: Write the failing test**

Add to `mod tests` in `crates/radio-tui/src/tui/update.rs`:

```rust
#[test]
fn update_available_sets_pending_and_notice() {
    let mut m = model();
    let rel = radio_core::update::Release {
        version: "9.9.9".into(),
        tarball_url: "http://localhost/r4dio.tar.gz".into(),
        sha256: "abc".into(),
    };
    update(&mut m, Msg::UpdateAvailable(rel.clone()));
    assert_eq!(m.pending_update, Some(rel));
    let notice = m.notice.as_deref().unwrap();
    assert!(notice.contains("9.9.9"));
    assert!(notice.contains("press U"));
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p radio-tui update_available_sets_pending_and_notice`
Expected: FAIL — `m.notice` is `None` (the current arm only sets `pending_update`).

- [ ] **Step 3: Implement**

Replace the `Msg::UpdateAvailable` arm:

```rust
Msg::UpdateAvailable(rel) => {
    model.notice = Some(format!(
        "new version v{} available — press U to update",
        rel.version
    ));
    model.pending_update = Some(rel);
    vec![]
}
```

(`notice` first: `rel` moves into `pending_update`.)

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p radio-tui update_available_sets_pending_and_notice`
Expected: PASS.

- [ ] **Step 5: Run the whole TUI test suite**

Run: `cargo test -p radio-tui`
Expected: all PASS — no other test asserts on `notice` after `UpdateAvailable`.

- [ ] **Step 6: fmt, clippy, commit**

```bash
cargo fmt
cargo clippy --all-targets -p radio-tui
git add crates/radio-tui/src/tui/update.rs
git commit -m "announce a new version in the tui at startup"
```

---

## Verification after both tasks

- `cargo test --workspace` green, `cargo clippy --all-targets` clean.
- Manual sanity (optional, plays audio out loud — warn the user first): run the TUI; with no newer release nothing appears and startup is instant. The notice itself can only be seen live once the next release ships.
- Push `dev`. No release PR — this rides along with whatever ships next.
