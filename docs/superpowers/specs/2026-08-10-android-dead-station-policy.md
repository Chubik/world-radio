# Android dead-station policy

**Spec. Written 2026-08-10.** The catalog cache shipped (PR #56) but nothing on Android ever
remembers a dead station: `onPlayerError` logs and reshuffles, and the same corpse comes straight
back into rotation. The CLI solved this in v1.7.0; this brings the same policy to Android.

## The policy (mirrors the CLI, decided 2026-08-09)

- **Hide on the first genuine stream failure.** "Genuine" means the station is at fault, not the
  device's network — see classification below.
- **A successful playback un-hides** the station and resets the blame budget.
- **Blame budget of 5:** after 5 hides with no success in between, stop hiding (keep skipping).
  This is the guard against a broken device network mass-hiding the catalog.
- **Applies to both scopes.** FAVOURITES ONLY also skips hidden stations — the star and the
  favourite record stay; only shuffle eligibility is affected, and one success restores it.
- **Persisted locally** in DataStore. Never synced, never in the backup file: stream health is a
  property of this device's network path, not of the account.

## Components

### 1. `StationHealth.kt` (new, pure logic — unit-testable without Android)

```kotlin
fun shouldBlame(errorCode: Int): Boolean
```

Blames the station for: `ERROR_CODE_IO_INVALID_HTTP_CONTENT_TYPE` (2003),
`ERROR_CODE_IO_BAD_HTTP_STATUS` (2004), `ERROR_CODE_IO_FILE_NOT_FOUND` (2005), all parsing errors
(3xxx), all decoding errors (4xxx). Does NOT blame: `ERROR_CODE_IO_UNSPECIFIED` (2000),
`ERROR_CODE_IO_NETWORK_CONNECTION_FAILED` (2001), `ERROR_CODE_IO_NETWORK_CONNECTION_TIMEOUT`
(2002), and any unknown code. Conservative default: when unsure, skip without blaming.

```kotlin
class HealthTracker(private val budget: Int = 5) {
    fun onError(blame: Boolean): Boolean  // true = hide this station
    fun onSuccess()                       // resets the blame counter
}
```

The tracker holds only the in-memory blame counter. `onError(blame = true)` returns `true`
(hide) while the counter is below the budget, and increments it; at the budget it returns
`false`. `onError(blame = false)` never hides and does not touch the counter. `onSuccess()`
resets the counter to zero.

### 2. `FavStore` additions (same pattern as `blocked`)

```kotlin
suspend fun currentHiddenDead(): Set<String>
suspend fun hideDead(uuid: String)
suspend fun unhideDead(uuid: String)
suspend fun pruneHiddenDead(keep: Set<String>)
```

Backed by a DataStore `stringSetPreferencesKey("hidden_dead")`. `pruneHiddenDead` intersects the
stored set with `keep`.

### 3. `PlaybackService` wiring

- `onPlayerError`: `shouldBlame(error.errorCode)` → if `tracker.onError(blame)` then
  `favStore.hideDead(current.uuid)`; then `shuffle()` exactly as today.
- `onIsPlayingChanged(true)`: `tracker.onSuccess()` and `favStore.unhideDead(current.uuid)`.
- In both pick sites (`shuffle()` and `startFrom`): read `currentHiddenDead()` alongside
  `currentBlocked()` and pass their **union** as `blocked` to `pickForScopeDetailed`. That one
  union covers the ALL scope and the FAVS scope (favs already filter through `blocked` via
  `FavLogic.pickFav`).
- **Not** applied to `fetchStations`, `fetchByUuids`, or the cache: health affects picking only,
  so the station stays in the catalog and the fav cache and can rehabilitate itself.
- After a successful catalog fetch in `fetchAndStore`:
  `pruneHiddenDead(catalogUuids ∪ favUuids)` so the set cannot grow unboundedly.

## Out of scope

Incremental catalog refresh (separate open item), any UI for viewing/clearing hidden stations,
syncing health across devices, changes to the CLI.

## Verification

- Unit tests (pure JVM): classification table above; tracker hides on first genuine failure,
  never hides on network failure, un-hides + budget reset on success, stops hiding at the budget;
  prune keeps only catalog/fav uuids; pick-site union actually excludes a hidden uuid
  (via `pickForScopeDetailed` with the union).
- `./gradlew test` green.
- Emulator check (never against real user data): play, kill a station's stream
  (or block its host), observe skip + no return of that uuid in subsequent shuffles; restart the
  app and confirm the station stays hidden.
