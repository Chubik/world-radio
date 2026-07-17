# sync replace-semantics — design

## Problem

Sync is union-only. `PUT /sync` on the server merges the client's list with the
stored list and returns the union. So a removed favourite always comes back:

- TUI, scope Favorites, press `f` → row unfavourited locally (`worker.rs:98`)
- `handle_sync` fires immediately (`worker.rs:99`) → `push` sends the local list
  (already without the removed id)
- server unions it with its stored list → the removed id is still there
- `worker.rs:221-224` re-adds it locally → UI redraws → "flashes and reappears"

Proven by `client.rs` test `push_returns_merged`: push `favs:["c"]` returns
`["a","b","c"]`. The same defect applies to `blocked` and `excluded_countries`.

Union propagates additions but never deletions.

## Solution

1. **Server `PUT /sync` = replace.** Store exactly `{favs, blocked,
   excluded_countries}` from the body, return the stored value. No union.
2. **Merge only at link time, done by the client.** When joining an existing
   account: `pull → union(local, server) → push(replace)`. Once.
3. **Every edit (toggle `f`, syncNow, run-sync) = push(replace).** A removed
   favourite disappears everywhere.
4. **No version-gate.** Stats/accounts were wiped for a clean start; there are
   no old accounts to protect.

Rationale for client-side merge: the server stays a dumb key/value store; all
sync policy lives in the two clients (CLI + Android), which are easier to test.

## Data flow

```
edit (toggle f / syncNow / run-sync):
    local → push(replace) → server = local → apply response verbatim

link to existing account (scan / paste / cli use):
    pull(server) → union(local, server) → push(replace) → both sides equal
```

## Components

### A. server `world-radio-sync` (SEPARATE repo, manual deploy)

- `PUT /sync` handler: replace stored state with request body; return stored state.
- `GET /sync` unchanged.
- Deploy: pull as `deployer`, `docker compose build && up -d`. Not part of an
  app release.

### B. CLI — `radio-tui` / `radio-core`

- `worker.rs::handle_sync` (194): drop the union-hydration loops (221-230). Apply
  the server's response as a replacement of local state.
- `worker.rs` ToggleFavorite (97): push(replace) via the same path.
- `sync_cmd.rs::run_sync` (139): apply pushed result verbatim (already close;
  server change makes it correct).
- **New: merge-on-link helper** `link_and_merge(key)` = `pull → union(local,
  server) → push`. Shared by the new `use` command.
- **New CLI command `sync use <key>`**: store the key, then `link_and_merge`.
  This is the only way for the CLI to join an existing account (today `login`
  only creates a new one).
- `login` (create-new) does NOT run merge-on-link: a brand-new account has
  nothing to merge. Merge-on-link is only for joining an existing account.

### C. Android — `PlaybackService` / `SyncActivity` / `FavStore`

- `PlaybackService.syncNow` (191): push(replace); `applyMerged` applies the
  response verbatim (it already does — server change makes it correct).
- `SyncMerge.mergedFavs` (union) is used ONLY in merge-on-link, not on every
  syncNow.
- **Merge-on-link** inserted at the two join-existing-account entry points:
  - `SyncActivity` scan result (`SyncActivity.kt:37`)
  - `SyncActivity` use_key / paste (`SyncActivity.kt:80`)
  - `create` (`SyncActivity.kt:95`) mints a NEW account → no merge (nothing to
    merge; symmetric with CLI `login`).
- The merge-on-link runs: `pull → union with local favs/blocked/excluded →
  applyMerged locally → push(replace)`.

## Error handling

- Merge-on-link push fails (offline): keep the key stored, keep local state, show
  "sync failed — check connection" (CLI) / toast (Android). Next syncNow retries.
- Pull returns nothing / server empty: union with empty = local; push seeds the
  server with local state. Correct for first device.

## Testing

- `radio-core/sync/client.rs`: rewrite `push_returns_merged` — the mock server no
  longer unions; push returns exactly what was sent.
- `radio-tui`: unit test `link_and_merge` unions local+server then pushes; test
  that a normal toggle push does NOT re-add removed ids.
- Android `FavStoreSyncLogicTest` / `SyncClientTest`: replace semantics on
  syncNow; union only on link.
- Server repo `world-radio-sync`: replace test (PUT stores exactly the body).

## Out of scope

- The stat dashboard "57 years ago" panel (separate repo `world-radio-stat`,
  cosmetic) — tracked separately.
- Any version-gate / migration for old clients.

## Verification

- Emulator + CLI: link two "devices" to one account, toggle a favourite off on
  one, confirm it stays off after sync and does not reappear. Never touch the
  user's real data dir (use a throwaway account/key).
