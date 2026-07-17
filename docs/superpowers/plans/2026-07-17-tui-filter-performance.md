# TUI Filter/Search Performance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make TUI filtering/search instant and non-blocking — the list stops jumping, `q` exits immediately, and a slow network can never hang the app.

**Architecture:** Keep the existing threads + channels model (no async). Coalesce queued searches so only the newest runs; return the local (cache) result first and fetch online in the background with a short timeout; decouple quit from the worker so it never waits.

**Tech Stack:** Rust, `std::sync::mpsc`, `std::thread`, `reqwest::blocking`, ratatui.

## Global Constraints

- No comments in code unless the step shows one.
- Code output (logs/messages) English, lowercase.
- No `else if`.
- No AI/Claude/personal mentions anywhere.
- Commit to `dev`; commit subjects are the public changelog — write them for users.
- Version is CI-owned; never hand-edit version fields.
- CI gate is `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` + tests — run all three locally before considering a task done. `single_match` and dead code are ERRORS under `-D warnings`; use guard-`if`, delete unused code.
- Test on the built TUI before claiming done; never touch the user's real data dir.

---

## File Structure

- `crates/radio-tui/src/tui/worker.rs` — coalesce helper + request loop; local-first in `search_all`; bounded online timeout.
- `crates/radio-tui/src/tui/mod.rs` — quit path stops joining the worker.

No new files. `WorkerReq` has no `Clone`/`PartialEq`, so coalescing moves owned values through a `Vec` and detects searches with `matches!`.

---

## Task 1: Coalesce helper (pure, unit-tested)

**Files:**
- Modify: `crates/radio-tui/src/tui/worker.rs`
- Test: same file's `mod tests`

**Interfaces:**
- Produces: `fn coalesce(pending: Vec<WorkerReq>) -> (Vec<WorkerReq>, Option<WorkerReq>)` — splits a drained batch into (non-search reqs in original order, last search if any). The returned `Option` is always a `WorkerReq::Search`.

- [ ] **Step 1: Write the failing test**

Add to `worker.rs` `mod tests`. `WorkerReq` isn't `PartialEq`, so assert via `matches!` and by inspecting the search's query.

```rust
    fn search_req(name: &str) -> WorkerReq {
        WorkerReq::Search(
            SearchQuery { name: Some(name.into()), countrycode: None, language: None, tag: None, codec: None, bitrate_min: None },
            crate::tui::model::BrowseFilters::default(),
        )
    }

    #[test]
    fn coalesce_keeps_only_last_search_and_preserves_other_reqs() {
        let batch = vec![
            search_req("a"),
            WorkerReq::SaveState,
            search_req("b"),
            WorkerReq::LoadFacets,
            search_req("c"),
        ];
        let (others, last) = coalesce(batch);
        // non-search reqs kept in order
        assert!(matches!(others.as_slice(), [WorkerReq::SaveState, WorkerReq::LoadFacets]));
        // only the last search survives
        match last {
            Some(WorkerReq::Search(q, _)) => assert_eq!(q.name.as_deref(), Some("c")),
            _ => panic!("expected last search 'c'"),
        }
    }

    #[test]
    fn coalesce_no_search_returns_all_others_and_none() {
        let (others, last) = coalesce(vec![WorkerReq::SaveState, WorkerReq::LoadFacets]);
        assert_eq!(others.len(), 2);
        assert!(last.is_none());
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p radio-tui coalesce_ 2>&1 | tail -15`
Expected: FAIL — `cannot find function coalesce`.

- [ ] **Step 3: Implement `coalesce`**

Add near the other free functions in `worker.rs` (not inside `mod tests`):

```rust
fn coalesce(pending: Vec<WorkerReq>) -> (Vec<WorkerReq>, Option<WorkerReq>) {
    let mut others = Vec::new();
    let mut last_search = None;
    for req in pending {
        match req {
            WorkerReq::Search(..) => last_search = Some(req),
            other => others.push(other),
        }
    }
    (others, last_search)
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p radio-tui coalesce_ 2>&1 | tail -15`
Expected: PASS (both tests).

- [ ] **Step 5: Commit**

