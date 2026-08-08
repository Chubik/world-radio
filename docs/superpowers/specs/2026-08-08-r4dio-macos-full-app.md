# r4dio for macOS — the full app

**Spec. Written 2026-08-08.** Supersedes the scope of
`2026-08-07-r4dio-macos-design.md`, which deliberately built a surface over the CLI.

**Design handoff:** `_private/design-docs/design/macos/handoff-2026-08-08.html` (also
`~/Downloads/r4dio macOS App - Handoff.html`). Seven screens, all mocked up. This spec turns those
mockups into buildable requirements; where the two disagree, the handoff wins on appearance and
this document wins on behaviour.

## The problem

The shipped v1.13.0 menubar app assumes the CLI is installed next to it. Everything that manages
state lives only in the terminal. Holding the shipped app, the user asked:

> "we did not install the CLI. how do I set up sync from that little radio? how do I switch to
> favourites? how do I clear favourites? how do I filter?"

There is no answer today. Worse, the `Sync…` window asks for a key the app gives no way to obtain —
a dead end presented as a feature.

**Decision (user, 2026-08-08): the macOS app becomes a full player plus the menubar mini.**

## What already exists — this is mostly wiring, not new logic

Verified in `radio-core` before writing this spec. Every backend capability the design needs is
already implemented and tested:

| Need | Existing API |
|---|---|
| Generate an account | `sync::client::Client::create_account() -> Result<String>` (`client.rs:34`) |
| Sign in / store / clear a key | `sync::store_key`, `load_key`, `clear_key` (`sync/mod.rs:15-27`) |
| Delete the account | `Client::delete(key)` (`client.rs:71`) |
| Search by name | `Catalog::search_offline_filtered(&SearchQuery)` (`catalog/catalog.rs:118`) |
| Countries with counts | `Catalog::facets(limit) -> Facets { countries: Vec<(String, u32)>, … }` (`catalog.rs:140`) |
| Favourites | `Catalog::favorite_ids`, `toggle_favorite` |
| Blocked stations | `Catalog::blacklist_ids`, `toggle_blacklist` |
| Excluded countries | `Catalog::set_excluded_countries` |

So the work is: IPC commands, a window, and the UI. **No new domain logic in `radio-core`.**

## Scope, in build order

The order is by user harm, not by size.

### 1. Sync — an account without a terminal

Three states of one flow, per the handoff:

- **Not signed in** — "Sync your ★ favorites across devices. No email, no password — just one
  anonymous ID." + `⚡ Generate account` + `Have an ID? Sign in`.
- **Save this — shown once** — the full ID `r4-XXXX-XXXX-XXXX`, a copy affordance, and the warning
  that there is no email and no recovery. Requires an explicit "Done, I saved it".
- **Signed in** — a **masked** ID (`r4-7K2P-····-4DF1`), the favourites count, a QR for signing in
  on another device, and `Sign out`.

**Binding rule: after the "Save this" screen, the full key is never rendered again.** Not in the
window, not in a tooltip, not in a log, not in an error message. The shipped sync window already
honours this (`type="password"`, field blanked on every refresh, `has_sync_key` returns only a
bool) — keep that property.

The QR must encode the same key the Android app scans (`SyncActivity` already implements scanning).

### 2. Favourites — see them, play them, clear them

A list of the synced favourites: flag, station name, codec/bitrate, `● now playing` on the live
one, and a `★` per row that removes it. A `⇄ Shuffle favorites` action at the foot.

Clicking a row plays that station. Needs an empty state — a new user has none, and "no favourites
yet" must not look like a failure.

The list must scroll: this is not capped at the four rows the mockup shows.

### 3. Mini panel — keep it stripped

Already done in commits `4b963eb` and `6e5098d`, and confirmed by the user on screen. Removed: the
spectrum (drawn from a hardcoded array, never connected to audio), the VOL control (macOS and the
keyboard already own volume), the "shuffle scope" caption, and browser text selection.

**One requirement not yet met: the station name must always be fully readable.** The handoff
specifies a long name scrolling rather than being truncated — `Katherine FM — Katherine — 101.7` is
a real case from the user's own listening, not an edge case. Today it ellipsises.

### 4. Tray menu — the only surface that survives fullscreen

Per the handoff:

```
⇄ Shuffle — all stations      ⌥⇧R
★ Shuffle — favorites
⏯ Play / Stop
☆ Add to favorites
▣ Open r4dio
◇ r4-7K2P-····-4DF1
▸ Start at login              ✓
✕ Quit r4dio
```

`Open r4dio` opens the main window on Favourites. `Sync…` stops being a separate window and becomes
the masked-ID line, which navigates to Account.

