# macOS window — TUI redesign

The main window is rebuilt around the TUI-style mockup: top tabs instead of a
sidebar, a now-playing strip that never leaves the screen, and station lists that
read like the terminal browser.

## Why

The sidebar groups five sections under three headings, which reads as a settings
window rather than a radio. The mockup answers that: the categories become a
horizontal row, what is playing is always visible, and a station row is the thing
you act on — arrow to it, press return, it plays.

## Scope

**In:** the window shell (tabs, now-playing strip, keyboard hints footer), TUI
station rows, keyboard navigation, filters as a column beside the browse list,
and a way into the full window from the tray panel.

**Out:**

- **HISTORY tab.** The data exists (`Play { id, at }`, synced, on disk) and drives
  "play last" from the tray, but a visible list only earns its place if the user
  hits "something good played and I did not star it". Not observed yet. Library
  ships as FAVOURITES + BLOCKED; adding a third sub-tab later is a `history()`
  command plus a list, because the data is already there.
- **Themes / APPEARANCE.** Deferred until the shell is on screen and it is clear
  what Settings actually needs. Theme is synced, so it is a data decision, not a
  visual one, and it should not ride along with a layout change.
- **A real SIGNAL column.** Stations carry `codec` and `bitrate`; there is no
  per-station signal measurement, and inventing one would be a lie in a column
  that looks measured. The scale is derived from bitrate and labelled as such.

## Shell

Five sections collapse into four tabs:

| Tab | Holds |
|---|---|
| NOW PLAYING | the current station, large |
| BROWSE | search + country tree, filters in a column beside it |
| LIBRARY | FAVOURITES / BLOCKED sub-tabs |
| SETTINGS | countries + account |

`SECTIONS` in `labels.js` currently lists the five old ids and `activeSection`
falls back to `favourites`. Both change together, and the tray sends section ids
(`show_main(app, "favourites")`, `"sync"`), so those call sites move with it —
an unmapped id must land on a real tab, not a blank pane.

## Now-playing strip

Directly under the tabs, on every tab. Reads `now_state`, which the popover
already polls. It is a status line, not a second player: no controls beyond what
the row itself offers, so it cannot drift out of sync with the panel.

## Station rows

`▸` cursor on the selected row, then STATION / CC / CODEC / SIGNAL. Clicking a row
already plays it (`views/browse.js` → `play_uuid`); the cursor makes that legible
rather than changing it. The signal scale comes from `bitrate` — five marks at
256k and up, fewer below. The currently-playing station can also show
`⏳ buffering…`, because that state is known from `now_state`.

`✗ dead` is not shown per row: deadness is tracked as health on the Android side,
and surfacing it here needs a backend path that does not exist yet.

## Keyboard

`↑ ↓` move the cursor, `↵` plays, `F` stars, `B` blocks, `⌘F` focuses search,
`⌘1–4` switch tabs. The footer shows these, contextual per tab, replacing the
large buttons. `⌥⇧R` (global shuffle) already exists and is untouched.

## Tray → window

Left-click on the tray icon keeps opening the popover — that answers "what is
playing" in one click, which is what a tray music app is for. The full window
gains a row inside the panel, so the path is click → panel → click, instead of
hiding behind a right-click menu.

## Testing

The UI tests are standalone `*.test.html` pages run in a browser, not in CI.
`labels.test.html` covers `activeSection`, so the section rework updates those
cases. Pure functions (tab mapping, signal scale from bitrate, keyboard target
resolution) go in `labels.js` where they can be tested that way; DOM wiring is
verified by running the window.

## Risks

- **`views/*.js` are not rewritten.** They mount into a host element and do their
  own rendering; the shell changes around them. Rewriting them would put list
  behaviour and layout in the same change with nothing to bisect.
- **The mockup describes the window, not the tray panel.** The panel entry is
  built so it survives the redesign rather than being folded into it.
