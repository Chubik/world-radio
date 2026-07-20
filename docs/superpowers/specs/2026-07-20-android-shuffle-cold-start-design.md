# Android shuffle() Cold-Start Hang — Design

## Problem

On a cold start of `PlaybackService` (process freshly created, e.g. the widget
tapped right after boot), `shuffle()` hangs and never plays. Instrumentation
(temporary logging, reverted) pinpointed the hang exactly:

```
shuffle() enter exo=true
shuffle: before scope        ← reached
loaded 954 stations          ← loadStations() thread finished
(shuffle: got scope)         ← NEVER printed
```

The coroutine stalls on its FIRST suspend line — `favStore.currentScope()`,
i.e. `store.data.map{}.first()`, the first DataStore("r4dio") emission in the
process. It is NOT `withReadyCatalog()` (an earlier fix guessed that spot and was
reverted).

Root cause: `shuffle()` runs on `Dispatchers.Main` and reads DataStore inline.
On a cold start several paths race for the first DataStore read at once —
`shuffle()` (Main), `syncNow()` (Main, also reads favStore), and `loadStations()`
(a background thread doing `runBlocking { favStore.currentExcluded() }`). The
first DataStore emission stalls under that contention, so the Main-dispatched
shuffle coroutine hangs indefinitely.

This is a pre-existing bug the widget path is merely the first to hit. Warm taps
(the app already played once, DataStore already emitted) do not reproduce it.

## Decision

Scope: fix `shuffle()` only (targeted). Do not touch `syncNow()` or other paths.

Move the three favStore reads in `shuffle()` off `Dispatchers.Main` and onto
`Dispatchers.IO`, each bounded by a `withTimeout` with a safe default fallback.
This unblocks the Main coroutine and guarantees shuffle makes progress even if the
first DataStore emission is slow:

- `currentScope()` → default `Scope.ALL` on timeout/failure
- `currentCachedFavs()` → default `emptyList()`
- `currentExcluded()` → default `emptySet()`

The fallback (ALL scope, no favs, no exclusions) is the correct degrade for
shuffle: it picks from the full catalogue, which is exactly what a scope-less
shuffle does. Timeout: 3000 ms per read.

`withReadyCatalog()` is left unchanged — it already works (`stations` is filled by
`loadStations()`; no DataStore race there).

## Data flow (after fix)

```
shuffle() [Main coroutine]
  → withContext(IO) { withTimeout(3000){currentScope}   ?: ALL
                      withTimeout(3000){currentCachedFavs}?: []
                      withTimeout(3000){currentExcluded} ?: [] }
  → withReadyCatalog()  (unchanged)
  → pickForScope(sc, cat, favs, excluded)
  → playPick / "nothing to play"
```

## Error handling

Each favStore read is wrapped in `runCatching { withTimeout(3000) { … } }`
`.getOrDefault(<default>)`. A `TimeoutCancellationException` or any read failure
yields the default; shuffle still plays. No exception escapes to crash the service.

## Testing / verification

Emulator-driven (same method already used to reproduce the hang): a temporary
`exported=true` test build lets an adb broadcast drive the real widget → session
path.

1. COLD: force-stop, broadcast WIDGET_SHUFFLE → a station plays (before the fix it
   hung at `currentScope`). Confirm a `playing <station>` log within a few seconds.
2. WARM: with a station playing, broadcast WIDGET_SHUFFLE → a different station.
3. Scope still honoured on a warm path: the fix only changes WHERE the reads run,
   not their result when DataStore is ready — a warm favs-scope shuffle still picks
   a favourite.
4. No crash / ANR; process stays alive.

All test-only edits (exported=true, logs) reverted before commit; working tree
clean except the shuffle() change.

## Out of scope

- `syncNow()` and other cold-start DataStore readers (chosen: targeted fix).
- Warming DataStore in `onCreate` (a broader alternative, not taken).
- Any widget UI / next-previous change (already done and committed).
