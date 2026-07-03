# r4dio — Android assets

Production-ready SVG glyphs from the designer handoff. All path-based, no font dependency.
Palette: bg `#15100B`, amber `#D49A3A`, amber-hi `#FFC457`, olive `#9EC074`.

## Files

| File | Use |
|------|-----|
| `ic_launcher_foreground.svg` | adaptive icon foreground layer (108×108, safe-zone centre 66×66) |
| `ic_launcher_background.svg` | adaptive icon background layer (amber CRT + top glow) |
| `ic_launcher_monochrome.svg` | monochrome layer for themed icons (Android 13+) |
| `ic_stat_r4dio.svg` | notification status-bar icon — clean `▌` variant (white, tinted by system) |
| `ic_stat_r4dio_waves.svg` | notification status-bar icon — `▌` + broadcast waves variant |
| `ic_play.svg` `ic_pause.svg` `ic_shuffle.svg` `ic_star.svg` | 24dp control glyphs (`currentColor`) for widget/notification |

## Import

- Android Studio → File ▸ New ▸ Vector Asset ▸ Local file (SVG) for each glyph.
- Adaptive icon = foreground + background as two layers; monochrome is a separate layer.
- Notification icon: pick one of the two `ic_stat_*` variants — both white on transparent,
  the system recolors them.
- Widget layouts (4×1 / 2×1) are mockups, not assets: built as RemoteViews, controls drawn
  from the 24dp glyph set above.

Full visual handoff: `handoff.html`. Design source components: `../mini/android.jsx`.
