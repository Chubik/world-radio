# TUI filter/search performance — design

## Problem

The TUI worker processes requests strictly one-at-a-time from an unbounded
channel with no coalescing and no cancellation (`worker.rs:69` — `for req in
req_rx`). Every Enter / filter change enqueues a fresh `Search`. This produces
every symptom the user hit:

| Symptom | Cause |
|---|---|
| list "jumps", flashes `0 results` / `connecting…` | N queued `Search`es each run and each renders an intermediate state |
| filtering 60k "lags" | a backlog of full filter passes drains serially |
| `q` hangs | quit does `worker_handle.join()` (`mod.rs:203`) which waits for the whole backlog |
| 6m hang → SIGTERM 143, audio stops | an online search blocks on the network and stalls the whole queue behind it |

Root cause: no "latest wins" for searches, no network timeout, and quit is
coupled to draining the worker.

## Solution (threads, not async)

The whole codebase is `reqwest::blocking` + `std::thread` + channels; a full
async/tokio migration would rewrite `radio-core` (shared by CLI + radio-mini) —
out of proportion to the fix. Threads + channels are already a concurrency
model; the fix is to use it correctly.

Four parts:

### A. Coalesce searches in the worker
Before handling a `Search`, drain the channel non-blocking (`try_recv` loop) and
keep only the **last** `Search`, replaying any non-`Search` requests in order.
Intermediate searches are dropped — only the newest query runs. This alone kills
the "jumps"/lag/backlog.

### B. Local-first, online in the background with a timeout
Any search returns the **local** (cache) result immediately — never empty, never
blocked. When there is a text query, an online fetch runs as a **separate step**
with a short timeout (~4s); if it returns, it sends an updated result; if it
times out or fails, the local result stands. The network can never block the
list. (Filtering-only is already local, from commit `e994331`.)

### C. Instant exit
Quit restores the terminal and returns immediately — it does **not**
`worker_handle.join()`. This is safe: state is already saved on every mutating
action (`save_all` in `worker.rs:80` / on toggle), so nothing is lost when the
worker thread is dropped with the process.

### D. Incremental results (already the shape)
"Got it → added to the list" is how online results already flow (a later
`SearchResults` replaces the rows). Coalesce + timeout make this smooth instead
of a pile-up.

## Data flow

```
Enter / filter → Search enqueued
worker: drain queue (try_recv) → keep last Search, replay other reqs
      → local result (instant, from cache)
      → if text query: online fetch (4s timeout) → updated result, or nothing
q → restore terminal + return   (worker dropped with process; state already on disk)
```

## Components

- `crates/radio-tui/src/tui/worker.rs`
  - the request loop: replace `for req in req_rx` with a loop that, on a `Search`,
    coalesces (drain via `try_recv`, keep last `Search`, run any interleaved
    non-`Search` reqs in arrival order).
  - `search_all` / `online_search`: emit the local result first, then attempt
    online with a bounded timeout; on timeout/err keep local.
  - the reqwest client for online search gets an explicit request timeout (~4s).
- `crates/radio-tui/src/tui/mod.rs`
  - quit path (~`mod.rs:200-205`): stop calling `worker_handle.join()`; restore
    the terminal and return. Keep the per-action save; drop the quit-time
    `SaveState`/`Shutdown`+join wait.

## Error handling

- Online timeout/failure → local result already shown; log at warn, no user-facing
  stall.
- Coalescing drops intermediate searches by design — not an error.
- Worker dropped at exit mid-request → fine; state persisted per-action.

## Testing

- **Coalesce (unit):** a pure helper `coalesce_search(pending: Vec<WorkerReq>) ->
  (Vec<WorkerReq> /* non-search, in order */, Option<Search> /* last */)` — given
  several `Search`es interleaved with other reqs, returns only the last `Search`
  and the other reqs in order. Test directly (no channel/network).
- **Local-first (unit):** `search_all` returns a local `SearchResults` even when
  online is unavailable (already covered in spirit by
  `online_only_when_there_is_a_text_query`; add one asserting a local result is
  produced without network).
- **Instant exit:** structural — the quit path no longer joins the worker; assert
  via code review that save happens per-action so no join is needed. (No good
  unit surface; verify live.)

## Out of scope

- Async/tokio migration.
- Priority channels (coalesce is enough).
- **Periodic catalog refresh** — separate spec (see the periodic-catalog-refresh
  memory); this spec does not change how the catalog is fetched, only how
  searches are scheduled and rendered.

## Verification

- Live in the TUI: rapid-fire Enter on a country → list settles to one result,
  no `0 results`/`connecting…` flicker; filtering is instant; `q` exits
  immediately; unplug network mid-text-search → local results stay, no hang.
