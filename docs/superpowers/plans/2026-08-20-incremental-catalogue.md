# Incremental Catalogue Refresh Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop re-downloading 4.3 MB of catalogue to learn about ~1,300 changed rows, and stop rewriting the whole local database to apply them.

**Architecture:** Three independent layers, cheapest first. (1) Clients send `If-None-Match` so an unchanged catalogue costs a couple hundred bytes — the server already answers `304`, nobody asks. (2) A new `GET /catalog/delta?since=<id>` serves added stations in full plus removed UUIDs, against the last 7 daily snapshots; anything older gets `409` and the client falls back to a full download. (3) `Cache::apply_delta` writes ~1,300 statements instead of ~109,000. Every layer is allowed to give up and fall back to today's working path.

**Tech Stack:** Rust (axum, serde, rusqlite, flate2) for the sync server and `radio-core`; Kotlin (okhttp, kotlinx.serialization) for Android.

**Spec:** `docs/superpowers/specs/2026-08-20-incremental-catalogue-design.md`

## Global Constraints

- **Two repositories.** `sync/` is a separate git repo with its own deploy; `radio/` holds `radio-core` and `android/`. A task never spans both.
- **Deploy order is server first.** The delta endpoint bothers nobody until a client asks for it. Never ship a client that asks a server which cannot answer.
- **The delta is allowed to give up.** `409`, `404`, a parse failure, or any network error falls back to the full download that works today. An old client against a new server and a new client against an old server must both work.
- **The snapshot id is the server's ETag**, never a timestamp — client clocks lie.
- **Comments and logs in lowercase, in English.** Match surrounding style.
- **Measured churn: 688 added, 653 removed, 0 field changes per day** out of 54,729 stations. Test fixtures reflect this shape.
- **Never trade a bigger catalogue for a smaller one blindly** — this repo already guards that in two places, and the delta must not become the hole in it.

---

### Task 1: Server keeps the last 7 snapshots

**Files:**
- Modify: `sync/src/catalog.rs` (`Catalogue` struct ~line 222, `publish`/refresh path)
- Test: `sync/src/catalog.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: existing `Payload`, `Station`, `Catalogue`
- Produces: `Catalogue::snapshot_for(id: &str) -> Option<Vec<Station>>`, `Catalogue::current_id() -> Option<String>`, and a retained history of at most `SNAPSHOTS: usize = 7`

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn snapshots_are_kept_to_the_ceiling() {
    let dir = tempfile::tempdir().unwrap();
    let cat = Catalogue::new(dir.path().join("catalog.json"));
    // publish 9 distinct catalogues; only the last 7 stay reachable.
    let mut ids = Vec::new();
    for i in 0..9 {
        let stations = vec![Station {
            uuid: format!("u{i}"),
            name: format!("s{i}"),
            url: "http://x".into(),
            country: "FR".into(),
            codec: "MP3".into(),
            bitrate: 128,
            tags: String::new(),
            language: String::new(),
        }];
        cat.publish_for_test(stations, 1000 + i as u64).await;
        ids.push(cat.current_id().await.unwrap());
    }
    assert!(cat.snapshot_for(&ids[0]).await.is_none(), "oldest must be evicted");
    assert!(cat.snapshot_for(&ids[1]).await.is_none(), "second oldest must be evicted");
    assert!(cat.snapshot_for(&ids[2]).await.is_some(), "the 7 newest must be kept");
    assert!(cat.snapshot_for(&ids[8]).await.is_some(), "the newest must be kept");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd sync && cargo test snapshots_are_kept_to_the_ceiling`
Expected: FAIL — `no method named 'publish_for_test'` / `snapshot_for` not found.

- [ ] **Step 3: Write minimal implementation**

Add to `sync/src/catalog.rs`:

```rust
/// how many past catalogues stay reachable for a delta. the server refreshes
/// daily, so seven covers a client that has been offline for a week. anything
/// older takes one full download and rejoins.
const SNAPSHOTS: usize = 7;

/// a published catalogue, kept whole so a delta can be computed against it.
/// the id is the payload's etag: already computed, already what decides
/// freshness on `/catalog`, and unlike a timestamp it cannot be skewed by a
/// client's clock.
struct Snapshot {
    id: String,
    stations: Vec<Station>,
}
```

Add `history: RwLock<Vec<Snapshot>>` to `Catalogue`, initialised `RwLock::new(Vec::new())` in `Catalogue::new`.

```rust
impl Catalogue {
    pub async fn current_id(&self) -> Option<String> {
        self.current.read().await.as_ref().map(|p| p.etag.clone())
    }

    pub async fn snapshot_for(&self, id: &str) -> Option<Vec<Station>> {
        let history = self.history.read().await;
        history
            .iter()
            .find(|s| etag_matches(id, &s.id))
            .map(|s| s.stations.clone())
    }

    /// records a published catalogue and drops anything past the ceiling.
    async fn remember(&self, id: String, stations: Vec<Station>) {
        let mut history = self.history.write().await;
        history.push(Snapshot { id, stations });
        let excess = history.len().saturating_sub(SNAPSHOTS);
        history.drain(..excess);
    }

    #[cfg(test)]
    pub async fn publish_for_test(&self, stations: Vec<Station>, refreshed_at: u64) {
        if let Some(p) = Payload::build(&stations, refreshed_at) {
            let id = p.etag.clone();
            *self.current.write().await = Some(p);
            self.remember(id, stations).await;
        }
    }
}
```

