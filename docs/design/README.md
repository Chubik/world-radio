# Design

Design briefs and delivered mockups for World Radio products.

## World Radio Mini (tray / menu-bar companion)

- `world-radio-mini-brief-onepager.md` — the brief, short version.
- `world-radio-mini-design-brief.md` — the brief, full version.
- `mini/` — delivered design: an interactive React canvas covering all four platforms
  (macOS menu-bar popover, Linux indicator + GNOME menu-only, Windows tray, Android
  notification + widget + compact screen), the tray/launcher icon set, and 7 themes
  (amber-crt default + alternates). CRT aesthetic per the brief.
  Open `mini/index.html` in a browser to view (loads React/Babel from a CDN).
- `screenshots/mini-mac.png`, `screenshots/mini-mac2.png` — macOS popover, all four
  states (playing / stopped / buffering / error), amber-crt.

All states, themes, and per-platform treatments match the brief. This is design only —
World Radio Mini is not implemented yet (see `BACKLOG.md`).

## r4dio — logo

- `logo/` — the `r4dio` wordmark (the `4` replaces the `a`, amber-hi accent), with a
  cursor lockup (`▌ r4dio` + "WORLD RADIO"), a domain lockup, and a square app
  icon / mark that works down to 16px (favicon / dock / package badge). Open
  `logo/index.html` in a browser.
- `screenshots/logo-canvas.png` — the logo exploration.

## World Radio TUI — engineer reference

- `tui-engineer-reference-v6.html` — the latest (v6) engineer reference for the
  single-page TUI: palettes (RGB / xterm-256 / 16-color), glyph reference, Ratatui
  widget mapping, layout + minimum size, list-row state styling, ready-to-paste Rust
  `Palette` constants, and open questions. (v1–v5 were earlier drafts of the same
  document — only v6 is kept.)
- `screenshots/full.png`, `body.png`, `modals.png` — the single-page layout.
