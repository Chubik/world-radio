# r4dio for macOS — the full app: implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to
> implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Spec:** `docs/superpowers/specs/2026-08-08-r4dio-macos-full-app.md`
**Design handoff:** `_private/design-docs/design/macos/handoff-2026-08-08.html` — seven mocked
screens. It wins on appearance; the spec wins on behaviour.

**Goal:** turn the shipped menubar surface into a full player, so someone who never installs the
CLI can create a sync account, manage favourites, unblock stations, filter countries and search the
catalogue.

**Architecture:** `radio-core` already implements every backend capability — this plan adds IPC
commands, one new window, and its UI. **No new domain logic in `radio-core`.**

## Global Constraints

- English code/comments/logs; comments **lowercase**, explaining WHY not WHAT. Trivial comments are
  a defect.
- **No AI/assistant mention anywhere** — code, comments, commit messages, no trailers.
- **No `else if`** — applies to JavaScript too.
- Commit subjects are the **public changelog**: user-facing, concise, lowercase.
- Amber CRT palette from `themes.js`; IBM Plex Mono for chrome, IBM Plex Sans for station names.
- Plain HTML/CSS/JS in `ui/` — no bundler, no framework.
- **Anything macOS-only sits behind `#[cfg(target_os = "macos")]`.** CI runs
  `clippy --workspace` on Linux and this already broke the build once.
- `cargo fmt`, `cargo clippy --workspace --all-targets`, `cargo test --workspace` clean before every
  commit. Baseline: **408 tests**.
- Work on `dev` in `/Users/vchub/dev/projects/world-radio/radio`.
- **Never launch the app without warning the user — it plays audio out loud on their machine.**
  Do not leave it running while waiting for a reply.

## Decisions — the spec's three open questions, answered

**1. One window, not two.** The `sync` popover is folded into the main window's Account section and
its window definition removed. Two windows for one flow is the thing the handoff argues against.

**2. Search runs offline only.** Measured on this machine, not assumed: the local cache
(`stations.db`, 33MB) holds **58,466 stations** — the whole catalogue. A name search takes
**0.29s**, the country facet query **0.09s**. Online search adds a network dependency for no
coverage. `search_offline_filtered` and `facets` are the only search paths.

**3. `Start at login` is cut from the first release.** It is a genuine handoff addition but needs
a login-item registration API and its own verification. Ship the seven screens first; a menu item
that silently does nothing is worse than its absence.

## File Structure

- `crates/r4dio-macos/src/commands.rs` — gains the account, catalogue and list commands.
- `crates/r4dio-macos/src/main.rs` — main window, revised tray menu, the global hotkey.
- `crates/r4dio-macos/tauri.conf.json` — the `main` window; the `sync` window is removed.
- `crates/r4dio-macos/ui/main.html` / `main.css` / `main.js` — **new**, the main window.
- `crates/r4dio-macos/ui/views/*.js` — **new**, one module per section, so no file grows past the
  point where it can be held in context.
- `crates/r4dio-macos/ui/labels.js` — gains pure helpers; stays DOM-free and testable.
- `crates/r4dio-macos/ui/labels.test.html` — gains their cases.
- `crates/r4dio-macos/ui/sync.html` / `sync.css` / `sync.js` — **deleted** (folded into the window).

---

### Task 1: The account commands and the Sync section

The most harmful gap: today `Sync…` asks for a key the app gives no way to obtain.

**Files:** modify `commands.rs`; create `ui/views/account.js`.

**Interfaces produced:**
- `create_account() -> Result<String, String>` — calls `sync::client::Client::create_account()`
  (`client.rs:34`), stores the key with `sync::store_key`, returns the **full** id for the
  save-this screen.
- `account_state() -> AccountState { signed_in: bool, masked: String, favourites: u32 }` —
  `masked` is `r4-7K2P-····-4DF1` shaped, **never the full key**.
- `sign_in(key: String) -> Result<(), String>` — validates, stores, pulls.
- `sign_out()` — `sync::clear_key`. Does not delete the server account.
- `delete_account() -> Result<(), String>` — `Client::delete(key)` (`client.rs:71`), then clears.

- [ ] **Step 1: Write the failing tests for the pure parts**

In `labels.js` add `maskKey(key)` and `accountStatus(state)`; add cases to `labels.test.html`:
`maskKey("r4-7K2P-9QXM-4DF1")` → `"r4-7K2P-····-4DF1"`; a short or empty key must not panic or
leak; `maskKey` output must never contain the middle segments. Run the page, watch them fail.

