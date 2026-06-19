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
