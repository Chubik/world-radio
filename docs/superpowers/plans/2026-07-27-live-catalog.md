# Live Catalog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the station catalog maintain itself — hide dead stations on the first genuine failure, absorb the server's liveness verdict for free, and refresh in the background during a long session.

**Architecture:** Four independent pieces, in dependency order. First classify playback errors at the audio boundary so a station failure can be told apart from the user's network dropping (Task 1). Then use that classification to hide a station after a single real failure, guarded by the existing `AUTO_SKIP_MAX` streak counter (Task 2). Then parse the API's `lastcheckok` field and hide obviously-dead stations at ingest without the user ever meeting them (Task 3). Finally, add an hourly TTL check to the TUI worker so a long-running session refreshes (Task 4).

**Tech Stack:** Rust (workspace crates `radio-core`, `radio-audio`, `radio-tui`), rusqlite + FTS5 catalog cache, serde, anyhow, cargo test.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-27-live-catalog-design.md`.
- Comments in English, lowercase. Comments ONLY in genuinely non-obvious places — explain "why", never "what". No trivial comments.
- **Do not use `else if` chains.** Use `match` or early returns.
- Logs in English and lowercase.
- Files stay under 600-800 lines; if a file would grow past that, split by responsibility.
- Run `cargo fmt` before every commit; `cargo clippy --workspace --all-targets` must be clean (the repo treats warnings as failures in CI).
- No AI / Claude / Anthropic mention anywhere. No personal data in code or comments.
- Never blind-overwrite an existing file: `Read` it first, then `Edit`.
- **Do NOT touch the Android app.** It has no catalog database and is explicitly out of scope; it gets its own spec later.
- Do NOT add our own stream probing, do NOT change dedup (`ON CONFLICT(stationuuid) DO UPDATE` is already correct), do NOT change the RU/BY ban applied on ingest.
- The user's own experience always outranks the server's verdict: a stale `lastcheckok == 1` must never clear a failure the user recorded.
- TDD: write the failing test first, watch it fail, then implement.

---

### Task 1: Classify playback errors at the audio boundary

`Status::Error(String)` currently carries a stringified `anyhow::Error`, and every consumer throws the payload away (`update.rs:262` matches `Status::Error(_)`). This task adds a machine-readable cause alongside the existing message, so later tasks can tell a dead stream from a dead network.

`Status::Error` is consumed in three other places (`radio-mini/src/state.rs:112`, `radio-tui/src/tui/view/header.rs:134`, plus tests). Adding a **new variant field would break all of them**, so instead add a separate enum and a new `Status` variant that carries it, leaving `Status::Error(String)` untouched and still working.

**Files:**
- Modify: `crates/radio-core/src/audio/command.rs` (add `FailureKind`, add `Status::StreamError`)
- Modify: `crates/radio-audio/src/slot.rs:262-278` (classify before sending)
- Test: `crates/radio-core/src/audio/command.rs` (inline `mod tests`, the file already has one)

**Interfaces:**
- Produces, for Tasks 2 and 3:
  - `pub enum FailureKind { StreamDead, NetworkDown }` in `radio_core::audio::command`, re-exported by `radio_audio`.
  - `Status::StreamError { message: String, kind: FailureKind }` — a NEW variant. `Status::Error(String)` stays exactly as it is.
  - `pub fn classify_failure(err: &anyhow::Error) -> FailureKind`.

- [ ] **Step 1: Write the failing test**

Add to the existing `mod tests` in `crates/radio-core/src/audio/command.rs`:

```rust
    #[test]
    fn classifies_http_status_as_stream_dead() {
        let e = anyhow::anyhow!("HTTP status client error (404 Not Found) for url (http://x/s)");
        assert_eq!(classify_failure(&e), FailureKind::StreamDead);
    }

    #[test]
    fn classifies_connection_refused_as_stream_dead() {
        let e = anyhow::anyhow!("tcp connect error: Connection refused (os error 61)");
        assert_eq!(classify_failure(&e), FailureKind::StreamDead);
    }

    #[test]
    fn classifies_dns_failure_as_stream_dead() {
        let e = anyhow::anyhow!("dns error: failed to lookup address information: nodename nor servname provided");
        assert_eq!(classify_failure(&e), FailureKind::StreamDead);
    }

    #[test]
    fn classifies_unsupported_format_as_stream_dead() {
        let e = anyhow::anyhow!("unsupported codec: core (format) error");
        assert_eq!(classify_failure(&e), FailureKind::StreamDead);
    }

    #[test]
    fn classifies_timeout_as_network_down() {
        let e = anyhow::anyhow!("operation timed out");
        assert_eq!(classify_failure(&e), FailureKind::NetworkDown);
    }

    #[test]
    fn classifies_unreachable_network_as_network_down() {
        let e = anyhow::anyhow!("tcp connect error: Network is unreachable (os error 51)");
        assert_eq!(classify_failure(&e), FailureKind::NetworkDown);
    }

    #[test]
    fn classifies_unknown_error_as_network_down() {
        // unknown causes must default to the safe direction: do not blame the station
        let e = anyhow::anyhow!("something we have never seen");
        assert_eq!(classify_failure(&e), FailureKind::NetworkDown);
    }

    #[test]
    fn stream_error_status_carries_kind() {
        let s = Status::StreamError {
            message: "boom".into(),
            kind: FailureKind::StreamDead,
        };
        assert_eq!(
            s,
            Status::StreamError {
                message: "boom".into(),
                kind: FailureKind::StreamDead,
            }
        );
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd /Users/vchub/dev/projects/world-radio/radio && cargo test -p radio-core --lib audio::command 2>&1 | tail -20`
Expected: FAIL — `cannot find function classify_failure`, `no variant StreamError`, `cannot find type FailureKind`.

- [ ] **Step 3: Implement**

In `crates/radio-core/src/audio/command.rs`, add above the `Status` enum:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// the origin answered and the answer was fatal — blame the station
    StreamDead,
    /// no usable connectivity — blame the network, never the station
    NetworkDown,
}
```

Add a variant to `Status` (leave `Error(String)` in place):

```rust
    StreamError {
        message: String,
        kind: FailureKind,
    },
```

And the classifier:

```rust
/// unknown causes deliberately fall through to NetworkDown: under-hiding is
/// recoverable, mass-hiding live stations is not.
pub fn classify_failure(err: &anyhow::Error) -> FailureKind {
    let text = format!("{err:#}").to_ascii_lowercase();
    const NETWORK: [&str; 4] = [
        "timed out",
        "network is unreachable",
        "no route to host",
        "network is down",
    ];
    const STREAM: [&str; 6] = [
        "http status",
        "connection refused",
        "dns error",
        "connection reset",
        "unsupported",
        "format",
    ];
    if NETWORK.iter().any(|n| text.contains(n)) {
        return FailureKind::NetworkDown;
    }
    match STREAM.iter().any(|s| text.contains(s)) {
        true => FailureKind::StreamDead,
        false => FailureKind::NetworkDown,
    }
}
```

Note the ordering: network markers are checked FIRST, because a timeout can also
contain the word "connect", and the safe direction wins ties.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd /Users/vchub/dev/projects/world-radio/radio && cargo test -p radio-core --lib audio::command 2>&1 | tail -12`
Expected: PASS, 8 new tests green.

- [ ] **Step 5: Send the classified status from the decoder**

In `crates/radio-audio/src/slot.rs`, the block at lines 262-278 currently sends
`Status::Error(e.to_string())`. Read it first, then change ONLY that send to:

```rust
            if !abort_thread.load(Ordering::Relaxed) {
                let kind = radio_core::audio::command::classify_failure(&e);
                let _ = status_tx.send(Status::StreamError {
                    message: e.to_string(),
                    kind,
                });
            }
```

Then make sure `radio_audio` re-exports the new type — in `crates/radio-audio/src/lib.rs:8` the line is
`pub use radio_core::audio::command::{Command, Status};`. Change it to:

```rust
pub use radio_core::audio::command::{classify_failure, Command, FailureKind, Status};
```

- [ ] **Step 6: Handle the new variant everywhere it must compile**

Adding a variant makes non-exhaustive matches fail to build. Fix the three
consumers so they treat `StreamError` exactly like `Error` does today —
behaviour changes in Task 2, not here:

- `crates/radio-mini/src/state.rs:112` — the arm is `Status::Error(_) => Phase::Error,`. Add `Status::StreamError { .. } => Phase::Error,` next to it.
- `crates/radio-tui/src/tui/view/header.rs:134` — the arm renders `✗ ERROR`. Add `Status::StreamError { .. }` to that same arm using `|`.
- `crates/radio-tui/src/tui/update.rs:262` — the arm is `Status::Error(_) => { ... auto_skip(model) }`. Add `Status::StreamError { .. }` to that same arm with `|` for now.

- [ ] **Step 7: Build and test the workspace**

Run: `cd /Users/vchub/dev/projects/world-radio/radio && cargo fmt && cargo clippy --workspace --all-targets 2>&1 | tail -15 && cargo test --workspace 2>&1 | tail -15`
Expected: clippy clean, all tests pass.

- [ ] **Step 8: Commit**

```bash
cd /Users/vchub/dev/projects/world-radio/radio
git add crates/radio-core/src/audio/command.rs crates/radio-audio/src/slot.rs crates/radio-audio/src/lib.rs crates/radio-mini/src/state.rs crates/radio-tui/src/tui/view/header.rs crates/radio-tui/src/tui/update.rs
git commit -m "feat(core): tell a dead stream apart from a dropped connection"
```

---

### Task 2: Hide a station after one real failure, guarded by the streak counter

Today `HIDE_THRESHOLD = 3` and every failure counts, so the user must personally hit the same dead station three times. This task drops the threshold to 1 and makes only `StreamDead` failures count — with the existing `AUTO_SKIP_MAX` streak as a blast-radius guard.

**Files:**
- Modify: `crates/radio-core/src/catalog/health.rs:3` (threshold)
- Modify: `crates/radio-tui/src/tui/update.rs:252-292` (the `auto_skip` path)
- Test: inline `mod tests` in both files

**Interfaces:**
- Consumes from Task 1: `FailureKind::{StreamDead, NetworkDown}`, `Status::StreamError { message, kind }`.
- Produces: nothing new for later tasks. `Catalog::note_play_failure(&mut self, uuid: &str)` keeps its exact signature — the decision of whether to call it moves to the caller.

- [ ] **Step 1: Write the failing test for the threshold**

In `crates/radio-core/src/catalog/health.rs`, add a `mod tests` at the end of the file (the file currently has none):

```rust
#[cfg(test)]
mod tests {
    use super::Health;

    #[test]
    fn one_failure_hides() {
        let mut h = Health::new();
        h.record_failure("u1");
        assert!(h.is_hidden("u1"));
    }

    #[test]
    fn success_clears_a_failure() {
        let mut h = Health::new();
        h.record_failure("u1");
        h.record_success("u1");
        assert!(!h.is_hidden("u1"));
    }

    #[test]
    fn untouched_station_is_not_hidden() {
        let h = Health::new();
        assert!(!h.is_hidden("u1"));
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd /Users/vchub/dev/projects/world-radio/radio && cargo test -p radio-core --lib catalog::health 2>&1 | tail -12`
Expected: FAIL on `one_failure_hides` — one failure gives count 1, threshold is still 3.

- [ ] **Step 3: Drop the threshold**

In `crates/radio-core/src/catalog/health.rs` change line 3:

```rust
const HIDE_THRESHOLD: u32 = 1;
```

- [ ] **Step 4: Run it to verify it passes**

Run: `cd /Users/vchub/dev/projects/world-radio/radio && cargo test -p radio-core --lib catalog::health 2>&1 | tail -12`
Expected: PASS.

- [ ] **Step 5: Write the failing test for the guard**

The TUI test module in `crates/radio-tui/src/tui/update.rs` already has tests around line 1253 that build a model and feed `Msg::AudioStatus(...)`; read them first and follow their exact construction style. Add:

```rust
    #[test]
    fn network_failure_does_not_mark_the_station() {
        let mut m = model_with_one_row("u1");
        let fx = update(
            &mut m,
            Msg::AudioStatus(Status::StreamError {
                message: "operation timed out".into(),
                kind: FailureKind::NetworkDown,
            }),
        );
        assert!(!fx.iter().any(|e| matches!(e, Effect::MarkFailed(_))));
    }

    #[test]
    fn stream_failure_marks_the_station() {
        let mut m = model_with_one_row("u1");
        let fx = update(
            &mut m,
            Msg::AudioStatus(Status::StreamError {
                message: "http status 404".into(),
                kind: FailureKind::StreamDead,
            }),
        );
        assert!(fx.iter().any(|e| matches!(e, Effect::MarkFailed(u) if u == "u1")));
    }

    #[test]
    fn long_failure_streak_stops_marking_stations() {
        let mut m = model_with_one_row("u1");
        m.auto_skip_count = AUTO_SKIP_MAX;
        let fx = update(
            &mut m,
            Msg::AudioStatus(Status::StreamError {
                message: "http status 404".into(),
                kind: FailureKind::StreamDead,
            }),
        );
        assert!(!fx.iter().any(|e| matches!(e, Effect::MarkFailed(_))));
    }
```

`model_with_one_row(uuid)` is a helper: if the existing tests already have an
equivalent (they build a model with `m.now.uuid` set and one browse row), reuse
that one and delete this note; otherwise add it next to the other test helpers,
constructing the model the same way the neighbouring tests do and setting
`m.now.uuid = Some(uuid.into())`.

- [ ] **Step 6: Run it to verify it fails**

Run: `cd /Users/vchub/dev/projects/world-radio/radio && cargo test -p radio-tui --lib 2>&1 | tail -20`
Expected: FAIL — `network_failure_does_not_mark_the_station` and
`long_failure_streak_stops_marking_stations` fail, because `auto_skip` currently
marks unconditionally.

- [ ] **Step 7: Implement the gate**

In `crates/radio-tui/src/tui/update.rs`, `auto_skip` currently takes only
`&mut Model`. Give it the failure kind and gate the health write. Read the
function first (it starts at line 275), then change its signature and the
marking block:

```rust
fn auto_skip(model: &mut Model, kind: FailureKind) -> Vec<Effect> {
    let mut effects = Vec::new();
    // a long streak is far more likely to be a broken local network than many
    // simultaneously dead stations, so stop blaming stations once it trips
    let blame_station =
        kind == FailureKind::StreamDead && model.auto_skip_count < AUTO_SKIP_MAX;
    if let Some(uuid) = model.now.uuid.clone() {
        model
            .browse
            .update_row(&uuid, |r| r.state = RowState::Disabled);
        if blame_station {
            effects.push(Effect::MarkFailed(uuid));
        }
    }
    if model.auto_skip_count >= AUTO_SKIP_MAX {
        return effects;
    }
    let Some(next) = model.browse.next_playable_below() else {
        return effects;
    };
    model.browse.selected = next;
    model.auto_skip_count += 1;
```

Leave the rest of the function body exactly as it is.

Then split the status arm at line 262 so the two error shapes pass the right
kind — the old `Status::Error(String)` has no classification, so it keeps
today's behaviour by reporting `StreamDead`:

```rust
        Status::Error(_) => {
            model.now.title = None;
            model.status = s;
            auto_skip(model, FailureKind::StreamDead)
        }
        Status::StreamError { kind, .. } => {
            let kind = *kind;
            model.now.title = None;
            model.status = s;
            auto_skip(model, kind)
        }
```

Add `FailureKind` to the file's imports from `radio_audio`.

- [ ] **Step 8: Run the tests**

Run: `cd /Users/vchub/dev/projects/world-radio/radio && cargo test -p radio-tui --lib 2>&1 | tail -15`
Expected: PASS, including the pre-existing auto-skip tests around line 1253-1280.

- [ ] **Step 9: Commit**

```bash
cd /Users/vchub/dev/projects/world-radio/radio
cargo fmt && cargo clippy --workspace --all-targets 2>&1 | tail -5
git add crates/radio-core/src/catalog/health.rs crates/radio-tui/src/tui/update.rs
git commit -m "fix(tui): drop a station from the list as soon as it fails to play"
```

---

### Task 3: Hide stations the server already knows are dead

The search API returns `lastcheckok` and `lastchecktime_iso8601` in the same
response we already fetch, and we currently discard both. Parse them, and on
ingest record a health failure for anything the server reports as dead.

Sampling the live API on 2026-07-27 (200 stations, `hidebroken=false`) returned
`lastcheckok=1` for all 200 with check dates spread from January to July — so this
is a cheap secondary filter, NOT a replacement for Task 2.

**Files:**
- Modify: `crates/radio-core/src/catalog/station.rs:4-25` (two new fields)
- Modify: `crates/radio-core/src/catalog/catalog.rs:25-27` (`ingest`)
- Test: inline `mod tests` in `crates/radio-core/src/catalog/catalog.rs` (already exists)

**Interfaces:**
- Consumes: nothing from Tasks 1-2.
- Produces: `Station.lastcheckok: u8` and `Station.lastchecktime_iso8601: String`, both `#[serde(default)]`.

- [ ] **Step 1: Write the failing test**

Add to the existing `mod tests` in `crates/radio-core/src/catalog/catalog.rs`,
following the construction style the neighbouring tests use for `Station` and
`Catalog`:

```rust
    #[test]
    fn ingest_hides_a_station_the_server_reports_dead() {
        let mut cat = catalog_in_memory();
        let mut s = station("u1");
        s.lastcheckok = 0;
        cat.ingest(&[s]).unwrap();
        assert!(cat.is_hidden("u1"));
    }

    #[test]
    fn ingest_keeps_a_station_the_server_reports_alive() {
        let mut cat = catalog_in_memory();
        let mut s = station("u1");
        s.lastcheckok = 1;
        cat.ingest(&[s]).unwrap();
        assert!(!cat.is_hidden("u1"));
    }

    #[test]
    fn a_stale_server_ok_does_not_revive_a_station_the_user_found_dead() {
        let mut cat = catalog_in_memory();
        cat.note_play_failure("u1");
        assert!(cat.is_hidden("u1"));
        let mut s = station("u1");
        s.lastcheckok = 1;
        cat.ingest(&[s]).unwrap();
        // the user's own experience outranks the server's stale verdict
        assert!(cat.is_hidden("u1"));
    }
```

`catalog_in_memory()` and `station(uuid)` are helpers: reuse the equivalents the
existing tests in this file already use (they build a `Catalog` over
`Cache::open_in_memory()` and a `Station` with default fields); if they are
inlined rather than factored out, add these two helpers next to the other tests
following the same construction.

- [ ] **Step 2: Run it to verify it fails**

Run: `cd /Users/vchub/dev/projects/world-radio/radio && cargo test -p radio-core --lib catalog::catalog 2>&1 | tail -15`
Expected: FAIL — `no field lastcheckok on type Station`.

- [ ] **Step 3: Add the fields**

In `crates/radio-core/src/catalog/station.rs`, add to the `Station` struct after
`votes`:

```rust
    /// the server's own last liveness probe: 1 = ok. stale in practice, so this
    /// is a cheap secondary filter, never a substitute for the user's experience.
    #[serde(default = "default_checkok")]
    pub lastcheckok: u8,
    #[serde(default)]
    pub lastchecktime_iso8601: String,
```

And below the struct:

```rust
fn default_checkok() -> u8 {
    1
}
```

The default is 1, not 0: a station coming from anywhere that does not carry the
field (our own cache rows, tests, older payloads) must not be treated as dead.

- [ ] **Step 4: Record the server's verdict on ingest**

In `crates/radio-core/src/catalog/catalog.rs`, `ingest` is currently:

```rust
    pub fn ingest(&self, stations: &[Station]) -> anyhow::Result<()> {
        self.cache.upsert(stations)
    }
```

It must become `&mut self` to touch health. Read the function and its callers
first, then:

```rust
    pub fn ingest(&mut self, stations: &[Station]) -> anyhow::Result<()> {
        for s in stations.iter().filter(|s| s.lastcheckok == 0) {
            self.health.record_failure(&s.stationuuid);
        }
        self.cache.upsert(stations)
    }
```

Only the dead are touched — a reported-alive station is left alone, so a stale
server "ok" cannot clear a failure the user recorded.

Then fix every caller that now needs a mutable binding. There are five, in three
crates — do not assume they are all in `radio-tui`:

- `crates/radio-tui/src/tui/worker.rs:397` and `:505` (the worker already owns
  `mut catalog`, so these should just work)
- `crates/radio-tui/src/main.rs:115`
- `crates/radio-mini/src/catalog_src.rs:64` (a test)
- `crates/radio-core/src/catalog/catalog.rs:503` (a test)

Re-run the search to confirm none were missed:
`cd /Users/vchub/dev/projects/world-radio/radio && grep -rn "\.ingest(" --include="*.rs" crates/ | grep -v "/target/"`

- [ ] **Step 5: Run the tests**

Run: `cd /Users/vchub/dev/projects/world-radio/radio && cargo test -p radio-core --lib 2>&1 | tail -15`
Expected: PASS.

- [ ] **Step 6: Bump the cache schema version**

The `stations` table has fixed columns and `init_schema`
(`crates/radio-core/src/catalog/cache.rs:35-67`) DROPS and recreates the tables
whenever the stored `PRAGMA user_version` is below `SCHEMA_VERSION`. Decide by
inspection whether `upsert`/row-reading actually needs the two new columns
persisted:

- If the new fields are only consumed at ingest time (they are — Step 4 reads
  them from the incoming payload, not from the database), then NO schema change
  and NO version bump is needed. Confirm by reading `upsert` and the row-mapping
  function in `cache.rs` and checking they enumerate columns explicitly.
- Only if a column is genuinely required, add it to the `CREATE TABLE`, add it to
  `upsert`, and bump `SCHEMA_VERSION` from 1 to 2 — accepting that this triggers
  a one-time full catalog re-sync for every existing user.

Record which branch you took, and why, in your report.

- [ ] **Step 7: Full workspace check**

Run: `cd /Users/vchub/dev/projects/world-radio/radio && cargo fmt && cargo clippy --workspace --all-targets 2>&1 | tail -15 && cargo test --workspace 2>&1 | tail -15`
Expected: clippy clean, all tests pass.

- [ ] **Step 8: Commit**

```bash
cd /Users/vchub/dev/projects/world-radio/radio
git add crates/radio-core/src/catalog/station.rs crates/radio-core/src/catalog/catalog.rs
git commit -m "feat(core): skip stations the directory already reports as offline"
```

---

### Task 4: Refresh the catalog during a long session

`should_sync` is consulted exactly once, at TUI startup
(`crates/radio-tui/src/tui/mod.rs:76`). A session left open for days never
refreshes. Add an hourly check inside the worker that already owns
`WorkerReq::SyncCatalog`.

**Files:**
- Modify: `crates/radio-tui/src/tui/worker.rs` (hourly tick)
- Test: inline `mod tests` in `crates/radio-core/src/catalog/mod.rs` (extend the existing `should_sync` tests)

**Interfaces:**
- Consumes: `radio_core::catalog::should_sync(last: Option<i64>, now: i64, ttl_secs: i64) -> bool`, already public and already tested; `WorkerReq::SyncCatalog` and the `CatalogSynced { count: usize }` message, both already defined.
- Produces: nothing for later tasks — this is the last one.

- [ ] **Step 1: Write the failing test for the refresh decision**

The decision "should the periodic tick sync now?" must be a pure function so it
can be tested without threads. Add to `crates/radio-core/src/catalog/mod.rs`
inside the existing `mod tests`:

```rust
    #[test]
    fn periodic_refresh_waits_for_the_ttl() {
        // an hour into a session that synced 10 minutes ago: not yet
        assert!(!should_refresh(Some(1_000_000 - 600), 1_000_000, DAY));
    }

    #[test]
    fn periodic_refresh_fires_once_the_ttl_expired() {
        assert!(should_refresh(Some(1_000_000 - 25 * 3600), 1_000_000, DAY));
    }

    #[test]
    fn periodic_refresh_fires_when_nothing_was_ever_synced() {
        assert!(should_refresh(None, 1_000_000, DAY));
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd /Users/vchub/dev/projects/world-radio/radio && cargo test -p radio-core --lib catalog 2>&1 | tail -12`
Expected: FAIL — `cannot find function should_refresh`.

- [ ] **Step 3: Implement**

In `crates/radio-core/src/catalog/mod.rs`, next to `should_sync`:

```rust
/// the periodic in-session check. same TTL rule as startup — named separately so
/// the two callers can diverge later without surprising each other.
pub fn should_refresh(last: Option<i64>, now: i64, ttl_secs: i64) -> bool {
    should_sync(last, now, ttl_secs)
}
```

- [ ] **Step 4: Run it to verify it passes**

Run: `cd /Users/vchub/dev/projects/world-radio/radio && cargo test -p radio-core --lib catalog 2>&1 | tail -12`
Expected: PASS.

- [ ] **Step 5: Add the hourly tick to the worker**

Read `crates/radio-tui/src/tui/worker.rs` first — in particular how the worker
loop receives on its channel, since the tick must not block request handling.

The loop at `worker.rs:70` is `while let Ok(first) = req_rx.recv()`, and the lines
just below it (72-77) drain further requests with `try_recv` to coalesce a burst
into one batch. **That coalescing must survive this change** — it is what keeps
rapid keystrokes from queueing redundant searches.

Restructure only the outer blocking receive:

```rust
    loop {
        let first = match req_rx.recv_timeout(Duration::from_secs(60)) {
            Ok(req) => req,
            Err(RecvTimeoutError::Timeout) => {
                // periodic housekeeping; no request to process this round
                maybe_refresh(&mut catalog, &paths, &msg_tx, &mut last_check);
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => break,
        };
        // ... the existing try_recv coalescing and dispatch, unchanged ...
    }
```

`Disconnected` must break the loop exactly as the closed channel does today —
getting this wrong turns a clean shutdown into a thread that spins forever.

The check itself, run on each timeout:

```rust
// wall-clock hour between checks; the TTL inside should_refresh is what
// actually decides, this only bounds how often we ask
const REFRESH_CHECK_SECS: i64 = 3600;
```

Keep a `last_check: i64` in the loop, initialised to the current unix time. On
each timeout, if `now - last_check >= REFRESH_CHECK_SECS`, set
`last_check = now` and then, when
`radio_core::catalog::should_refresh(catalog.last_sync().ok().flatten(), now, 86_400)`
is true, run the same handler `WorkerReq::SyncCatalog` runs. Call that handler
directly rather than pushing a request into the channel the worker itself is
draining.

Unix time in this file: follow the same pattern `mod.rs:72-75` uses
(`SystemTime::now().duration_since(UNIX_EPOCH)`, `unwrap_or(0)`).

- [ ] **Step 6: Verify the refresh does not disturb the visible list**

The sync handler emits `CatalogSynced`, which `update.rs` already routes through
`catalog_refresh_effect` (`crates/radio-tui/src/tui/update.rs:716`) — that
function re-searches when a filter is active and restores the popular seed when
it is not. Read it and confirm the periodic path reuses it rather than
introducing a second, parallel refresh path. Do not add new list-rebuilding
logic.

Run: `cd /Users/vchub/dev/projects/world-radio/radio && cargo test -p radio-tui --lib 2>&1 | tail -12`
Expected: PASS — the existing `catalog_synced_*` gating tests must stay green.

- [ ] **Step 7: Full workspace check**

Run: `cd /Users/vchub/dev/projects/world-radio/radio && cargo fmt && cargo clippy --workspace --all-targets 2>&1 | tail -15 && cargo test --workspace 2>&1 | tail -15`
Expected: clippy clean, all tests pass.

- [ ] **Step 8: Commit**

```bash
cd /Users/vchub/dev/projects/world-radio/radio
git add crates/radio-core/src/catalog/mod.rs crates/radio-tui/src/tui/worker.rs
git commit -m "feat(tui): keep the station list fresh without restarting the app"
```

---

### Task 5: Live verification (controller-run, not a subagent)

**Files:** none — verification only.

This task is run by the controller. The TUI is a full-screen terminal app, so it
must be driven on an **isolated tmux socket** (`tmux -S <sock>`) — a plain
`tmux new-session` leaks into the user's own terminal. It must also run against an
isolated data directory (`HOME=<sandbox>`), never the user's real one.

- [ ] **Step 1: Build a release binary and prepare a sandbox**

```bash
cd /Users/vchub/dev/projects/world-radio/radio && cargo build --release -p radio-tui
SANDBOX=$(mktemp -d)
```

- [ ] **Step 2: Drive the TUI on an isolated socket**

```bash
tmux -S /tmp/r4dio-verify.sock new-session -d -x 200 -y 50 \
  "HOME=$SANDBOX /Users/vchub/dev/projects/world-radio/radio/target/release/r4dio"
sleep 25
tmux -S /tmp/r4dio-verify.sock capture-pane -p -t 0 | head -40
```
Expected: the catalog syncs on first run and the station list populates.

- [ ] **Step 3: Confirm a failing station disappears after ONE failure**

Play stations until one fails (`✗ ERROR` in the header, or the row turning
disabled). Confirm the row is struck out / disabled immediately rather than after
three attempts, and that `$SANDBOX` contains a `station_health.json` naming it.

- [ ] **Step 4: Confirm the network guard**

With the TUI running, disable networking briefly (or point it at an unreachable
state) and let several stations fail in a row. Confirm that
`station_health.json` does NOT grow by one entry per attempted station — the
streak guard must stop recording after `AUTO_SKIP_MAX`.

- [ ] **Step 5: Clean up**

```bash
tmux -S /tmp/r4dio-verify.sock kill-server
rm -rf "$SANDBOX"
```
Confirm the user's real data directory was never touched:
`ls -la ~/Library/Application\ Support/net.vchub.r4dio` — mtime must predate this session.

---

## Notes for the reviewer

The highest-risk change is Task 2: a misclassified error now hides a station after
a single failure instead of three. Two independent guards bound the damage
(classification defaults to `NetworkDown` for anything unrecognised, and the
`AUTO_SKIP_MAX` streak stops health writes entirely). Verify BOTH are actually
wired, not just present.

Second: Task 3 changes `Catalog::ingest` from `&self` to `&mut self`. Confirm
every caller was updated and that no caller silently lost its ability to ingest.

Third: Task 4 changes the worker's receive from blocking to timeout-based. Confirm
`Disconnected` still terminates the loop — getting that wrong turns a clean
shutdown into a spinning thread.