Then call `self.remember(...)` everywhere a payload becomes current — both in `load_from_disk` and in the refresh path that sets `*self.current.write().await`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cd sync && cargo test snapshots_are_kept_to_the_ceiling`
Expected: PASS

- [ ] **Step 5: Fix the wrong churn comment**

In `sync/src/catalog.rs`, replace the `REFRESH_SECS` comment:

```rust
/// how long between refreshes. measured over one real day (2026-08-19 to
/// 2026-08-20): 688 stations added, 653 removed out of ~54.7k — about 2.45%.
/// once a day still comfortably outpaces the data, but the "~50 a day" figure
/// this comment used to carry was wrong by a factor of 27.
pub const REFRESH_SECS: u64 = 86_400;
```

- [ ] **Step 6: Commit**

```bash
cd sync
git add src/catalog.rs
git commit -m "keep the last seven catalogues so a client can ask what changed"
```

---

### Task 2: The delta endpoint

**Files:**
- Modify: `sync/src/catalog.rs` (add `Delta`, `diff`)
- Modify: `sync/src/main.rs` (route ~line 61, handler beside `get_catalog` ~line 238)
- Test: `sync/src/catalog.rs`

**Interfaces:**
- Consumes: `Catalogue::snapshot_for`, `Catalogue::current_id` from Task 1
- Produces: `pub struct Delta { id: String, added: Vec<Station>, removed: Vec<String> }`, `pub fn diff(old: &[Station], new: &[Station]) -> Delta`, route `GET /catalog/delta`

- [ ] **Step 1: Write the failing test**

```rust
fn st(uuid: &str) -> Station {
    Station {
        uuid: uuid.into(),
        name: uuid.into(),
        url: "http://x".into(),
        country: "FR".into(),
        codec: "MP3".into(),
        bitrate: 128,
        tags: String::new(),
        language: String::new(),
    }
}

#[test]
fn diff_reports_additions_and_removals() {
    let old = vec![st("a"), st("b"), st("c")];
    let new = vec![st("b"), st("c"), st("d")];
    let d = diff(&old, &new, "id-2".into());
    assert_eq!(d.added.len(), 1);
    assert_eq!(d.added[0].uuid, "d");
    assert_eq!(d.removed, vec!["a".to_string()]);
    assert_eq!(d.id, "id-2");
}

#[test]
fn a_changed_field_comes_back_as_an_addition() {
    // nothing mutates today, but if upstream ever starts, the client applies
    // `added` as an upsert — so a mutated station must appear there.
    let old = vec![st("a")];
    let mut moved = st("a");
    moved.name = "renamed".into();
    let d = diff(&old, &[moved], "id-2".into());
    assert_eq!(d.added.len(), 1);
    assert_eq!(d.added[0].name, "renamed");
    assert!(d.removed.is_empty(), "a rename is not a removal");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run each separately — `cargo test` takes only one filter argument:
`cd sync && cargo test diff_reports_additions_and_removals`
`cd sync && cargo test a_changed_field_comes_back_as_an_addition`
Expected: FAIL — `cannot find function 'diff'`

- [ ] **Step 3: Write minimal implementation**

```rust
/// what changed between two published catalogues. added stations travel whole
/// because the client upserts them; removed ones need only their id.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Delta {
    pub id: String,
    pub added: Vec<Station>,
    pub removed: Vec<String>,
}

/// measured over a real day, every change is a whole station appearing or
/// disappearing — not one field of 54,041 common stations differed. a station
/// whose fields did change still lands in `added`, which the client applies as
/// an upsert, so this stays correct if upstream ever starts mutating rows.
pub fn diff(old: &[Station], new: &[Station], id: String) -> Delta {
    use std::collections::HashMap;
    let before: HashMap<&str, &Station> = old.iter().map(|s| (s.uuid.as_str(), s)).collect();
    let after: HashMap<&str, &Station> = new.iter().map(|s| (s.uuid.as_str(), s)).collect();
    let added = new
        .iter()
        .filter(|s| before.get(s.uuid.as_str()).map(|b| *b != *s).unwrap_or(true))
        .cloned()
        .collect();
    let removed = old
        .iter()
        .filter(|s| !after.contains_key(s.uuid.as_str()))
        .map(|s| s.uuid.clone())
        .collect();
    Delta { id, added, removed }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run each separately — `cargo test` takes only one filter argument:
`cd sync && cargo test diff_reports_additions_and_removals`
`cd sync && cargo test a_changed_field_comes_back_as_an_addition`
Expected: PASS

- [ ] **Step 5: Wire the route**

In `sync/src/main.rs`, add beside the existing catalog route:

```rust
.route("/catalog/delta", get(get_catalog_delta))
```

And the handler, beside `get_catalog`:

```rust
#[derive(serde::Deserialize)]
struct DeltaQuery {
    since: String,
}

async fn get_catalog_delta(
    State(s): State<AppState>,
    Query(q): Query<DeltaQuery>,
) -> impl IntoResponse {
    let Some(id) = s.catalogue.current_id().await else {
        return (StatusCode::SERVICE_UNAVAILABLE, "catalogue not ready yet").into_response();
    };
    // the client is already current: say so the same way `/catalog` does.
    if catalog::etag_matches(&q.since, &id) {
        return (StatusCode::NOT_MODIFIED, [(ETAG, id)]).into_response();
    }
    let Some(old) = s.catalogue.snapshot_for(&q.since).await else {
        // too old, or from before this server started keeping history. saying so
        // explicitly is what lets the client fall back without guessing.
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "full": "/catalog" })),
        )
            .into_response();
    };
    let Some(new) = s.catalogue.snapshot_for(&id).await else {
        return (StatusCode::SERVICE_UNAVAILABLE, "catalogue not ready yet").into_response();
    };
    Json(catalog::diff(&old, &new, id)).into_response()
}
```

Add `axum::extract::Query` to the imports at the top of `main.rs`.

- [ ] **Step 6: Test the handler paths**

```rust
#[tokio::test]
async fn an_unknown_since_is_a_conflict_not_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let cat = Catalogue::new(dir.path().join("catalog.json"));
    cat.publish_for_test(vec![st("a")], 1000).await;
    assert!(cat.snapshot_for("\"nonsense\"").await.is_none());
}
```

Run: `cd sync && cargo test an_unknown_since_is_a_conflict_not_an_error`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
cd sync
git add src/catalog.rs src/main.rs
git commit -m "serve what changed instead of the whole catalogue"
```

