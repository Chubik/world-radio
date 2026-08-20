# Incremental catalogue refresh

Every client re-downloads the whole catalogue on every refresh — 4.3 MB gzipped,
54,729 stations — to learn about roughly 1,300 changed rows. This replaces that
with a delta, and stops rewriting the entire local database to apply it.

## What the numbers actually are

Measured against the live catalogue on 2026-08-19 and 2026-08-20, one real day
apart — not taken from the constants in the code:

| | |
|---|---|
| Catalogue | 54,729 stations · 13.5 MB JSON · 4.3 MB gzipped |
| Added in one day | 688 |
| Removed in one day | 653 |
| **Total churn** | **1,341 rows — 2.45%** |
| Existing stations whose fields changed | **0** |

Two of these matter more than the rest.

**The comment in `sync/src/catalog.rs` is wrong.** It says the catalogue "changes
by ~50 stations a day out of ~59k", and that number justifies the once-a-day
refresh interval. The real figure is 1,341 — off by a factor of 27. The refresh
interval is still defensible, but not for the reason written next to it. Fix the
comment while we are in there.

**Nothing mutates.** Across 54,041 stations present in both snapshots, not one
field differed. Every change is a whole station appearing or disappearing. This
is what makes the delta format trivial: added stations in full, removed stations
as bare UUIDs. No field-level merge, no conflict rules, no last-writer-wins.

If upstream ever does start mutating rows, `added` already covers it — the
client applies it as an upsert, so a mutated station arrives as a replacement of
itself. Nothing needs to change to handle that later.

## Three layers

The layers are independent and each is useful alone. They are listed cheapest
first, and that is also the order to build them.

### Layer 1 — clients revalidate

The server already answers `304 Not Modified`: `etag_matches` in
`sync/src/catalog.rs` exists precisely for this, and carries a comment about
Cloudflare weakening ETags that could only have been written by someone testing
it. **No client sends `If-None-Match`.** Verified by grep across the Android
source and every Rust crate: zero hits outside the sync server itself.

So on a day when the catalogue has not changed at all, every client still
downloads 4.3 MB to conclude nothing happened.

Each client stores the ETag alongside the catalogue, sends it back as
`If-None-Match`, and treats `304` as "done, nothing to do".

This alone takes a no-change refresh from 4.3 MB to a couple hundred bytes. It
does nothing on a day with changes — that is layer 2.

### Layer 2 — a delta on the wire

New endpoint:

    GET /catalog/delta?since=<snapshot-id>

Answers:

    { "id": "<new snapshot id>", "added": [ <station>, ... ], "removed": [ "<uuid>", ... ] }

The server keeps the **last 7 daily snapshots**. Asked for a `since` it still
holds, it answers the delta. Asked for anything older or unknown, it answers
`409 Conflict` with a body naming the full-catalogue URL, and the client falls
back to downloading everything — exactly what it does today.

Seven days is chosen against the refresh interval, not arbitrarily: the server
refreshes daily, so seven snapshots covers a client that has been offline for a
week. A phone left in a drawer for a month takes one full download and rejoins.

At the measured churn this is roughly **100–150 KB in place of 4.3 MB** — about
30x — and the saving grows as the catalogue does, because churn is proportional
to change, not to size.

**The snapshot id is the server's ETag, not a timestamp.** A client's clock can
be wrong, skewed, or reset; the ETag is already computed, already stored, and
already the thing that decides freshness in layer 1. Reusing it means the two
layers cannot disagree about what version a client holds.

### Layer 3 — apply the delta to the database

`Cache::replace_all` opens a transaction, runs `DELETE FROM stations`, `DELETE
FROM stations_fts`, then inserts all 54,729 rows twice over — once into the table
and once into the FTS index. That is roughly 109,000 statements to apply 1,341
changes.

Add `Cache::apply_delta(added, removed)` beside it: upsert the added rows,
delete the removed ones, both tables, one transaction. About 1,300 statements
instead of 109,000.

`replace_all` stays exactly as it is. It is the fallback path and the first-run
path, and it is not going away.

