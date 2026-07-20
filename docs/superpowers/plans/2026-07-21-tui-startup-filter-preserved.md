# TUI Startup Filter Preserved Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep a saved search query + status filter applied on startup, instead of the background catalogue sync overwriting the list with the raw popular seed.

**Architecture:** The worker's `handle_sync_catalog` / `handle_quick_top` push `Msg::SearchResults(seed_rows_by_popularity(...))` — raw rows that ignore the active query+filters and wipe them a beat after startup. Fix: stop pushing raw rows; the UI (which owns the filter) re-issues a proper `Search` when it receives the already-sent `CatalogSynced` / `QuickTopReady` signal. Delete the now-dead seed helper.

**Tech Stack:** Rust, TEA-style TUI (radio-tui crate), worker thread + Msg/Effect.

## Global Constraints

- No comments in code unless a step shows one; lowercase logs; no AI/personal mentions.
- Commit to `dev`; commit subjects are the public changelog — write them for users.
- Version is CI-owned; never hand-edit versions.
- No `else if`.
- CI runs `cargo fmt --check` and `clippy --workspace --all-targets -- -D warnings` — a dead function is an ERROR there. Run both locally before pushing.
- TDD: add/adjust unit tests in update.rs before the code where the plan shows tests.

## File Structure

- `crates/radio-tui/src/tui/worker.rs` — drop the two raw-seed sends; delete `seed_rows_by_popularity`; fix `handle_quick_top`'s `count`.
- `crates/radio-tui/src/tui/update.rs` — `CatalogSynced` and `QuickTopReady` handlers emit `Effect::Search(current query, filters)`; add unit tests.

Two files. No new files.

---

## Task 1: UI re-issues Search after catalogue population

**Files:**
- Modify: `crates/radio-tui/src/tui/update.rs`
  - `Msg::CatalogSynced` handler (~line 115-120)
  - `Msg::QuickTopReady` handler (~line 125-131)
  - tests section (~line 1540-1600)

**Interfaces:**
- Consumes: `model.browse.filters.to_query(&model.browse.query) -> SearchQuery`; `model.browse.filters: BrowseFilters`; `Effect::Search(SearchQuery, BrowseFilters)`; existing `autoplay_random_if_pending(model) -> Vec<Effect>`.
- Produces: both handlers return their autoplay effects PLUS an `Effect::Search` carrying the model's current query+filters.

- [ ] **Step 1: Write failing tests**

