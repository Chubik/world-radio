# One source of truth for listening settings

## The problem

A single setting is stored in four places on one desktop, and none of them owns it:

```
config.toml [filters].countries  -> draws the station list in the TUI
profile.json countries + at      -> what sync publishes
model.browse.filters.countries   -> in memory, drives the UI
model.profile.countries          -> in memory, drives sync
```

They are written at different moments by different paths: `config.toml` only at exit
(`crates/radio-tui/src/tui/mod.rs:245-256`), `profile.json` on every user change and on every sync
response. Nothing can reconcile them, because `config.toml` carries no timestamp.

The user hit this directly: a filter set on the desktop never reached the phone, because settings
chosen before the sync feature existed were never stamped. That specific gap is now closed
(`a9cb3d2`), but the structure that produced it is intact and has already produced three defects of
the same shape — an unstamped change, a filter-clear that published nothing, and an Android scope
tap that reached no other device.

The worst consequence is permanent, not transient: device B changes the filter while device A's TUI
is closed. A's `profile.json` takes the new value on the next sync, but A's next launch reads the
list from `config.toml` (stale), and A's exit writes that stale value straight back. Nothing heals
it — not another sync, not a restart. Only a manual edit on A.

## The rule

**Every setting has exactly one owner, and the owner follows the nature of the setting.**

| File | Owns | Why |
|---|---|---|
| `profile.json` | country filter, browse scope, theme, play history — each with its stamp | must look the same on every device |
| `config.toml` | layout, spectrum style, crossfade, keybindings, volume, glyph/colour tier | belongs to this machine; syncing it is meaningless |

`config.toml` stops storing `[filters]` and `theme`. In memory there is **one** copy: the UI reads
the filter from the same place it writes it, so "forgot to stamp" becomes structurally impossible
rather than merely unlikely.

`Model.browse.filters` keeps the fields that are genuinely per-session and unsynced (`query`,
`hide_unplayable`, `tags`, `codecs`, `bitrate_min`); only the three synced fields move out. Note
`tags` and `codecs` stay local but are still fixed for multi-value search below — that is a
separate defect in the same function, not a sync concern.

## Migration

First launch of the new build: values still present in `config.toml` are adopted into
`profile.json` if that field has never been stamped (`Profile::adopt_existing`, already implemented
and covered), then the `[filters]` and `theme` keys are dropped from `config.toml` on the next
write. An older build reading the new `config.toml` sees the defaults — the same downgrade cost
already accepted for `history.json`, and it must be stated in the release notes.

## Multi-country filtering

`BrowseFilters::to_query` (`crates/radio-tui/src/tui/model.rs:127-130`) sends only the first
element of each multi-value group:

```rust
countrycode: self.countries.first().cloned(),
```

Filtering is two-stage — the query narrows, then `matches_filters`
(`crates/radio-tui/src/tui/worker.rs:384-406`) ORs across the whole list. The second stage cannot
rescue what the first discarded: with `["UA","US"]` the SQL returns Ukrainian stations only, so US
stations are unreachable. Two tests currently pin contradictory contracts — `model.rs:601-616`
pins first-only as intended, `worker.rs:767-777` pins the full OR as intended.

The query carries the whole list. The local SQL takes `IN (...)`. Where a source accepts one value
only (the remote API), query each and union the results. The same first-only truncation applies to
`tags` and `codecs`; fixing countries without them would leave the same trap one field over, so
they are fixed together.

Note the behaviour is currently inconsistent by scope: in scopes other than All the query is
bypassed entirely and `narrow` alone applies, so multiple countries already work there. After this
change every scope behaves the same.

## macOS becomes a full client

macOS is a reduced client today, and the reductions are not what a user can see or predict. It
stores, merges and re-publishes the country filter but never applies it
(`crates/r4dio-macos/src/catalog_src.rs:15-18` searches with no country restriction, so
`pick_shuffle` ignores it), so on one account the filter governs Android and the TUI but not the
Mac.

**Root cause, and why the filter fell out here specifically.** `MiniState` holds the entire
catalogue — ~58k rows — pre-materialised in memory as `all` (`crates/r4dio-macos/src/state.rs:68-149`),
invalidated by hand on every mutation. Picking reads that Vec, so a filter cannot apply "for free"
the way it does where the catalogue is queried. macOS picks from a materialised list, the TUI
narrows a query, Android intersects predicates: **three independent implementations of one pick,
sharing no filter rules.** That is why the filter is missing on exactly one of them, and it is the
same defect class this repo has now hit six times — correct code that the path that matters never
reaches.

The fix is therefore structural, not a patch at the call site: pick from a filtered query instead
of a materialised list, and hoist one shared predicate — `allowed_station(station, excluded,
blocked, included)` — into `radio-core::catalog` so the TUI and macOS agree by construction rather
than by discipline. Favourites bypass the filter, matching the rule already encoded on Android
(`android/.../Catalog.kt:96-97`): an explicit star outranks a broad taste filter.

