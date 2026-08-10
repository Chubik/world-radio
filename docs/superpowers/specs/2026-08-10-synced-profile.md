# Synced profile: filter, scope, history, theme

**Spec. Written 2026-08-10.** Trigger: the user had "UA only" filtered on the CLI, and Android —
freshly synced — shuffled the whole planet. Only favourites, blocked, and excluded countries
travel through the account today; the active filter, the shuffle scope, listening history, and
the theme are all per-device. The user's decision: the account carries the whole listening
profile — set it anywhere, have it everywhere.

## What syncs and what stays local (decided 2026-08-10)

| Through the account | Stays on the device |
|---|---|
| favourites, blocked, excluded countries *(already)* | keep-awake |
| **shuffle filter** (included countries, v1) | hidden_dead (stream health is per-network) |
| **scope** (ALL / FAVS) | platform hotkeys |
| **history** (what played, when) | autostart / launch behavior |
| **theme** | |

## Two merge semantics — do not mix them

**Record sets with tombstones** (existing machinery: favs/blocked/excluded): unchanged.

**LWW scalars** (new: `shuffle_filter`, `scope`, `theme`): a single value with a client-side
change timestamp. Server keeps `{"value": …, "at": <unix>}`; an incoming value with a newer `at`
replaces the stored one, an older one is ignored; the response always carries the winner. Setting
the filter to empty is a normal write ("no filter"), not a deletion.

**History** (new): a record set of `{uuid, at}` where `at` is the last time that station played.
Union-merge by uuid keeping the newest `at`; server caps the set to the newest **200** entries.
No tombstones — clearing history is a local action and does not propagate (v1).

## Server (`sync` repo — separate, auto-deploys on push to main)

- Migration in the existing style (`excluded_countries` precedent): four new TEXT columns —
  `shuffle_filter`, `scope`, `theme` (LWW JSON, `DEFAULT ''` = never set), `history`
  (records JSON, `DEFAULT '[]'`).
- `/sync` request/response extended with the four fields; every field optional — an old client
  that omits them gets them back untouched (backward compatible both ways).
- The shuffle filter value shape: `{"countries": ["UA", …]}` — an object, so genre/bitrate can
  join later without a schema change.
- The sync DB backup-before-deploy already exists; the migration runs on start as usual.

## CLI

- **Publish:** the browse filter's country list becomes the synced shuffle filter — stamped at
  the moment the user changes it, pushed on the next sync. Scope changes and theme changes stamp
  the same way. Every completed play appends `{uuid, at}` to a local history log that syncs as
  the history set.
- **Apply:** a newer incoming filter replaces the browse country selection (visible in the
  filter panel, exactly as if set locally); newer scope/theme apply the same way. Merged history
  lands in the existing history file.
- The CLI's current history file records plays already — it becomes the local half of the sync.

## Android

- **Apply filter:** ALL-scope picks intersect with the filter countries (same pick-site pattern
  as `blocked + hidden`; fetch and cache untouched). FAVS scope is exempt — the codebase's
  standing asymmetry: an explicit star outranks a broad taste filter.
- **Scope:** the local scope becomes the synced one (LWW both ways).
- **Push history:** every play pushes `{uuid, at}` — this is what makes "started it in the car,
  saw it at home" work. No history UI yet (comes with the redesign); pushing is the point.
- **Theme:** stored and synced; applied when Android grows themes (redesign). Harmless until then.
- **Indicator:** until the Catalog/Filters screen exists, an active filter shows as a small
  pill on the home screen (pattern of the existing excluded-countries indicator): `filter: UA`.

## Out of scope

Genre/bitrate in the filter (the JSON shape allows it later), history deletion sync, per-device
theme overrides, any new Android screens (redesign sub-projects own those), macOS app changes
(it shares radio-core with the CLI and picks the profile up through the same sync client).

## Verification

- Server unit tests: LWW newer-wins/older-loses/absent-untouched per scalar; history union cap
  at 200; old-client payload (fields absent) leaves everything unchanged.
- CLI: publish stamps and pushes; apply overwrites browse countries; play appends history.
- Android: pick honors filter in ALL, ignores in FAVS; empty filter = unrestricted; play pushes.
- End-to-end on live: set "UA only" in the TUI → sync → Android shuffle plays only [UA]
  stations (logcat station names); play something on Android → TUI history shows it.
