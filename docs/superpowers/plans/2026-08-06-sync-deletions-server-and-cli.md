# Sync Deletions — Server + CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a deletion on one device survive the next sync from another device, instead of being resurrected.

**Architecture:** Every synced item gains a server-stamped timestamp and a `gone` flag. The server owns the single merge rule — greater timestamp wins, ties go to the deletion — and understands both the old (plain array) and new (array + `changed` delta) request shapes, so clients can be upgraded one at a time. Clients stop merging entirely; they report only which items they changed since their last successful sync.

**Tech Stack:** Rust (axum + rusqlite on the server, reqwest on the CLI), serde_json, SQLite.

**Scope:** server (`world-radio-sync` repo) and CLI (`radio` repo) only. Android is a separate plan — different language and test stack, and the server's dual-format support means it can ship later without breaking anything.

## Global Constraints

- All code, comments and log strings in **English**; comments and logs in lowercase style.
- Comments only where the logic is non-obvious — explain WHY, not WHAT. Trivial comments are a defect.
- No AI/assistant mention anywhere: code, comments, commit messages. No co-author trailers.
- Commit subjects are the public changelog — write them for users, concise, lowercase.
- **Do not use `else if`** (project style rule).
- `sync` is a **separate git repo** at `/Users/vchub/dev/projects/world-radio/sync`, pushed and deployed manually. `radio` is at `/Users/vchub/dev/projects/world-radio/radio`. Never commit one into the other.
- The server must keep answering old-format requests exactly as it does today — an un-upgraded phone is in the field.
- The server stamps all timestamps. A client-supplied time is never trusted.
- Tombstone lifetime: **90 days** (`7_776_000` seconds).
- Existing server tests: 7 in `src/store.rs`. Existing CLI sync tests live in `crates/radio-core/src/sync/client.rs`. Both suites must stay green.

---

## File Structure

**Server (`sync` repo):**
- Create: `src/merge.rs` — the record type and the pure merge rule. No IO, no SQL. This is the one place the rule exists.
- Modify: `src/store.rs` — schema migration, read/write of the record columns, tombstone pruning.
- Modify: `src/main.rs:89-101` — `put_sync` accepts the optional `changed` delta and calls the merge.

**CLI (`radio` repo):**
- Create: `crates/radio-core/src/sync/pending.rs` — the local operation log (what changed since the last sync) and its JSON persistence.
- Modify: `crates/radio-core/src/sync/client.rs` — `SyncData` gains an optional `changed` field; `push` sends it.
- Modify: `crates/radio-core/src/catalog/catalog.rs` — record an entry on each mutation.

---

### Task 1: The merge rule (server, pure)

The whole feature rests on this function. It is pure so it can be exhaustively tested without a database.

**Files:**
- Create: `/Users/vchub/dev/projects/world-radio/sync/src/merge.rs`
- Modify: `/Users/vchub/dev/projects/world-radio/sync/src/main.rs` (add `mod merge;` beside the existing `mod` lines)

**Interfaces:**
- Produces:
  - `pub struct Record { pub id: String, pub at: i64, pub gone: bool }` — derives `Serialize, Deserialize, Debug, PartialEq, Clone`
  - `pub struct Change { pub id: String, pub gone: bool }` — derives `Deserialize, Debug, PartialEq, Clone`
  - `pub fn merge(stored: &[Record], present: &[String], changed: &[Change], now: i64) -> Vec<Record>`
  - `pub fn prune(records: &[Record], now: i64) -> Vec<Record>`
  - `pub fn present_ids(records: &[Record]) -> Vec<String>`
  - `pub const TOMBSTONE_TTL: i64 = 7_776_000;`

- [ ] **Step 1: Write the failing test**

