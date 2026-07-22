# TUI Popular Seed On Empty Filter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the regression where an empty-filter startup ends on an alphabetical list — restore the popular (votes-DESC) order for the no-filter case, while keeping the saved-filter fix intact.

**Architecture:** `CatalogSynced`/`QuickTopReady` currently always re-issue `Effect::Search`. For an empty filter that yields `ORDER BY name` (alphabetical). Fix: gate — emit `Effect::Search` only when a filter is active; otherwise emit a new `Effect::PopularSeed` that a new `WorkerReq::PopularSeed` serves via `catalog.list_by_popularity`, restoring popular order. UI-gated so it never blindly overwrites a filtered list.

**Tech Stack:** Rust, radio-tui (TEA: Msg/Effect/WorkerReq), radio-core catalog.

## Global Constraints

- No comments in code unless a step shows one; lowercase logs; no AI/personal mentions.
- Commit to `dev`; subjects are the public changelog — write for users.
- Version CI-owned; never hand-edit.
- No `else if` — use `match` or guard `if`.
- CI: `cargo fmt --check` + `clippy --workspace --all-targets -- -D warnings`. Run both locally.
- TDD: unit tests in update.rs before the handler change.

## File Structure

- `crates/radio-tui/src/tui/message.rs` — add `Effect::PopularSeed`.
- `crates/radio-tui/src/tui/mod.rs` — map `Effect::PopularSeed → WorkerReq::PopularSeed` in `run_effects`.
- `crates/radio-tui/src/tui/worker.rs` — add `WorkerReq::PopularSeed` + `handle_popular_seed`.
- `crates/radio-tui/src/tui/update.rs` — gate in `CatalogSynced`/`QuickTopReady`; tests.

---

## Task 1: Worker serves a popular seed on demand

**Files:**
- Modify: `crates/radio-tui/src/tui/worker.rs` (`WorkerReq` enum ~line 8-31; `handle_req` ~line 100-149; new fn near `handle_quick_top`)
- Modify: `crates/radio-tui/src/tui/message.rs` (`Effect` enum ~line 74)
- Modify: `crates/radio-tui/src/tui/mod.rs` (`run_effects` match ~line 334)

**Interfaces:**
- Produces: `WorkerReq::PopularSeed` and `Effect::PopularSeed`; a worker handler that sends `Msg::SearchResults(popular_rows)`.
- Consumes: `catalog.list_by_popularity(&[String], usize) -> anyhow::Result<Vec<Station>>`, `catalog.favorite_ids() -> &[String]`, `catalog.is_favorite(&str)`, `catalog.is_hidden(&str)`, `station_to_row(&Station, bool, bool) -> StationRow`.

- [ ] **Step 1: Add Effect::PopularSeed**

In `message.rs`, in the `Effect` enum (after `LoadFacets`), add:

```rust
    PopularSeed,
```

- [ ] **Step 2: Map the effect to a worker request**

In `mod.rs` `run_effects`, after the `Effect::LoadFacets` arm, add:

```rust
            Effect::PopularSeed => {
                let _ = req_tx.send(WorkerReq::PopularSeed);
            }
```

- [ ] **Step 3: Add WorkerReq::PopularSeed + dispatch**

In `worker.rs`, add to the `WorkerReq` enum (after `QuickTop`):

```rust
    PopularSeed,
```

In `handle_req`, after the `WorkerReq::QuickTop` arm, add:

```rust
        WorkerReq::PopularSeed => handle_popular_seed(catalog, msg_tx),
```

- [ ] **Step 4: Implement handle_popular_seed**

Add near `handle_quick_top` (use the same mapping mod.rs uses for its startup seed):

```rust
fn handle_popular_seed(catalog: &Catalog, msg_tx: &Sender<Msg>) {
    let fav_ids = catalog.favorite_ids().to_vec();
    match catalog.list_by_popularity(&fav_ids, 200) {
        Ok(stations) => {
            let rows: Vec<StationRow> = stations
                .iter()
                .map(|s| {
                    let uuid = &s.stationuuid;
                    station_to_row(s, catalog.is_favorite(uuid), catalog.is_hidden(uuid))
                })
                .collect();
            if !rows.is_empty() {
                let _ = msg_tx.send(Msg::SearchResults(rows));
            }
        }
        Err(e) => {
            crate::log_warn!("worker: popular seed failed: {e}");
        }
    }
}
```

