# Design Brief — World Radio Mini

## 1. What it is

**World Radio Mini** is a tiny, always-available companion to **World Radio** (a
terminal radio player for 30,000+ live internet stations). It lives in the system
tray / menu bar and lets you start listening and shuffle stations without opening a
full window. The headline action is **Shuffle** — one tap and a station plays.

It is a companion, not a replacement: "open the full app" is one of its menu actions.

**Tagline direction:** "Your radio, one tap away."

## 2. Goal of this brief

Produce the **visual design** (mockups + icon set) for World Radio Mini across all four
target platforms. We need pixel-accurate screens we can hand to engineering, plus the
tray/menu-bar icon in every state. This is design only — no implementation.

## 3. Aesthetic — retro-CRT, carried over from World Radio

The main app has a strong identity: **retro CRT / monospace** with 14 switchable themes
(default **amber-crt**: warm amber text on near-black, faint scanline glow; others include
nord, gruvbox, dracula, solarized, catppuccin, etc.). The wordmark uses a `▌WR` block
marker + "World Radio".

**Carry this vibe into Mini.** The mini window should feel like a small glowing CRT panel —
amber-on-black by default — not a generic OS widget. This is the product's signature and the
reason it should feel "very interesting by design." Show the **amber-crt** default plus 2–3
alternate themes (e.g. nord, dracula) so we see it themed.

Reference the main app's spectrum analyzer (a live FFT in block glyphs: bars / mirror / dots /
wave) — a tiny version of it inside the mini window would be a strong, on-brand detail.

## 4. The mini window — content & states

The window is **small** (target ~ 260×120 px desktop; adapt per platform). Design what it shows,
in priority order (most important fits first when space is tight):

1. **Now playing** — station name + the live ICY "now playing" track text (scrolling if long).
2. **Shuffle button** — primary, prominent. (Plus the secondary shuffle scope, see §6.)
3. **Play / Stop** toggle.
4. **Volume** — small control or indicator.
5. **Mini spectrum** — optional tiny FFT visual (on-brand).
6. **Country / codec / bitrate** of the current station — minor, only if it fits.

**Design ALL of these states** (not just "playing"):

- **Playing** — station + now-playing text + active controls.
- **Stopped / idle** — nothing playing yet; inviting "Shuffle to start" affordance.
- **Buffering** — the next station is loading (the app crossfades; the previous keeps playing
  until the next buffers — reflect that "connecting…" moment).
- **Error / offline** — stream failed or no network; clear, calm recovery state.

## 5. Per-platform targets (all four)

Each platform has a different tray paradigm — design for each, don't assume one fits all:

- **macOS (Apple Silicon / arm)** — **menu bar**: monochrome template icon top-right; clicking
  opens a **popover** anchored under the icon containing the mini window. Match macOS popover
  shape (arrow, rounded corners) but with our CRT content inside.
- **Linux (Ubuntu first; others welcome)** — **AppIndicator / StatusNotifier** tray icon
  (GNOME + KDE). Left-click opens the mini window; right-click opens the menu. Note GNOME may
  only support the menu (no left-click action) — design the **menu** to be fully usable on its
  own (shuffle, scope, play/stop, volume, open app, quit as menu items).
- **Windows** — **system tray**: colored icon, left-click opens the mini window, right-click
  opens the context menu. (Lower priority than mac/Linux but include it.)
- **Android** — NOT a tray. Design: (a) a **notification media player** (the primary surface —
  station, now-playing, shuffle/play/stop actions on the notification), and (b) a small
  **home-screen widget** with the same controls, and (c) the compact app screen it opens to.
  Follow Android media-notification conventions but keep the CRT visual identity.

## 6. Interaction & menu

- **Primary:** Shuffle (big, obvious).
- **Shuffle scope** — two modes the design must expose: **shuffle all stations** vs **shuffle
  my favorites**. Show how the user picks the scope (toggle, segmented control, or menu items).
- **Context / right-click (or Android overflow) menu items:** Shuffle (all), Shuffle favorites,
  Play / Stop, Volume, **Open World Radio** (full app), Theme (optional), Quit.
- Design the **menu / popover** layout, not just the window.

## 7. Icon set (a separate deliverable)

The tray / menu-bar icon in **multiple states**:

- **Idle / stopped**, **Playing** (e.g. subtle equalizer/pulse), and optionally **buffering**.
- **macOS:** monochrome **template** icon (single color, system-tinted), @1x/@2x.
- **Windows / Linux / Android:** full-color app icon + small tray/notification icons.
- Based on the `▌WR` block marker / "World Radio" identity. Provide a launcher/app icon too.

## 8. Config sync (planned, not built — design for it but don't depend on it)

We plan a future **cross-device sync** (Mullvad-account style: one generated key, no
email/password — enter the same key on another device to pull your config, favorites and
settings). It is **not implemented yet**, and Mini may pull config from it later.

For this brief: include a **lightweight "settings / sync" affordance** in the menu (e.g. a
"Settings" or "Connect (enter key)" item) as a placeholder, but the mini window must work
**fully without sync** — sync is additive, never required. Don't over-design this; one entry
point is enough.

## 9. Deliverables

- Mini window mockups in **all 4 states** (playing / stopped / buffering / error).
- Each in the **amber-crt** default + **2–3 alternate themes**.
- **Per-platform** treatments: macOS popover, Windows tray window, Linux indicator window +
  menu-only variant, Android notification player + home widget + compact screen.
- **Context/popover menu** layouts including shuffle-scope selection.
- **Icon set**: tray/menu-bar icons in idle/playing states (mac monochrome template + colored
  variants) and a launcher icon.
- Source files in **Figma** (shared link), plus exported PNGs at the densities each platform
  needs (mac @1x/@2x; Android mdpi→xxxhdpi).
- A short notes layer on spacing, type (monospace), colors (hex), and the scanline/glow effect
  so engineering can reproduce the CRT look.

## 10. Constraints & notes

- **Small first:** the desktop window must read clearly at ~260×120 px. Don't design a big panel.
- Monospace type and the amber/scanline treatment are the signature — keep it legible, not noisy.
- Public, open-source project (MIT). No personal data, names, or third-party logos in the design.
- Out of scope: the full World Radio app screens (already designed), the actual sync UI flow,
  marketing/site pages.

## 11. Open questions for the designer to flag

- Does the CRT/scanline effect read well at tray-icon size, or do we need a simplified mark?
- For GNOME (menu-only): is the menu alone a good enough experience, or should we push a small
  separate window?
- Android: notification player vs widget — which should be the hero, and how much CRT styling
  survives Android's media-notification constraints?
