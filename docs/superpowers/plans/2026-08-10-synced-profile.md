# Synced Profile Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The account carries the whole listening profile — shuffle filter (countries), scope, theme, and history — so "UA only" set on the desktop constrains Android's shuffle, and a station played in the car shows up in the desktop history.

**Architecture:** Two merge semantics, kept strictly apart: the existing tombstone record-sets (favs/blocked/excluded — untouched) and new LWW scalars (`shuffle_filter`, `scope`, `theme`: client-stamped `at`, newer wins) plus a capped union set for history (union by uuid keeping newest `at`, cap 200, no tombstones). Every new payload field is optional on both sides, so old clients and the old server interoperate unchanged.

**Tech Stack:** Rust (axum + rusqlite on the server, serde), Kotlin (OkHttp + kotlinx.serialization on Android).

**Spec:** `docs/superpowers/specs/2026-08-10-synced-profile.md`

## Global Constraints

- Three repos, three deploy paths: `sync` (separate repo at `../sync`, AUTO-DEPLOYS on push to its main — do not push until the task is reviewed), `radio` (this repo, branch dev), Android lives inside `radio`.
- Old-client compatibility is a hard requirement: a payload without the new fields must leave them untouched server-side, and a server response without them must change nothing client-side.
- Filter value shape exactly: `{"countries": ["UA", …]}` inside the LWW wrapper.
- History cap exactly 200, newest by `at` win; no tombstones.
- FAVS scope is exempt from the filter (explicit star outranks a broad taste filter).
- Fetch/cache paths are never filtered — the filter applies at pick sites only (the `blocked + hidden` precedent).
- No `else if`; comments only for constraints, lowercase-first; strings/logs English lowercase-first; commit subjects are the public changelog.
- Rust: `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` green before every commit (fmt bit us last time — run it).
- Android: `cd android && ./gradlew test` green before commit.

---

### Task 1: Server — LWW scalars and capped history

**Repo/dir:** `/Users/vchub/dev/projects/world-radio/sync` (its own git repo, branch main; commit but DO NOT push — the controller pushes after review because pushing deploys).

**Files:**
- Modify: `src/merge.rs` (add `Lww`, `merge_lww`, `merge_history` + tests)
- Modify: `src/store.rs` (Account fields, columns + migration in the `excluded_countries` style at :48-57, apply merges inside `Store::sync`)
- Test: same files' `mod tests`

**Interfaces:**
- Consumes: existing `Record` (`{id, at, gone}`), `Store::sync(key_hash, account, changed)`.
- Produces (Tasks 2-3 mirror these field names in their payloads):

```rust
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Lww {
    pub value: serde_json::Value,
    pub at: i64,
}
pub fn merge_lww(stored: Option<Lww>, incoming: Option<Lww>) -> Option<Lww>
pub fn merge_history(stored: &[Record], incoming: &[Record], cap: usize) -> Vec<Record>
pub const HISTORY_CAP: usize = 200;
```

Account JSON fields (all `#[serde(default)]`, skipped when `None`/empty on serialize):
`shuffle_filter: Option<Lww>`, `scope: Option<Lww>`, `theme: Option<Lww>`,
`history: Vec<Record>`.

- [ ] **Step 1: Write the failing merge tests**

In `src/merge.rs` `mod tests`:

