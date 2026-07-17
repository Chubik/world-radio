# Sync Replace-Semantics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A removed favourite (or blocked/excluded entry) stays removed across sync instead of reappearing, by switching the sync protocol from union-merge to replace, with a one-time union only when linking to an existing account.

**Architecture:** The server (`world-radio-sync`, a separate repo at `../sync`) becomes a dumb key/value store: `PUT /sync` replaces stored state with the request body. All sync policy moves to the two clients. Normal edits push a full replacement. Joining an existing account runs `pull → union(local, server) → push` once on the client.

**Tech Stack:** Rust (server `../sync`, CLI `radio-core`/`radio-tui`), Kotlin (Android), axum + rusqlite (server), reqwest (CLI), okhttp (Android).

## Global Constraints

- No comments in code unless explicitly requested.
- All code output (logs, messages) in English, lowercase.
- No `else if` constructs.
- No AI/Claude/personal mentions anywhere.
- Commit to `dev` branch only; commit subjects are the public changelog — write them for users.
- Version is CI-owned; never hand-edit `Cargo.toml` / `build.gradle.kts` version fields.
- `../sync` is a SEPARATE git repo — its commits and deploy are independent of the app release. Deploy manually: pull as `deployer`, `docker compose build && up -d`.
- Test Android/CLI on the emulator before release; never touch the user's real data dir.

---

## File Structure

**Server repo `../sync`:**
- `src/store.rs` — rename `merge` → `replace`, drop `union` in the write path (keep `union` fn only if still used; it will not be). Update tests.
- `src/main.rs:96` — call site `store.merge` → `store.replace`.

**CLI (`radio-core` + `radio-tui`, this repo):**
- `crates/radio-core/src/catalog/favorites.rs` — add `Favorites::set_from(&mut self, ids)`.
- `crates/radio-core/src/catalog/catalog.rs` — add `set_favorites`, `set_blacklist`.
- `crates/radio-tui/src/tui/worker.rs:194` — `handle_sync`: apply server response as replacement (no union hydration loops).
- `crates/radio-tui/src/sync_cmd.rs` — add `SyncCmd::Use { key }` + `link_and_merge`; `run_sync` applies response verbatim.
- `crates/radio-core/src/sync/client.rs` — rewrite `push_returns_merged` test to replace semantics.

**Android (this repo):**
- `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt:191` — `syncNow` applies response verbatim (already does; server change makes it correct — add a regression test only).
- `android/app/src/main/kotlin/net/vchub/r4dio/SyncActivity.kt` — merge-on-link at scan (`:37`) and paste (`:80`).
- `android/app/src/main/kotlin/net/vchub/r4dio/FavStore.kt` — `linkMerge` helper using `SyncMerge.mergedFavs`.
- Tests: `FavStoreSyncLogicTest.kt`, `SyncClientTest.kt`.

---

## Task 1: Server — replace instead of union (repo `../sync`)

**Files:**
- Modify: `../sync/src/store.rs:87-111` (rename `merge`→`replace`, body becomes replace)
- Modify: `../sync/src/main.rs:96` (call site)
- Test: `../sync/src/store.rs` tests module — replace the three `merge_*` tests (`merge_unions_both_sets`, `merge_unknown_key_creates_account`, `merge_unions_excluded_countries`) with replace versions

**Interfaces:**
- Produces: `Store::replace(&self, key_hash: &str, incoming: &Account) -> Option<Account>` — stores `incoming` verbatim (upsert-recreates the row if missing), returns the stored `Account`.

Note: the tests module uses `tempfile::tempdir()` + `open(path)` (there is no `open_in_memory`), and the `acc(&[&str], &[&str], &[&str])` helper already exists. Follow that pattern exactly.

- [ ] **Step 1: Rewrite the failing tests**

In `../sync/src/store.rs` tests, DELETE the three tests `merge_unions_both_sets`, `merge_unknown_key_creates_account`, and `merge_unions_excluded_countries`, and add:

