# TUI Startup Filter Preserved — Design

## Problem

On startup, with a saved search query (e.g. "club") and a saved status filter
(e.g. Favorites), the station list is NOT filtered — it shows the full/popular
list — until the user presses Tab or enters search, which re-applies the filter.

Root cause (found in code, not guessed): the worker's catalogue-population paths
each deliver the raw popular seed AS IF it were search results, overwriting the
active filter:

- `handle_sync_catalog` (worker.rs:475-478): after `replace_catalog`, sends
  `Msg::SearchResults(seed_rows_by_popularity(catalog))`.
- `handle_quick_top` (worker.rs:501-505): after `ingest`, sends the same.

Startup sequence:
1. `mod.rs:145` emits `WorkerReq::Search(restored_query, filters)` → worker returns
   correctly filtered rows. ✓
2. But `SyncCatalog` / `QuickTop` (mod.rs:169/171) finish a beat later and send
   `Msg::SearchResults(seed)` — the full popular list, ignoring query+status —
   overwriting `rows`. The filter is wiped. ✗
3. Pressing Tab/search emits a fresh `Search` → filter re-applies. ✓

The worker is stateless about query/filters (it only receives them via
`WorkerReq::Search`), so the seed paths have no knowledge of the active filter and
send raw rows.

## Decision

The catalogue-population paths must not push raw rows. Instead, after the catalogue
changes, the UI (which owns the current query+filters) re-issues a proper search.
The signals `CatalogSynced` / `QuickTopReady` are already sent by those paths — use
them.

- **worker.rs**: remove the `Msg::SearchResults(seed_rows_by_popularity(...))` send
  from both `handle_sync_catalog` and `handle_quick_top`. Keep the
  `CatalogSynced { count }` / `QuickTopReady { count }` sends. `seed_rows_by_popularity`
  (worker.rs:458) is used ONLY by those two sends, so after removing them it becomes
  dead code — CI runs `clippy -D warnings`, which fails on a dead function, so it
  MUST be deleted in the same change. (Note: `handle_quick_top` still needs
  `count` — derive it from the ingested catalogue, e.g. the length used for the
  `QuickTopReady { count }` payload; the current code takes `count = rows.len()`
  from the seed, so replace that with an equivalent count that does not require
  building the seed rows.)
- **update.rs**: in the `CatalogSynced` and `QuickTopReady` handlers, emit
  `Effect::Search(filters.to_query(query), filters.clone())` (the same expression
  `SubmitSearch` uses), combined with the existing `autoplay_random_if_pending`
  effects.

Chosen behaviour when the filter is empty (normal start, no query, status=All):
still re-Search. `handle_search` with an empty query + status All returns the same
popular list (`search_local` with no filter), so the user sees the same content —
one code path always, no special-casing.

## Data flow (after fix)

```
SyncCatalog done → replace_catalog → send CatalogSynced{count}   (no raw seed)
QuickTop done    → ingest          → send QuickTopReady{count}   (no raw seed)
   ↓ (UI)
CatalogSynced / QuickTopReady handler:
   → autoplay_random_if_pending (unchanged)
   → Effect::Search(filters.to_query(query), filters.clone())
   ↓ (worker)
handle_search → filtered rows honouring current query + status → SearchResults → rows
```

## Error handling

- No new failure modes. `Effect::Search` is the existing, exercised path.
- If the catalogue sync failed, `CatalogSyncFailed` (unchanged) still just clears
  loading — no re-Search needed there (no new rows).

## Testing / verification

Unit (update.rs, existing test style):
1. `CatalogSynced` handler now emits an `Effect::Search` carrying the model's
   current query + filters (assert the effect is present with the right query/status).
2. `QuickTopReady` handler emits the same.
3. A `CatalogSynced` with a Favorites status + query set produces an
   `Effect::Search` whose query.name and filters.status match (filter preserved).

Live (release build): start the TUI with a saved query (e.g. "club") + status
Favorites; the list is filtered immediately and STAYS filtered after the background
catalogue sync completes (previously it reverted to the popular list). Also verify
a plain start (no query, status All) still shows the popular list.

## Out of scope

- The startup `Search` emission in `mod.rs:145` (already correct; left as-is).
- The audio-stutter bug (separate backlog item).
- Any change to `seed_rows_by_popularity` itself (still used by nothing after this?
  verify: if it becomes unused, that is a separate cleanup — do not remove in this
  change unless the compiler flags it dead).