```rust
    fn lww(v: &str, at: i64) -> Lww {
        Lww { value: serde_json::json!({ "countries": [v] }), at }
    }

    #[test]
    fn lww_newer_incoming_wins() {
        let out = merge_lww(Some(lww("UA", 10)), Some(lww("PL", 20)));
        assert_eq!(out, Some(lww("PL", 20)));
    }

    #[test]
    fn lww_older_incoming_loses() {
        let out = merge_lww(Some(lww("UA", 20)), Some(lww("PL", 10)));
        assert_eq!(out, Some(lww("UA", 20)));
    }

    #[test]
    fn lww_absent_incoming_keeps_stored() {
        assert_eq!(merge_lww(Some(lww("UA", 5)), None), Some(lww("UA", 5)));
        assert_eq!(merge_lww(None, None), None);
    }

    #[test]
    fn lww_first_write_lands() {
        assert_eq!(merge_lww(None, Some(lww("UA", 5))), Some(lww("UA", 5)));
    }

    fn hrec(id: &str, at: i64) -> Record {
        Record { id: id.into(), at, gone: false }
    }

    #[test]
    fn history_unions_by_uuid_keeping_newest_at() {
        let stored = vec![hrec("a", 10), hrec("b", 20)];
        let incoming = vec![hrec("a", 30), hrec("c", 5)];
        let out = merge_history(&stored, &incoming, 200);
        assert_eq!(out.iter().find(|r| r.id == "a").unwrap().at, 30);
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn history_caps_to_newest_n() {
        let stored: Vec<Record> = (0..250).map(|i| hrec(&format!("s{i}"), i)).collect();
        let out = merge_history(&stored, &[], 200);
        assert_eq!(out.len(), 200);
        assert!(out.iter().all(|r| r.at >= 50));
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd /Users/vchub/dev/projects/world-radio/sync && cargo test merge 2>&1 | tail -5`
Expected: compile FAIL — `Lww`, `merge_lww`, `merge_history` do not exist.

- [ ] **Step 3: Implement in merge.rs**

```rust
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Lww {
    pub value: serde_json::Value,
    pub at: i64,
}

pub const HISTORY_CAP: usize = 200;

pub fn merge_lww(stored: Option<Lww>, incoming: Option<Lww>) -> Option<Lww> {
    match (stored, incoming) {
        (s, None) => s,
        (None, i) => i,
        (Some(s), Some(i)) => match i.at > s.at {
            true => Some(i),
            false => Some(s),
        },
    }
}

pub fn merge_history(stored: &[Record], incoming: &[Record], cap: usize) -> Vec<Record> {
    let mut by_id: BTreeMap<&str, Record> = BTreeMap::new();
    for r in stored.iter().chain(incoming) {
        match by_id.get(r.id.as_str()) {
            Some(have) if have.at >= r.at => {}
            _ => {
                by_id.insert(&r.id, r.clone());
            }
        }
    }
    let mut out: Vec<Record> = by_id.into_values().collect();
    out.sort_by(|a, b| b.at.cmp(&a.at));
    out.truncate(cap);
    out
}
```

Run: `cargo test merge` — PASS.

- [ ] **Step 4: Wire the store**

In `src/store.rs`:
- `Account` gains (all with `#[serde(default)]`; add `#[serde(skip_serializing_if = "Option::is_none")]` on the three options):

```rust
    pub shuffle_filter: Option<merge::Lww>,
    pub scope: Option<merge::Lww>,
    pub theme: Option<merge::Lww>,
    pub history: Vec<merge::Record>,
```

- Schema: add `shuffle_filter TEXT NOT NULL DEFAULT ''`, `scope TEXT NOT NULL DEFAULT ''`, `theme TEXT NOT NULL DEFAULT ''`, `history TEXT NOT NULL DEFAULT '[]'` to CREATE TABLE, plus a migration loop over the four names exactly like the `fav_records` loop at :60 (empty-string default for the option columns, `'[]'` for history; `''` deserializes as `None` via a small helper: store empty string when None, `serde_json::to_string` when Some).
- In `Store::sync`, after the existing three-set merge: `merge_lww` each scalar, `merge_history(&stored_history, &account.history, merge::HISTORY_CAP)`, persist, and include all four in the returned merged `Account`.

- [ ] **Step 5: Store-level old-client test**

In `src/store.rs` tests (follow the existing store test style — in-memory or tempfile DB):

```rust
    #[test]
    fn old_client_payload_leaves_profile_untouched() {
        let store = test_store();
        let key = "k1";
        store.create(key, 0);
        let mut acc = store::Account::default();
        acc.shuffle_filter = Some(merge::Lww { value: serde_json::json!({"countries":["UA"]}), at: 10 });
        store.sync(key, &acc, &Default::default()).unwrap();
        // an old client sends no profile fields at all
        let legacy = store::Account { favs: vec!["f1".into()], ..Default::default() };
        let merged = store.sync(key, &legacy, &Default::default()).unwrap();
        assert_eq!(merged.shuffle_filter.unwrap().at, 10);
    }
```