```rust
    #[test]
    fn replace_overwrites_both_sets() {
        let dir = tempfile::tempdir().unwrap();
        let s = open(dir.path().join("t.db").to_str().unwrap());
        s.create_account("h1");
        s.replace("h1", &acc(&["a", "b"], &["x"], &[])).unwrap();
        let out = s.replace("h1", &acc(&["b"], &[], &[])).unwrap();
        assert_eq!(out.favs, vec!["b"]);
        assert!(out.blocked.is_empty());
        assert_eq!(s.get("h1").unwrap().favs, vec!["b".to_string()]);
    }

    #[test]
    fn replace_unknown_key_creates_account() {
        let dir = tempfile::tempdir().unwrap();
        let s = open(dir.path().join("t.db").to_str().unwrap());
        let out = s.replace("nope", &acc(&["a"], &[], &[])).unwrap();
        assert_eq!(out.favs, vec!["a".to_string()]);
        assert_eq!(s.get("nope").unwrap().favs, vec!["a".to_string()]);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd ../sync && cargo test replace_ 2>&1 | tail -20`
Expected: FAIL — `no method named replace found for struct Store`.

- [ ] **Step 3: Rename `merge` → `replace` and drop union in the write path**

In `../sync/src/store.rs`, replace the `merge` fn (lines 87-111) with:

```rust
    pub fn replace(&self, key_hash: &str, incoming: &Account) -> Option<Account> {
        // upsert: a valid key must never be orphaned by missing server state, so
        // recreate the row from the client copy if it's gone, then store verbatim.
        self.create_account(key_hash);
        let c = self.conn.lock().unwrap();
        c.execute(
            "UPDATE accounts SET favs=?1, blocked=?2, excluded_countries=?3, updated_at=?4 WHERE key_hash=?5",
            rusqlite::params![
                serde_json::to_string(&incoming.favs).unwrap(),
                serde_json::to_string(&incoming.blocked).unwrap(),
                serde_json::to_string(&incoming.excluded_countries).unwrap(),
                now(),
                key_hash
            ],
        )
        .ok()?;
        Some(incoming.clone())
    }
```

- [ ] **Step 4: Update the call site**

In `../sync/src/main.rs:96`, change `s.store.merge(` to `s.store.replace(`.

- [ ] **Step 5: Remove the now-unused `union` fn and its test**

First confirm nothing else uses it: `grep -rn "union(" ../sync/src`. Expected: only the definition (`store.rs:13`) and the `union_dedups_and_sorts` test remain (the three `merge_*` tests were deleted in Step 1). Delete `pub fn union(...)` and the `union_dedups_and_sorts` test. If the grep shows another caller, keep `union` and skip this step.

- [ ] **Step 6: Run the full server test suite**

Run: `cd ../sync && cargo test 2>&1 | tail -20`
Expected: PASS, no warnings about unused `union`/`merge`.

- [ ] **Step 7: Commit (in the `../sync` repo)**

```bash
cd ../sync && git add -A && git commit -m "fix(sync): PUT /sync replaces stored state instead of unioning, so removals propagate"
```

Note: do NOT deploy yet — deploy is a separate manual step after the whole feature is verified.

---

## Task 2: CLI core — `set_from` on Favorites + set methods on Catalog

**Files:**
- Modify: `crates/radio-core/src/catalog/favorites.rs`
- Modify: `crates/radio-core/src/catalog/catalog.rs`
- Test: `crates/radio-core/src/catalog/favorites.rs` tests module

**Interfaces:**
- Produces: `Favorites::set_from(&mut self, ids: Vec<String>)` — replaces contents with `ids` (dedup, preserve given order).
- Produces: `Catalog::set_favorites(&mut self, ids: Vec<String>)`, `Catalog::set_blacklist(&mut self, ids: Vec<String>)`.

- [ ] **Step 1: Write the failing test**

In `crates/radio-core/src/catalog/favorites.rs` tests, add:

```rust
    #[test]
    fn set_from_replaces_contents() {
        let mut f = Favorites::new();
        f.toggle("old1");
        f.toggle("old2");
        f.set_from(vec!["new1".into(), "new2".into(), "new1".into()]);
        assert_eq!(f.ids(), &["new1".to_string(), "new2".to_string()]);
        assert!(!f.contains("old1"));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p radio-core set_from_replaces_contents 2>&1 | tail -15`
Expected: FAIL — `no method named set_from`.

- [ ] **Step 3: Implement `set_from`**

In `crates/radio-core/src/catalog/favorites.rs`, inside `impl Favorites` (after `toggle`):

```rust
    pub fn set_from(&mut self, ids: Vec<String>) {
        self.ids.clear();
        self.set.clear();
        for id in ids {
            if self.set.insert(id.clone()) {
                self.ids.push(id);
            }
        }
    }
```

