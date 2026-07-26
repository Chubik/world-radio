# Android Home Screen — Design

Source: Claude Design project "radio" (a5023d91-…), handoff files
`r4dio - Home Screen.html` + `r4dio-home.jsx`. Local reference copies saved under
`docs/design/android/`. The design is the visual authority; this spec is how it
maps onto the existing app.

## Problem

The Android app has no real screen. `MainActivity` is a headless controller that
connects a `MediaController`, kicks off autoplay, and immediately `finish()`es.
Every control lives in the media notification, which has run out of room — after
Previous/Next were added (for steering-wheel shuffle), the **Sync** button was
pushed out of the visible notification, so the user can no longer reach Sync.

## Decision

Turn `MainActivity` into the real home screen from the design: a **giant
shuffle target** (tap anywhere → shuffle, eyes-free), a **now-playing** block, a
row of **secondary controls** (play/pause · star · scope · stop), and a labelled
**SYNC** bar that opens the existing SyncActivity.

- **UI stack: classic Android XML Views** (`setContentView(R.layout.activity_main)`),
  matching SyncActivity and the widget. No Jetpack Compose (none in the project;
  adding it for one screen bloats the APK and splits the stack).
- **State the screen shows and where it reads it:**
  - *Playing / paused* → `controller.isPlaying` + `Player.Listener.onIsPlayingChanged`.
  - *Station name* → `mediaMetadata.station`.
  - *country · codec* → `mediaMetadata.artist` (the service packs "SA · MP3 · 128k"
    there; parse the leading country and codec tokens for display).
  - *fav (★/☆) and scope (ALL / FAVS)* → **NOT visible to a controller today**.
    The service holds them in `favStore`. Publish them via
    `session.setSessionExtras(Bundle)` with keys `fav` (boolean) and `scope`
    (string "all"/"favs"); the screen reads them in
    `MediaController.Listener.onExtrasChanged`. This keeps the service the single
    source of truth. The service re-publishes the extras wherever it already calls
    `refreshCustomLayout()` (after shuffle, star toggle, scope toggle, connect).
- **Actions** map to the existing custom session commands — no new playback logic:
  - shuffle target tap → `CMD_SHUFFLE`
  - play/pause → `CMD_TOGGLE`
  - star → `CMD_STAR`
  - scope → `CMD_SCOPE`
  - stop → `CMD_STOP`
  - SYNC bar → start `SyncActivity` (existing)
- **Autoplay-on-launch stays**: the current `MainActivity` shuffles on connect when
  nothing is loaded. Keep that — but do NOT `finish()`; the screen stays up.

## Screen structure (from the design)

Portrait `activity_main.xml`, amber-CRT theme, dark ground `#15100B`:

1. **Status area** — the app does not draw the system status bar; use normal
   Android status bar. (The design's mock status bar is not part of the app.)
2. **Giant shuffle stage** — a single large tappable container, `weight=1`, that
   fills most of the screen; tap anywhere on it fires shuffle. Contains:
   - **Now-playing**: a kicker line — `NOW PLAYING` + an equaliser animation when
     playing, or `PAUSED` + `OFF AIR` when paused; the **station name** (mono, ~30sp,
     bold); a **context row**: country · codec · `★ FAVOURITE`/`☆ not saved` · a
     **scope pill** (`ALL STATIONS` or `FAVOURITES ONLY · N`).
   - **Hero**: a ~200dp ring with the shuffle glyph, label `TAP ANYWHERE — SHUFFLE`
     and subtext `random station · eyes-free`. When scope=favs but there are no
     favourites, the ring turns danger-red and the label reads
     `NO FAVOURITES YET — STAR ONE FIRST`.
3. **Secondary controls** — a 4-up row of quiet buttons: play/pause (reflects
   state), star (filled when current is fav), scope (shows ALL or FAVS + "scope"
   sublabel), stop (danger tone). Each is smaller than the shuffle target.
4. **SYNC bar** — full-width, amber outline, label `SYNC` + sub `link desktop ↔
   phone`. Opens SyncActivity.

**Landscape variant** (`layout-land/activity_main.xml`): the stage and the controls
sit side-by-side (design shows a horizontal split) — useful for a car mount. Same
components, re-flowed.

## Visual system

Colours already exist in `res/colors.xml` and match the design tokens (bg #15100B,
panel #1B1510, amber #D49A3A, amber-hi #FFC457, accent/orange #FF8A3D, bright/fg
#FFF0C0, dim #6E5430, rule #3A2C17, danger #D96A55). Add any missing ones
(`olive #9EC074` for the LIVE dot, `mute #8a7f64`).

Type: the design uses **IBM Plex Mono** (labels, station name, all mono UI) and
**IBM Plex Sans** (running text). Bundle both as `res/font/` resources
(static weights 400/500/600/700 that the design uses). If bundle size is a concern,
the fallback is the platform monospace + sans, but the design is built on IBM Plex —
default to bundling it.

Icons already exist as vector drawables and are reused: `ic_shuffle`, `ic_play`,
`ic_pause`, `ic_star` / `ic_star_outline`, `ic_scope_all` / `ic_scope_favs`,
`ic_sync`. `ic_stop` is MISSING and must be added as a vector drawable (the
design's stop glyph is a rounded square: 24dp viewport, `M6 6 h12 v12 h-12 z`
with rx≈2, `currentColor` fill). All other control icons already exist and are
reused as-is.

## Error handling / edge cases

- Controller connect fails → the screen still shows (station name/idle state);
  don't crash, don't finish. Buttons no-op until connected.
- Empty favourites + favs scope → the warn state (red ring + prompt), matching the
  design; shuffle on that state falls back to the full catalogue per existing
  service behaviour (the warn is a hint, not a hard block).
- `mediaMetadata.artist` missing/malformed → show the name only, omit the context
  line gracefully.

## Testing / verification

- Unit: the metadata-parsing helper (artist string "SA · MP3 · 128k" → country
  "SA", codec "MP3") — pure function, table-tested.
- Emulator (never touch the real user data dir): launch the app → home screen
  shows, a station is playing, name + context + scope pill render; tap the stage →
  a different station; play/pause toggles the kicker + button; star toggles ★/☆ and
  the extra updates; scope toggles ALL↔FAVS pill; SYNC opens SyncActivity; no crash.
  Confirm both portrait and landscape.

## Scope / out of scope

- **In:** activity_main.xml (+ layout-land), MainActivity rewrite, session extras
  for fav/scope, IBM Plex fonts, strings, any missing colour/icon.
- **Out (separate later specs):** Sync screen redesign (states A/B redline —
  separate handoff), notification/launcher/widget asset refresh, widget layout
  redesign. The notification is untouched by this work.