(Adapt constructor names to the real test helpers in store.rs — the file already has tests; follow their setup verbatim.)

- [ ] **Step 6: fmt, clippy, full tests, commit (NO push)**

```bash
cargo fmt && cargo fmt --check && cargo clippy --all-targets 2>&1 | tail -3 && cargo test 2>&1 | tail -3
git add src/merge.rs src/store.rs
git commit -m "carry the listening profile in the account"
```

---

### Task 2: CLI — publish and apply the profile

**Repo/dir:** `/Users/vchub/dev/projects/world-radio/radio` (branch dev).

**Files:**
- Modify: `crates/radio-core/src/sync/client.rs` (SyncData fields)
- Create: `crates/radio-core/src/sync/profile.rs` (local profile state + stamps)
- Modify: `crates/radio-core/src/sync/mod.rs` (export)
- Modify: `crates/radio-tui/src/tui/worker.rs` (sync op includes profile; response applies it)
- Modify: `crates/radio-tui/src/tui/update.rs` (filter/scope/theme changes stamp the profile)
- Test: `profile.rs` mod tests + existing sync tests stay green

**Interfaces:**
- Consumes: server field names from Task 1 (`shuffle_filter`, `scope`, `theme`, `history`, `Lww {value, at}`, `Record {id, at, gone}`).
- Produces for Task 3 (Android mirrors): the JSON wire shapes only — no Rust interfaces cross into Android.