```bash
git add crates/radio-tui/src/tui/worker.rs
git commit -m "feat(tui): coalesce helper to keep only the newest queued search"
```

---

## Task 2: Drain-and-coalesce the worker request loop

**Files:**
- Modify: `crates/radio-tui/src/tui/worker.rs` (the `for req in req_rx` loop, ~line 69)

**Interfaces:**
- Consumes: `coalesce` (Task 1), the existing `handle(...)` match arms.

- [ ] **Step 1: Extract the per-request match into a function**

Currently the loop body is `for req in req_rx { match req { ...all arms... } }`. Wrap the match in a helper so it can be called for each drained request. Rename the loop body:

```rust
fn handle_req(
    req: WorkerReq,
    catalog: &mut Catalog,
    paths: &WorkerPaths,
    msg_tx: &Sender<Msg>,
) -> bool {
    // returns true if this request means "shut down"
    match req {
        WorkerReq::Shutdown => return true,
        // ... move EVERY existing arm here unchanged, except Shutdown ...
    }
    false
}
```

Move all existing match arms from the loop into `handle_req` verbatim. The arms currently reference `catalog`, `paths`, `msg_tx` — those become the params. Keep the trailing `save_all(&catalog, &paths);` that runs after the loop (see Step 3).

- [ ] **Step 2: Replace the loop with recv + drain + coalesce**

```rust
    std::thread::spawn(move || {
        while let Ok(first) = req_rx.recv() {
            let mut batch = vec![first];
            while let Ok(more) = req_rx.try_recv() {
                batch.push(more);
            }
            let (others, last_search) = coalesce(batch);
            let mut shutdown = false;
            for req in others {
                if handle_req(req, &mut catalog, &paths, &msg_tx) {
                    shutdown = true;
                    break;
                }
            }
            if shutdown {
                break;
            }
            if let Some(search) = last_search {
                handle_req(search, &mut catalog, &paths, &msg_tx);
            }
        }
        save_all(&catalog, &paths);
    })
```

Note ordering: non-search reqs (toggles, saves, sync) run first in arrival order, then the single newest search runs last — so the visible result reflects the latest state.

- [ ] **Step 3: Build and run existing tests**

Run: `cargo build -p radio-tui 2>&1 | tail -10 && cargo test -p radio-tui 2>&1 | tail -6`
Expected: builds; all tests pass. Fix any borrow errors (the arms now take `&mut catalog` / `&paths` / `&msg_tx` as params — adjust `&`/`&mut` at call sites inside `handle_req` to match what each arm already used).

- [ ] **Step 4: Commit**

```bash
git add crates/radio-tui/src/tui/worker.rs
git commit -m "fix(tui): drain and coalesce queued searches so rapid filtering runs once, not once per keypress"
```

---

## Task 3: Local-first search with a bounded online timeout

**Files:**
- Modify: `crates/radio-tui/src/tui/worker.rs` (`search_all`, `online_search`)
- Test: same file's `mod tests`

**Interfaces:**
- Consumes: `catalog.search_offline_filtered`, `api::resolve()`.
- Produces: `search_all` sends a local `SearchResults` first, then (for text queries) attempts online with a short timeout and sends an updated `SearchResults` if it succeeds.

Context (verified in `radio-core/src/catalog/api.rs`): `RadioBrowser` is
`{ base_url: String, client: reqwest::blocking::Client }`. Both constructors
(`with_base_url`, `with_mirror_ip`) hardcode `.timeout(180s)`, and `resolve()`
(line 85) picks a live mirror then calls one of them. The 180s request timeout is
what hangs the worker. We add a timeout-parameterised resolver that mirrors
`resolve()` but threads a caller-set timeout into the client.

- [ ] **Step 1: Add timeout-parameterised constructors + resolver in radio-core**

In `crates/radio-core/src/catalog/api.rs`, add timeout-taking variants that reuse
the existing bodies verbatim except for the `.timeout(...)` value, and a resolver
that uses them. Keep the existing `with_base_url` / `with_mirror_ip` / `resolve`
unchanged (CLI paths still want the long timeout). Add:

