# r4dio Home Screen — redline reference

Extracted from Claude Design project "radio" → `r4dio - Home Screen.html` +
`r4dio-home.jsx`. Values are from the design's CSS (px → treat as dp/sp).

## Colours (design tokens → res/colors.xml)
- bg `#15100B` · panel `#1B1510` · panelHi `#211b13`
- amber `#D49A3A` · amberHi `#FFC457` · orange/accent `#FF8A3D` · olive(LIVE) `#9EC074`
- fg/bright `#FFF0C0` · mute `#8a7f64` · dim `#6E5430` · rule `#3A2C17` · ruleSoft `#241f18`
- danger `#D96A55` · heroInk `#241a08`

## Type
- IBM Plex Mono — kicker, station name, all labels/pills, sync label
- IBM Plex Sans — running/system text
- Station name: mono 700, ~30sp (landscape 26sp), letter-spacing -.01em, line 1.05
- Kicker: mono 10sp, letter-spacing .22em, uppercase, dim
- Context row: mono 11sp, mute
- Scope pill: mono 9.5sp, letter-spacing .1em, padding 3×9dp, radius 20dp
- Hero label: mono 700, 14sp, letter-spacing .14em, amberHi
- Hero sub: mono 10.5sp, dim
- Secondary btn label: mono 600, 10sp; sublabel 8sp dim
- Sync label: mono 700, 15sp, letter-spacing .16em

## Layout (portrait)
- Screen padding: main 8dp top / 16dp sides / 16dp bottom; gap 14dp
- **Stage** (giant tap target, weight 1): border 1dp rule, panel bg, radius 22dp,
  padding 22×20dp. Contains now-playing (top) + hero (centered, flex 1).
- **Hero ring**: 200dp circle (landscape 168dp), 2dp border rgba(amberHi,.28),
  radial amber glow, breathing animation. Shuffle glyph 128dp (land 108dp).
  Warn state: danger border/glow, no animation.
- **Secondary controls**: grid 4 columns, gap 10dp. Each button: border 1dp rule,
  panel bg, radius 14dp, padding 12×6×9dp, min-height 66dp, icon 26dp. `on` state:
  amber border + rgba(amberHi,.09) bg + amberHi icon/label. Stop = danger tone.
- **Sync bar**: full width, height 56dp (land 50dp), radius 14dp, transparent bg,
  1dp amber border, amber text. Icon 22dp. Layout: [icon] SYNC …… [sub right].

## Layout (landscape / car mount)
- `r4-main` becomes horizontal (row): stage on the left (flex 1.35), the
  now-playing + controls + sync stacked on the right (flex 1). Controls grid → 2 cols.

## States (drive the UI from these)
- playback: playing (kicker "NOW PLAYING" + eq bars + LIVE dot) | paused ("PAUSED" + "OFF AIR")
- scope: all (pill "ALL STATIONS") | favs (pill "FAVOURITES ONLY · N", amberHi)
- current fav: "★ FAVOURITE" (amber) | "☆ not saved" (mute)
- warn: scope=favs & 0 favourites → red hero ring + "NO FAVOURITES YET — STAR ONE FIRST"

## Actions → existing session commands
- stage tap → CMD_SHUFFLE
- play/pause → CMD_TOGGLE · star → CMD_STAR · scope → CMD_SCOPE · stop → CMD_STOP
- sync bar → start SyncActivity

## Icons (existing drawables reused)
ic_shuffle · ic_play · ic_pause · ic_star / ic_star_outline · ic_scope_all /
ic_scope_favs · ic_sync. ic_stop must be ADDED (rounded square, 24dp).

## Sync sheet
The design's home also has an inline sync bottom-sheet (QR + key). For THIS work the
sync bar opens the existing full SyncActivity instead — the sheet is superseded by
the separate Sync Screen redesign handoff. Do not build the inline sheet here.