Key shapes (exact history-file layout and worker call sites must be read from the actual code first — `crates/radio-core/src/catalog.rs` history handling and the `WorkerReq::Sync` arm in worker.rs; the brief's code is the shape, not gospel):

```rust
// profile.rs — persisted in the data dir as profile.json
#[derive(Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct Profile {
    pub countries: Vec<String>,
    pub countries_at: i64,
    pub scope: String,
    pub scope_at: i64,
    pub theme: String,
    pub theme_at: i64,
}
impl Profile {
    pub fn load(path: &Path) -> Profile;
    pub fn save(&self, path: &Path) -> anyhow::Result<()>;
    pub fn set_countries(&mut self, countries: Vec<String>, now: i64); // no-op if equal
    pub fn set_scope(&mut self, scope: &str, now: i64);
    pub fn set_theme(&mut self, theme: &str, now: i64);
    // returns true when anything changed, so the caller knows to re-render
    pub fn apply_newer(&mut self, filter: Option<(Vec<String>, i64)>, scope: Option<(String, i64)>, theme: Option<(String, i64)>) -> bool;
}
```

- [ ] **Step 1: TDD profile.rs** — failing tests first (`set_countries` stamps and no-ops on equal value; `apply_newer` takes newer, rejects older; `load` of a missing file is default), run red, implement, run green.
- [ ] **Step 2: Extend SyncData** in client.rs with the four optional fields (`#[serde(default, skip_serializing_if = "Option::is_none")]` for the three `Lww`-shaped ones — define a local `Lww {value: serde_json::Value, at: i64}` mirror — and `#[serde(default)] history: Vec<HistoryRecord>` with `HistoryRecord {id, at, gone}`).
- [ ] **Step 3: Wire the worker** — on sync: load Profile + map to `SyncData` fields (countries → `{"countries": [...]}` Lww, history from the history file mapped to `{id: uuid, at: played_at, gone: false}`); on response: `apply_newer` + merge history records back into the history file (union by uuid, newest at — reuse the same rule, do NOT invent a second one); when `apply_newer` returned true, push the new filter into `model.browse.filters.countries` and the scope/theme into the model via the existing message channel (find the message the sync path already uses for "state changed, re-render" and extend it — do not add a parallel channel).
- [ ] **Step 4: Stamp on change** — in update.rs, the arms that toggle browse countries, change scope, and change theme call the matching `Profile::set_*` with `now`. Locate them by grepping for where `filters.countries` is mutated, where scope flips, and where theme cycles.
- [ ] **Step 5:** `cargo fmt && cargo fmt --check && cargo clippy --all-targets && cargo test --workspace` green; commit `sync the listening profile from the cli`.

---

### Task 3: Android — honor the filter, sync scope, push history, show the pill

**Repo/dir:** `/Users/vchub/dev/projects/world-radio/radio` (branch dev), `android/`.

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/SyncClient.kt` (SyncData fields + Lww/HistoryRecord data classes)
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/FavStore.kt` (persist profile: filter countries + stamps; scope stamping)
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/FavSync.kt` (include profile in push, apply newer from response)
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/Catalog.kt` (`allowedStation`/pick path gains an optional `included: Set<String>` — empty set = unrestricted)
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt` (pass filter at the ALL-scope pick sites; push `{uuid, at}` on每 play; scope read/write through the synced value)
- Modify: the home screen state (`HomeState.kt` / `MainActivity.kt`) for the `filter: UA` pill next to the existing excluded-countries indicator
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/` — new `ProfileSyncTest.kt` + extended `CatalogFilterTest.kt`

**Interfaces:**
- Consumes: wire shapes from Task 1 (`shuffle_filter: {value: {countries: [...]}, at}`, `scope: {value: "ALL"|"FAVS", at}`, `history: [{id, at, gone}]`).
- Produces: nothing downstream.

Key rules to encode in tests (write them first, red → green):

```kotlin
    @Test fun filter_restricts_all_scope_picks()      // only filter-country stations picked
    @Test fun empty_filter_is_unrestricted()
    @Test fun favs_scope_ignores_the_filter()          // star outranks taste filter
    @Test fun newer_remote_filter_replaces_local()     // LWW apply
    @Test fun older_remote_filter_is_ignored()
    @Test fun play_appends_history_record()            // {uuid, at} queued for next push
```

Implementation notes (verify each against the actual code, the line numbers move):
- The filter intersects at pick time exactly where `blocked + hidden` already union — `shuffle()` and both `startFrom` calls in PlaybackService; `pickForScopeDetailed`'s ALL arm gets the include-set, the FAVS arm does not. Follow the shape of `allowedStation(st, excluded, blocked)` — add `included: Set<String> = emptySet()` with `included.isEmpty() || st.country.uppercase() in included`.
- The fetch (`fetchStations`) and the fav resolution (`fetchByUuids`) stay unfiltered — spec constraint.
- History: keep a small pending-plays set in FavStore (`history_pending` DataStore key, `uuid|at` strings), drained into the next sync push, merged response replaces nothing locally (Android has no history UI yet — do not build storage beyond the pending queue).
- Scope: on user toggle, stamp `scope_at`; on sync response apply LWW; the widget/notification path reads the same stored scope it already reads.
- The pill: follow the excluded-countries indicator's existing render path; text `filter: UA` (join up to 3 codes with `·`, then `+N`).
- Theme: store the synced value + stamp in FavStore (`theme` / `theme_at`) and round-trip it through sync exactly like scope — nothing reads it yet (themes arrive with the redesign); that is deliberate, not an omission.

- [ ] **Step 1:** failing tests → red run → implement → `cd android && ./gradlew test` green.
- [ ] **Step 2:** commit `android: the shuffle filter and scope follow the account`.

---

## Verification after all tasks (controller, not subagents)

1. Review each task, then: push `sync` main (deploys automatically; DB backup runs pre-migration) and verify `/health` + a real sync round-trip from the CLI afterwards.
2. Push radio dev.
3. Live end-to-end (needs the user or the emulator + real account): TUI set countries=UA → sync → emulator Android app sync → 10 shuffles → logcat station names all [UA]; play one station on the emulator → TUI sync → history shows it.
4. Note in memory: theme applies on Android only when themes exist (redesign); scope LWW may surprise if two devices toggle within the same second — last write wins is accepted.
