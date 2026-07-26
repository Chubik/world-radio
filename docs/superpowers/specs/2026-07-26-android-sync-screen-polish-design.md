# Android sync screen — brand polish

Date: 2026-07-26
Status: approved

## Context

`SyncActivity` already implements the structure of the Claude Design handoff
(`r4dio - Sync Screen Handoff.html`, design project "radio"
`a5023d91-924d-4f7b-9561-12f6cc7477a5`): a pinned app-bar with an always-visible
DONE button, states A (not linked) and B (linked), the key field, the key box with
COPY, the QR on a light card, LOG OUT / DELETE, and HIDE COUNTRIES. Spacing
(20dp body, 56dp bar, 18dp gaps) and the shape drawables already match the redline.

What does not match is the typography. The screen uses the system `monospace`
family, while the home screen shipped in v1.6.0 uses the bundled IBM Plex Mono and
Sans. Side by side the two screens read as different applications. A few smaller
details also drifted from the redline.

This work closes that gap. It is presentation only.

## Goals

Make the sync screen typographically identical to the home screen, and fix the
remaining redline deviations, without touching sync behaviour.

## Non-goals

- No change to sync logic, networking, merge semantics, or the QR encoder.
- No migration to Material Components (`MaterialButton` / `MaterialToolbar`), which
  the handoff suggests. The project has no Material Components dependency, and the
  existing `TextView` + shape-drawable approach already produces the redline result
  and matches the home screen. Adding the dependency would buy nothing here.
- No button-height change. The redline lists LOG OUT / DELETE at 44dp; they stay at
  50dp to match every other button on the screen and to keep a comfortable touch
  target. The difference is not visible.
- No icons. The handoff shows SVG icons on the buttons; the user chose to ship
  text-only buttons for now and revisit icons later.

## Changes

### 1. Typography

Replace `android:fontFamily="monospace"` with the bundled families, matching the
redline's weights:

| Element | Family | Weight | Size |
|---|---|---|---|
| App-bar title | `@font/ibm_plex_mono` | 700 | 16sp |
| Button labels | `@font/ibm_plex_mono` | 600 | 13sp |
| Sync key (`key_shown`, `key_input`) | `@font/ibm_plex_mono` | 400 | 12sp |
| Field labels (SYNC KEY, YOUR SYNC KEY) | `@font/ibm_plex_mono` | 400 caps | 10sp, +0.18em |
| Status pill, OR divider, QR caption | `@font/ibm_plex_mono` | 600 / 400 | 12 / 10sp |
| Lede paragraph, copy hint | `@font/ibm_plex_sans` | 400 | 13 / 11sp |

Weights come from `android:fontWeight` (minSdk is 26, so this is supported) rather
than `textStyle="bold"`, which would resolve to 700 everywhere and lose the 600
distinction the redline calls for.

### 2. Strings move to resources

Every user-facing literal currently inline in `activity_sync.xml` becomes a
`sync_*` string resource. This is a project rule already honoured on the home
screen. The set:

`sync_title`, `sync_lede`, `sync_key_label`, `sync_key_hint`, `sync_use_key`,
`sync_or`, `sync_create`, `sync_scan`, `sync_synced`, `sync_your_key`,
`sync_copy`, `sync_copy_hint`, `sync_qr_caption`, `sync_logout`, `sync_delete`,
`sync_hide_countries`, `sync_feedback`, `sync_done`.

Toast and dialog strings inside `SyncActivity.kt` are out of scope — they are not
part of the visual redline and moving them would touch logic files for no visual
gain. They stay as they are.

### 3. SYNCED pill turns olive

The redline specifies the linked-state pill in olive `#9EC074` — the "live, working"
colour, distinct from the amber used for actions. It currently renders amber and
blends into the surrounding accents. The `olive` colour already exists in
`colors.xml`; the pill's text colour and `pill_synced` stroke both move to it.

### 4. Unicode glyphs removed

Button labels drop their leading glyphs: `✓ USE KEY` → `USE KEY`, `+ CREATE NEW` →
`CREATE NEW`, `⧉ COPY` → `COPY`, `⇥ LOG OUT` → `LOG OUT`, `🗑 DELETE` → `DELETE`,
`⊘ HIDE COUNTRIES` → `HIDE COUNTRIES`, `✉` drops from the feedback line. The
app-bar keeps `✕` on DONE, and the pill keeps `⊙ SYNCED` — both are shown in the
handoff mockup itself, and both are geometric glyphs with no emoji presentation.

These glyphs render in whatever the system font provides, so they vary across
vendors, and `🗑` in particular can resolve to a colour emoji that clashes with the
monochrome amber palette.

## Files

- `android/app/src/main/res/layout/activity_sync.xml` — all four changes.
- `android/app/src/main/res/values/strings.xml` — new `sync_*` entries.
- `android/app/src/main/res/drawable/pill_synced.xml` — stroke amber → olive.

`SyncActivity.kt` is not modified. It binds by id, and no id changes.

## Verification

- `./gradlew assembleDebug` green.
- Emulator drive of both states: state A on a fresh install (no sync key), then
  state B by creating an account, confirming the key box, the QR bitmap, and the
  olive pill render correctly.
- Screenshots compared against the handoff mockups for both states.
- Confirm DONE still closes the screen and that the QR still scans (the light card
  and its quiet-zone padding are untouched, but the screen is verified end to end).

## Risks

Low. The only functional surface is the layout ids, which do not change. The
plausible failure is a typo in a font or string reference, which fails loudly at
build or inflate time rather than silently.