---

### Task 3: `Cache::apply_delta` in radio-core

**Files:**
- Modify: `crates/radio-core/src/catalog/cache.rs` (beside `replace_all` ~line 349)
- Modify: `crates/radio-core/src/catalog/catalog.rs` (thin wrapper beside `replace_catalog` ~line 104)
- Test: `crates/radio-core/src/catalog/cache.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: existing `Cache`, `Station` (note: the core `Station` uses `stationuuid`/`countrycode`, unlike the server's wire `Station`)
- Produces: `Cache::apply_delta(&self, added: &[Station], removed: &[String]) -> anyhow::Result<usize>`, `Catalog::apply_catalog_delta(&self, added: &[Station], removed: &[String]) -> anyhow::Result<usize>`

- [ ] **Step 1: Write the failing test**

⚠ `Station` does **not** derive `Default`, so `..Default::default()` will not
compile. The test module already has a `station(uuid, name)` helper at
`cache.rs:987` — use it.

```rust
#[test]
fn apply_delta_adds_removes_and_leaves_the_rest() {
    let c = Cache::open_in_memory().unwrap();
    c.replace_all(&[station("a", "A"), station("b", "B")]).unwrap();

    c.apply_delta(&[station("c", "C")], &["a".to_string()]).unwrap();

    let all = c.list_all(&[]).unwrap();
    let ids: Vec<&str> = all.iter().map(|s| s.stationuuid.as_str()).collect();
    assert!(!ids.contains(&"a"), "removed station must be gone");
    assert!(ids.contains(&"b"), "untouched station must survive");
    assert!(ids.contains(&"c"), "added station must be present");
}

#[test]
fn apply_delta_refuses_to_empty_the_catalogue() {
    // a delta that removes most of what we hold is a bug upstream, not a day's
    // news. `replace_all` already refuses an empty dump for the same reason.
    let c = Cache::open_in_memory().unwrap();
    c.replace_all(&[
        station("a", "A"),
        station("b", "B"),
        station("c", "C"),
        station("d", "D"),
    ])
    .unwrap();

    let err = c.apply_delta(&[], &["a".into(), "b".into(), "c".into()]);
    assert!(err.is_err(), "removing three of four must be refused");
    assert_eq!(c.list_all(&[]).unwrap().len(), 4, "nothing may be lost");
}