```rust
    pub fn with_base_url_timeout(base_url: impl Into<String>, secs: u64) -> Self {
        let client = reqwest::blocking::Client::builder()
            .user_agent("world-radio/1.1")
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(secs))
            .build()
            .expect("client build");
        Self { base_url: base_url.into(), client }
    }

    pub fn with_mirror_ip_timeout(ip: IpAddr, secs: u64) -> Self {
        let client = reqwest::blocking::Client::builder()
            .user_agent("world-radio/1.1")
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(secs))
            .resolve(MIRROR_HOST, SocketAddr::new(ip, 443))
            .build()
            .expect("client build");
        Self { base_url: FALLBACK_BASE.to_string(), client }
    }
```

And a free fn beside `resolve()`:

```rust
pub fn resolve_with_timeout(secs: u64) -> RadioBrowser {
    let probe = std::time::Duration::from_secs(3);
    let ips = mirror_ips();
    match pick_alive_ip(&ips, |ip| is_mirror_alive(*ip, probe)) {
        Some(ip) => RadioBrowser::with_mirror_ip_timeout(ip, secs),
        None => RadioBrowser::with_base_url_timeout(FALLBACK_BASE, secs),
    }
}
```

Read `api.rs` first and match the exact field names / constants (`MIRROR_HOST`,
`FALLBACK_BASE`, `mirror_ips`, `pick_alive_ip`, `is_mirror_alive` all exist).

- [ ] **Step 2: No new unit test for local-first (documented reason)**

The worker `mod tests` module has no `Catalog` fixture — it only builds bare
`Station` values (`station()` helper). Building an on-disk `Catalog` (temp db +
ingest) just to assert `search_local` returns `SearchResults` would test
`search_offline_filtered` (already covered in `radio-core`), not the scheduling
change. So this task adds **no** local-first unit test; local-first is verified
live in Task 5 (unplug network mid-text-search → local results stay). The
coalesce unit tests (Task 1) already guard the queue behaviour. Do not fabricate
a catalog-backed test here.

- [ ] **Step 3: Split `search_all` into local + online-augment**

Refactor so the local result is produced and sent first, then online runs:

```rust
fn search_local(catalog: &Catalog, q: &SearchQuery) -> (Msg, bool) {
    let msg = match catalog.search_offline_filtered(q) {
        Ok(stations) => Msg::SearchResults(rows_from(catalog, &stations)),
        Err(e) => Msg::SearchFailed(e.to_string()),
    };
    (msg, false)
}
```

In `handle_search` (the `StatusFilter::All` arm), for a text query send the local result immediately, then attempt online and send an updated result if it returns:

```rust
    // local first — instant, never blocks
    let (local, _) = search_local(catalog, q);
    let _ = msg_tx.send(drop_unplayable(narrow_msg(local, filters), filters.hide_unplayable));
    if should_search_online(q) {
        match online_search_bounded(catalog, q) {
            Ok(rows) => {
                let _ = msg_tx.send(drop_unplayable(narrow_msg(Msg::SearchResults(rows), filters), filters.hide_unplayable));
            }
            Err(e) => crate::log_warn!("worker: online search failed ({e}), keeping local results"),
        }
    }
```

Where `online_search_bounded` is a copy of `online_search` but calls
`api::resolve_with_timeout(4)` instead of `api::resolve()`:

```rust
fn online_search_bounded(catalog: &Catalog, q: &SearchQuery) -> anyhow::Result<Vec<StationRow>> {
    let rb = api::resolve_with_timeout(4);
    let stations = rb.search(q)?;
    catalog.ingest(&stations)?;
    let filtered = catalog.search_offline_filtered(q)?;
    Ok(rows_from(catalog, &filtered))
}
```

Read the current `handle_search` and preserve its exact `SetOffline` / message
flow for the non-`All` status arms; only the `All` arm changes to local-first.
The old `search_all` / `online_search` may become unused — delete them if so
(dead code fails `-D warnings`).

- [ ] **Step 4: Run tests + build**

Run: `cargo build -p radio-tui 2>&1 | tail -8 && cargo test -p radio-tui 2>&1 | tail -6`
Expected: builds; tests pass. Remove `search_all` if it's now unused (dead code fails `-D warnings`), or keep it only if still referenced.