- [ ] **Step 2: Implement the commands**

Each returns `Result<_, String>` so a failure surfaces to the user rather than being swallowed —
the shipped sync window's `set_sync_key` bool is the pattern to follow, not to repeat.

- [ ] **Step 3: Build the three-state section**

Per the handoff: **not signed in** (copy + `⚡ Generate account` + `Have an ID? Sign in`) →
**save this, shown once** (full id, copy, "no email, no recovery", explicit "Done, I saved it") →
**signed in** (masked id, favourites count, QR, `Sign out`).

**Binding: after the save-this screen the full key is never rendered again** — not in the DOM, a
title attribute, a log, or an error string. The QR encodes the same key `SyncActivity` scans on
Android.

- [ ] **Step 4: Prove the key does not leak**

Generate an account, complete the flow, then search the rendered DOM and every console message for
the full key. Report what you searched and what you found. **This is the one check in this task
that cannot be skipped.**

- [ ] **Step 5: Commit** — `set up device sync without the terminal`

---

### Task 2: The main window shell and Favourites

**Files:** modify `tauri.conf.json`, `main.rs`; create `ui/main.html`, `main.css`, `main.js`,
`ui/views/favourites.js`.

**Interfaces produced:**
- A `main` window: titled, resizable, traffic lights, in the app switcher. **Not** a popover.
  900×600 per the handoff.
- `favourites() -> Vec<StationRow>` where `StationRow { uuid, name, country, codec, bitrate,
  is_playing }`.
- `play_uuid(uuid: String)`, `remove_favourite(uuid: String)`, `shuffle_favourites()`.

- [ ] **Step 1: The window and its policy dance**

An accessory app cannot activate, so the window opens behind everything unless the policy flips.
`show_sync` in `main.rs` already does this correctly — `set_regular` on open, `set_accessory` on
close, `prevent_close` so the close button hides rather than quitting. Reuse that shape.

- [ ] **Step 2: Sidebar navigation**

Groups per the handoff: **Library** (Favorites, Browse) · **Filters** (Countries, Blocked) ·
**Account** (Sync). Footer line: `◔ 3 excluded · ⛌ 14 blocked`, from real counts.

Section switching is pure: put `activeSection(id)` in `labels.js` with tests, not in the DOM code.

- [ ] **Step 3: The favourites list**

Flag, name, codec/bitrate, `● now playing` on the live row, `★` per row to remove. Row click plays.
`⇄ Shuffle favorites` at the foot.

**Must scroll** — the mockup shows four rows; the user has seven and it will grow. **Must have an
empty state** — a new user has none, and it must not read as an error.

- [ ] **Step 4: Verify against real data**

The user's account has **7 favourites**. Open the window and confirm all seven render, scroll, and
that removing one persists across a restart. Report what you saw.

- [ ] **Step 5: Commit** — `see and manage your favourites in the app`

---

### Task 3: Blocked stations and country filters

Two sections, one shape: a list with a per-row toggle.

**Files:** modify `commands.rs`; create `ui/views/blocked.js`, `ui/views/countries.js`.

**Interfaces produced:**
- `blocked() -> Vec<StationRow>`, `unblock(uuid: String)`.
- `countries() -> Vec<CountryRow { code, name, count, excluded }>` — from
  `Catalog::facets(limit)` (`catalog.rs:140`), which already returns `Vec<(String, u32)>`.
- `set_excluded(codes: Vec<String>)` — `Catalog::set_excluded_countries`.

- [ ] **Step 1: Blocked**

List with `Unblock` per row. The user has **14**; they are invisible outside the CLI today.

- [ ] **Step 2: Countries**

Searchable list with checkboxes, `3 of 194` style count.

**Two binding rules:**
- **Exclusions only. No whitelist.** The user refused an "only UA" mode on 2026-08-07.
- **RU/BY must never appear in this list** — not as a row, a disabled row, or a pre-ticked
  exclusion. They are hard-filtered in code (`cache.rs:419`) and that stays invisible. Write a test
  asserting the country list excludes them.

- [ ] **Step 3: Verify**

Confirm the 14 blocked and the 3 excluded (`CH, CN, IN`) match the account. Unblock one, restart,
confirm it stayed unblocked. Confirm RU and BY appear nowhere in the countries list.

- [ ] **Step 4: Commit** — `unblock stations and choose which countries to skip`

---

### Task 4: Browse and search

The largest section. 58,466 stations locally.

**Files:** modify `commands.rs`; create `ui/views/browse.js`.