**Note `Start at login` is new and not otherwise specified in this document** — it is a genuine
addition from the handoff. If it complicates the first release, drop the item rather than shipping
a toggle that does nothing.

### 5. Blocked stations — 14 of them, invisible today

The user's account holds 14 blocked stations (measured against the live server). They can only be
managed by a TUI keybinding. The window lists them with an `Unblock` per row.

### 6. Country filters — exclusions only

The user's account excludes `CH, CN, IN`. A searchable list of countries with checkboxes, showing
what is excluded and how many.

**Two binding rules:**
- **Exclusions only — there is no whitelist.** The user explicitly ruled out an "only UA" mode on
  2026-08-07. Do not build one.
- **RU/BY never appear in this list at all** — not as an entry, not as a disabled row, not as a
  pre-ticked exclusion. They are hard-filtered in code (`cache.rs:419`, `Catalog.kt:9`) and that
  stays invisible. This is a project requirement, kept private.

### 7. Browse / Search — the largest piece

A search field over the catalogue by name, and below it a country tree that expands to stations.

- Only countries **that have stations**, each with a count — `facets()` already returns exactly
  this shape.
- A result already in favourites is marked as such (`already in ★` in the mockup).
- A `☆` per row adds to favourites.
- The catalogue is ~56,000 stations: results must be bounded and the tree lazy. Do not render
  56,000 rows.

## The main window

- A real macOS window: titled, resizable, traffic lights, in the app switcher. **Not** a popover.
  The handoff mocks it at 900×600 with window chrome.
- Sidebar navigation, grouped as the handoff shows: **Library** (Favorites, Browse) ·
  **Filters** (Countries, Blocked) · **Account** (Sync). A footer line summarises state:
  `◔ 3 excluded · ⛌ 14 blocked`.
- Opening it from an accessory app needs the same policy dance the sync window already does
  (`ActivationPolicy::Regular` while open, back to `Accessory` on close, `prevent_close` so the
  close button hides rather than quits). That pattern is in `main.rs` and works.

## Fullscreen — a platform constraint, not a defect

**The panel cannot appear over another app's fullscreen space.** Confirmed by documentation and by
three failed attempts this session: collection-behaviour flags (`CanJoinAllSpaces` +
`FullScreenAuxiliary`, verified applied — the window reported `257`), a raised window level
(`101`), `orderFrontRegardless`, and finally converting the window with `tauri-nspanel`. None
worked; the plugin also broke click-to-close and was reverted.

The reason is structural: that behaviour is only valid for `NSPanel` subclasses, and even a
converted window does not reliably get it. See
[the write-up](https://philz.blog/nspanel-nonactivating-style-mask-flag/) and tauri issues
[#11488](https://github.com/tauri-apps/tauri/issues/11488), [#13034](https://github.com/tauri-apps/tauri/issues/13034).

**The design works around it rather than fighting it:**

- **A global hotkey for shuffle, `⌥⇧R`** — the daily action, with nothing to open. This is the
  highest-value item in the whole spec for how the user actually works: they are in a fullscreen
  terminal or Slack most of the day.
- **The tray menu** for everything else, because macOS draws it and it works everywhere.

A global hotkey needs the macOS Accessibility permission. **Ask for it right after first launch
with an explanation of why** — an unexplained permission prompt reads as spam. If the user declines,
the app must stay fully usable without the hotkey.

## Out of scope

- Signing and notarisation (unchanged from the previous spec).
- A whitelist country mode — explicitly refused by the user.
- Making the panel work over fullscreen — established above as not achievable.
- Themes beyond Amber CRT.

## Constraints

- Amber CRT palette from `themes.js`; IBM Plex Mono for chrome, IBM Plex Sans for station names.
- Plain HTML/CSS/JS served from `ui/` — no bundler, no framework. This has held for the panel and
  the sync window and should hold here.
- All code, comments and log strings in English; comments lowercase, explaining why not what.
- No AI/assistant reference anywhere in the product.
- No `else if`.
- Commit subjects are the public changelog.
- `cargo fmt`, `cargo clippy --workspace --all-targets`, `cargo test --workspace` clean.
  **Anything macOS-only must sit behind `#[cfg(target_os = "macos")]`** — CI runs clippy over the
  whole workspace on Linux, and this already broke the build once this session.

## Open questions for the plan

1. **One window or two?** The main window and the existing `sync` popover both exist. Folding the
   sync window into the main window's Account section is what the handoff shows — confirm before
   building both.
2. **Where does search run?** `search_offline_filtered` works against the local cache; the API
   client also has an online `search`. Offline is instant and works on a plane; online reaches all
   56k. Which one backs the Browse field, or both?
3. **`Start at login`** — genuinely new. Decide in the plan whether it makes the first cut.