This is the layer that answers the note in memory about the catalogue growing
45x and turning previously-free operations into main-thread stalls.

## What each side does

**`sync` (separate repo, deploys itself)** — retain the last 7 payloads rather
than only `current`; add the delta endpoint; fix the wrong churn comment.

**Android** — `Catalog.kt`: the revalidation and delta sequence below, in
Kotlin. Note that the memory entry claiming Android "refetches the top-1000" is
**out of date** — since the move to our own server it fetches the full catalogue
from `CATALOG_URL`, and `fetchOnce` is only the radio-browser fallback.

### The CLI and macOS are out of scope, and why

They cannot use any of this yet. `handle_sync_catalog` in
`radio-tui/src/tui/worker.rs` calls `api::resolve()`, which resolves to
`all.api.radio-browser.info` — **not r4dio.net**. The CLI and the macOS app have
never fetched the catalogue from our own server; only Android was moved over.

So an ETag, a `304`, and a delta endpoint are all unreachable for them: they are
talking to somebody else's server, which does not speak our protocol.

Moving them across is worthwhile on its own — it would replace 13 upstream
requests and 19 seconds with a single 4.3 MB response — but it is a different
change with a different risk: different field names (`uuid`/`name`/`country`
against `stationuuid`/`countrycode`), a different payload shape, and a fallback
to radio-browser that has to keep working when our server is down. It gets its
own spec, its own plan and its own release.

`radio-core`'s `apply_delta` (layer 3) is still built here, in `cache.rs`. It is
where the shared cache lives, and having it ready is most of what the CLI switch
will later need.

## The refresh sequence

Every client follows the same shape:

1. Hold no catalogue → full download. Done.
2. Send `If-None-Match: <etag>`. `304` → done, nothing changed.
3. Otherwise ask for the delta with the stored snapshot id.
4. Anything other than `200` — `409` (snapshot too old), `404` (server has no
   delta endpoint at all), a parse failure, any network error → full download.
5. Apply the delta, store the new snapshot id.

Step 4 is the whole safety design: **the delta is an optimisation that is allowed
to give up.** Any doubt at all falls back to the path that works today. A new
client against an old server, or an old client against a new server, both work —
the old client never asks for a delta, and the new one gets a 404 and falls back.

## Failure behaviour

`replace_all` refuses an empty dump today ("refusing to replace catalog with an
empty dump"), and the same instinct applies here: a delta whose `removed` list
covers most of the catalogue is a bug, not a day's news. `apply_delta` rejects a
delta that would remove more than half the local rows, and the caller falls back
to a full download.

A delta that fails mid-apply rolls back with the transaction. The stored
snapshot id is written only after a successful commit, so an interrupted refresh
retries the same delta rather than skipping it.

## Testing

The measurement above came from two real snapshots a day apart, and those are
the fixture: a test that builds a delta between them and asserts the row counts
match what the two dumps actually differ by.

Beyond that, per layer: that a `304` leaves the catalogue untouched; that a
`409` triggers the full path; that `apply_delta` and `replace_all` reach an
identical database given the same end state; that the mass-removal guard trips;
that an old client and a new server still work in both directions.

The last one matters most and is the easiest to skip. Test it against real
binaries, not mocks — this repo has a memory entry about a fixture that bypassed
the very path it was meant to prove.

## Deploy order

The server ships **first**. The new endpoint bothers nobody until a client asks
for it, and clients must never ask a server that cannot answer.

`sync` is its own repository with its own deploy — see the memory on deploy
architecture. Probe the live API to confirm what is deployed rather than reading
the repo.

## Not doing

- Field-level merge — nothing mutates, and `added` covers it if that changes.
- Deltas of arbitrary depth — 7 days against a daily refresh, then a full dump.
- Removing `replace_all` — it stays as the fallback and first-run path.
- Moving the CLI and macOS onto our `/catalog` — worth doing, its own spec.
- Touching how often clients refresh, or the Data Saver question. Separate
  decision, already recorded separately.
