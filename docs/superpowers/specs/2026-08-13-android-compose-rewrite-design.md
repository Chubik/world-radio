# Android Compose Rewrite — Design Spec

**Date:** 2026-08-13. **Design source:** `_private/design/2026-08-10-android-handoff.html`
(mockups, not in git — private directory) and `_private/design/2026-08-10-android-full-app-design-brief.md`.

## Why

Android is a minimal shuffle player. The CLI is the reference client: 56k catalogue, live search,
five filter dimensions, favourites, blocklist, history, full now-playing with a spectrum, 14 themes.
The user asked for the same app on the phone, and approved a **full rewrite in Jetpack Compose**
rather than a mixed XML/Compose codebase: *"ми не будемо підтримувати зоопарк"*.

## The decision that shapes everything

**Everything with a UI moves to Compose.** Two things cannot and stay as they are:

- **The 4×2 widget** — Android requires `RemoteViews`; Compose cannot render it. `widget_radio.xml`
  and `widget_radio_small.xml` stay, untouched.
- **The media notification** — Media3 draws it. Slots are fixed (play/pause, prev/next) and the
  steering-wheel keys depend on them ([[media3-notification-slots]]).

`PlaybackService` has no UI and is not rewritten, but it must change: today UI state travels as a
`Bundle` of session extras read by a single Activity. Four screens cannot share that.

## Measured starting point (2026-08-13)

- 21 Kotlin files, 3474 lines. `PlaybackService` 892, `MainActivity` 439, `SyncActivity` 302.
- No Compose, no Fragments, no ViewModel, no Navigation, no RecyclerView, **no list of any kind**.
- 5 layout files, all LinearLayout. Zero ConstraintLayout.
- `minSdk 26`, `compileSdk 37`, AGP 9.1.1, Kotlin 2.4.0.
- **Theming is hardcoded twice over:** 132 `@color/` references across 28 XML files, *plus* ~20
  imperative `getColor(R.color.X)` / `setBackgroundResource(...)` calls in `MainActivity`. An
  XML-only theming pass would silently miss the second layer.
- APK 6.6 MB. Compose is expected to add 2–3 MB.

## Facts that change the design

1. **The theme already syncs and is silently discarded.** `SyncProfile.theme`/`themeAt` travels on
   the wire, is stored in DataStore (`FavStore` `keyTheme`), and **nothing on Android reads it**.
   Changing the theme on the desktop today does nothing visible on the phone. Phase 1 makes this
   live, which is why themes come first rather than last.

2. **The palette is 9 slots, not a Material colour scheme.** `crates/radio-tui/src/tui/theme.rs`
   defines `bg, fg, accent, hot, dim, ok, err, info, peak` for each of 14 themes. `amber-crt` is
   exactly today's `colors.xml`, so Android's current look is preserved by construction.

3. **`hifi-paper` is the only light theme** (bg `#EFE6CC`, luminance 0.90); the other 13 are dark.
   Nothing may assume a dark background — that is what the light theme will expose.

4. **`scope` carries five values on the wire** (`all`, `favorites`, `recent`, `blocked`, `dead`),
   Android understands two. `crates/radio-core/src/sync/scope.rs:2-4` warns that collapsing the
   others into "all" silently changes what other devices see. The design resolves this: `recent`
   and `blocked` become Library tabs, so the values become reachable instead of collapsed.

5. **History is a push queue, not a history.** `history_pending` holds `uuid|epochSecs`, is capped
   at 200, and `drainPlays()` **deletes entries once the server accepts them** — on a linked account
   it empties every sync. It also stores no station names. A History tab needs storage that is not
   this queue. `Profile.kt:167-169` states the current behaviour is deliberate.

6. **Blocked has no local write path.** `blocked_uuids` is written only by sync and restore; there
   is no block/unblock on Android at all.

7. **Favourites are the one list ready to render**: `cached_favs` already holds full station objects
   (uuid, name, url, country, codec, bitrate).

