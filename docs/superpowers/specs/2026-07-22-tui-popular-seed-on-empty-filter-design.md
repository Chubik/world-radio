# TUI Popular Seed On Empty Filter — Design

## Problem

The startup-filter fix made `CatalogSynced` / `QuickTopReady` re-issue
`Effect::Search(query, filters)`. For an EMPTY filter (no query, status All), that
Search runs `cache.search()` whose SQL ends `ORDER BY name` — an ALPHABETICAL full
catalogue. The previous behaviour restored the POPULAR list (`list_by_popularity`,
votes DESC). So after a background catalogue sync (fresh install, or >24h stale —
the common "opened it again next day" path), a no-filter user now sees obscure
alphabetical stations at the top instead of popular ones. Verified regression
(final whole-branch review).

## Decision

In the `CatalogSynced` / `QuickTopReady` handlers, re-issue `Search` ONLY when a
filter is actually active; otherwise restore the popular seed.

- "Filter active" =
  `!query.trim().is_empty() || filters.status != StatusFilter::All || !filters.is_empty()`
  (`BrowseFilters::is_empty()` already covers countries/tags/codecs/bitrate; status
  is checked separately because it is not part of `is_empty()`).
- Filter active → `Effect::Search(filters.to_query(query), filters.clone())` (current).
- Filter empty → a new `Effect::PopularSeed` → new `WorkerReq::PopularSeed` → worker
  runs `catalog.list_by_popularity(&fav_ids, 200)`, maps rows via the existing
  `station_to_row`, and sends `Msg::SearchResults(popular_rows)`.

This keeps the startup-filter fix (active filters preserved through sync) AND
restores popular ordering for the empty case. The popular-seed logic returns, but
now UI-gated (never a blind overwrite — that gating is what fixed the original bug).

Scope: `worker.rs` (new `PopularSeed` req + handler), the effect layer + `update.rs`
(new `Effect::PopularSeed` + the gate). Core `cache.search()` is NOT changed
(chosen over widening its ORDER BY, to avoid touching every empty search).

## Data flow (after fix)

```
CatalogSynced / QuickTopReady handler:
  → autoplay_random_if_pending (unchanged)
  → filter_active ? Effect::Search(query,filters) : Effect::PopularSeed
     ↓ (worker)
  Effect::Search      → handle_search → filtered rows (ORDER BY name within filter)
  Effect::PopularSeed → handle_popular_seed → list_by_popularity(&fav_ids,200) → SearchResults(popular)
```

## Components

### worker.rs
- Add `WorkerReq::PopularSeed`.
- `handle_req`: `WorkerReq::PopularSeed => handle_popular_seed(catalog, msg_tx)`.
- `handle_popular_seed`: `catalog.list_by_popularity(&fav_ids, 200)` (fav_ids from
  `catalog.favorite_ids()`), map each via `station_to_row(s, is_favorite, is_hidden)`
  (same mapping mod.rs:59 uses), send `Msg::SearchResults(rows)` if non-empty. On
  error, `log_warn!` (lowercase) and send nothing — the existing seed/list stays.
- `coalesce`: `PopularSeed` is a non-Search req, so it flows through the `others`
  path like `LoadFacets` — no special coalescing needed. (It is idempotent; running
  it more than once yields the same popular list.)

### effect layer + update.rs
- Add `Effect::PopularSeed` to the Effect enum; map it to `WorkerReq::PopularSeed`
  wherever effects are run into worker requests (mirror how `Effect::Search` maps).
- `CatalogSynced` / `QuickTopReady` handlers: compute `filter_active` and push
  either `Effect::Search(...)` or `Effect::PopularSeed`.

## Error handling

- `list_by_popularity` error → logged, no message sent; the current list is left as
  is (no crash, no blank screen beyond what already showed).
- No new panic paths.

## Testing / verification

Unit (update.rs):
1. `CatalogSynced` with empty query + status All + empty filters → emits
   `Effect::PopularSeed` (NOT `Effect::Search`).
2. `CatalogSynced` with query "club" → emits `Effect::Search` with that query
   (existing behaviour still holds).
3. `CatalogSynced` with empty query but status Favorites → emits `Effect::Search`
   (status makes the filter active).
4. Same three for `QuickTopReady`.

Live (release build, isolated HOME): with a populated catalogue and `should_sync`
forced (stale/again-next-day):
- Empty query start → after sync completes, the list is the POPULAR order (top rows
  are high-vote stations), not alphabetical.
- query "club" start → stays filtered through sync (regression fix didn't break the
  original fix).

## Out of scope

- `cache.search()` ORDER BY (left as-is).
- The already-committed startup-filter fix commits (cc565f3, 387f292) — this is
  additive on top.
- Android work / audio bug.
