# Live catalog — faster dead-station hiding and background refresh

Date: 2026-07-27
Status: approved

## Context

The station catalog is a snapshot. It syncs once at TUI startup when the 24h TTL
has expired (`should_sync`, called only from `radio-tui/src/tui/mod.rs:76`), and
never again while the app runs. Dead stations are hidden only after the user
personally fails to play them **three times** (`HIDE_THRESHOLD = 3` in
`catalog/health.rs`), which makes the user the sole detector of dead streams in a
51,000-station catalog.

Three consequences, all reported by the user:

1. Dead stations keep surfacing — you have to hit the same corpse three times.
2. A long-running session never refreshes; the catalog ages for as long as the app
   stays open (and radio is background software, so sessions are long).
3. New stations never appear mid-session.

## Goals

Make the catalog maintain itself: hide dead stations on first failure without the
user having to hunt them, absorb the server's own liveness verdict for free, and
refresh in the background during a long session.

## Non-goals

- **No Android work.** The Android app has no catalog database at all — it holds
  `@Volatile var stations` in memory and refetches 1000 stations per cold start,
  with no TTL. Giving it a cache is a different architecture (Room/DataStore +
  WorkManager) and gets its own spec. Backlog.
- **No stream probing of our own.** Actively pinging 51k streams is real traffic
  for little gain over the two cheaper signals below.
- **No dedup work.** `ingest` upserts on `stationuuid` via
  `ON CONFLICT DO UPDATE`, which is already correct.
- **No change to the RU/BY ban** applied on ingest.
- No change to how the visible list is built or sorted.

## Design

Three independent pieces.

### 1. Hide on first real failure, with error classification

`HIDE_THRESHOLD` drops from 3 to 1, but only failures attributable to the
*station* count. Failures attributable to the *user's network* must not.

This is feasible because the cause already reaches the decision site and is simply
discarded: `slot.rs:276` sends `Status::Error(e.to_string())`, and
`update.rs:262` matches `Status::Error(_)`, throwing the payload away.

Introduce a classification at the audio boundary, carried on the status:

- `StreamDead` — the origin answered and the answer was fatal: HTTP 4xx/5xx, DNS
  resolution failure for the stream host, connection refused, or a body that
  decodes to nothing. Counts against the station.
- `NetworkDown` — no usable connectivity: connect timeout with no route, or a
  transport error that would affect any host equally. Never counts.

Only `StreamDead` reaches `note_play_failure`. `NetworkDown` marks the row
transiently in the UI (as today) but writes nothing to `station_health.json`.

**The blast-radius guard already exists.** `AUTO_SKIP_MAX = 5`
(`update.rs:252`) stops auto-skipping after five consecutive failures. Reuse that
same counter as the health guard: once `auto_skip_count` reaches the maximum,
stop recording health failures for the rest of the streak, regardless of
classification. A run of five failures is far more likely to be a broken local
network than five simultaneously dead stations, and misclassification is then
capped at four wrongly-hidden stations rather than an entire browsing session.

Hidden stations remain recoverable: `WorkerReq::RecheckAll` already clears all
health, and per-station `clear` exists.

### 2. Absorb the server's liveness verdict

`Station` does not currently parse the check fields the API already returns in the
same response. Add them:

- `lastcheckok: u8` (1 = the server's last probe succeeded)
- `lastchecktime_iso8601: String`

On `ingest`, a station arriving with `lastcheckok == 0` is recorded as a health
failure immediately, so it is hidden without the user ever meeting it. This costs
no extra requests — the fields ride along in the existing search response.

**This is a secondary signal, deliberately.** Sampling the live API on 2026-07-27
(200 stations, `hidebroken=false`) returned `lastcheckok=1` for all 200, with
`lastchecktime` values spread from 2026-01-14 to 2026-07-18 — most from January.
The server's verdicts are stale and it mostly does not return known-dead stations
at all. So this filter catches the few obvious corpses cheaply, and piece 1
remains the primary mechanism.

Ingest must not *clear* health for stations reporting `lastcheckok == 1`: a stale
"ok" from January must not resurrect a station the user found dead last week. The
user's own experience always wins over the server's.

### 3. Refresh during a long session

Today the TTL is consulted once, at startup. Add a periodic check in the existing
TUI worker: every hour of wall-clock, evaluate `should_sync` against the stored
`last_sync` with the same 24h TTL, and if it has expired, run the same catalog
sync path startup uses.

The refresh is silent. It updates the database and must not rebuild or reorder the
list under the user's cursor: no selection jump, no scroll jump, no visible
repaint beyond rows that genuinely changed. New stations become visible on the
user's next search or filter change, not by mutating the list mid-scroll.

Reuse the existing machinery rather than adding a parallel path: the worker
already owns `WorkerReq::SyncCatalog` and the `CatalogSynced` message, and the
startup gating logic in `update.rs` (`catalog_refresh_effect`) already decides
correctly between re-searching an active filter and restoring the popular seed.

## Files

- `crates/radio-core/src/catalog/health.rs` — threshold 3 → 1.
- `crates/radio-core/src/catalog/station.rs` — parse `lastcheckok`,
  `lastchecktime_iso8601`.
- `crates/radio-core/src/catalog/catalog.rs` — record dead-on-ingest; keep
  user-recorded failures authoritative.
- `crates/radio-audio/src/slot.rs` (and the status type it sends) — classify the
  error into `StreamDead` / `NetworkDown`.
- `crates/radio-tui/src/tui/update.rs` — read the classification in `auto_skip`;
  suppress health writes past `AUTO_SKIP_MAX`.
- `crates/radio-tui/src/tui/worker.rs` — hourly TTL check.

## Testing

Unit tests carry this; there is no reliable way to fake a dead stream in CI.

- Threshold: one `StreamDead` hides; one `NetworkDown` does not.
- Guard: a streak past `AUTO_SKIP_MAX` records no further health failures.
- Ingest: `lastcheckok == 0` hides on ingest; `lastcheckok == 1` does **not** clear
  an existing user-recorded failure.
- Classification: a table of representative errors mapped to the expected variant.
- TTL: the hourly check fires a sync when the TTL has expired and does not when it
  has not (`should_sync` already has tests; extend to the periodic caller).

Manual verification: run the TUI, confirm a failing station disappears after one
failure and that pulling the network does not mass-hide the catalog.

## Risks

The real risk is wrongly hiding live stations. Two independent mitigations bound
it: classification (network errors never count) and the `AUTO_SKIP_MAX` guard
(a bad streak stops writing health at all). `RecheckAll` remains the escape
hatch, and the hidden set is a local JSON file the user can delete.

The secondary risk is that error classification is imprecise across the many ways
a stream can fail. Where a failure cannot be confidently attributed to the
station, it must be treated as `NetworkDown` — the safe direction is to under-hide,
since the old behaviour (three strikes) is the fallback the user already lives
with.