- [ ] **Step 4: Add Catalog set methods**

In `crates/radio-core/src/catalog/catalog.rs`, after `favorite_ids` (line 148) and near `blacklist_ids`:

```rust
    pub fn set_favorites(&mut self, ids: Vec<String>) {
        self.favorites.set_from(ids);
    }

    pub fn set_blacklist(&mut self, ids: Vec<String>) {
        self.blacklist.set_from(ids);
    }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p radio-core set_from_replaces_contents 2>&1 | tail -15 && cargo build -p radio-core 2>&1 | tail -5`
Expected: test PASS, build OK.

- [ ] **Step 6: Commit**

```bash
git add crates/radio-core/src/catalog/favorites.rs crates/radio-core/src/catalog/catalog.rs
git commit -m "feat(core): set_from/set_favorites/set_blacklist to replace lists wholesale for sync"
```

---

## Task 3: CLI worker — apply sync response as replacement

**Files:**
- Modify: `crates/radio-tui/src/tui/worker.rs:194-244` (`handle_sync`)
- Test: `crates/radio-tui/src/tui/worker.rs` (add a unit test module if none; otherwise inline)

**Interfaces:**
- Consumes: `Catalog::set_favorites`, `Catalog::set_blacklist`, `Catalog::set_excluded_countries` (existing), `SyncClient::push`.

- [ ] **Step 1: Rewrite `handle_sync` to replace, not hydrate**

In `crates/radio-tui/src/tui/worker.rs`, replace the union-hydration block (the `for uuid in &merged.favs { ... }` and `for uuid in &merged.blocked { ... }` loops, lines 221-230) with:

```rust
    catalog.set_favorites(merged.favs.clone());
    catalog.set_blacklist(merged.blocked.clone());
    catalog.set_excluded_countries(merged.excluded_countries.clone());
    save_all(catalog, paths);
```

Leave the `ExcludedCountriesChanged` message and the `announce` notice below it unchanged. Remove the now-redundant standalone `set_excluded_countries` call at old line 231 (it is now inside the block above — ensure it appears exactly once).

- [ ] **Step 2: Build to confirm it compiles**

Run: `cargo build -p radio-tui 2>&1 | tail -10`
Expected: OK.

- [ ] **Step 3: Verify existing worker tests still pass**

Run: `cargo test -p radio-tui 2>&1 | tail -20`
Expected: PASS. If `toggle_favorite_from_browse_emits_toggle_and_savestate_no_loadfav` exists and still passes, good — the UI toggle path is unchanged.

- [ ] **Step 4: Commit**

```bash
git add crates/radio-tui/src/tui/worker.rs
git commit -m "fix(tui): apply sync result as a replacement so an unfavourited station stays removed"
```

---

## Task 4: CLI client test — replace semantics

**Files:**
- Modify: `crates/radio-core/src/sync/client.rs:127-146` (`push_returns_merged`)

**Interfaces:**
- Consumes: `SyncClient::push` (unchanged signature).

- [ ] **Step 1: Rewrite the mock test to replace semantics**

In `crates/radio-core/src/sync/client.rs`, replace `push_returns_merged` with:

```rust
    #[test]
    fn push_returns_server_state_verbatim() {
        let mut server = mockito::Server::new();
        server
            .mock("PUT", "/sync")
            .with_body(r#"{"favs":["c"],"blocked":[]}"#)
            .create();
        let c = SyncClient::new(server.url());
        let d = c
            .push(
                "r4-k",
                &SyncData {
                    favs: vec!["c".into()],
                    blocked: vec![],
                    excluded_countries: vec![],
                },
            )
            .unwrap();
        assert_eq!(d.favs, vec!["c".to_string()]);
    }
```

- [ ] **Step 2: Run it**

