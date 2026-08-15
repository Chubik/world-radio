# macOS window — one screen

The main window is rebuilt around the second TUI-style mockup: what is playing
stays on the left at all times, and one list on the right answers "what next".

> **Two mockups existed.** The first drew four tabs (NOW PLAYING / BROWSE /
> LIBRARY / SETTINGS) and was built, then replaced the same day by the second,
> which folds all of it into one screen. This spec describes the **second**, the
> one that shipped. The mockup is styled after the terminal client, but this was
> app work — `radio-tui` was not touched.

## Why one screen

The most common action is choosing something new while still hearing — and
seeing — what is on now. Separate tabs hid that context every time the user went
looking. The left panel never changes; the right list filters by segment without
moving anything the eye is resting on. Settings stays a tab of its own because
theme, shortcuts and account are monthly decisions, not listening ones.

## Layout

**LIBRARY** — a 300px panel on the left: live equalizer, BUF and UP gauges,
station name, track, metadata, status line, transport (shuffle / play-stop),
three named actions (favourite / retry / info), volume, and the next station
queued by shuffle. On the right: search, an All / Favourites / History segment,
a filter chip row, and the station list.

**SETTINGS** — APPEARANCE / COUNTRIES / BLOCKED / SHORTCUTS / ACCOUNT.

## What the rows show

`▸` cursor, a star that toggles, name with its genre, flag and code, codec, and
a signal column. A station that keeps failing reads `✗ dead`; the one buffering
right now reads `⏳ buffering`. History swaps the signal column for WHEN.

## Where the data comes from

Everything drawn is something the backend actually knows:

| Shown | Source |
|---|---|
| track (`♪ …`) | ICY metadata on `Status::Playing { title }` |
| genre | `Station.tags`, first tag that is not a frequency |
| `✗ dead` | the health tracker, distinct from a user block |
| BUF % | `ringbuf` occupancy of the decode rings |
| UP | wall-clock stamp taken when the station started |
| NEXT | shuffle's next pick, drawn ahead and then actually played |
| signal | bitrate, and labelled as such — no signal is measured anywhere |

**`4 234 listeners` from the mockup is not built**: no such field exists in the
catalogue, and inventing one would be a number that looks measured.

## The keyboard

`↑ ↓` move, `↵` plays, SPACE play/stop, `F` favourite, `r` shuffle, `R` retry,
`M` mute, `I` info, `1–3` segments, `⌘F` search, `⌘1–2` tabs. `⌥⇧R` (global)
is untouched. Every key in the footer and the shortcuts list is handled — a hint
for a key that does nothing teaches the user to stop reading the row.

## The equalizer

Five styles (bars / mirror / dots / wave / off) and a sensitivity slider, both in
Settings → APPEARANCE. The analyser is `radio_core::spectrum` — the same FFT the
terminal client has always used, moved into core so both meters read one sound
the same way. It replaced a hardcoded 14-number array that could not move at all.

Style and gain travel with the account in the synced `settings` bag, falling back
to a local file for a machine that has never signed in.

## Traps this created, and how they are held

- **The tray sends old section names** (`favourites`, `sync`). `targetFor()` maps
  each onto a tab and sub-view; six tests pin it. Without that, "Open r4dio"
  opens a blank pane — the same unreached-path defect class this repo keeps
  producing.
- **The country switch is drawn inverted.** On means "plays"; what is stored,
  synced and sent is still the *excluded* list, because the account, the phone
  and the terminal all speak that. A test pins that the wire format is unchanged.
- **The queue must be kept.** Whatever NEXT names is what shuffle plays, and it
  is queued after `now` moves — queueing before it lets the panel promise the
  station that just started.

## Testing

`make check-window` parses every window module, checks every `getElementById`
against `main.html`, and drives the panel against a fake backend. It exists
because a duplicate `const` shipped a blank window while every Rust test passed:
`cargo` never looks at the JavaScript.
