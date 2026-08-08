# r4dio for macOS — design brief: the full app

**Status:** brief for design. Not a plan, not a spec. Written 2026-08-08.
**Decision (user, 2026-08-08):** the macOS app becomes a full player *plus* the menubar mini —
not a companion that assumes the CLI is installed.

---

## Why this brief exists

The shipped menubar app was designed as a *surface* on top of the CLI. Everything that manages
state — creating a sync account, listing and clearing favourites, filtering countries, browsing
the catalogue — lives only in the CLI.

The user, holding the shipped app, asked the questions that expose this:

> "we did not install the CLI. how do I set up sync from that little radio? add, or scan, or what?
> how do I switch to favourites? how do I clear favourites? how do I filter?"

There is no answer in the product today. A user who downloads `r4dio-macos.zip` and never touches
a terminal gets a shuffle button and nothing else. Sync is worse than missing: the `Sync…` window
asks for a key it gives the user no way to obtain.

## What already exists — do not redraw these

The August design pack (`/tmp/r4dio-design-new/mini/`, re-extract from `~/Downloads/radio (1).zip`)
already solves more than half of this. Reuse it rather than inventing:

- **`account.jsx` → `AccountSheet`** — a 280px sheet with two screens: `start` ("Sync your ★
  favorites across devices. No email, no password — just one anonymous ID." + **⚡ Generate
  account** + "Have an ID? Sign in") and `id` (shows `r4-XXXX-XXXX-XXXX` with a copy affordance).
  **This is the missing answer to "how do I set up sync without the CLI".** It was designed and
  never built.
- **`account.jsx` → `FavListPanel`** — a 264px list: flag, station name, codec/bitrate, `● now
  playing` on the live one, a `★` per row to remove it, and a `⇄ SHUFFLE FAVORITES` footer button.
  **This is the answer to "how do I see and clear favourites".** Also never built.
- **`account.jsx` → `FavToast`** — "Added to ★ Favorites" confirmation.
- **`mini-window.jsx`** — the menubar panel, already built and shipping.
- **`themes.js`** — the Amber CRT palette. `bg #15100b`, `panel #1b1510`, `fg #d49a3a`,
  `hi #ffc457`, `dim #6e5430`, `rule #3a2c17`, `ok #9ec074`, `err #d96a5a`, `bright #fff0c0`,
  scanline 0.16. IBM Plex Mono for chrome, IBM Plex Sans for station names.

## What needs designing

### 1. The main window — the piece that does not exist at all

Opened from the tray menu's `Open r4dio`. A real window (titled, resizable, in the app switcher),
not a popover. It needs to answer, in one place:

- **Favourites** — the list, with removal. `FavListPanel` is the component; it needs to grow into a
  full pane: scrolling for tens of stations, an empty state, and a way to play one directly.
- **Browsing / search** — today there is no way to find a station by name or country from the app.
  The CLI has the full catalogue. What does search look like here? A field plus results, or a
  browse-by-country tree? **This is the biggest open question in the brief.**
- **Country filters** — the user's account currently excludes `CH, CN, IN`, set from the CLI and
  invisible in the app. Needs a surface: pick countries to exclude, see what is excluded.
  Note there is deliberately *no* whitelist ("only UA") — the user ruled that out on 2026-08-07.
- **Blocked stations** — a station can be blocked ("never play this"). The user has 14. There is
  no way to see or unblock them anywhere except the CLI's keybinding. Same pane as favourites?
- **Sync** — `AccountSheet` slots in here. Generating an account must be possible *from the app*.
  Signing in on a second device needs the key: shown as text to copy, and as a QR to scan from the
  phone (Android already scans QR — see `SyncActivity`).

### 2. The mini panel — what it should keep

Recently stripped, after the user called out things that looked functional but were not:

- **Removed:** the spectrum bars (they were drawn from a hardcoded array and never touched the
  audio), the VOL control (macOS already owns volume, and the keyboard has keys for it), the
  "shuffle scope" caption, and browser text-selection.
- **Kept:** station name (**the user was explicit: the name must always be visible**), country ·
  codec · bitrate, the favourite star, `⇄ SHUFFLE`, play/pause, `ALL / ★ FAVS`.

Open question for design: with the main window existing, what is the mini panel *for*? Probably:
what is playing, shuffle, star, and a way into the main window. Anything heavier belongs in the
window.

### 3. Fullscreen — a platform constraint design must work around

**The panel cannot appear over another app's fullscreen space.** This is not a bug we can fix: that
behaviour is only valid for `NSPanel` subclasses, and converting the window (tried via
`tauri-nspanel`) did not deliver it and broke click-to-close. The user works fullscreen most of the
time, so **the design must not assume the panel is reachable.**

Implications to design for:
- The tray *menu* works everywhere, because macOS draws it. It is the only reliable surface in
  fullscreen. What belongs in it? Today: Shuffle, Play/Stop, Open r4dio, Sync…, Quit.
- The user suggested a **keyboard shortcut for shuffle** — change station without opening anything.
  This is likely the single highest-value addition for how they actually work. Global hotkeys need
  a macOS permission prompt; the design should account for asking.

## Constraints

- Amber CRT palette only. IBM Plex Mono / Sans.
- No AI/assistant reference anywhere in the product.
- RU/BY stations are hard-filtered everywhere; that is private and never surfaced as a setting.
- The sync account is anonymous — one ID, no email, no password. Never display a stored key back
  to the screen once saved; the current sync window treats it as a secret and should stay that way.
- Whatever is drawn must be buildable as plain HTML/CSS/JS: the crate serves `ui/` as static files
  with no bundler and no framework, and that constraint has held well.

## What to hand back

Screens for: the main window (favourites, browse/search, filters, blocked, sync), the revised mini
panel, and the tray menu's contents. Plus a view on the fullscreen problem — whether the answer is
the menu, a hotkey, or something else entirely.