(Confirm `StationRow` is in scope in worker.rs — it is used by `station_to_row`'s
return type already; no new import expected. If clippy flags a missing import, add it.)

- [ ] **Step 5: fmt + clippy + build**

Run: `cd /Users/vchub/dev/projects/world-radio/radio && cargo fmt -p radio-tui && cargo clippy -p radio-tui --all-targets -- -D warnings 2>&1 | tail -20`
Expected: clean.

- [ ] **Step 6: Tests compile/pass**

Run: `cd /Users/vchub/dev/projects/world-radio/radio && cargo test -p radio-tui 2>&1 | tail -12`
Expected: all pass (177 + no new failures).

- [ ] **Step 7: Commit**

```bash
git add crates/radio-tui/src/tui/message.rs crates/radio-tui/src/tui/mod.rs crates/radio-tui/src/tui/worker.rs
git commit -m "feat(tui): popular-seed request to restore the popular list on demand"
```

---

## Task 2: Gate the re-Search — popular seed when no filter is active

**Files:**
- Modify: `crates/radio-tui/src/tui/update.rs` (`CatalogSynced` ~line 115, `QuickTopReady` ~line 125; tests ~line 1600+)

**Interfaces:**
- Consumes: `Effect::PopularSeed` (Task 1); `model.browse.query: String`; `model.browse.filters: BrowseFilters` with `.status`, `.is_empty()`; `StatusFilter::All`.
- Produces: both handlers emit `Effect::Search` when a filter is active, else `Effect::PopularSeed`.

- [ ] **Step 1: Write failing tests**

In update.rs tests (mirror the existing helper `Model::new(Theme::AmberCrt, ColorTier::Truecolor, Glyphs::unicode())` and the two tests added earlier `catalog_synced_reissues_search_with_current_filter` etc.):

```rust
    #[test]
    fn catalog_synced_empty_filter_uses_popular_seed() {
        let mut m = Model::new(Theme::AmberCrt, ColorTier::Truecolor, Glyphs::unicode());
        m.browse.query = String::new();
        m.browse.filters.status = StatusFilter::All;
        let effects = update(&mut m, Msg::CatalogSynced { count: 10 });
        assert!(
            effects.iter().any(|e| matches!(e, Effect::PopularSeed)),
            "empty filter must restore the popular seed, not an alphabetical Search"
        );
        assert!(
            !effects.iter().any(|e| matches!(e, Effect::Search(_, _))),
            "empty filter must NOT emit a Search"
        );
    }

    #[test]
    fn catalog_synced_status_favorites_still_searches() {
        let mut m = Model::new(Theme::AmberCrt, ColorTier::Truecolor, Glyphs::unicode());
        m.browse.query = String::new();
        m.browse.filters.status = StatusFilter::Favorites;
        let effects = update(&mut m, Msg::CatalogSynced { count: 10 });
        assert!(
            effects.iter().any(|e| matches!(e, Effect::Search(_, f) if f.status == StatusFilter::Favorites)),
            "an active status filter must still re-Search"
        );
    }

    #[test]
    fn quick_top_ready_empty_filter_uses_popular_seed() {
        let mut m = Model::new(Theme::AmberCrt, ColorTier::Truecolor, Glyphs::unicode());
        m.browse.query = String::new();
        m.browse.filters.status = StatusFilter::All;
        let effects = update(&mut m, Msg::QuickTopReady { count: 5 });
        assert!(
            effects.iter().any(|e| matches!(e, Effect::PopularSeed)),
            "empty filter must restore the popular seed on quick-top too"
        );
    }
```

(Keep the two earlier tests `catalog_synced_reissues_search_with_current_filter` /
`quick_top_ready_reissues_search_with_current_filter` — with a non-empty query they
must still assert an `Effect::Search`. They remain valid since "club" makes the
filter active.)

- [ ] **Step 2: Run, confirm they fail**

Run: `cd /Users/vchub/dev/projects/world-radio/radio && cargo test -p radio-tui empty_filter_uses_popular_seed status_favorites_still_searches 2>&1 | tail -20`
Expected: FAIL (handlers always emit Search today).

- [ ] **Step 3: Gate both handlers**

Current `CatalogSynced` handler (after the earlier fix):

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

Replace the push with a gated choice:

```rust
        Msg::CatalogSynced { count } => {
            model.catalog_count = Some(count);
            model.catalog_loading = false;
            model.browse.pending_online_search = Some(Instant::now());
            let mut effects = autoplay_random_if_pending(model);
            effects.push(catalog_refresh_effect(&model.browse));
            effects
        }
```

Do the same for `QuickTopReady`:

```rust
        Msg::QuickTopReady { count } => {
            if model.catalog_count.is_none() {
                model.catalog_count = Some(count);
            }
            model.catalog_loading = false;
            let mut effects = autoplay_random_if_pending(model);
            effects.push(catalog_refresh_effect(&model.browse));
            effects
        }
```

Add the shared helper (near the other free fns in update.rs; `BrowseState` /
`StatusFilter` are already in scope in this module):

```rust
fn catalog_refresh_effect(browse: &crate::tui::model::BrowseState) -> Effect {
    let filter_active = !browse.query.trim().is_empty()
        || browse.filters.status != crate::tui::model::StatusFilter::All
        || !browse.filters.is_empty();
    match filter_active {
        true => Effect::Search(browse.filters.to_query(&browse.query), browse.filters.clone()),
        false => Effect::PopularSeed,
    }
}
```

(Use the module's existing import paths for `BrowseState`/`StatusFilter`/`Effect` —
if they are already imported via `use` at the top of update.rs, drop the
`crate::tui::model::` prefixes to match the file's style. Match how the neighbouring
code refers to `StatusFilter` and `Effect`.)

- [ ] **Step 4: Run new + old tests**

Run: `cd /Users/vchub/dev/projects/world-radio/radio && cargo test -p radio-tui 2>&1 | tail -15`
Expected: the 3 new tests pass; the 2 earlier filter tests still pass; full suite green.

- [ ] **Step 5: fmt + clippy**

Run: `cd /Users/vchub/dev/projects/world-radio/radio && cargo fmt -p radio-tui && cargo clippy -p radio-tui --all-targets -- -D warnings 2>&1 | tail -15`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/radio-tui/src/tui/update.rs
git commit -m "fix(tui): keep the popular list on a plain startup, only filter when you actually set one"
```

---

## Task 3: Live verification

**Files:** none.

Use an ISOLATED HOME (`HOME=<sandbox>` so `ProjectDirs` writes under the sandbox —
NEVER touch the real `~/Library/Application Support/net.vchub.r4dio`). Two-stage:
first populate the catalogue with an empty query, quit (saves), then test.

- [ ] **Step 1: Build release**

Run: `cd /Users/vchub/dev/projects/world-radio/radio && cargo build --release -p radio-tui 2>&1 | tail -4`

- [ ] **Step 2: Populate catalogue (stage 1)**

Sandbox: `SB=<scratchpad>/r4dio_seed_home`; `mkdir -p "$SB/Library/Application Support/net.vchub.r4dio"`.
Launch in tmux under `HOME=$SB`, wait ~20 s until the header shows a large result
count (catalogue synced, `stations.db` present). Quit with `q`.

- [ ] **Step 3: Empty-query start → POPULAR order (the regression case)**

Ensure the saved config has `query = ""` and `status = "all"`. Force `should_sync`
by making the catalogue look stale — simplest: delete the `last_sync` marker or set
it far in the past (or just rely on the fresh sync path). Relaunch under `HOME=$SB`,
wait through the background sync (~15-30 s), capture the list. The TOP rows must be
high-popularity stations (e.g. well-known names), NOT alphabetical (no wall of
"0-9"/"A…" obscure stations). Compare the first few rows against a known popular
station set — they should be vote-ordered, matching pre-regression behaviour.

- [ ] **Step 4: Saved-filter start → still filtered (no re-break)**

Set `query = "club"`, relaunch, confirm the list shows the "club"-filtered results
immediately and stays filtered through the background sync (the original fix still
holds).

- [ ] **Step 5: Cleanup + report**

Remove the sandbox HOME. Confirm the real user data dir was never touched (check its
mtime predates the test). Report: empty start = popular order; "club" start = stays
filtered; no panic. Then STOP for release.

---

## Deploy (manual, AFTER user approval — NOT part of task execution)

Ships in ONE release with the Android widget/eyes-free work + the startup-filter
fix. User asked for a MINOR version bump (1.4.x → 1.5.0) — tell CI/PR to bump minor,
not patch. PR `dev`→`main`, admin-merge; CI tags, builds CLI+APK, auto-deploys.