In the update.rs tests module, add (adapt to the file's existing test helpers/imports — there are already tests around line 1540-1600 that call `update(&mut m, Msg::CatalogSynced { count })` and inspect effects):

```rust
    #[test]
    fn catalog_synced_reissues_search_with_current_filter() {
        let mut m = Model::new(Theme::default(), ColorTier::Truecolor, Glyphs::ascii());
        m.browse.query = "club".to_string();
        m.browse.filters.status = StatusFilter::Favorites;
        let effects = update(&mut m, Msg::CatalogSynced { count: 10 });
        assert!(
            effects.iter().any(|e| matches!(
                e,
                Effect::Search(q, f)
                    if q.name.as_deref() == Some("club") && f.status == StatusFilter::Favorites
            )),
            "CatalogSynced must re-issue Search with the current query+filter"
        );
    }

    #[test]
    fn quick_top_ready_reissues_search_with_current_filter() {
        let mut m = Model::new(Theme::default(), ColorTier::Truecolor, Glyphs::ascii());
        m.browse.query = "club".to_string();
        m.browse.filters.status = StatusFilter::Favorites;
        let effects = update(&mut m, Msg::QuickTopReady { count: 5 });
        assert!(
            effects.iter().any(|e| matches!(
                e,
                Effect::Search(q, f)
                    if q.name.as_deref() == Some("club") && f.status == StatusFilter::Favorites
            )),
            "QuickTopReady must re-issue Search with the current query+filter"
        );
    }
```

(Match `Model::new`'s real signature and the test module's existing imports for
`Theme`, `ColorTier`, `Glyphs`, `StatusFilter`, `Effect` — copy how the neighbouring
tests construct a `Model` and reference these types. If a helper like
`test_model()` already exists in that module, use it instead of `Model::new`.)

- [ ] **Step 2: Run the tests, confirm they fail**

Run: `cargo test -p radio-tui catalog_synced_reissues_search quick_top_ready_reissues_search 2>&1 | tail -20`
Expected: FAIL (handlers don't emit `Effect::Search` yet).

- [ ] **Step 3: Emit Effect::Search in both handlers**

Current `CatalogSynced` handler:

```rust
        Msg::CatalogSynced { count } => {
            model.catalog_count = Some(count);
            model.catalog_loading = false;
            model.browse.pending_online_search = Some(Instant::now());
            autoplay_random_if_pending(model)
        }
```

Change to (append a re-Search to the autoplay effects):

```rust
        Msg::CatalogSynced { count } => {
            model.catalog_count = Some(count);
            model.catalog_loading = false;
            model.browse.pending_online_search = Some(Instant::now());
            let mut effects = autoplay_random_if_pending(model);
            let q = model.browse.filters.to_query(&model.browse.query);
            effects.push(Effect::Search(q, model.browse.filters.clone()));
            effects
        }
```

Current `QuickTopReady` handler:

```rust
        Msg::QuickTopReady { count } => {
            if model.catalog_count.is_none() {
                model.catalog_count = Some(count);
            }
            model.catalog_loading = false;
            autoplay_random_if_pending(model)
        }
```

Change to:

```rust
        Msg::QuickTopReady { count } => {
            if model.catalog_count.is_none() {
                model.catalog_count = Some(count);
            }
            model.catalog_loading = false;
            let mut effects = autoplay_random_if_pending(model);
            let q = model.browse.filters.to_query(&model.browse.query);
            effects.push(Effect::Search(q, model.browse.filters.clone()));
            effects
        }
```

- [ ] **Step 4: Run the new tests, confirm they pass**

Run: `cargo test -p radio-tui catalog_synced_reissues_search quick_top_ready_reissues_search 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Run the whole radio-tui test suite (no regressions)**

Run: `cargo test -p radio-tui 2>&1 | tail -20`
Expected: all pass. If a pre-existing test asserted `CatalogSynced`/`QuickTopReady` produced NO effects or a specific effect list, update it to allow the added `Effect::Search` (the new behaviour is correct); note any such change in the report.

- [ ] **Step 6: Commit**

```bash
git add crates/radio-tui/src/tui/update.rs
git commit -m "fix(tui): keep your saved search and filter applied after the catalog refreshes on startup"
```

---

## Task 2: Worker stops pushing the raw seed

**Files:**
- Modify: `crates/radio-tui/src/tui/worker.rs`
  - `handle_sync_catalog` (~line 469-491)
  - `handle_quick_top` (~line 493-512)
  - delete `seed_rows_by_popularity` (~line 458)

**Interfaces:**
- Consumes: nothing new.
- Produces: `handle_sync_catalog` sends only `CatalogSynced { count }`; `handle_quick_top` sends only `QuickTopReady { count }`; no `Msg::SearchResults(seed)` from either.

- [ ] **Step 1: Drop the raw-seed send in handle_sync_catalog**

Current (inside the `Ok(count)` arm):

```rust
            Ok(count) => {
                let _ = catalog.set_last_sync(now_secs());
                let rows = seed_rows_by_popularity(catalog);
                if !rows.is_empty() {
                    let _ = msg_tx.send(Msg::SearchResults(rows));
                }
                let _ = msg_tx.send(Msg::CatalogSynced { count });
            }
```

Change to:

```rust
            Ok(count) => {
                let _ = catalog.set_last_sync(now_secs());
                let _ = msg_tx.send(Msg::CatalogSynced { count });
            }
```

- [ ] **Step 2: Drop the raw-seed send in handle_quick_top and fix count**

Current:

```rust
        Ok(stations) => {
            if let Err(e) = catalog.ingest(&stations) {
                crate::log_warn!("worker: quick-top ingest failed: {e}");
                return;
            }
            let rows = seed_rows_by_popularity(catalog);
            let count = rows.len();
            if !rows.is_empty() {
                let _ = msg_tx.send(Msg::SearchResults(rows));
            }
            let _ = msg_tx.send(Msg::QuickTopReady { count });
        }
```

Change to (count from the ingested stations, no seed built):

```rust
        Ok(stations) => {
            if let Err(e) = catalog.ingest(&stations) {
                crate::log_warn!("worker: quick-top ingest failed: {e}");
                return;
            }
            let count = stations.len();
            let _ = msg_tx.send(Msg::QuickTopReady { count });
        }
```

- [ ] **Step 3: Delete the dead seed_rows_by_popularity function**

Remove the whole `fn seed_rows_by_popularity(catalog: &Catalog) -> Vec<StationRow> { … }`
(around line 458). After Steps 1-2 nothing calls it; leaving it fails
`clippy -D warnings`. If deleting it makes an import (e.g. `StationRow` or a
popularity helper) unused, remove that import too — the compiler/clippy will name it.

- [ ] **Step 4: fmt + clippy + build**

Run: `cargo fmt -p radio-tui && cargo clippy -p radio-tui --all-targets -- -D warnings 2>&1 | tail -20`
Expected: no warnings/errors (this is the CI gate for this crate).

- [ ] **Step 5: Full workspace tests**

Run: `cargo test -p radio-tui 2>&1 | tail -15`
Expected: all pass. (If a worker test referenced `seed_rows_by_popularity` or the
raw-seed send, update it; note in the report.)

- [ ] **Step 6: Commit**

```bash
git add crates/radio-tui/src/tui/worker.rs
git commit -m "fix(tui): stop the catalog refresh from replacing your filtered list with the popular list"
```

---

## Task 3: Live verification

**Files:** none.

- [ ] **Step 1: Build release**

Run: `cargo build --release -p radio-tui 2>&1 | tail -5`
Expected: BUILD OK.

- [ ] **Step 2: Reproduce the original bug is GONE (saved filter persists)**

The TUI reads its saved query+filters from config. Set up a config with a query and
Favorites status (or, if easier, start the TUI, type a query + switch to Favorites,
quit so it saves, then restart). On restart:
- The list is filtered immediately AND stays filtered after the background catalogue
  sync finishes (watch for a few seconds — previously it reverted to the full popular
  list a beat after start).

Drive it in a tmux/pty session per the project's TUI-run pattern; capture the list
before and ~5 s after startup to confirm it does not revert. If favourites are empty,
use a plain text query (e.g. "club") with status All to see the filter persist.

- [ ] **Step 3: Plain start still shows the popular list**

Start with an empty query + status All → the list shows the popular stations (the
re-Search with an empty filter returns the same popular content). Confirm non-empty.

- [ ] **Step 4: Report and STOP**

Confirm: saved query+filter persists through the startup catalogue sync; plain start
still populated; no panic; fmt+clippy+tests green. Then STOP for the user to decide
on release.

---

## Deploy (manual, AFTER user approval — NOT part of task execution)

Separate CLI concern from the Android bundle. PR `dev`→`main`, admin-merge; CI bumps
version, tags, builds CLI+APK, auto-deploys. (May ship together with the Android
widget work in one release if the user chooses.)