Run: `cargo test -p radio-core push_returns_server_state_verbatim 2>&1 | tail -15`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/radio-core/src/sync/client.rs
git commit -m "test(core): sync client push returns server state verbatim (replace, not union)"
```

---

## Task 5: CLI — `sync use <key>` command with merge-on-link

**Files:**
- Modify: `crates/radio-tui/src/sync_cmd.rs`
- Test: `crates/radio-tui/src/sync_cmd.rs` tests module

**Interfaces:**
- Consumes: `sync::store_key`, `SyncClient::pull`, `SyncClient::push`, `Favorites::load/save`, `sync::is_valid_format`.
- Produces: `fn merge_on_link(local: SyncData, server: SyncData) -> SyncData` — per-field union preserving local order then appending server-only ids. Pure fn for testing.

Note: `run_sync` (`sync_cmd.rs:139`) already saves the pushed result via `favorites_from(merged.*).save(...)`, which REPLACES each file wholesale — so once the server does replace, `run_sync` is already correct. Do NOT change it.

- [ ] **Step 1: Write the failing pure-merge test**

In `crates/radio-tui/src/sync_cmd.rs` tests module, add:

```rust
    #[test]
    fn merge_on_link_unions_each_field() {
        let local = SyncData { favs: vec!["a".into(), "b".into()], blocked: vec![], excluded_countries: vec![] };
        let server = SyncData { favs: vec!["b".into(), "c".into()], blocked: vec!["x".into()], excluded_countries: vec!["US".into()] };
        let m = merge_on_link(local, server);
        assert_eq!(m.favs, vec!["a".to_string(), "b".into(), "c".into()]);
        assert_eq!(m.blocked, vec!["x".to_string()]);
        assert_eq!(m.excluded_countries, vec!["US".to_string()]);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p radio-tui merge_on_link_unions_each_field 2>&1 | tail -15`
Expected: FAIL — `cannot find function merge_on_link`.

- [ ] **Step 3: Implement `merge_on_link` (pure) + a `union_ids` helper**

In `crates/radio-tui/src/sync_cmd.rs`:

```rust
fn union_ids(local: &[String], server: &[String]) -> Vec<String> {
    let mut out = local.to_vec();
    for id in server {
        match out.contains(id) {
            true => {}
            false => out.push(id.clone()),
        }
    }
    out
}

fn merge_on_link(local: SyncData, server: SyncData) -> SyncData {
    SyncData {
        favs: union_ids(&local.favs, &server.favs),
        blocked: union_ids(&local.blocked, &server.blocked),
        excluded_countries: union_ids(&local.excluded_countries, &server.excluded_countries),
    }
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p radio-tui merge_on_link_unions_each_field 2>&1 | tail -15`
Expected: PASS.

- [ ] **Step 5: Add the `Use` subcommand variant**

In `crates/radio-tui/src/sync_cmd.rs`, in `enum SyncCmd`, add after `Delete`:

```rust
    Use { key: String },
```

And in `run`'s match, add:

```rust
        SyncCmd::Use { key } => use_key(key),
```

- [ ] **Step 6: Implement `use_key` (store + pull + merge + push replace + save)**

Add:

```rust
fn use_key(key: &str) -> anyhow::Result<()> {
    match sync::is_valid_format(key) {
        false => {
            println!("invalid key");
            return Ok(());
        }
        true => {}
    }
    sync::store_key(key)?;
    let local = SyncData {
        favs: Favorites::load(&fav_path()).ids().to_vec(),
        blocked: Favorites::load(&blacklist_path()).ids().to_vec(),
        excluded_countries: Favorites::load(&excluded_path()).ids().to_vec(),
    };
    let server = client().pull(key)?;
    let merged = merge_on_link(local, server);
    let stored = client().push(key, &merged)?;
    favorites_from(stored.favs.clone()).save(&fav_path())?;
    favorites_from(stored.blocked.clone()).save(&blacklist_path())?;
    favorites_from(stored.excluded_countries.clone()).save(&excluded_path())?;
    println!(
        "linked and merged: {} favourites, {} blocked, {} excluded countries",
        stored.favs.len(),
        stored.blocked.len(),
        stored.excluded_countries.len()
    );
    Ok(())
}
```

- [ ] **Step 7: Confirm `is_valid_format` exists and is exported**

Run: `grep -rn "pub fn is_valid_format" crates/radio-core/src/sync/`
Expected: a match in `key.rs`. It is re-exported via `radio_core::sync::is_valid_format` (see `sync/mod.rs:5`). If the signature differs, adapt the call.

- [ ] **Step 8: Build + full CLI tests**

Run: `cargo build -p radio-tui 2>&1 | tail -10 && cargo test -p radio-tui 2>&1 | tail -20`
Expected: build OK, tests PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/radio-tui/src/sync_cmd.rs
git commit -m "feat(cli): sync use <key> to link an existing account, merging favourites once on join"
```

---

## Task 6: Android — merge-on-link helper in FavStore

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/FavStore.kt`
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/FavStoreSyncLogicTest.kt`
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/SyncClientTest.kt` (retire the union-named `push_returnsMerged`)

**Interfaces:**
- Produces: `SyncMerge.mergedData(local: SyncData, server: SyncData): SyncData` — per-field union, local first then server-only ids.

- [ ] **Step 1: Write the failing test**

In `android/app/src/test/kotlin/net/vchub/r4dio/FavStoreSyncLogicTest.kt`, add:

```kotlin
    @Test
    fun mergedData_unions_each_field() {
        val local = SyncData(favs = listOf("a", "b"), blocked = emptyList(), excluded_countries = emptyList())
        val server = SyncData(favs = listOf("b", "c"), blocked = listOf("x"), excluded_countries = listOf("US"))
        val m = SyncMerge.mergedData(local, server)
        assertEquals(listOf("a", "b", "c"), m.favs)
        assertEquals(listOf("x"), m.blocked)
        assertEquals(listOf("US"), m.excluded_countries)
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd android && ./gradlew testDebugUnitTest --tests "*FavStoreSyncLogicTest.mergedData_unions_each_field" 2>&1 | tail -20`
Expected: FAIL — unresolved reference `mergedData`.

- [ ] **Step 3: Implement `mergedData` in `SyncMerge`**

In `android/app/src/main/kotlin/net/vchub/r4dio/FavStore.kt`, extend the `SyncMerge` object:

```kotlin
object SyncMerge {
    fun mergedFavs(local: Set<String>, remote: List<String>): Set<String> = local + remote

    private fun unionIds(local: List<String>, server: List<String>): List<String> =
        local + server.filterNot { local.contains(it) }

    fun mergedData(local: SyncData, server: SyncData): SyncData =
        SyncData(
            favs = unionIds(local.favs, server.favs),
            blocked = unionIds(local.blocked, server.blocked),
            excluded_countries = unionIds(local.excluded_countries, server.excluded_countries),
        )
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd android && ./gradlew testDebugUnitTest --tests "*FavStoreSyncLogicTest.mergedData_unions_each_field" 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Retire the union-named client test (replace semantics)**

In `android/app/src/test/kotlin/net/vchub/r4dio/SyncClientTest.kt`, replace `push_returnsMerged` (lines 51-59) with a replace-semantics regression test — the server now echoes exactly what was pushed:

```kotlin
    @Test
    fun push_returnsServerStateVerbatim() {
        val server = MockWebServer()
        server.enqueue(MockResponse().setBody("""{"favs":["c"],"blocked":[]}"""))
        server.start()
        val d = clientFor(server).push("r4-k", SyncData(listOf("c"), emptyList()))
        assertEquals(SyncData(listOf("c"), emptyList()), d)
        server.shutdown()
    }
```

- [ ] **Step 6: Run Android unit tests**

Run: `cd android && ./gradlew testDebugUnitTest 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/FavStore.kt android/app/src/test/kotlin/net/vchub/r4dio/FavStoreSyncLogicTest.kt android/app/src/test/kotlin/net/vchub/r4dio/SyncClientTest.kt
git commit -m "feat(android): SyncMerge.mergedData unions favourites/blocked/excluded for merge-on-link"
```

---

## Task 7: Android — run merge-on-link at scan and paste entry points

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/SyncActivity.kt:31-42` (scan), `:76-82` (paste)
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt` (expose a callable link-merge action) OR do the pull/merge/push inline in `SyncActivity`

**Interfaces:**
- Consumes: `SyncClient.pull`, `SyncClient.push`, `FavStore.applyMerged`, `SyncMerge.mergedData`, `FavStore.current*`.
- Produces: `SyncActivity.linkAndMerge(key: String)` — suspend fn: store key, pull, merge with local, applyMerged, push(replace).

- [ ] **Step 1: Add `linkAndMerge` to `SyncActivity`**

In `android/app/src/main/kotlin/net/vchub/r4dio/SyncActivity.kt`, add a private suspend fun (uses the activity's `favStore` and `syncClient`):

```kotlin
    private suspend fun linkAndMerge(key: String) {
        favStore.setSyncKey(key)
        val local = SyncData(
            favs = favStore.currentFavUuids().toList(),
            blocked = favStore.currentBlocked().toList(),
            excluded_countries = favStore.currentExcluded().toList(),
        )
        val server = withContext(Dispatchers.IO) { syncClient.pull(key) } ?: SyncData(emptyList(), emptyList())
        val merged = SyncMerge.mergedData(local, server)
        favStore.applyMerged(merged.favs.toSet(), merged.blocked.toSet(), merged.excluded_countries.toSet())
        withContext(Dispatchers.IO) { syncClient.push(key, merged) }
    }
```

Confirm `SyncActivity` already holds a `syncClient` and `favStore` field; if not, add `private val syncClient = SyncClient()` and reuse the existing `favStore`. Verify with `grep -n "syncClient\|favStore" SyncActivity.kt`.

- [ ] **Step 2: Wire scan result to `linkAndMerge`**

In the `scanner` result callback (`SyncActivity.kt:36-40`), replace:

```kotlin
            else -> lifecycleScope.launch {
                favStore.setSyncKey(contents)
                render()
                toast("key imported")
            }
```

with:

```kotlin
            else -> lifecycleScope.launch {
                linkAndMerge(contents)
                render()
                toast("key imported")
            }
```

- [ ] **Step 3: Wire paste (`use_key`) to `linkAndMerge`**

In the `use_key` click handler (`SyncActivity.kt:76-82`), replace the `true ->` branch body:

```kotlin
                true -> lifecycleScope.launch { favStore.setSyncKey(k); render(); toast("key set") }
```

with:

```kotlin
                true -> lifecycleScope.launch { linkAndMerge(k); render(); toast("key set") }
```

Leave `create` (`:83-99`) untouched — a new account has nothing to merge.

- [ ] **Step 4: Build the app**

Run: `cd android && ./gradlew assembleDebug 2>&1 | tail -15`
Expected: BUILD SUCCESSFUL. Fix any unresolved imports (`withContext`, `Dispatchers`, `SyncData`) — mirror the imports already used in `PlaybackService.kt`.

- [ ] **Step 5: Run Android unit tests**

Run: `cd android && ./gradlew testDebugUnitTest 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/SyncActivity.kt
git commit -m "fix(android): merge favourites once when joining an existing account via scan or pasted key"
```

---

## Task 8: End-to-end verification on emulator + CLI

**Files:** none (verification only).

- [ ] **Step 1: Deploy the server change to a test instance OR point clients at a local server**

Preferred: run the server locally. Run: `cd ../sync && cargo run 2>&1 | tail -5` (defaults to a local `sync.db`; binds a port — check `main.rs` for the address). Point a debug build at it, OR deploy `../sync` to Hetzner first (only if a throwaway account is used).

- [ ] **Step 2: CLI replace check**

- Create an account: `world-radio sync login` (or via TUI). Note the key.
- Add two favourites in the TUI, press sync.
- Unfavourite one (scope Favorites, press `f`).
- Confirm it does NOT reappear after the sync round-trip.
- `world-radio sync status` shows the reduced count on the server.

- [ ] **Step 3: CLI merge-on-link check**

- On a second data dir (throwaway `XDG_DATA_HOME`), add a different favourite.
- `world-radio sync use <key-from-step-2>`.
- Confirm the merged set = both devices' favourites (union once), and the server now holds the union.

- [ ] **Step 4: Android checks (emulator)**

Boot emulator, install the debug APK. Repeat: link via scan/paste → favourites merge once; toggle a favourite off → it stays off after sync. Use the emulator workflow; never touch the real data dir.

- [ ] **Step 5: Full test sweep**

Run: `cargo test 2>&1 | tail -20 && cd android && ./gradlew testDebugUnitTest 2>&1 | tail -20 && cd ../../sync && cargo test 2>&1 | tail -20`
Expected: all PASS.

- [ ] **Step 6: Report readiness**

Summarise: replace works, merge-on-link works, all tests green. Then STOP for the user to decide on deploy (server manual deploy + app release PR `dev`→`main`).

---

## Deploy (manual, AFTER user approval — NOT part of task execution)

1. Server `../sync`: as `deployer`, pull, `docker compose build && docker compose up -d`. Verify `PUT /sync` replaces.
2. App: PR `dev`→`main`, admin-merge; CI bumps version, tags, builds CLI+Android, auto-triggers deploy.