Two further defects surfaced by the audit, both fixed here because they live on the same pick path:

- **The shuffle pool ignores the blacklist** (`catalog_src.rs:15-18` — only browse's `visible()`
  filters it), so macOS can shuffle into a station the user blocked elsewhere.
- **Plays are never announced to the mirror.** macOS receives nothing and sends nothing: a station
  played on the Mac does not appear on the phone or the TUI, though the reverse works.

**Live propagation.** macOS has no event listener at all — no `MirrorClient`, no `events(`, no
listener thread. It syncs at launch (`src/main.rs:89-91`) and inside `sign_in`
(`src/account.rs:97`) and nowhere else. The `sync` command exists and is registered
(`commands.rs:199-205`) but no UI code invokes it — dead code. macOS gains the same listener the
CLI and Android already have: reconnect loop, key re-check, single-flight debounce, and a
doorbell-triggered re-sync; plus the existing `sync` command wired to a real control so a manual
sync is possible.

**The active filter becomes visible.** Today the main window's footer counts *excluded* countries —
the opposite setting — and nothing anywhere names `profile.countries`. A filter that is invisible
*and* unapplied is indistinguishable from a broken account. macOS shows the active filter the way
Android does (`FILTER: UA·PL +2`, `android/.../HomeState.kt:44-51`).

**Scope stays at two.** macOS represents `All` and `Favorites`; an incoming
`recent`/`blocked`/`dead` continues to leave the local scope untouched and is re-published
verbatim, never approximated. This is deliberate and already tested
(`crates/r4dio-macos/src/backend.rs:26-29` and its four tests). One cleanup belongs here though:
`parse_scope` (`commands.rs:6-11`) is a third wire→scope parser that defaults unknown values to
`All`, contradicting `scope_from_wire`'s deliberate `None`. It is replaced by
`radio_core::sync::Scope::from_wire`.

**Volume** is in-memory only on macOS and resets to 0.8 every launch (`state.rs:83`). It is
per-machine, so it belongs in the machine-local config, not in `profile.json`.

Explicitly **not** in this work, to keep the scope honest: adding themes to macOS (the CSS is
hardcoded amber and themeable UI is redesign work), adding Recent/Blocked/Dead views, and adding a
"block station" action. They are recorded in the plan as follow-ups.

**Do not regress** what macOS alone has: the in-app updater, the tray with live-refreshed labels,
the ⌥⇧R global hotkey that works over another app's fullscreen space, popover positioning and its
reopen guard, the activation-policy switching, the server-side QR render that keeps the raw key out
of the DOM, and the RU/BY filter repeated at the UI boundary.

## Out of scope

- **Favourites, blocked, hidden countries** — set-merge with tombstones, they self-heal and work.
- **Theme on Android and macOS** — stored and re-published, rendered by nothing until the redesign.
- **History on Android** — a push queue with no local view, deliberate.
- **macOS: Recent/Blocked/Dead views, a block-station action, a real spectrum** (`state.rs:55-60`
  fakes an FFT from a fixed array) — real gaps, recorded as follow-ups.
- **The `no_emoji` exit asymmetry** (`mod.rs:241-244`) — real but unrelated; noted, not fixed here.

## How we know it works

Each claim is checked on the real path, not only in tests. The desktop and macOS checks run against
a live account; the Android side is emulator-verified, per this project's rule.

- filter changed on the phone while the TUI is closed -> launching the TUI shows the **new** filter
  (today it shows the old one and writes it back)
- UA+US selected -> the list contains stations from both
- macOS shuffle with a UA filter -> plays Ukrainian stations only
- macOS shuffle never picks a station blocked on another device
- a station played on the Mac appears on the phone and in the TUI
- a filter changed on the phone reaches an open macOS app without relaunching it
- the active filter is visible on macOS, not merely in effect
- exiting the TUI does not write a stale value over a synced one
- **migration from the user's real `config.toml`** — an existing filter moves into `profile.json`
  with nothing lost

## Risks

- Startup and shutdown paths are load-bearing and easy to break quietly; the migration runs exactly
  once per device and cannot be re-run to fix a bad outcome.
- Reading the filter from `profile` in the UI touches many call sites; a missed one shows a stale
  filter rather than failing loudly.
- Downgrading loses the settings that moved out of `config.toml`, as above.
- Replacing macOS's materialised 58k-row `MiniState.all` with a query changes the hot path behind
  the popover and the tray. Both are latency-visible, and the tray refreshes its labels on
  mouse-down specifically so the drawn menu is never stale (`src/main.rs:306-318`) — a slower pick
  would show up there first.
- macOS shares the data dir with a possibly-running TUI, and `save_merged` exists precisely for
  that concurrency (`backend.rs:386-390`). Anything new that writes settings from macOS has to
  respect it.
- This branch touches four surfaces at once (shared core, TUI, macOS, and the wire it all agrees
  on). The single largest risk is a change that is correct in the core and never reached on one
  surface — the exact failure this branch exists to remove. Every task states which real user path
  proves it, and no task is done on tests alone.