Create `/Users/vchub/dev/projects/world-radio/sync/src/merge.rs` containing ONLY this test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn rec(id: &str, at: i64, gone: bool) -> Record {
        Record { id: id.into(), at, gone }
    }

    #[test]
    fn a_new_client_deletion_removes_a_stored_item() {
        let stored = vec![rec("a", 100, false)];
        let out = merge(&stored, &[], &[Change { id: "a".into(), gone: true }], 200);
        assert_eq!(out, vec![rec("a", 200, true)]);
        assert_eq!(present_ids(&out), Vec::<String>::new());
    }

    #[test]
    fn a_tombstone_survives_a_later_push_that_still_lists_the_item() {
        // the resurrection bug: another device pushes its stale state, which still
        // contains "a". without the tombstone winning, "a" would come back.
        let stored = vec![rec("a", 200, true)];
        let out = merge(&stored, &["a".into()], &[], 300);
        assert_eq!(out, vec![rec("a", 200, true)]);
        assert_eq!(present_ids(&out), Vec::<String>::new());
    }

    #[test]
    fn re_adding_after_a_deletion_wins_because_it_is_newer() {
        let stored = vec![rec("a", 200, true)];
        let out = merge(&stored, &[], &[Change { id: "a".into(), gone: false }], 300);
        assert_eq!(out, vec![rec("a", 300, false)]);
        assert_eq!(present_ids(&out), vec!["a".to_string()]);
    }

    #[test]
    fn a_deletion_wins_a_tie_on_timestamp() {
        let stored = vec![rec("a", 300, false)];
        let out = merge(&stored, &[], &[Change { id: "a".into(), gone: true }], 300);
        assert_eq!(out, vec![rec("a", 300, true)]);
    }

    #[test]
    fn an_unchanged_item_keeps_its_stored_timestamp() {
        let stored = vec![rec("a", 100, false)];
        let out = merge(&stored, &["a".into()], &[], 999);
        assert_eq!(out, vec![rec("a", 100, false)]);
    }

    #[test]
    fn an_old_client_adds_an_unknown_id_stamped_now() {
        // no `changed` field at all: every listed id is simply present.
        let out = merge(&[], &["a".into()], &[], 500);
        assert_eq!(out, vec![rec("a", 500, false)]);
    }

    #[test]
    fn an_old_client_omitting_an_id_does_not_delete_it() {
        // absence is not a deletion — only an explicit `changed` entry is.
        let stored = vec![rec("a", 100, false)];
        let out = merge(&stored, &[], &[], 500);
        assert_eq!(out, vec![rec("a", 100, false)]);
    }

    #[test]
    fn prune_drops_expired_tombstones_but_keeps_live_items() {
        let recs = vec![
            rec("old-gone", 1, true),
            rec("fresh-gone", 1_000_000, true),
            rec("ancient-live", 1, false),
        ];
        let out = prune(&recs, 1 + TOMBSTONE_TTL);
        assert_eq!(
            out.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            vec!["fresh-gone", "ancient-live"]
        );
    }

    #[test]
    fn present_ids_returns_only_live_items_in_order() {
        let recs = vec![rec("a", 1, false), rec("b", 2, true), rec("c", 3, false)];
        assert_eq!(present_ids(&recs), vec!["a".to_string(), "c".to_string()]);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /Users/vchub/dev/projects/world-radio/sync && cargo test merge::`
Expected: FAIL — `cannot find type Record`, `cannot find function merge`.

- [ ] **Step 3: Write minimal implementation**

Prepend to `src/merge.rs`, above the test module:

```rust
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// how long a deletion is remembered. after this a silent device can resurrect
/// the item — accepted, in exchange for the table not growing without bound.
pub const TOMBSTONE_TTL: i64 = 7_776_000;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Record {
    pub id: String,
    pub at: i64,
    pub gone: bool,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct Change {
    pub id: String,
    pub gone: bool,
}

/// merges one set. `present` is what the client currently holds (the legacy
/// array), `changed` is what it altered since its last sync. the server stamps
/// `now` on changed items only — everything else keeps the time already stored,
/// so a device pushing stale state cannot outrank another device's deletion.
pub fn merge(stored: &[Record], present: &[String], changed: &[Change], now: i64) -> Vec<Record> {
    let mut by_id: BTreeMap<&str, Record> = BTreeMap::new();
    for r in stored {
        by_id.insert(&r.id, r.clone());
    }
    // a plain listing only asserts existence; it must never outrank a tombstone,
    // so an id already known is left exactly as it is.
    for id in present {
        if !by_id.contains_key(id.as_str()) {
            by_id.insert(id, Record { id: id.clone(), at: now, gone: false });
        }
    }
    for ch in changed {
        let beats_stored = match by_id.get(ch.id.as_str()) {
            None => true,
            // ties go to the deletion, so a delete and an add in the same second
            // resolve the same way on every device.
            Some(prev) => now > prev.at || (now == prev.at && ch.gone),
        };
        if beats_stored {
            by_id.insert(&ch.id, Record { id: ch.id.clone(), at: now, gone: ch.gone });
        }
    }
    by_id.into_values().collect()
}

pub fn prune(records: &[Record], now: i64) -> Vec<Record> {
    records
        .iter()
        .filter(|r| !r.gone || now - r.at < TOMBSTONE_TTL)
        .cloned()
        .collect()
}

pub fn present_ids(records: &[Record]) -> Vec<String> {
    records
        .iter()
        .filter(|r| !r.gone)
        .map(|r| r.id.clone())
        .collect()
}
```

Add `mod merge;` to `src/main.rs` beside the existing `mod key;` / `mod mirror;` / `mod store;` lines.

- [ ] **Step 4: Run test to verify it passes**

Run: `cd /Users/vchub/dev/projects/world-radio/sync && cargo test merge::`
Expected: PASS, 9 tests.

Note `prune_drops_expired_tombstones_but_keeps_live_items` expects the surviving order `fresh-gone, ancient-live`. `merge` returns a `BTreeMap`-ordered (id-sorted) vector, but `prune` preserves input order — the test builds its input by hand, so this is consistent.

- [ ] **Step 5: Run the whole suite and commit**

Run: `cd /Users/vchub/dev/projects/world-radio/sync && cargo test`
Expected: PASS — 9 new + 7 existing.

```bash
cd /Users/vchub/dev/projects/world-radio/sync
git add src/merge.rs src/main.rs
git commit -m "remember deletions so another device cannot bring them back"
```

---

### Task 2: Persist records in SQLite

The merge rule needs somewhere to live. The legacy columns keep being written so old clients see no change.

**Files:**
- Modify: `/Users/vchub/dev/projects/world-radio/sync/src/store.rs` — `open()` (schema, around line 17), `get()`, and `replace()` (line 81)

**Interfaces:**
- Consumes: `merge::{Record, Change, merge, prune, present_ids}` (Task 1).
- Produces:
  - `store::Account` gains `#[serde(default, skip_serializing_if = "Vec::is_empty")] pub fav_records: Vec<Record>` and the same for `blocked_records`, `country_records`.
  - `Store::sync(&self, key_hash: &str, incoming: &Account, changed: &ChangeSets) -> Option<Account>`
  - `pub struct ChangeSets { pub favs: Vec<Change>, pub blocked: Vec<Change>, pub excluded_countries: Vec<Change> }` — derives `Deserialize, Default, Debug`
  - `replace()` stays, unchanged and still used by nothing new, so existing tests keep passing.

- [ ] **Step 1: Write the failing test**

Append to the existing `mod tests` in `src/store.rs`:

```rust
    #[test]
    fn sync_propagates_a_deletion_to_a_stale_pusher() {
        let dir = tempfile::tempdir().unwrap();
        let s = open(dir.path().join("t.db").to_str().unwrap());
        s.create_account("h1");

        // device A adds "a" and "b"
        let a = Account { favs: vec!["a".into(), "b".into()], ..Default::default() };
        s.sync("h1", &a, &ChangeSets::default()).unwrap();

        // device B deletes "b"
        let b = Account { favs: vec!["a".into()], ..Default::default() };
        let deletion = ChangeSets {
            favs: vec![crate::merge::Change { id: "b".into(), gone: true }],
            ..Default::default()
        };
        let after = s.sync("h1", &b, &deletion).unwrap();
        assert_eq!(after.favs, vec!["a".to_string()]);

        // device A pushes its stale state, which still lists "b"
        let stale = Account { favs: vec!["a".into(), "b".into()], ..Default::default() };
        let out = s.sync("h1", &stale, &ChangeSets::default()).unwrap();
        assert_eq!(out.favs, vec!["a".to_string()], "b was resurrected");
    }

    #[test]
    fn sync_from_an_old_client_still_adds_items() {
        let dir = tempfile::tempdir().unwrap();
        let s = open(dir.path().join("t.db").to_str().unwrap());
        s.create_account("h1");
        let a = Account {
            favs: vec!["a".into()],
            blocked: vec!["x".into()],
            excluded_countries: vec!["DE".into()],
            ..Default::default()
        };
        let out = s.sync("h1", &a, &ChangeSets::default()).unwrap();
        assert_eq!(out.favs, vec!["a".to_string()]);
        assert_eq!(out.blocked, vec!["x".to_string()]);
        assert_eq!(out.excluded_countries, vec!["DE".to_string()]);
    }

    #[test]
    fn sync_survives_a_reopen_of_the_database() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.db");
        let p = path.to_str().unwrap();
        {
            let s = open(p);
            s.create_account("h1");
            let a = Account { favs: vec!["a".into(), "b".into()], ..Default::default() };
            s.sync("h1", &a, &ChangeSets::default()).unwrap();
            let deletion = ChangeSets {
                favs: vec![crate::merge::Change { id: "b".into(), gone: true }],
                ..Default::default()
            };
            s.sync("h1", &Account { favs: vec!["a".into()], ..Default::default() }, &deletion);
        }
        // the tombstone must be on disk, not just in memory
        let s = open(p);
        let stale = Account { favs: vec!["a".into(), "b".into()], ..Default::default() };
        let out = s.sync("h1", &stale, &ChangeSets::default()).unwrap();
        assert_eq!(out.favs, vec!["a".to_string()]);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /Users/vchub/dev/projects/world-radio/sync && cargo test store::`
Expected: FAIL — `no method named sync`, `cannot find type ChangeSets`.

- [ ] **Step 3: Write minimal implementation**

In `src/store.rs`:

Add to the imports at the top:

```rust
use crate::merge::{merge, present_ids, prune, Change, Record};
```

Extend `Account` (currently lines 5-11) with the record fields, keeping the legacy ones:

```rust
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
pub struct Account {
    pub favs: Vec<String>,
    pub blocked: Vec<String>,
    #[serde(default)]
    pub excluded_countries: Vec<String>,
    // the timestamped view. omitted from responses when empty so an old client
    // sees exactly the payload it saw before.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fav_records: Vec<Record>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_records: Vec<Record>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub country_records: Vec<Record>,
}

#[derive(Deserialize, Default, Debug)]
pub struct ChangeSets {
    #[serde(default)]
    pub favs: Vec<Change>,
    #[serde(default)]
    pub blocked: Vec<Change>,
    #[serde(default)]
    pub excluded_countries: Vec<Change>,
}
```

In `open()`, after the existing `excluded_countries` migration block, add the same pattern for the three record columns:

```rust
    for col in ["fav_records", "blocked_records", "country_records"] {
        let sql = format!(
            "SELECT 1 FROM pragma_table_info('accounts') WHERE name='{col}'"
        );
        let has: bool = conn
            .prepare(&sql)
            .and_then(|mut s| s.exists([]))
            .unwrap_or(false);
        if !has {
            conn.execute_batch(&format!(
                "ALTER TABLE accounts ADD COLUMN {col} TEXT NOT NULL DEFAULT '[]';"
            ))
            .expect("migrate records column");
        }
    }
```

Add these helpers and the `sync` method to `impl Store`, next to `replace`:

```rust
    fn read_records(&self, key_hash: &str) -> (Vec<Record>, Vec<Record>, Vec<Record>) {
        let c = self.conn.lock().unwrap();
        let parse = |s: String| serde_json::from_str::<Vec<Record>>(&s).unwrap_or_default();
        c.query_row(
            "SELECT fav_records, blocked_records, country_records FROM accounts WHERE key_hash=?1",
            rusqlite::params![key_hash],
            |r| {
                Ok((
                    parse(r.get::<_, String>(0)?),
                    parse(r.get::<_, String>(1)?),
                    parse(r.get::<_, String>(2)?),
                ))
            },
        )
        .unwrap_or_default()
    }

    /// merge-aware replacement for [`Store::replace`]. the stored records are the
    /// authority on what was deleted; the client's arrays only assert what it
    /// still holds, which on its own cannot express a removal.
    pub fn sync(
        &self,
        key_hash: &str,
        incoming: &Account,
        changed: &ChangeSets,
    ) -> Option<Account> {
        self.create_account(key_hash);
        let (sf, sb, sc) = self.read_records(key_hash);
        let ts = now();

        // migrate on first touch: rows written before this feature have empty
        // record columns but populated legacy ones.
        let seed = |recs: Vec<Record>, legacy: &[String]| -> Vec<Record> {
            match recs.is_empty() {
                false => recs,
                true => legacy
                    .iter()
                    .map(|id| Record { id: id.clone(), at: 0, gone: false })
                    .collect(),
            }
        };
        let stored_now = self.get(key_hash).unwrap_or_default();
        let sf = seed(sf, &stored_now.favs);
        let sb = seed(sb, &stored_now.blocked);
        let sc = seed(sc, &stored_now.excluded_countries);

        let favs = prune(&merge(&sf, &incoming.favs, &changed.favs, ts), ts);
        let blocked = prune(&merge(&sb, &incoming.blocked, &changed.blocked, ts), ts);
        let countries = prune(
            &merge(&sc, &incoming.excluded_countries, &changed.excluded_countries, ts),
            ts,
        );

        let out = Account {
            favs: present_ids(&favs),
            blocked: present_ids(&blocked),
            excluded_countries: present_ids(&countries),
            fav_records: favs,
            blocked_records: blocked,
            country_records: countries,
        };

        let c = self.conn.lock().unwrap();
        c.execute(
            "UPDATE accounts SET favs=?1, blocked=?2, excluded_countries=?3, \
             fav_records=?4, blocked_records=?5, country_records=?6, updated_at=?7 \
             WHERE key_hash=?8",
            rusqlite::params![
                serde_json::to_string(&out.favs).unwrap(),
                serde_json::to_string(&out.blocked).unwrap(),
                serde_json::to_string(&out.excluded_countries).unwrap(),
                serde_json::to_string(&out.fav_records).unwrap(),
                serde_json::to_string(&out.blocked_records).unwrap(),
                serde_json::to_string(&out.country_records).unwrap(),
                ts,
                key_hash
            ],
        )
        .ok()?;
        Some(out)
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd /Users/vchub/dev/projects/world-radio/sync && cargo test`
Expected: PASS — 3 new store tests, 9 merge tests, 7 pre-existing.

- [ ] **Step 5: Commit**

```bash
cd /Users/vchub/dev/projects/world-radio/sync
git add src/store.rs
git commit -m "keep a per-item history so a sync can tell a removal from a stale copy"
```

---

### Task 3: Wire the endpoint

**Files:**
- Modify: `/Users/vchub/dev/projects/world-radio/sync/src/main.rs:89-101` (`put_sync`)

**Interfaces:**
- Consumes: `store::{Account, ChangeSets}`, `Store::sync` (Task 2).
- Produces: `PUT /sync` accepts a body with an optional `changed` object; the response carries both the legacy arrays and the records.

- [ ] **Step 1: Write the implementation**

There is no unit test here — `main.rs` has none today (`grep -c '#\[test\]' src/main.rs` returns 0) and the handler is a three-line adapter over `Store::sync`, which Task 2 covers. Task 5 exercises this path end to end against a running server.

Replace `put_sync` (lines 89-101) with:

```rust
#[derive(serde::Deserialize)]
struct SyncBody {
    #[serde(flatten)]
    account: store::Account,
    // absent for clients built before deletions were tracked; an empty delta
    // then means "nothing changed here", which is exactly right for them.
    #[serde(default)]
    changed: store::ChangeSets,
}

async fn put_sync(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<SyncBody>,
) -> impl IntoResponse {
    match bearer(&headers) {
        None => StatusCode::UNAUTHORIZED.into_response(),
        Some(k) => match s.store.sync(&key::hash_key(&k), &body.account, &body.changed) {
            None => StatusCode::UNAUTHORIZED.into_response(),
            Some(merged) => Json(merged).into_response(),
        },
    }
}
```

- [ ] **Step 2: Verify it compiles and the suite is green**

Run: `cd /Users/vchub/dev/projects/world-radio/sync && cargo build && cargo test && cargo clippy --all-targets`
Expected: build OK, all tests pass, no clippy errors.

- [ ] **Step 3: Prove the old wire format still works, against a real server**

```bash
cd /Users/vchub/dev/projects/world-radio/sync
cargo run &
sleep 3
KEY=$(curl -s -X POST http://127.0.0.1:8138/account | python3 -c "import json,sys; print(json.load(sys.stdin)['key'])")
# old-format request: no `changed` field at all
curl -s -X PUT http://127.0.0.1:8138/sync -H "Authorization: Bearer $KEY" \
  -H 'Content-Type: application/json' \
  -d '{"favs":["a","b"],"blocked":[],"excluded_countries":["DE"]}'
echo
# new-format request deleting "b"
curl -s -X PUT http://127.0.0.1:8138/sync -H "Authorization: Bearer $KEY" \
  -H 'Content-Type: application/json' \
  -d '{"favs":["a"],"blocked":[],"excluded_countries":["DE"],"changed":{"favs":[{"id":"b","gone":true}]}}'
echo
# the resurrection attempt: old-format push that still lists "b"
curl -s -X PUT http://127.0.0.1:8138/sync -H "Authorization: Bearer $KEY" \
  -H 'Content-Type: application/json' \
  -d '{"favs":["a","b"],"blocked":[],"excluded_countries":["DE"]}'
echo
kill %1
```

Expected: the first response lists `"favs":["a","b"]`; the second `"favs":["a"]`; the third **still** `"favs":["a"]` — `b` stays deleted. If the port is taken, check `src/main.rs` for the bind address rather than guessing.

- [ ] **Step 4: Commit**

```bash
cd /Users/vchub/dev/projects/world-radio/sync
git add src/main.rs
git commit -m "accept a list of what changed so deletions reach the other device"
```

---

### Task 4: CLI — record what changed and send it

**Files:**
- Create: `/Users/vchub/dev/projects/world-radio/radio/crates/radio-core/src/sync/pending.rs`
- Modify: `/Users/vchub/dev/projects/world-radio/radio/crates/radio-core/src/sync/mod.rs` (add `mod pending;` and re-export)
- Modify: `/Users/vchub/dev/projects/world-radio/radio/crates/radio-core/src/sync/client.rs:4-9` (`SyncData`) and `:53-63` (`push`)

**Interfaces:**
- Produces:
  - `pub struct Change { pub id: String, pub gone: bool }` — derives `Serialize, Deserialize, Debug, Clone, PartialEq`
  - `pub struct Pending { pub favs: Vec<Change>, pub blocked: Vec<Change>, pub excluded_countries: Vec<Change> }` — derives `Serialize, Deserialize, Debug, Default, Clone, PartialEq`
  - `impl Pending`: `pub fn note(&mut self, set: Set, id: &str, gone: bool)`, `pub fn load(path: &Path) -> Pending`, `pub fn save(&self, path: &Path) -> std::io::Result<()>`, `pub fn is_empty(&self) -> bool`, `pub fn clear(&mut self)`
  - `pub enum Set { Favs, Blocked, Countries }`
  - `SyncData` gains `#[serde(default, skip_serializing_if = "Pending::is_empty")] pub changed: Pending`

- [ ] **Step 1: Write the failing test**

Create `crates/radio-core/src/sync/pending.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_records_a_deletion() {
        let mut p = Pending::default();
        p.note(Set::Favs, "a", true);
        assert_eq!(p.favs, vec![Change { id: "a".into(), gone: true }]);
    }

    #[test]
    fn the_latest_action_on_an_id_replaces_the_earlier_one() {
        // star, un-star, star again between two syncs is one net add — sending
        // the whole history would let the server apply them out of order.
        let mut p = Pending::default();
        p.note(Set::Favs, "a", false);
        p.note(Set::Favs, "a", true);
        p.note(Set::Favs, "a", false);
        assert_eq!(p.favs, vec![Change { id: "a".into(), gone: false }]);
    }

    #[test]
    fn sets_are_kept_apart() {
        let mut p = Pending::default();
        p.note(Set::Favs, "a", true);
        p.note(Set::Blocked, "a", false);
        p.note(Set::Countries, "DE", true);
        assert_eq!(p.favs.len(), 1);
        assert_eq!(p.blocked.len(), 1);
        assert_eq!(p.excluded_countries.len(), 1);
    }

    #[test]
    fn is_empty_is_true_only_with_no_entries_at_all() {
        let mut p = Pending::default();
        assert!(p.is_empty());
        p.note(Set::Countries, "DE", true);
        assert!(!p.is_empty());
        p.clear();
        assert!(p.is_empty());
    }

    #[test]
    fn it_round_trips_through_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sync_pending.json");
        let mut p = Pending::default();
        p.note(Set::Favs, "a", true);
        p.save(&path).unwrap();
        assert_eq!(Pending::load(&path), p);
    }

    #[test]
    fn a_missing_or_corrupt_file_loads_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope.json");
        assert!(Pending::load(&missing).is_empty());
        let bad = dir.path().join("bad.json");
        std::fs::write(&bad, "not json").unwrap();
        assert!(Pending::load(&bad).is_empty());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /Users/vchub/dev/projects/world-radio/radio && cargo test -p radio-core pending::`
Expected: FAIL — `cannot find type Pending`.

- [ ] **Step 3: Write minimal implementation**

Prepend to `pending.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Change {
    pub id: String,
    pub gone: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum Set {
    Favs,
    Blocked,
    Countries,
}

/// what this device changed since its last successful sync. the server needs it
/// because a plain list of what we still hold cannot express a removal.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct Pending {
    #[serde(default)]
    pub favs: Vec<Change>,
    #[serde(default)]
    pub blocked: Vec<Change>,
    #[serde(default)]
    pub excluded_countries: Vec<Change>,
}

impl Pending {
    pub fn note(&mut self, set: Set, id: &str, gone: bool) {
        let list = match set {
            Set::Favs => &mut self.favs,
            Set::Blocked => &mut self.blocked,
            Set::Countries => &mut self.excluded_countries,
        };
        list.retain(|c| c.id != id);
        list.push(Change { id: id.to_string(), gone });
    }

    pub fn is_empty(&self) -> bool {
        self.favs.is_empty() && self.blocked.is_empty() && self.excluded_countries.is_empty()
    }

    pub fn clear(&mut self) {
        self.favs.clear();
        self.blocked.clear();
        self.excluded_countries.clear();
    }

    pub fn load(path: &Path) -> Pending {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let body = serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string());
        std::fs::write(path, body)
    }
}
```

In `crates/radio-core/src/sync/mod.rs`, add `mod pending;` and `pub use pending::{Change, Pending, Set};` alongside the existing exports.

In `client.rs`, extend `SyncData` (lines 4-9) and `push`:

```rust
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct SyncData {
    pub favs: Vec<String>,
    pub blocked: Vec<String>,
    #[serde(default)]
    pub excluded_countries: Vec<String>,
    // omitted entirely when there is nothing pending, so the request stays
    // byte-identical to the old format for an unchanged device.
    #[serde(default, skip_serializing_if = "Pending::is_empty")]
    pub changed: Pending,
}
```

Add `use crate::sync::Pending;` to `client.rs` if it is not already in scope. `push` needs no change — it serialises `SyncData` as a whole — but confirm the existing `SyncData` literals in the codebase still compile; add `..Default::default()` where a literal now misses the field.

- [ ] **Step 4: Run test to verify it passes**

Run: `cd /Users/vchub/dev/projects/world-radio/radio && cargo test -p radio-core sync::`
Expected: PASS — 6 new pending tests plus the existing client tests.

- [ ] **Step 5: Confirm the wire format is unchanged when nothing is pending**

Run: `cd /Users/vchub/dev/projects/world-radio/radio && cargo test -p radio-core sync::client::tests`
Expected: PASS. The existing `push_returns_server_state_verbatim` test asserts the old behaviour; it must not need editing.

- [ ] **Step 6: Commit**

```bash
cd /Users/vchub/dev/projects/world-radio/radio
git add crates/radio-core/src/sync/
git commit -m "cli: remember what you changed between syncs"
```

---

### Task 5: CLI — feed the log from the mutation points, and prove it end to end

**Files:**
- Modify: `/Users/vchub/dev/projects/world-radio/radio/crates/radio-core/src/catalog/catalog.rs` — `set_excluded_countries` (line 45), `toggle_blacklist` (line 101), `toggle_favorite` (line 141)
- Modify: the CLI sync command path (`crates/radio-tui/src/sync_cmd.rs`) to load, send and clear the log

**Interfaces:**
- Consumes: `Pending`, `Set`, `Change` (Task 4); `SyncData.changed` (Task 4); the server from Tasks 1-3.

- [ ] **Step 1: Record each mutation**

The three mutation points are one-liners delegating to a `Favorites` set, and `toggle` returns the new membership:

```rust
    pub fn toggle_favorite(&mut self, uuid: &str) -> bool {
        self.favorites.toggle(uuid)
    }

    pub fn toggle_blacklist(&mut self, uuid: &str) -> bool {
        self.blacklist.toggle(uuid)
    }
```

Add a `pub pending: Pending` field to `Catalog` (initialised `Pending::default()` wherever the struct is built — grep for the constructor), `use crate::sync::{Pending, Set};` at the top, and record the outcome. `toggle` returns `true` when the item is now IN the set, so `gone` is its negation:

```rust
    pub fn toggle_favorite(&mut self, uuid: &str) -> bool {
        let now_in = self.favorites.toggle(uuid);
        self.pending.note(Set::Favs, uuid, !now_in);
        now_in
    }

    pub fn toggle_blacklist(&mut self, uuid: &str) -> bool {
        let now_in = self.blacklist.toggle(uuid);
        self.pending.note(Set::Blocked, uuid, !now_in);
        now_in
    }
```

`set_excluded_countries` replaces the whole set, so it has to diff. Capture the old ids before overwriting:

```rust
    pub fn set_excluded_countries(&mut self, codes: Vec<String>) {
        let before: Vec<String> = self.excluded_countries.ids().to_vec();
        let mut f = Favorites::new();
        for code in codes {
            let up = code.to_uppercase();
            if !f.contains(&up) {
                f.toggle(&up);
            }
        }
        // a wholesale replace hides both directions; the sync log needs each one
        // named, because an id that merely stopped being listed is not a deletion.
        for old in &before {
            if !f.contains(old) {
                self.pending.note(Set::Countries, old, true);
            }
        }
        for new in f.ids() {
            if !before.contains(new) {
                self.pending.note(Set::Countries, new, false);
            }
        }
        self.excluded_countries = f;
    }
```

Keep the rest of the original body as it stands — read lines 45-60 and preserve whatever follows the loop; only the diff block and the `before` capture are new.

- [ ] **Step 3: Write the failing test for the country diff**

The country diff is the only non-trivial logic here, so it gets a test. Add to the existing test module in `catalog.rs`:

```rust
    #[test]
    fn changing_the_country_filter_records_both_directions() {
        let mut c = test_catalog();
        c.set_excluded_countries(vec!["DE".into(), "FR".into()]);
        c.set_excluded_countries(vec!["FR".into(), "IT".into()]);
        // DE was removed, IT was added, FR was untouched by the second call
        let mut got: Vec<(String, bool)> = c
            .pending
            .excluded_countries
            .iter()
            .map(|ch| (ch.id.clone(), ch.gone))
            .collect();
        got.sort();
        assert_eq!(
            got,
            vec![("DE".to_string(), true), ("FR".to_string(), false), ("IT".to_string(), false)]
        );
    }
```

Use whatever constructor the existing tests in that file use in place of `test_catalog()` — read them first.

- [ ] **Step 4: Run it, implement, run again**

Run: `cd /Users/vchub/dev/projects/world-radio/radio && cargo test -p radio-core catalog::`
Expected: FAIL first, then PASS after Step 2's implementation.

- [ ] **Step 5: Send and clear the log on a successful sync**

`run_sync` (`crates/radio-tui/src/sync_cmd.rs:141`) currently builds a bare `SyncData`. Add the pending log, and clear it **only after** `push` succeeds — a failed sync that dropped the log would lose the deletion silently:

```rust
    let pending_path = pending_path();
    let pending = Pending::load(&pending_path);
    let local = SyncData {
        favs: favs.ids().to_vec(),
        blocked: blocked.ids().to_vec(),
        excluded_countries: excluded.ids().to_vec(),
        changed: pending,
    };
    let merged = client().push(&key, &local)?;
    // only now: the server has the delta, so replaying it would be wrong.
    Pending::default().save(&pending_path)?;
```

Add the path helper beside the existing `fav_path()` / `blacklist_path()` / `excluded_path()` (lines 33-43), following whatever they do:

```rust
fn pending_path() -> std::path::PathBuf {
    radio_core::paths::data_dir().join("sync_pending.json")
}
```

Read lines 33-43 first and match their exact style — if they use a different helper than `paths::data_dir()`, use that one.

- [ ] **Step 5b: Delete the CLI's link-time union**

`merge_on_link` and `union_ids` (`sync_cmd.rs:167-183`) union local and server state, which is the same resurrection bug the server now fixes — a deletion held by the server is undone by the linking device. With the server merging authoritatively, the client must send its own state plus its delta and take the answer.

Find the caller of `merge_on_link` (`grep -n merge_on_link crates/radio-tui/src/sync_cmd.rs`), replace it with the same push-and-accept shape `run_sync` uses, then delete both functions and any now-unused tests for them.

Run `cargo clippy --workspace --all-targets` afterwards: an orphaned helper shows up as a dead-code warning, which is the check that the removal was complete.

- [ ] **Step 6: End-to-end against a local server**

```bash
cd /Users/vchub/dev/projects/world-radio/sync && cargo run &
sleep 3
cd /Users/vchub/dev/projects/world-radio/radio && cargo build --release
# point the CLI at the local server, create a key, star something, sync,
# un-star, sync, then push stale state from a second data dir and confirm
# the un-star survives.
```

The CLI's server URL is hardcoded to `https://r4dio.net` in `crates/radio-core/src/sync/client.rs`. Add a `R4DIO_SYNC_URL` environment override **as part of this step** — one line in `SyncClient::new`, defaulting to the current constant — rather than editing the constant by hand for the test and reverting it.

Expected: the un-starred station does not come back.

- [ ] **Step 7: Full suite, format, lint, commit**

```bash
cd /Users/vchub/dev/projects/world-radio/radio
cargo fmt && cargo test && cargo clippy --workspace --all-targets
git add -A crates
git commit -m "cli: an unstarred station stays unstarred on your other devices"
```

---

## Self-Review

**Spec coverage:**
- Record shape (`id`/`at`/`gone`) and the merge rule → Task 1
- Server-side timestamping → Task 1 (`now` is a parameter, supplied by the store in Task 2)
- Tie goes to the deletion → Task 1, `a_deletion_wins_a_tie_on_timestamp`
- Re-adding after deletion works → Task 1, `re_adding_after_a_deletion_wins_because_it_is_newer`
- 90-day tombstone pruning → Task 1 (`prune`, `TOMBSTONE_TTL`), applied in Task 2
- Dual wire format, old clients unaffected → Task 2 (`skip_serializing_if`), Task 3 (`#[serde(default)] changed`), verified in Task 3 Step 3
- Migration of existing rows → Task 2 (`seed`, plus the `pragma_table_info` column migration)
- CLI operation log in `sync_pending.json` → Tasks 4 and 5
- Client-side union removal → Task 5b for the CLI. **Correction to the spec:** the spec says only
  Android unions, but the CLI has the same bug — `merge_on_link` / `union_ids` at
  `sync_cmd.rs:167-183`, found while writing this plan. The CLI's `run_sync` was already correct,
  which is what the spec's "the CLI already accepts the server response verbatim" refers to.
  Android's `SyncMerge.mergedData` remains out of scope, in the separate Android plan.

**Type consistency:** `Record`/`Change` on the server (`merge.rs`) and `Change`/`Pending` on the CLI (`pending.rs`) are separate types in separate repos that must serialise compatibly. Both use `{"id": ..., "gone": ...}`; the server never deserialises `at` from a client, and the CLI never sends it. Task 3's curl check is what proves the two agree on the wire.

**Placeholder scan:** clean. Every code step carries literal code. Three steps ask the implementer to
read specific line ranges before editing (`set_excluded_countries`'s tail, the `*_path()` helpers,
the `merge_on_link` caller) — those are bounded lookups of code this plan quotes the surroundings
of, not deferred decisions.