## Navigation (from the handoff)

Four bottom tabs, everything one tap deep. Now Playing is **not** a tab: a mini-player strip sits
above the tabs whenever something is playing and expands to full screen.

```
HOME            CATALOG          LIBRARY              SETTINGS
shuffle         search+filters   favs/blocked/history themes/countries/account
        └──────── mini-player strip (when playing) ────────┘
                        ↓ tap
                  NOW PLAYING (full screen, spectrum)
```

**Home does not change.** Same giant shuffle target, same eyes-free behaviour, now with thin tabs
underneath. The brief is explicit: the shuffle gesture, the widget and the notification are not to
be touched.

## Phases, each independently releasable

| # | Phase | Delivers | Why this order |
|---|---|---|---|
| 1 | **Shell + themes** | Compose, 4 tabs, Home ported verbatim, 14 themes live from the synced value, mini-player | Nothing else can be built until the shell and the token system exist. Makes the dead synced theme work. |
| 2 | **Catalog** | search over the cached catalogue, filter sheet (country/genre/codec/bitrate), active-filter chips, `Clear all` | Biggest missing piece; answers the user's filter complaints properly. |
| 3 | **Library** | Favourites / Blocked / History under one segmented control; local block/unblock; real history storage | Needs the list idiom from phase 2; unlocks the `recent`/`blocked` scope values. |
| 4 | **Now Playing** | full screen, metadata, ★/block/station page, spectrum | Depends on the mini-player from phase 1. |
| 5 | **Settings** | theme picker (14), excluded countries, blocked count, account/sync ported from `SyncActivity` | Retires `SyncActivity`, the last XML activity. |
| 6 | **First launch** | catalogue progress `18,402 / 56,000`, "Play favourites" as soon as sync lands | Needs phases 1–5 to have something to show while loading. |

Each phase is its own plan, written when the previous phase is in the user's hands, so its lessons
land in the next one. This document is the shared contract between them.

## Global constraints (apply to every phase)

- Repo `radio`, branch `dev`, everything under `android/`.
- **No Material Design look.** Material 3 may be used as scaffolding, but the app must read as
  r4dio: mono type (`ibm_plex_mono`), pill shapes, the 9-slot palette. The brief: brand over
  "standard material".
- **Every colour comes from a token.** No `Color(0xFF...)` literals in screen code, no
  `@color/` in new XML. A theme swap that misses one surface is the failure mode.
- **The RU/BY ban holds on every ingest and display path** ([[exclude-russian-stations]]).
- Home's shuffle gesture, the widget, and the media notification are not to be redesigned.
- All code, comments, strings and commit messages English, lowercase-first. Comments only where they
  state a constraint. Commit subjects are the public changelog.
- Gate before every commit: `cd android && ./gradlew test` green, real output pasted in the report.
- **Never launch playback to prove something** ([[never-start-playback-to-prove-things]]) — read
  `catalog.json`, the rendered screen, and logcat instead.
- Verified on the emulator, not by reading code. Screenshots prove layout, **not colour** — check
  colour pairings against the palette ([[screenshots-prove-layout-not-legibility]]).

## Risks

- **The rewrite touches the one screen that works.** Home is the product. Phase 1 ports it verbatim
  and is not allowed to redesign it; any visual difference from the current release is a defect.
- **Two sources of truth during phase 1.** `MainActivity` state fields and `PlaybackService` extras
  must be replaced by one shared state holder, not duplicated. The repo's known defect class is
  correct code the real path never reaches ([[unreached-logic-is-this-repos-defect-class]]).
- **APK growth.** 6.6 MB today; Compose adds 2–3 MB. Acceptable, but measure at phase 1 and report.
- **The light theme.** 13 dark themes hide contrast bugs that `hifi-paper` exposes. Every phase must
  screenshot at least one screen in `hifi-paper`.