- [ ] **Step 5: Commit**

```bash
git add crates/radio-tui/src/tui/worker.rs crates/radio-core/src/catalog/api.rs
git commit -m "fix(tui): show local results instantly and fetch online in the background with a 4s cap, so a slow network never hangs search"
```

---

## Task 4: Instant exit — quit stops waiting on the worker

**Files:**
- Modify: `crates/radio-tui/src/tui/mod.rs` (quit path, ~lines 200-206)

**Interfaces:**
- Consumes: existing `restore_terminal`, `out_cfg.save`, per-action `save_all` in the worker (already runs on every mutation).

- [ ] **Step 1: Read the current quit path**

Read `crates/radio-tui/src/tui/mod.rs` around lines 190-210. It currently: saves config, restores terminal, sends `SaveState` + `Shutdown`, then `worker_handle.join()`. The join is what blocks on a busy/hung worker.

- [ ] **Step 2: Drop the join wait**

Config save + terminal restore stay. Remove the `worker_handle.join()` wait so quit returns immediately. State is already persisted per-action (`save_all` after each mutating `WorkerReq`), so no data is lost. Concretely, replace:

```rust
    let restore_result = restore_terminal(&mut terminal);
    let _ = req_tx.send(WorkerReq::SaveState);
    let _ = req_tx.send(WorkerReq::Shutdown);
    if let Err(e) = worker_handle.join() {
        eprintln!("worker thread panicked: {e:?}");
    }
    loop_result.and(restore_result)
```

with:

```rust
    let restore_result = restore_terminal(&mut terminal);
    // state is saved on every mutation, so exit does not wait for the worker;
    // signal shutdown best-effort and return immediately.
    let _ = req_tx.send(WorkerReq::Shutdown);
    drop(worker_handle);
    loop_result.and(restore_result)
```

If `worker_handle` is later unused and clippy warns, prefix binding with `_` at its declaration or keep the `drop(worker_handle)` as shown (drop consumes it — no warning).

- [ ] **Step 3: Build + tests + gate**

Run: `cargo build -p radio-tui 2>&1 | tail -8 && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3 && cargo test -p radio-tui 2>&1 | tail -6`
Expected: builds clean (no warnings), tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/radio-tui/src/tui/mod.rs
git commit -m "fix(tui): q exits immediately instead of waiting for background work to finish"
```

---

## Task 5: Live verification on the built TUI

**Files:** none (verification only).

- [ ] **Step 1: Build release**

Run: `cargo build --release -p radio-tui 2>&1 | tail -3`
Expected: Finished.

- [ ] **Step 2: Drive it (tmux, isolated socket)**

Use a throwaway tmux socket so the host session is untouched:

```bash
tmux -L r4perf new-session -d -s t -x 210 -y 55
tmux -L r4perf send-keys -t t "./target/release/r4dio" Enter
sleep 10
# rapid-fire filtering: focus filters, country group, hammer Enter
tmux -L r4perf send-keys -t t Tab; tmux -L r4perf send-keys -t t Right
tmux -L r4perf send-keys -t t Down; tmux -L r4perf send-keys -t t Down
for i in 1 2 3 4 5; do tmux -L r4perf send-keys -t t Enter; done
sleep 3
tmux -L r4perf capture-pane -t t -p | grep -iE "results|connecting" | head -3
# quit must be instant
tmux -L r4perf send-keys -t t "q"; sleep 2
tmux -L r4perf has-session -t t 2>/dev/null && echo "Q HUNG" || echo "q exited cleanly"
tmux -L r4perf kill-server 2>/dev/null
```

Expected: after rapid Enter, the list settles to a single result with no lingering `connecting…`/`0 results`; `q exited cleanly`.

- [ ] **Step 3: Report**

Confirm: filtering settles once (coalesce), no flicker, `q` instant. Note any residual jump or delay. Then STOP for the user to decide on release.

---

## Deploy (manual, AFTER user approval — NOT part of task execution)

PR `dev`→`main`, admin-merge; CI bumps version, tags, builds CLI+Android, auto-deploys. This batch also carries the earlier uncommitted TUI work already on `dev` (favourites cursor, three-state country Enter, local filtering, x removal).