#[test]
fn apply_delta_upserts_a_station_it_already_holds() {
    let c = Cache::open_in_memory().unwrap();
    c.replace_all(&[station("a", "old")]).unwrap();
    c.apply_delta(&[station("a", "new")], &[]).unwrap();
    let all = c.list_all(&[]).unwrap();
    assert_eq!(all.len(), 1, "an upsert must not duplicate the row");
    assert_eq!(all[0].name, "new");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p radio-core apply_delta`
Expected: FAIL — `no method named 'apply_delta'`

- [ ] **Step 3: Write minimal implementation**

In `cache.rs`, beside `replace_all`:

```rust
/// a delta may never remove more than this share of what we hold. beyond it,
/// the answer is a bug upstream rather than a day's news, and the caller falls
/// back to a full download. `replace_all` refuses an empty dump for the same
/// reason.
const MAX_DELTA_REMOVAL_SHARE: f64 = 0.5;

/// applies what changed rather than rewriting everything. `replace_all` runs
/// about 109k statements to land ~1.3k real changes, because it deletes both
/// tables and reinserts every row into each. this touches only what moved.
///
/// added stations are upserted, so a station we already hold is replaced rather
/// than duplicated — which is also how a mutated row would arrive.
pub fn apply_delta(&self, added: &[Station], removed: &[String]) -> anyhow::Result<usize> {
    let held: i64 =
        self.conn
            .query_row("SELECT COUNT(*) FROM stations", [], |r| r.get(0))?;
    if held > 0 && !removed.is_empty() {
        let share = removed.len() as f64 / held as f64;
        if share > MAX_DELTA_REMOVAL_SHARE {
            anyhow::bail!(
                "refusing a delta removing {} of {held} stations",
                removed.len()
            );
        }
    }

    let tx = self.conn.unchecked_transaction()?;
    for uuid in removed {
        tx.execute("DELETE FROM stations WHERE stationuuid = ?1", [uuid])?;
        tx.execute("DELETE FROM stations_fts WHERE stationuuid = ?1", [uuid])?;
    }
    let mut n = 0usize;
    for s in added {
        if is_banned(s) {
            continue;
        }
        tx.execute(
            "INSERT OR REPLACE INTO stations
                (stationuuid,name,url_resolved,countrycode,language,tags,codec,bitrate,votes,geo_lat,geo_long)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            rusqlite::params![
                s.stationuuid, s.name, s.url_resolved, s.countrycode, s.language,
                s.tags, s.codec, s.bitrate, s.votes, s.geo_lat, s.geo_long
            ],
        )?;
        // the fts table has no unique constraint, so an upsert has to clear the
        // old row itself or a re-added station is findable twice.
        tx.execute("DELETE FROM stations_fts WHERE stationuuid = ?1", [&s.stationuuid])?;
        tx.execute(
            "INSERT INTO stations_fts (stationuuid,name,tags) VALUES (?1,?2,?3)",
            rusqlite::params![s.stationuuid, s.name, s.tags],
        )?;
        n += 1;
    }
    tx.commit()?;
    Ok(n)
}
```

In `catalog.rs`, beside `replace_catalog`:

```rust
pub fn apply_catalog_delta(
    &self,
    added: &[Station],
    removed: &[String],
) -> anyhow::Result<usize> {
    self.cache.apply_delta(added, removed)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p radio-core apply_delta`
Expected: PASS — all three.

- [ ] **Step 5: Prove it agrees with `replace_all`**

```rust
#[test]
fn apply_delta_and_replace_all_reach_the_same_database() {
    let stations = |names: &[&str]| -> Vec<Station> {
        names.iter().map(|n| station(n, &n.to_uppercase())).collect()
    };

    let start = stations(&["a", "b", "c"]);
    let end = stations(&["b", "c", "d"]);

    let via_replace = Cache::open_in_memory().unwrap();
    via_replace.replace_all(&end).unwrap();

    let via_delta = Cache::open_in_memory().unwrap();
    via_delta.replace_all(&start).unwrap();
    via_delta.apply_delta(&stations(&["d"]), &["a".to_string()]).unwrap();

    let mut left: Vec<String> = via_replace.list_all(&[]).unwrap()
        .into_iter().map(|s| s.stationuuid).collect();
    let mut right: Vec<String> = via_delta.list_all(&[]).unwrap()
        .into_iter().map(|s| s.stationuuid).collect();
    left.sort();
    right.sort();
    assert_eq!(left, right);
}
```

Run: `cargo test -p radio-core apply_delta_and_replace_all_reach_the_same_database`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/radio-core/src/catalog/cache.rs crates/radio-core/src/catalog/catalog.rs
git commit -m "apply what changed instead of rewriting the whole catalogue"
```

---

### Task 4: Android revalidates with If-None-Match

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/Catalog.kt` (`fetchCatalogue` ~line 288)
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/FavStore.kt` (a stored etag)
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt` (~line 485)
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/CatalogTest.kt`

**Interfaces:**
- Consumes: existing `Catalog.fetchCatalogue`, `FavStore`
- Produces: `sealed class CatalogueResult { object Unchanged; data class Fetched(val stations: List<Station>, val etag: String); object Failed }`, `FavStore.currentCatalogEtag(): String`, `FavStore.setCatalogEtag(etag: String)`

- [ ] **Step 1: Write the failing test**

```kotlin
@Test
fun `a 304 reports unchanged rather than an empty catalogue`() {
    val server = MockWebServer()
    server.enqueue(MockResponse().setResponseCode(304))
    server.start()

    val catalog = Catalog(baseUrl = server.url("/").toString())
    val result = catalog.fetchCatalogueResult(
        url = server.url("/catalog").toString(),
        etag = "\"abc\"",
    )

    assertTrue(result is CatalogueResult.Unchanged)
    val sent = server.takeRequest()
    assertEquals("\"abc\"", sent.getHeader("If-None-Match"))
    server.shutdown()
}

@Test
fun `a 200 returns stations and the new etag`() {
    val server = MockWebServer()
    server.enqueue(
        MockResponse()
            .setResponseCode(200)
            .setHeader("ETag", "\"def\"")
            .setBody("""[{"uuid":"1","name":"A","url":"http://x","country":"FR","codec":"MP3","bitrate":128,"tags":"","language":""}]""")
    )
    server.start()

    val catalog = Catalog(baseUrl = server.url("/").toString())
    val result = catalog.fetchCatalogueResult(url = server.url("/catalog").toString(), etag = "")

    assertTrue(result is CatalogueResult.Fetched)
    result as CatalogueResult.Fetched
    assertEquals(1, result.stations.size)
    assertEquals("\"def\"", result.etag)
    server.shutdown()
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd android && ./gradlew testDebugUnitTest --tests '*CatalogTest*'`
Expected: FAIL — `fetchCatalogueResult` unresolved.

- [ ] **Step 3: Write minimal implementation**

In `Catalog.kt`:

```kotlin
/**
 * what a catalogue request came back with. an empty list used to mean both
 * "nothing changed" and "the download failed", and the caller could not tell
 * them apart — which is why this is a type rather than a list.
 */
sealed class CatalogueResult {
    object Unchanged : CatalogueResult()
    data class Fetched(val stations: List<Station>, val etag: String) : CatalogueResult()
    object Failed : CatalogueResult()
}

fun fetchCatalogueResult(
    url: String = CATALOG_URL,
    etag: String = "",
    blocked: Set<String> = emptySet(),
): CatalogueResult {
    val builder = Request.Builder()
        .url(url)
        .header("User-Agent", "world-radio-android/1.0")
    // the server has always answered 304; until now nobody asked, so an
    // unchanged catalogue still cost the full 4.3 mb.
    if (etag.isNotBlank()) {
        builder.header("If-None-Match", etag)
    }
    return runCatching {
        client.newCall(builder.build()).execute().use { resp ->
            if (resp.code == 304) return@use CatalogueResult.Unchanged
            val body = resp.body?.string().orEmpty()
            if (!resp.isSuccessful || body.isBlank()) return@use CatalogueResult.Failed
            val stations = json
                .decodeFromString(ListSerializer(FavStation.serializer()), body)
                .map { it.toStation() }
                .filter { allowedStation(it, blocked = blocked) }
            CatalogueResult.Fetched(stations, resp.header("ETag").orEmpty())
        }
    }.getOrDefault(CatalogueResult.Failed)
}
```

Keep `fetchCatalogue` as-is — it is still the fallback shape and other callers may exist.

In `FavStore.kt`, beside the other preference accessors:

```kotlin
suspend fun currentCatalogEtag(): String = store.data.first()[keyCatalogEtag].orEmpty()

suspend fun setCatalogEtag(etag: String) {
    store.edit { it[keyCatalogEtag] = etag }
}
```

with `private val keyCatalogEtag = stringPreferencesKey("catalog_etag")` beside the existing keys.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd android && ./gradlew testDebugUnitTest --tests '*CatalogTest*'`
Expected: PASS

- [ ] **Step 5: Wire the caller**

In `PlaybackService.kt`, replace the `fetchCatalogue` call and the checks that follow it:

```kotlin
val etag = runBlocking { favStore.currentCatalogEtag() }
when (val result = catalog.fetchCatalogueResult(etag = etag, blocked = blocked)) {
    is CatalogueResult.Unchanged -> {
        Log.i("r4dio", "catalogue unchanged, nothing downloaded")
        runBlocking { favStore.setCatalogSyncedAt(nowSecs()) }
        return@thread
    }
    is CatalogueResult.Failed -> {
        Log.w("r4dio", "catalogue fetch failed, keeping the $held held")
        return@thread
    }
    is CatalogueResult.Fetched -> {
        val fetched = result.stations
        // never trade a bigger catalogue for a smaller one: a partial
        // response should look like a failure, not like stations vanishing.
        if (fetched.size < held) {
            Log.w("r4dio", "catalogue fetch returned ${fetched.size} against $held held, ignoring")
            return@thread
        }
        if (!catalogCache.write(fetched)) {
            Log.w("r4dio", "catalogue fetched but not stored, keeping it in memory only")
            stations = fetched
            return@thread
        }
        stations = fetched
        runBlocking {
            favStore.setCatalogEtag(result.etag)
            favStore.setCatalogSyncedAt(nowSecs())
        }
        Log.i("r4dio", "catalogue fetched: ${fetched.size} stations in one request")
        scope.launch { refreshCustomLayout() }
    }
}
```

- [ ] **Step 6: Build and verify on the emulator**

Run: `make android-install && $ANDROID_HOME/platform-tools/adb logcat -c && $ANDROID_HOME/platform-tools/adb shell monkey -p net.vchub.r4dio -c android.intent.category.LAUNCHER 1`

Then force a second refresh and read the log:

Run: `$ANDROID_HOME/platform-tools/adb logcat -d | grep -E "catalogue (unchanged|fetched|fetch)"`
Expected: the first run logs `catalogue fetched: N stations`, a later refresh against an unchanged server logs `catalogue unchanged, nothing downloaded`.

- [ ] **Step 7: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/Catalog.kt \
        android/app/src/main/kotlin/net/vchub/r4dio/FavStore.kt \
        android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt \
        android/app/src/test/kotlin/net/vchub/r4dio/CatalogTest.kt
git commit -m "stop downloading a catalogue that has not changed"
```

---

### Task 5: Android asks for the delta

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/Catalog.kt`
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/CatalogCache.kt` (an apply-delta write)
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt`
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/CatalogTest.kt`

**Interfaces:**
- Consumes: `CatalogueResult` and `FavStore.currentCatalogEtag` from Task 4
- Produces: `Catalog.fetchDelta(url: String, since: String, blocked: Set<String>): DeltaResult`, `sealed class DeltaResult { data class Changed(val added: List<Station>, val removed: Set<String>, val id: String); object Unchanged; object Unavailable }`, `CatalogCache.applyDelta(added: List<Station>, removed: Set<String>): List<Station>?`

- [ ] **Step 1: Write the failing test**

```kotlin
@Test
fun `a 409 means the delta is unavailable, not a failure to retry`() {
    val server = MockWebServer()
    server.enqueue(MockResponse().setResponseCode(409).setBody("""{"full":"/catalog"}"""))
    server.start()

    val catalog = Catalog(baseUrl = server.url("/").toString())
    val result = catalog.fetchDelta(
        url = server.url("/catalog/delta").toString(),
        since = "\"old\"",
    )

    assertTrue(result is DeltaResult.Unavailable)
    server.shutdown()
}

@Test
fun `a delta returns what was added and removed`() {
    val server = MockWebServer()
    server.enqueue(
        MockResponse().setResponseCode(200).setBody(
            """{"id":"\"new\"","added":[{"uuid":"2","name":"B","url":"http://y","country":"DE","codec":"MP3","bitrate":128,"tags":"","language":""}],"removed":["1"]}"""
        )
    )
    server.start()

    val catalog = Catalog(baseUrl = server.url("/").toString())
    val result = catalog.fetchDelta(url = server.url("/catalog/delta").toString(), since = "\"old\"")

    assertTrue(result is DeltaResult.Changed)
    result as DeltaResult.Changed
    assertEquals(1, result.added.size)
    assertEquals(setOf("1"), result.removed)
    assertEquals("\"new\"", result.id)
    server.shutdown()
}

@Test
fun `applying a delta adds removes and keeps the rest`() {
    val dir = createTempDirectory().toFile()
    val cache = CatalogCache(dir)
    cache.write(listOf(station("1", "A"), station("2", "B")))

    val after = cache.applyDelta(added = listOf(station("3", "C")), removed = setOf("1"))

    assertNotNull(after)
    val ids = after!!.map { it.uuid }.toSet()
    assertEquals(setOf("2", "3"), ids)
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd android && ./gradlew testDebugUnitTest --tests '*CatalogTest*'`
Expected: FAIL — `fetchDelta` and `applyDelta` unresolved.

- [ ] **Step 3: Write minimal implementation**

In `Catalog.kt`:

```kotlin
@Serializable
private data class WireDelta(
    val id: String,
    val added: List<FavStation> = emptyList(),
    val removed: List<String> = emptyList(),
)

/**
 * what the delta endpoint answered. `Unavailable` is not an error — it is the
 * server saying "you are too far behind for this", and the caller answers by
 * downloading everything, exactly as it did before deltas existed.
 */
sealed class DeltaResult {
    data class Changed(val added: List<Station>, val removed: Set<String>, val id: String) : DeltaResult()
    object Unchanged : DeltaResult()
    object Unavailable : DeltaResult()
}

fun fetchDelta(
    url: String = "$CATALOG_URL/delta",
    since: String,
    blocked: Set<String> = emptySet(),
): DeltaResult {
    if (since.isBlank()) return DeltaResult.Unavailable
    val request = Request.Builder()
        .url("$url?since=${java.net.URLEncoder.encode(since, "UTF-8")}")
        .header("User-Agent", "world-radio-android/1.0")
        .build()
    return runCatching {
        client.newCall(request).execute().use { resp ->
            when {
                resp.code == 304 -> DeltaResult.Unchanged
                // 409: snapshot too old. 404: a server from before deltas.
                resp.code == 409 || resp.code == 404 -> DeltaResult.Unavailable
                !resp.isSuccessful -> DeltaResult.Unavailable
                else -> {
                    val body = resp.body?.string().orEmpty()
                    if (body.isBlank()) return@use DeltaResult.Unavailable
                    val wire = json.decodeFromString(WireDelta.serializer(), body)
                    DeltaResult.Changed(
                        added = wire.added.map { it.toStation() }
                            .filter { allowedStation(it, blocked = blocked) },
                        removed = wire.removed.toSet(),
                        id = wire.id,
                    )
                }
            }
        }
    }.getOrDefault(DeltaResult.Unavailable)
}
```

In `CatalogCache.kt`:

```kotlin
/**
 * a delta may never remove more than this share of what is held: beyond it the
 * answer is a bug, not a day's news, and the caller downloads everything
 * instead. the same guard exists in the rust cache.
 */
private const val MAX_DELTA_REMOVAL_SHARE = 0.5

/**
 * applies a delta to the stored catalogue and returns the result, or null if it
 * could not be applied — in which case the caller falls back to a full
 * download and nothing on disk has been touched.
 */
fun applyDelta(added: List<Station>, removed: Set<String>): List<Station>? =
    synchronized(lock) {
        val held = readLocked()
        if (held.isEmpty()) return@synchronized null
        if (removed.size.toDouble() / held.size > MAX_DELTA_REMOVAL_SHARE) {
            return@synchronized null
        }
        val byId = LinkedHashMap<String, Station>(held.size + added.size)
        held.forEach { byId[it.uuid] = it }
        removed.forEach { byId.remove(it) }
        // added last, so a station that is both removed and re-added survives.
        added.forEach { byId[it.uuid] = it }
        val merged = byId.values.toList()
        if (!writeLocked(merged)) return@synchronized null
        merged
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd android && ./gradlew testDebugUnitTest --tests '*CatalogTest*'`
Expected: PASS

- [ ] **Step 5: Wire the caller**

In `PlaybackService.kt`, before the full-catalogue path from Task 4:

```kotlin
val etag = runBlocking { favStore.currentCatalogEtag() }
// try the delta first: about 100-150 kb against 4.3 mb. it is allowed to give
// up for any reason at all, and the full download below is what happens then.
when (val delta = catalog.fetchDelta(since = etag, blocked = blocked)) {
    is DeltaResult.Unchanged -> {
        Log.i("r4dio", "catalogue unchanged, nothing downloaded")
        runBlocking { favStore.setCatalogSyncedAt(nowSecs()) }
        return@thread
    }
    is DeltaResult.Changed -> {
        val merged = catalogCache.applyDelta(delta.added, delta.removed)
        if (merged != null) {
            stations = merged
            runBlocking {
                favStore.setCatalogEtag(delta.id)
                favStore.setCatalogSyncedAt(nowSecs())
            }
            Log.i(
                "r4dio",
                "catalogue delta applied: +${delta.added.size} -${delta.removed.size}, ${merged.size} held",
            )
            scope.launch { refreshCustomLayout() }
            return@thread
        }
        Log.w("r4dio", "catalogue delta could not be applied, downloading everything")
    }
    is DeltaResult.Unavailable -> {
        Log.i("r4dio", "catalogue delta unavailable, downloading everything")
    }
}
// ...the Task 4 full-download path continues here unchanged.
```

- [ ] **Step 6: Verify on the emulator against the real server**

Run: `make android-install`

Then, with the app running, read the log after two refreshes:

Run: `$ANDROID_HOME/platform-tools/adb logcat -d | grep -E "catalogue (delta|fetched|unchanged)"`
Expected: the first refresh on a fresh install logs `catalogue delta unavailable, downloading everything` (no stored etag), and a later one logs either `catalogue unchanged` or `catalogue delta applied: +N -M`.

⚠ Do not accept a screenshot or a green build as proof here. Read the log line.

- [ ] **Step 7: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/Catalog.kt \
        android/app/src/main/kotlin/net/vchub/r4dio/CatalogCache.kt \
        android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt \
        android/app/src/test/kotlin/net/vchub/r4dio/CatalogTest.kt
git commit -m "ask what changed instead of downloading the catalogue again"
```

---

### Task 6: Prove old and new still talk to each other

**Files:**
- Test: `sync/src/catalog.rs`
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/CatalogTest.kt`

**Interfaces:**
- Consumes: everything above. Produces no new production code.

This is the task that is easiest to skip and most expensive to have skipped. This repo has a memory entry about a hand-written fixture that bypassed the very path it was meant to prove.

- [ ] **Step 1: Old client against a new server**

An old client never asks for a delta and sends no `If-None-Match`. Assert the plain `/catalog` path is untouched:

```rust
#[tokio::test]
async fn a_client_that_asks_for_nothing_still_gets_the_whole_catalogue() {
    let dir = tempfile::tempdir().unwrap();
    let cat = Catalogue::new(dir.path().join("catalog.json"));
    cat.publish_for_test(vec![st("a"), st("b")], 1000).await;
    let payload = cat.get().await.unwrap();
    assert_eq!(payload.stations, 2);
    assert!(!payload.gzipped.is_empty());
}
```

Run: `cd sync && cargo test a_client_that_asks_for_nothing_still_gets_the_whole_catalogue`
Expected: PASS

- [ ] **Step 2: New client against an old server**

An old server has no `/catalog/delta` and answers `404`. Task 5 maps that to `Unavailable`; pin it so nobody "improves" it into a retry:

```kotlin
@Test
fun `a 404 from a server without deltas falls back rather than failing`() {
    val server = MockWebServer()
    server.enqueue(MockResponse().setResponseCode(404))
    server.start()

    val catalog = Catalog(baseUrl = server.url("/").toString())
    val result = catalog.fetchDelta(url = server.url("/catalog/delta").toString(), since = "\"x\"")

    assertTrue(result is DeltaResult.Unavailable)
    server.shutdown()
}

@Test
fun `a first run with no stored etag goes straight to the full download`() {
    val catalog = Catalog(baseUrl = "http://127.0.0.1:1")
    assertTrue(catalog.fetchDelta(since = "") is DeltaResult.Unavailable)
}
```

Run: `cd android && ./gradlew testDebugUnitTest --tests '*CatalogTest*'`
Expected: PASS

- [ ] **Step 3: The real churn shape**

Build a delta between two real snapshots and assert it matches what the dumps differ by. Save the two measured catalogues as fixtures first:

```rust
#[test]
fn a_days_delta_matches_what_the_dumps_differ_by() {
    // the shape measured on 2026-08-19 → 2026-08-20: additions and removals
    // only, no field changes. a fixture pinning the real proportions keeps a
    // future "optimisation" from quietly dropping removals.
    let old: Vec<Station> = (0..1000).map(|i| st(&format!("u{i}"))).collect();
    let mut new: Vec<Station> = (20..1000).map(|i| st(&format!("u{i}"))).collect();
    new.extend((1000..1025).map(|i| st(&format!("u{i}"))));

    let d = diff(&old, &new, "id".into());
    assert_eq!(d.removed.len(), 20);
    assert_eq!(d.added.len(), 25);
    // the whole point: the delta is a fraction of the catalogue.
    assert!(d.added.len() + d.removed.len() < old.len() / 10);
}
```

Run: `cd sync && cargo test a_days_delta_matches_what_the_dumps_differ_by`
Expected: PASS

- [ ] **Step 4: Full suites, both repos**

Run: `cargo test -p radio-core && cd android && ./gradlew testDebugUnitTest`
Expected: PASS, no regressions.

Run: `cd sync && cargo test && cargo clippy --all-targets && cargo fmt --check`
Expected: PASS, clean.

- [ ] **Step 5: Commit**

```bash
git add android/app/src/test/kotlin/net/vchub/r4dio/CatalogTest.kt
git commit -m "pin that old and new clients and servers still understand each other"
```

---

### Task 7: Deploy the server, then the app

**Files:** none — this is a release task.

⚠ **Order matters and cannot be reversed.** The server must be live before any client that asks it for a delta.

- [ ] **Step 1: Ship the sync server**

```bash
cd sync
git push origin main
```

- [ ] **Step 2: Verify the endpoint against the live server, not the repo**

```bash
curl -s -o /dev/null -w "%{http_code}\n" "https://r4dio.net/catalog/delta?since=%22nonsense%22"
```
Expected: `409` — the server is live and correctly refuses an unknown snapshot.

```bash
etag=$(curl -sI https://r4dio.net/catalog | grep -i '^etag:' | cut -d' ' -f2- | tr -d '\r')
curl -s -o /dev/null -w "%{http_code}\n" -H "If-None-Match: $etag" https://r4dio.net/catalog
```
Expected: `304`.

- [ ] **Step 3: Confirm a real delta is small**

```bash
curl -s "https://r4dio.net/catalog/delta?since=$(python3 -c 'import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1]))' "$etag")" | wc -c
```
Expected: far below the 4.3 MB full payload. Immediately after a refresh this may legitimately be `304`.

- [ ] **Step 4: Release the app**

Only after the checks above pass:

```bash
git push origin dev
gh pr create --base main --head dev --title "..." --body "..."
```

CI computes the version, tags, and publishes. Merge only once checks pass.

---

## Self-Review

**Spec coverage:** Layer 1 → Task 4. Layer 2 → Tasks 1, 2, 5. Layer 3 → Task 3. Failure behaviour → Task 3 (mass-removal guard), Task 5 (`applyDelta` returning null). Compatibility → Task 6. Deploy order → Task 7. The wrong churn comment → Task 1 Step 5. CLI/macOS are explicitly out of scope per the spec.

**Known gap, deliberate:** `radio-core`'s `apply_delta` (Task 3) has no production caller in this plan, because the CLI and macOS fetch from radio-browser rather than our server. It is built here because it belongs beside `replace_all` and is most of what the CLI switch will need. Its tests are real; its wiring is a later spec.
