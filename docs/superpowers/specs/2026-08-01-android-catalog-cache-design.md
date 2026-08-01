# Android catalogue cache

Date: 2026-08-01
Status: approved

## Context

The Android app has no catalogue storage at all. `PlaybackService` holds
`@Volatile private var stations: List<Station>` in memory and calls
`Catalog.fetchStations()` on every cold start, fetching the top 1000 stations by
click count (~173 KB). Nothing is kept between launches.

Three consequences, all reported by the user:

1. **Slow start** — pressing shuffle waits on a network round-trip before anything
   plays.
2. **Useless offline** — on the underground or without signal the app has no
   stations at all, not even to try.
3. **Cache holds whatever the top-1000 happened to include**, with banned and
   hidden-country stations filtered out afterwards, so the usable list is smaller
   than it looks.

The CLI solved the equivalent problem long ago (SQLite catalogue + TTL). This gives
the phone the same behaviour without the same machinery.

## Goals

Play instantly from a stored catalogue, work with no network, and make the stored
1000 stations genuinely 1000 *usable* stations.

## Non-goals

- **No Room / SQLite.** At ~173 KB for 1000 stations a plain file is enough, and
  Room would add a dependency, a schema, and migrations for no benefit at this
  size.
- **No WorkManager.** Refresh happens inside `PlaybackService`, which is already
  running whenever the app matters.
- **No catalogue search on the phone.** Browsing/searching the full 52k catalogue
  is a separate feature; this is about what shuffle draws from.
- **No change to the CLI or the sync server.**
- **No growth beyond 1000 stations.** The user explicitly chose to keep the memory
  footprint small.

## Design

### Storage: a plain file, not DataStore

The cache is a JSON file at `filesDir/catalog.json`. It must **not** go into the
existing Preferences DataStore: DataStore holds its entire contents in memory and
rewrites the whole file on every edit, so putting ~173 KB of catalogue beside the
favourites would inflate memory and make unrelated writes expensive.

Only the fields shuffle and the now-playing UI need are stored — uuid, name, url,
country, codec, bitrate — mirroring the existing `FavStation` pattern that already
persists favourites as JSON.

The freshness timestamp (`catalog_synced_at`, epoch seconds) goes in the existing
DataStore next to the other settings, where small values belong.

Writes are atomic: serialise to `catalog.json.tmp`, then rename over
`catalog.json`. A process killed mid-write must never leave a half-file.

Reads are total: any failure — missing file, truncated JSON, schema drift — is
treated as "no cache" and falls through to the network. A corrupt cache must never
crash the app or block playback.

### Fetching: over-fetch, then filter to a full 1000

The radio-browser API can only include a country (`countrycode=X`), never exclude
one, so the exclusions have to be applied client-side. Doing that naively — fetch
1000, drop the banned ones — leaves fewer than 1000 usable stations.

Instead the fetch asks for **1500** stations (`order=clickcount&reverse=true`,
which is already what the app requests today — the API's "top by popularity"),
applies the filters, and keeps the first 1000 that survive. If fewer than 1000
survive, the cache stores what there is; the app is still fully functional.

Two distinct filters, applied at different times, and this separation is
deliberate:

- **The RU/BY ban** (`isExcluded`, country codes plus name substrings) is applied
  **at fetch time**, so banned stations never reach the cache file at all. This
  matches how the CLI applies its ban on ingest and keeps the rule impossible to
  bypass by editing settings.
- **The user's hidden countries** (`FavStore.currentExcluded()`) are applied at
  fetch time **as well**, so the stored 1000 are 1000 the user can actually hear.
  They remain applied at pick time too — `pickForScope` already does this — so
  that hiding a country takes effect immediately without waiting for a refresh.

### Behaviour: play now, refresh in the background

On service start:

1. Read `catalog.json`. If it has stations, they become the in-memory list
   immediately — shuffle works with no network round-trip.
2. If the cache is empty or unreadable, fetch synchronously as today. This is only
   the first-ever launch.
3. If `catalog_synced_at` is older than the TTL (24 hours, matching the CLI), start
   a background refresh **without blocking playback**. When it returns, it replaces
   the in-memory list and rewrites the cache file.
4. If the refresh fails (no network, API down), log it and keep using the cache.
   A failed refresh must never clear the cache or stop playback.

`withReadyCatalog()`, which currently fetches when the in-memory list is empty,
keeps that role as the last-resort path.

## Files

- `android/app/src/main/kotlin/net/vchub/r4dio/CatalogCache.kt` — new: read, atomic
  write, and the slim serialisable station shape.
- `android/app/src/main/kotlin/net/vchub/r4dio/Catalog.kt` — over-fetch and filter
  to a target count.
- `android/app/src/main/kotlin/net/vchub/r4dio/FavStore.kt` — the
  `catalog_synced_at` key and its accessors.
- `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt` — load from
  cache on start, background refresh on stale TTL.

## Testing

JVM unit tests, no device required:

- Round-trip: stations written then read back are equal.
- A truncated or garbage file reads as empty, and does not throw.
- An atomic write leaves no `.tmp` behind on success.
- Over-fetch filtering: given 1500 stations of which some are banned, the result is
  1000 and contains none of the banned ones.
- Fewer survivors than the target yields all survivors rather than an error.
- TTL: stale timestamp triggers refresh, fresh does not (reusing the same
  boundary logic the CLI's `should_sync` tests cover).

Device verification on the emulator: first launch fetches; second launch plays with
no network request; airplane mode still shuffles; and no RU/BY station appears in
the cache file.

## Risks

The realistic failure is a corrupt or partially written cache making the app
unable to play. Both are addressed structurally: atomic rename means a reader
never sees a partial file, and read failures degrade to "no cache" rather than
propagating.

The second risk is the cache going stale forever if refresh never succeeds — but
since a stale cache still plays, this degrades to "old stations", not "no
stations", which is the correct direction.
