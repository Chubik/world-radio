# Design Brief — World Radio Mini (one-pager)

*Full version: `world-radio-mini-design-brief.md`.*

**What:** A tiny tray / menu-bar companion to **World Radio** (terminal player, 30,000+ live
stations). Headline action: **Shuffle** — one tap, a station plays. Companion, not replacement
("open full app" is a menu item).

**Aesthetic:** Retro **CRT / monospace**, carried from the main app — amber text on near-black
with a faint scanline glow (default theme **amber-crt**). Show it themed in 2–3 alternates
(nord, dracula). Optional tiny FFT spectrum inside the window = on-brand signature detail.
*Not* a generic OS widget.

**Mini window (~260×120 px desktop), content by priority:**
now-playing (station + live ICY track text) → **Shuffle** → Play/Stop → Volume → mini spectrum
→ country/codec/bitrate (only if it fits).

**Design all 4 states:** Playing · Stopped/idle ("shuffle to start") · Buffering ("connecting…",
previous keeps playing) · Error/offline.

**Per platform (all four — different tray paradigms):**
- **macOS (arm):** menu-bar monochrome template icon → **popover** with the CRT window inside.
- **Linux (Ubuntu+):** AppIndicator icon; left-click = window, right-click = menu. GNOME may be
  menu-only — make the **menu fully usable alone**.
- **Windows:** system-tray colored icon; click = window, right-click = menu.
- **Android (not a tray):** **media notification player** (primary) + **home-screen widget** +
  compact app screen. Keep CRT identity within Android media conventions.

**Interaction / menu:** Shuffle (all) · Shuffle favorites *(scope selector — show how)* ·
Play/Stop · Volume · Open World Radio · Theme *(opt)* · Quit. Design the menu/popover, not just
the window.

**Icons (separate deliverable):** tray/menu-bar icon in idle + playing states — mac monochrome
template (@1x/@2x) + colored variants (Win/Linux/Android) + launcher icon. Based on the `▌WR`
block marker.

**Config sync (planned, NOT built):** future Mullvad-style key sync may pull config later.
Include one lightweight "Settings / Connect" menu entry as a placeholder — but Mini must work
**fully without sync**. Don't over-design it.

**Deliverables:** Figma (shared) + exported PNGs (mac @1x/@2x; Android mdpi→xxxhdpi). Mini window
× 4 states × (amber-crt + 2–3 themes); per-platform treatments; menu/popover layouts; icon set;
a notes layer with hex colors, monospace type, spacing, scanline/glow.

**Constraints:** small first (legible at ~260×120). Public MIT project — no personal data, names,
or third-party logos. Out of scope: full app screens, the sync flow, marketing pages.

**Flag back to us:** Does the CRT/scanline read at tray-icon size? Is GNOME menu-only enough?
On Android — notification vs widget as hero, and how much CRT survives?
