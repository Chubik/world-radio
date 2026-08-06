# Sync Deletions — Design

**Status:** approved 2026-08-06

## The problem

Removing a favourite, un-hiding a country or un-blocking a station on one device does not
propagate. Worse, the other device *resurrects* it: measured on 2026-08-06, clearing
`excluded_countries` on the server and then syncing the phone left the phone showing `1 HIDDEN`
**and put `["DE"]` back on the server**.

Cause: nothing records *when* an item appeared or disappeared. Each client pushes its whole
current state and the server stores it verbatim (`store.rs:81 replace`), so the last device to
sync wins — and that is usually the device that deleted nothing. `SyncMerge.mergedData` on
Android makes it worse at link time by unioning local and server state, but the resurrection
happens even without it.

Note what is *not* broken: the server already replaces rather than unions (commit `8af5542`),
and the CLI already accepts the server response verbatim. Only deletion memory is missing.

## The rule

Every item carries the moment of its last action and whether that action was a removal:

```json
{ "id": "uuid-a", "at": 1754500000, "gone": false }
{ "id": "uuid-b", "at": 1754500120, "gone": true }
```

**Merge:** for each `id`, the record with the greater `at` wins; on a tie, `gone: true` wins.

One symmetric rule covers every case — added here / deleted there / both added / both deleted.
A `gone: true` record is a tombstone: the memory of a deletion, without which resurrection is
inevitable.

**Deliberately NOT "deletion always wins":** that would make re-starring impossible. The user
was explicit — *"перемагає час дії"*. Un-starring a station does not remove the station itself;
it keeps appearing in shuffle, so re-starring it is always reachable.

**The server stamps the time**, not the client. A device with a wrong clock must not be able to
win with a future timestamp. Clients only report *which* items they changed since their last
sync; the server assigns `at` on receipt.

**Tombstone lifetime: 90 days.** Records with `gone: true` older than that are dropped on write.
A device silent for more than 90 days can still resurrect an item; that is accepted.

## Compatibility

The server understands both wire formats, so each side can ship independently:

- **Old client** sends `{"favs": ["a","b"], ...}` → the server treats it as "these ids are
  present, stamped now" and behaves exactly as today.
- **New client** sends timestamped records plus a delta of what it changed.
- **Responses carry both**: the legacy arrays (for old clients) and the record list (for new
  ones). An un-upgraded phone keeps working throughout the rollout.

Concretely, a new client's `PUT /sync` body adds one optional field per set. `changed` lists
only what this device altered since its last successful sync; everything else is carried in the
legacy arrays and keeps whatever `at` the server already holds:

```json
{
  "favs": ["a", "c"],
  "blocked": [],
  "excluded_countries": [],
  "changed": {
    "favs":    [{ "id": "c", "gone": false }, { "id": "b", "gone": true }],
    "blocked": [],
    "excluded_countries": []
  }
}
```

A body without `changed` is an old client. Note `b` appears only in `changed` — a deletion is
absent from the legacy array by definition, which is exactly why the array alone cannot express
it.

## What changes where

### `sync` (server) — the substantive work

- `store::Account` gains a parallel record representation. Three new columns hold the
  timestamped records; the existing `favs` / `blocked` / `excluded_countries` columns stay and
  keep being written, which is what keeps old clients working. Migration follows the pattern
  already in `store.rs:31` (`pragma_table_info` + `ALTER TABLE`): existing ids become records
  with `at = accounts.updated_at`, `gone = false`.
- `PUT /sync` stops being a blind `replace`. It merges the incoming state into the stored state
  under the rule above, stamps `at` for the items the client flagged as changed, preserves the
  stored `at` for everything else, prunes expired tombstones, saves, and returns the result.
- **The merge rule lives here and only here.** Clients do not reimplement it.

### `radio` (CLI + Android) — the thin part

Both clients currently push flat state. They must additionally report a **delta**: which items
they changed since the last successful sync. That needs a small local operation log — on each
toggle record `(id, gone)`, and clear the log when a sync succeeds.

- CLI mutation points: `catalog.rs:141 toggle_favorite`, `catalog.rs:101 toggle_blacklist`,
  `catalog.rs:45 set_excluded_countries`. The log is a new `sync_pending.json` in the XDG data
  dir beside `favorites.json`, written through the existing `save_state` path so it cannot
  drift from the state it describes.
- Android: new keys in DataStore beside `fav_uuids`, written from `FavStore.toggleFav`,
  `setExcluded` and the blocked-station path.
- **`SyncMerge.mergedData` is deleted.** Clients no longer merge; this also removes the
  union-at-link-time bug in `SyncActivity.linkAndMerge`.

### Unchanged

`favorites.json` read format, `pickFav` / `reconcileFavCache`, mirror playback, keys and auth.

## Order of work

**Server → CLI → Android.** The server accepts both formats, so every step ships on its own and
nothing breaks in between.

## Testing

- The merge rule is a pure function: a table of cases (added / deleted / both / equal `at` /
  expired tombstone) on the server, mirrored in the clients' delta-building logic.
- Compatibility: an old-format request against the new server must produce today's behaviour.
- End-to-end, the scenario measured on 2026-08-06: hide DE on device A, clear it on device B,
  sync both, assert the pill clears on A **and** the server stays cleared. The same run must
  confirm that re-hiding afterwards still works, so a tombstone does not become permanent.
- The country filter test needs a **baseline**: DE appears in roughly 4 of 40 shuffles
  unfiltered, so "I saw no DE" proves nothing on its own.

## Risks

- **Clock skew** — mitigated by server-side stamping.
- **A delta lost before a successful sync** (crash between toggle and sync) degrades to today's
  behaviour for that item: it is pushed as unchanged and the server keeps its own timestamp.
  Acceptable — no data is destroyed.
- **The 90-day tombstone window** is a deliberate trade; a device silent longer can resurrect.