**Interfaces produced:**
- `search(name: String, limit: u32) -> Vec<StationRow>` — `search_offline_filtered`
  (`catalog.rs:118`).
- `stations_in(country: String, limit: u32) -> Vec<StationRow>`.
- `add_favourite(uuid: String)`.

- [ ] **Step 1: Search**

A name field; results show flag, name, codec/bitrate, `already in ★` when applicable, and a `☆` to
add. **Bound the result count** — never render 58k rows. Debounce input; a search is 0.29s measured.

- [ ] **Step 2: The country tree**

Only countries that have stations, each with its count — `facets()` returns exactly that. Expand a
country to load its stations **lazily**, not upfront.

- [ ] **Step 3: Verify at scale**

Search a common word (`jazz` matches 671 locally — measured) and confirm the UI stays responsive
and the list is bounded. Expand a large country (US has 7,666) and confirm it does not freeze.

- [ ] **Step 4: Commit** — `search the whole catalogue and browse by country`

---

### Task 5: The tray menu and the shuffle hotkey

The only surface that survives another app's fullscreen space.

**Files:** modify `main.rs`, `Cargo.toml`.

- [ ] **Step 1: The revised menu**

```
⇄ Shuffle — all stations      ⌥⇧R
★ Shuffle — favorites
⏯ Play / Stop
☆ Add to favorites
▣ Open r4dio
◇ r4-7K2P-····-4DF1
✕ Quit r4dio
```

`Open r4dio` opens the main window on Favourites. The masked-id line navigates to Account. The
separate `Sync…` window is gone. `Start at login` is **cut** — see Decisions.

- [ ] **Step 2: The global hotkey `⌥⇧R`**

Use `tauri-plugin-global-shortcut`. **Check the crate's real API before writing the call** — this
plan has been wrong about Tauri signatures before, and guessing cost a whole task last time.

- [ ] **Step 3: The permission prompt**

A global hotkey needs macOS Accessibility. **Ask right after first launch with an explanation of
why** — an unexplained prompt reads as spam. **If the user declines, everything else must keep
working.** Verify that path explicitly: decline, then confirm the panel, the menu and the window
are unaffected.

- [ ] **Step 4: Verify in the situation it exists for**

Enter a fullscreen app. Press `⌥⇧R`. The station must change with nothing opening. This is the
whole point of the task — report honestly whether it worked.

- [ ] **Step 5: Commit** — `change station from anywhere with a keyboard shortcut`

---

### Task 6: The mini panel's scrolling name

The one requirement from the handoff not yet met.

**Files:** modify `ui/app.css`, `ui/app.js`.

- [ ] **Step 1: Scroll instead of truncate**

`Katherine FM — Katherine — 101.7` is a real station from the user's own listening — the current
CSS ellipsises it. The handoff specifies the full name scrolling. **The user was explicit: the name
must always be visible.**

Scroll only when it overflows; a short name must sit still, not drift.

- [ ] **Step 2: Verify both cases** — a long name scrolls fully, a short one does not move.

- [ ] **Step 3: Commit** — `show the whole station name, however long`

---

### Task 7: Ship it

- [ ] **Step 1: Full verification**

`cargo fmt`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`, plus the
`labels.test.html` page. Then build the bundle: `cd crates/r4dio-macos && cargo tauri build
--target universal-apple-darwin` and confirm `lipo -archs` reports **both** `x86_64` and `arm64`.

- [ ] **Step 2: Release**

`dev` → `main` PR, admin-merge with `[minor]`. Pushing to `dev` and opening the PR starts two runs;
the merge 409s while either is going. Wait for both.

---

## Self-Review

**Spec coverage:** Sync → Task 1 · Favourites → Task 2 · Blocked and Countries → Task 3 ·
Browse/Search → Task 4 · Tray and hotkey → Task 5 · Mini panel name → Task 6 · Release → Task 7.
The main window shell is folded into Task 2 because a sidebar with nothing behind it cannot be
verified.

**The three open questions are answered above with measurements, not guesses** — 58,466 stations,
0.29s search, 0.09s facets, all measured on this machine.

**Two places this plan expects to be wrong, flagged rather than hidden:** the global-shortcut
plugin's API (Task 5 Step 2 says check, not guess — the last plan's tray API guess cost a full
fix round), and whether folding the sync window in breaks the Android QR pairing flow, which is
only truly provable with a phone (Task 1 Step 4).

**Explicitly not built:** a whitelist country mode, `Start at login`, online search, and any
further attempt to make the panel appear over fullscreen — the spec establishes why that last one
is not achievable.
