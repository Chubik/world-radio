# One Source of Truth Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every listening setting has exactly one owner, multi-country filters actually filter, and macOS becomes a full client instead of a reduced one.

**Architecture:** `profile.json` owns everything that syncs (filter countries, scope, theme, history); `config.toml` owns only what belongs to this machine. The TUI keeps one in-memory copy — `model.profile` — and the UI reads the filter from the same place it writes it. One shared `allowed_station` predicate in `radio-core` replaces three independent pick implementations, so a filter cannot apply on one surface and silently not on another.

**Tech Stack:** Rust (radio-core, radio-tui, r4dio-macos/Tauri), rusqlite, serde.

**Spec:** `docs/superpowers/specs/2026-08-11-one-source-of-truth-design.md`

## Global Constraints

- Branch `dev` in this repo. Do not push; the controller pushes after review. Never touch the `sync` repo (`../sync`) or `ops` — no server change is needed by this plan.
- **The defect class this plan exists to remove:** correct code the real user path never reaches. Six instances have shipped. Every task names the real user path that proves it, and no task is done on tests alone.
- **Never start playback to prove something.** Launching r4dio plays audio out loud on the user's machine, and audio proves nothing here: this plan is about which stations are *listed and picked*, not how they sound. Read the rendered screen (drive the TUI in a pty and read the visible rows), read what was written to disk, or read what went over the wire. Start playback only when the thing under test is playback itself, and say so first.
- Old-client/old-server compatibility on the wire stays untouched: this plan changes no payload shape and no merge rule. If a task finds itself editing `crates/radio-core/src/sync/session.rs`'s payload shape, stop and escalate.
- Migration runs once per device and cannot be re-run — a wrong outcome is unrecoverable for that user. Every migration step is proven on a copy of a real `config.toml` before it is committed.
- Favourites bypass the country filter on every surface (an explicit star outranks a broad taste filter). Hidden countries and the blacklist always outrank both.
- Fetch/cache paths are never filtered by the country filter — it applies at pick and query sites only.
- No `else if`. Comments only where they state a constraint, lowercase-first. All strings/logs English lowercase-first.
- Gate before every commit: `cargo fmt && cargo fmt --check && cargo clippy --all-targets && cargo test --workspace`, all green, real output pasted into the report.
- Commit subjects are the published changelog: lowercase, concrete, user-facing. No AI/assistant mention anywhere, no trailers, no `Co-Authored-By`.
- **Do not regress macOS-only capabilities:** the in-app updater, the tray with live-refreshed labels, the ⌥⇧R global hotkey (works over another app's fullscreen space), popover positioning and its `REOPEN_GUARD`, activation-policy switching, the server-side QR render that keeps the raw key out of the DOM, single-instance takeover, and the RU/BY filter repeated at the UI boundary (`crates/r4dio-macos/src/catalog_src.rs:90-109`).

---

### Task 1: One shared `allowed_station` predicate

**Why first:** every later task depends on there being exactly one filter rule. Today the rule exists three times (TUI `matches_filters`, macOS `active_stations`, Android `allowedStation`) and macOS's copy silently lacks the country filter.

**Files:**
- Modify: `crates/radio-core/src/catalog/filter.rs` (add the predicate + `SearchQuery.countrycodes`)
- Test: same file's `mod tests`

**Interfaces:**
- Consumes: `Station` (`crates/radio-core/src/catalog/mod.rs` re-export), existing `SearchQuery` at `filter.rs:2-9`.
- Produces (Tasks 2-6 call these verbatim):

```rust
/// the one rule every surface picks by. `included` empty means unrestricted;
/// hidden countries and blocked stations always outrank it.
pub fn allowed_station(
    station: &Station,
    excluded_countries: &[String],
    blocked: &[String],
    included_countries: &[String],
) -> bool
```

`SearchQuery` gains `pub countrycodes: Vec<String>` and **keeps** `countrycode: Option<String>` for now (Task 2 removes it) so this task compiles standalone.

- [ ] **Step 1: Write the failing tests**

In `crates/radio-core/src/catalog/filter.rs` `mod tests`:

```rust
    // `Station` does not derive Default (crates/radio-core/src/catalog/station.rs:3),
    // so every one of its 13 fields is named here rather than spread.
    fn st(uuid: &str, country: &str) -> Station {
        Station {
            stationuuid: uuid.into(),
            name: "n".into(),
            url_resolved: "u".into(),
            countrycode: country.into(),
            language: String::new(),
            tags: String::new(),
            codec: String::new(),
            bitrate: 0,
            votes: 0,
            geo_lat: None,
            geo_long: None,
            lastcheckok: 1,
            lastchecktime_iso8601: String::new(),
        }
    }

    #[test]
    fn an_empty_include_set_allows_every_country() {
        assert!(allowed_station(&st("a", "PL"), &[], &[], &[]));
    }

    #[test]
    fn an_include_set_admits_only_its_countries() {
        let inc = vec!["UA".to_string(), "US".to_string()];
        assert!(allowed_station(&st("a", "UA"), &[], &[], &inc));
        assert!(allowed_station(&st("b", "US"), &[], &[], &inc));
        assert!(!allowed_station(&st("c", "PL"), &[], &[], &inc));
    }

    #[test]
    fn country_matching_ignores_case() {
        let inc = vec!["ua".to_string()];
        assert!(allowed_station(&st("a", "UA"), &[], &[], &inc));
    }

    // a blocked station stays blocked even inside the filter, and a hidden
    // country stays hidden even when the filter names it.
    #[test]
    fn blocked_and_excluded_outrank_the_include_set() {
        let inc = vec!["UA".to_string()];
        let blocked = vec!["a".to_string()];
        assert!(!allowed_station(&st("a", "UA"), &[], &blocked, &inc));
        let excluded = vec!["UA".to_string()];
        assert!(!allowed_station(&st("b", "UA"), &excluded, &[], &inc));
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p radio-core --lib filter 2>&1 | tail -5`
Expected: compile FAIL — `allowed_station` not found.

- [ ] **Step 3: Implement**

```rust
pub fn allowed_station(
    station: &Station,
    excluded_countries: &[String],
    blocked: &[String],
    included_countries: &[String],
) -> bool {
    let country = station.countrycode.to_uppercase();
    if excluded_countries.iter().any(|c| c.to_uppercase() == country) {
        return false;
    }
    if blocked.iter().any(|b| b == &station.stationuuid) {
        return false;
    }
    match included_countries.is_empty() {
        true => true,
        false => included_countries
            .iter()
            .any(|c| c.to_uppercase() == country),
    }
}
```

Add `#[serde(default)] pub countrycodes: Vec<String>` to `SearchQuery` (it derives `Default`, so nothing else breaks).

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p radio-core --lib filter 2>&1 | tail -5` — PASS.

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo fmt --check && cargo clippy --all-targets 2>&1 | tail -3 && cargo test --workspace 2>&1 | tail -3
git add crates/radio-core/src/catalog/filter.rs
git commit -m "one rule decides which stations a filter allows"
```

---

### Task 2: The query carries every selected country

**Files:**
- Modify: `crates/radio-core/src/catalog/cache.rs` (the `where_parts` builder inside `search_limited`, around `:169-230`)
- Modify: `crates/radio-core/src/catalog/filter.rs` (drop `countrycode`, keep `countrycodes`)
- Modify: `crates/radio-tui/src/tui/model.rs:120-133` (`to_query`)
- Modify: every other `SearchQuery` construction site — find them with `grep -rn "SearchQuery {" crates/`
- Test: `cache.rs` `mod tests`, `model.rs` `mod tests`

**Interfaces:**
- Consumes: `SearchQuery.countrycodes` from Task 1.
- Produces: `SearchQuery` no longer has `countrycode`; `to_query` fills `countrycodes`, `tag`/`codec` become `tags`/`codecs` (`Vec<String>`).

**Read first:** `crates/radio-core/src/catalog/cache.rs:169-230` builds SQL from `where_parts` + `params`; follow that exact shape rather than inventing a new one. The remote API path is `crates/radio-tui/src/tui/worker.rs:514-524`.

- [ ] **Step 1: Write the failing tests**

In `cache.rs` `mod tests` — that file builds its fixtures with `Cache::open_in_memory()` +
`replace_all(&[...])` and a `bare()` spread helper (see `search_excludes_user_countries` at
`cache.rs:567-580`); follow that shape exactly:

```rust
    #[test]
    fn a_search_returns_every_selected_country() {
        let c = Cache::open_in_memory().unwrap();
        c.replace_all(&[
            Station { stationuuid: "a".into(), name: "ua one".into(), countrycode: "UA".into(), ..bare() },
            Station { stationuuid: "b".into(), name: "us one".into(), countrycode: "US".into(), ..bare() },
            Station { stationuuid: "c".into(), name: "pl one".into(), countrycode: "PL".into(), ..bare() },
        ])
        .unwrap();
        let q = SearchQuery {
            countrycodes: vec!["UA".into(), "US".into()],
            ..Default::default()
        };
        let got = c.search(&q, &[]).unwrap();
        let mut ids: Vec<&str> = got.iter().map(|s| s.stationuuid.as_str()).collect();
        ids.sort();
        assert_eq!(ids, vec!["a", "b"], "both selected countries must come back");
    }

    #[test]
    fn an_empty_country_list_does_not_restrict() {
        let c = Cache::open_in_memory().unwrap();
        c.replace_all(&[
            Station { stationuuid: "a".into(), name: "ua one".into(), countrycode: "UA".into(), ..bare() },
            Station { stationuuid: "b".into(), name: "pl one".into(), countrycode: "PL".into(), ..bare() },
        ])
        .unwrap();
        let got = c.search(&SearchQuery::default(), &[]).unwrap();
        assert_eq!(got.len(), 2);
    }
```

In `model.rs` `mod tests` — **replace** the existing first-only test (`model.rs:601-616`, which pins the contradictory contract):

```rust
    #[test]
    fn to_query_carries_every_selected_country() {
        let f = BrowseFilters {
            countries: vec!["UA".into(), "US".into()],
            ..Default::default()
        };
        assert_eq!(f.to_query("").countrycodes, vec!["UA".to_string(), "US".to_string()]);
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p radio-core --lib cache 2>&1 | tail -5` — FAIL (only UA returned, or compile error on `countrycodes`).

- [ ] **Step 3: Implement the SQL**

In `search_limited`, replace the single-country clause with a placeholder list:

```rust
        if !q.countrycodes.is_empty() {
            let marks = vec!["?"; q.countrycodes.len()].join(",");
            where_parts.push(format!("UPPER(countrycode) IN ({marks})"));
            for c in &q.countrycodes {
                params.push(Box::new(c.to_uppercase()));
            }
        }
```

Do the same for `tags` and `codecs` (the spec fixes them together — the same truncation, one field over). Then update `to_query` and every construction site the grep found.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test --workspace 2>&1 | grep -E "^test result|FAILED" | tail -6` — all PASS.

- [ ] **Step 5: The remote API path**

`worker.rs:514-524` sends one country to the remote API. The API accepts one; query each selected country and union the results, deduping by `stationuuid`. Add a test that two countries produce one merged list with no duplicates.

- [ ] **Step 6: Gate and commit**

```bash
cargo fmt && cargo fmt --check && cargo clippy --all-targets 2>&1 | tail -3 && cargo test --workspace 2>&1 | tail -3
git add crates/radio-core crates/radio-tui
git commit -m "search every country you picked, not just the first"
```

**Real-path proof required in the report:** launch the TUI, select UA and US, and state how many stations of each appear. Tests alone do not close this task.

---

### Task 3: `profile.json` owns the synced settings

**Files:**
- Modify: `crates/radio-tui/src/tui/config.rs` (drop `filters` and `theme` from `Config`)
- Modify: `crates/radio-tui/src/tui/mod.rs:66-67,161-190,241-256` (startup read, migration, exit write)
- Modify: `crates/radio-tui/src/tui/model.rs` (`Model.browse.filters` loses `status`/`countries`; they are read from `model.profile`)
- Modify: `crates/radio-tui/src/tui/update.rs` (all filter/scope/theme read and write sites)
- Test: `config.rs` and `mod.rs` tests

**Interfaces:**
- Consumes: `Profile::adopt_existing(&[String], &str, &str, i64) -> bool` (already implemented, `crates/radio-core/src/sync/profile.rs:78-102`).
- Produces: `Config` without `filters`/`theme`; `Model` exposes the filter through `model.profile.countries` and the scope through `model.profile.scope`.

**This is the largest task. Read `crates/radio-tui/src/tui/mod.rs:161-260` end to end before editing** — startup order and the exit write are load-bearing, and the exit write is what currently clobbers synced values.

- [ ] **Step 1: Write the failing migration test**

In `crates/radio-tui/src/tui/config.rs` `mod tests`:

```rust
    // an old config still carrying filters and a theme must hand them over
    // exactly once, and must not resurrect them afterwards.
    #[test]
    fn an_old_config_hands_its_settings_to_the_profile() {
        let raw = r#"
theme = "monokai"
[filters]
status = "all"
countries = ["UA"]
"#;
        let cfg: Config = toml::from_str(raw).unwrap();
        let legacy = cfg.legacy_settings();
        assert_eq!(legacy.countries, vec!["UA".to_string()]);
        assert_eq!(legacy.theme.as_deref(), Some("monokai"));
    }

    #[test]
    fn a_new_config_writes_no_filters_or_theme() {
        let cfg = Config::default();
        let out = toml::to_string(&cfg).unwrap();
        assert!(!out.contains("[filters]"), "filters must not be written: {out}");
        assert!(!out.contains("theme"), "theme must not be written: {out}");
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p radio-tui --bins config 2>&1 | tail -5` — FAIL (`legacy_settings` missing; the new config still writes both).

- [ ] **Step 3: Implement the config split**

Keep the fields deserializable but never serialize them, so an old file still reads and a new file stops carrying them:

```rust
    // read-only migration carriers: an old config still holds these, a new one
    // never writes them. profile.json owns both from this build on.
    #[serde(default, skip_serializing)]
    pub legacy_theme: Option<String>,
    #[serde(default, skip_serializing, rename = "filters")]
    pub legacy_filters: Option<BrowseFilters>,
```

Add `Config::legacy_settings()` returning `{ countries: Vec<String>, scope: Option<String>, theme: Option<String> }` from those carriers.

- [ ] **Step 4: Wire startup**

In `mod.rs`, after loading the profile, call `adopt_existing` with the legacy values (not with `model.browse.filters`, which no longer holds them), save the profile when it reports `true`, then seed the UI **from the profile**:

```rust
    let profile_path = data.join("profile.json");
    let mut profile = radio_core::sync::Profile::load(&profile_path);
    let legacy = config.legacy_settings();
    if profile.adopt_existing(&legacy.countries, legacy.scope.as_deref().unwrap_or(""), legacy.theme.as_deref().unwrap_or(""), now_secs) {
        let _ = profile.save(&profile_path);
    }
    model.profile = profile;
```

Then set `model.theme` from `model.profile.theme` via `Theme::try_from_slug` (falling back to the config default when unset), and seed the browse scope/countries from `model.profile`.

- [ ] **Step 5: Remove the exit clobber**

In `mod.rs:241-256`, the exit write must no longer carry `filters` or `theme`. Everything else it writes stays.

- [ ] **Step 6: Update every read site**

`grep -rn "browse.filters.countries\|browse.filters.status" crates/radio-tui/src` and route each through `model.profile`. Stamping stays exactly where it is (`stamp_profile_from_filters`) — the point is that there is no second copy left to drift.

- [ ] **Step 7: Run the gate**

Run: `cargo test --workspace 2>&1 | grep -E "^test result|FAILED" | tail -6` — all PASS.

- [ ] **Step 8: Prove the migration on real data**

```bash
SP=$(mktemp -d); mkdir -p "$SP/Library/Application Support/net.vchub.r4dio"
cp "$HOME/Library/Application Support/net.vchub.r4dio/config.toml" "$SP/Library/Application Support/net.vchub.r4dio/"
HOME=$SP timeout 12 ./target/debug/r4dio </dev/null >/dev/null 2>&1
cat "$SP/Library/Application Support/net.vchub.r4dio/profile.json"
grep -c "filters" "$SP/Library/Application Support/net.vchub.r4dio/config.toml" || echo "filters gone: ok"
```

Expected: `profile.json` carries the countries and theme from the real config; `config.toml` no longer has `[filters]`. **Never run this against `$HOME` directly.**

- [ ] **Step 9: Commit**

```bash
git add crates/radio-tui
git commit -m "your filters and theme now live in one place and survive a sync"
```

---

### Task 4: The TUI shows a synced filter it did not choose

**Files:**
- Modify: `crates/radio-tui/src/tui/update.rs` (`apply_profile_synced`, around `:730-757`)
- Test: `update.rs` `mod tests`

**Interfaces:**
- Consumes: `Model.profile` as the single copy (Task 3).
- Produces: nothing downstream.

- [ ] **Step 1: Write the failing test**

```rust
    // a filter that arrived from another device must reach the list, not just
    // the profile — this is the path that was broken before this branch.
    #[test]
    fn a_synced_filter_redraws_the_list() {
        let mut m = model();
        let fx = update(
            &mut m,
            Msg::ProfileSynced {
                profile: {
                    let mut p = radio_core::sync::Profile::default();
                    p.set_countries(vec!["UA".into()], 100);
                    p
                },
                countries: Some(vec!["UA".into()]),
                scope: None,
                theme: None,
            },
        );
        assert_eq!(m.profile.countries, vec!["UA".to_string()]);
        assert!(
            fx.iter().any(|e| matches!(e, Effect::Search(..))),
            "a synced filter must trigger a re-search: {fx:?}"
        );
    }
```

- [ ] **Step 2: Run to verify it fails**, then implement so `apply_profile_synced` assigns the whole profile and emits `Effect::Search` when the countries moved. Run again — PASS.

- [ ] **Step 3: Gate and commit**

```bash
cargo fmt && cargo fmt --check && cargo clippy --all-targets 2>&1 | tail -3 && cargo test --workspace 2>&1 | tail -3
git add crates/radio-tui/src/tui/update.rs
git commit -m "a filter set on another device redraws the list here"
```

**Real-path proof:** with two linked data dirs, change the filter on device B, sync device A, and state what A's list showed before and after.

---

### Task 5: macOS picks through the shared rule

**Files:**
- Modify: `crates/r4dio-macos/src/catalog_src.rs:15-18` (`all_stations`)
- Modify: `crates/r4dio-macos/src/state.rs:100-125` (`set_all`/`active_stations`/`pick_shuffle`)
- Modify: `crates/r4dio-macos/src/backend.rs` (pass the filter in; invalidate on sync)
- Modify: `crates/r4dio-macos/src/commands.rs:6-11` (delete `parse_scope`)
- Test: `state.rs` and `backend.rs` `mod tests`

**Interfaces:**
- Consumes: `radio_core::catalog::allowed_station` (Task 1), `radio_core::sync::Scope::from_wire`.
- Produces: nothing downstream.

**Read first:** `crates/r4dio-macos/src/state.rs:68-149`. `MiniState.all` is the entire catalogue materialised in memory and hand-invalidated; the spec's risk section calls out that the popover and tray are latency-visible.

- [ ] **Step 1: Write the failing tests**

`MiniState` does not derive `Default` (`state.rs:68`) — build it the way the existing tests in that
file already do. The file's existing helper is `st(uuid, url)` with an empty country
(`state.rs:160-170`); add a sibling that sets one rather than changing `st` and disturbing the
three tests that use it:

```rust
    fn st_in(uuid: &str, country: &str) -> StationPick {
        StationPick {
            uuid: uuid.into(),
            name: uuid.into(),
            url: format!("http://{uuid}"),
            country: country.into(),
            codec: String::new(),
            bitrate: 0,
        }
    }

    #[test]
    fn the_shuffle_pool_honours_the_country_filter() {
        let mut s = new_state();
        s.set_all(vec![st_in("a", "UA"), st_in("b", "PL")]);
        s.set_filter(vec!["UA".into()]);
        for _ in 0..20 {
            assert_eq!(s.pick_shuffle().unwrap().uuid, "a");
        }
    }

    #[test]
    fn an_empty_filter_leaves_the_pool_whole() {
        let mut s = new_state();
        s.set_all(vec![st_in("a", "UA"), st_in("b", "PL")]);
        s.set_filter(vec![]);
        assert_eq!(s.active_stations().len(), 2);
    }

    // a station blocked on another device must not be shuffled into here.
    #[test]
    fn the_shuffle_pool_skips_blocked_stations() {
        let mut s = new_state();
        s.set_all(vec![st_in("a", "UA"), st_in("b", "UA")]);
        s.set_blocked(vec!["a".into()]);
        for _ in 0..20 {
            assert_eq!(s.pick_shuffle().unwrap().uuid, "b");
        }
    }

    // the star outranks the taste filter, exactly as on android.
    #[test]
    fn favourites_ignore_the_country_filter() {
        let mut s = new_state();
        s.set_favorites(vec![st_in("f", "PL")]);
        s.set_filter(vec!["UA".into()]);
        s.set_scope(Scope::Favorites);
        assert_eq!(s.pick_shuffle().unwrap().uuid, "f");
    }
```

`new_state()` is whatever the file already uses to construct a `MiniState` — read
`state.rs:68-99` and follow it rather than adding a `Default` impl.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p r4dio-macos --bins 2>&1 | tail -6` — FAIL.

- [ ] **Step 3: Implement**

`MiniState` gains `filter: Vec<String>` and `blocked: Vec<String>`; `active_stations` applies `allowed_station` for the `All` scope and returns favourites untouched for `Favorites`. `Backend::sync` sets the filter from the merged profile and `Backend::new` sets it at startup. Delete `parse_scope` and call `radio_core::sync::Scope::from_wire` instead.

- [ ] **Step 4: Run to verify they pass**, then the full gate.

- [ ] **Step 5: Commit**

```bash
git add crates/r4dio-macos
git commit -m "the macos app plays only the countries you filtered to"
```

**Real-path proof:** launch the macOS app with a UA filter on the account and report the countries of ten shuffles, plus that a station blocked elsewhere never appeared.

---

### Task 6: macOS follows the account live

**Files:**
- Modify: `crates/r4dio-macos/src/main.rs` (spawn the listener next to the existing startup sync at `:89-91`)
- Modify: `crates/r4dio-macos/src/backend.rs` (announce plays; apply incoming)
- Modify: `crates/r4dio-macos/ui/main.js` and `ui/app.js` (invoke the existing `sync` command from a real control)
- Test: `backend.rs` `mod tests`

**Interfaces:**
- Consumes: `radio_core::mirror::MirrorClient::events`, `StreamEvent` — the CLI's listener is `crates/radio-tui/src/tui/mod.rs:129-149` and its dispatcher is `:39-58`. Mirror them; do not invent a second shape.
- Produces: nothing downstream.

- [ ] **Step 1: Write the failing tests**

The CLI's debounce is a plain `AtomicBool` swapped by the listener and cleared after the sync runs
(`crates/radio-tui/src/tui/mod.rs:43,52` and `:120`) — there is no `ResyncGate` type in Rust; that
name is Kotlin's. Use the same `AtomicBool` shape, and extract the dispatch into a free function so
the test drives the same code the listener thread runs (the CLI does exactly this with
`dispatch_stream_event`, `tui/mod.rs:39-58`). `crates/r4dio-macos/src/backend.rs` has a `mod tests`
at `:456` but no `test_backend()` helper — follow whatever construction those existing tests use.

```rust
    // two doorbells in flight must still cost exactly one resync.
    #[test]
    fn rapid_doorbells_queue_one_resync() {
        let queued = std::sync::atomic::AtomicBool::new(false);
        let mut syncs = 0;
        for _ in 0..2 {
            if !queued.swap(true, std::sync::atomic::Ordering::SeqCst) {
                syncs += 1;
            }
        }
        assert_eq!(syncs, 1, "a burst must queue one sync, not one per event");
        queued.store(false, std::sync::atomic::Ordering::SeqCst);
        assert!(!queued.swap(true, std::sync::atomic::Ordering::SeqCst));
    }

    // a doorbell must reach the sync path; a play event must not be mistaken
    // for one (that would resync on every station change anywhere).
    #[test]
    fn only_a_doorbell_triggers_a_resync() {
        let queued = std::sync::atomic::AtomicBool::new(false);
        dispatch_stream_event(StreamEvent::ProfileChanged, &queued);
        assert!(queued.load(std::sync::atomic::Ordering::SeqCst));
    }
```

The announce side is proven on the real path rather than by a unit test — see the proof line below.

- [ ] **Step 2: Run to verify they fail**, implement the listener (reconnect loop, key re-check, single-flight debounce — copy the CLI's shape), the announce, and wire the dead `sync` command to a control in the Sync pane. Run again — PASS.

- [ ] **Step 3: Gate and commit**

```bash
cargo fmt && cargo fmt --check && cargo clippy --all-targets 2>&1 | tail -3 && cargo test --workspace 2>&1 | tail -3
git add crates/r4dio-macos
git commit -m "the macos app picks up changes the moment they happen"
```

**Real-path proof:** with the macOS app open, change the filter on another device and report how long until the Mac reflected it without a relaunch; and play a station on the Mac and confirm it appeared on the other device.

---

### Task 7: macOS shows the active filter

**Files:**
- Modify: `crates/r4dio-macos/ui/labels.js:121-123` (`filter_summary`)
- Modify: `crates/r4dio-macos/ui/main.js:47-55`, `ui/index.html` (popover indicator)
- Modify: `crates/r4dio-macos/src/commands.rs` (expose the filter to the webview)
- Test: `labels.js` has no test harness — add the label logic as a pure function in `crates/r4dio-macos/src/tray.rs` style if a Rust-side test is possible; otherwise assert the command's output shape in `commands.rs` tests.

**Interfaces:**
- Consumes: the filter from `MiniState` (Task 5).
- Produces: nothing downstream.

Follow Android's wording so the two surfaces read the same (`android/.../HomeState.kt:44-51`): `FILTER: UA·PL +2` — up to three codes joined with `·`, then `+N`. Hide it when the scope is favourites, as Android does.

- [ ] **Step 1:** write the label test (exact strings: `[]` → hidden; `["UA"]` → `FILTER: UA`; `["UA","PL","DE","FR"]` → `FILTER: UA·PL·DE +1`), run red, implement, run green.
- [ ] **Step 2:** gate and commit `show which countries the filter is limiting you to`.

**Real-path proof:** a screenshot of the macOS window with a UA filter active.

---

### Task 8: macOS remembers its volume

**Files:**
- Modify: `crates/r4dio-macos/src/state.rs:83` and `backend.rs` (persist volume)
- Test: `backend.rs` `mod tests`

Volume is per-machine, so it goes to a machine-local file — **not** `profile.json`. macOS shares the data dir with a possibly-running TUI; `save_merged` (`backend.rs:386-390`) exists for that concurrency and must be respected.

- [ ] **Step 1:** failing test (`a_saved_volume_survives_a_restart`), red, implement, green.
- [ ] **Step 2:** gate and commit `the macos app remembers how loud you had it`.

---

## Verification after all tasks (controller, not subagents)

1. Full gate green on the branch; whole-branch review before anything is pushed.
2. Live, on one real account, in this order: TUI filter UA+US → both countries in the list; change the filter on the phone with the TUI closed → launch the TUI → it shows the **new** filter; macOS shuffle honours the filter and skips a station blocked elsewhere; a play on the Mac reaches the phone; a phone change reaches an open Mac without relaunch.
3. Migration proven on a copy of the user's real `config.toml` (never `$HOME`).
4. Release notes must state that downgrading loses settings that moved out of `config.toml`, the same cost already accepted for `history.json`.

## Follow-ups recorded, deliberately not in this plan

macOS themes (needs themeable CSS — redesign work), macOS Recent/Blocked/Dead views, a block-station action on macOS, a real spectrum instead of `state.rs:55-60`'s fixed array, and the `no_emoji` exit asymmetry (`crates/radio-tui/src/tui/mod.rs:241-244`).
