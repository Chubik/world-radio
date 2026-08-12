# The station name, visible while you are driving

## What the user asked for

While Waze is open and r4dio is shuffling in the background, he wants to see which station it
switched to — briefly, then gone.

## Why the notification is not the answer

Media3 already draws a playback notification carrying the station name, but it sits in the shade.
A driver following turn-by-turn directions never pulls the shade down, so the name is effectively
invisible at exactly the moment it is wanted.

## What it does

On every station change, a small panel appears over whatever is on screen — the station name, the
country, a couple of seconds, gone. It is drawn by the app itself over other apps, so it does not
depend on the shade being opened, and it disappears on its own so it can never sit on top of a map.

```
r4dio  ↓  Radio ROKS Ballads · UA
```

**Only while another app is in front.** If r4dio's own screen is visible, the name is already there
and a panel over it would be noise. The overlay is for the case it exists for: playback the user
cannot see.

**Only on a real change.** A pause, a resume, or the same station re-buffering after a network blip
must not show anything — the panel means "you are now on a different station", and firing it for
anything else teaches the user to ignore it.

## The permission, and how it is asked for

Drawing over other apps needs `SYSTEM_ALERT_WINDOW`, which Android will not grant from a dialog:
the user has to be sent to a settings screen and toggle it there. That is heavy enough that it must
never be sprung on someone.

So the overlay is **off until the user turns it on**, from a pill on the home screen next to the
existing ones. Tapping it explains what it does in one line and opens the settings screen. If the
permission is later revoked in system settings, the pill goes back to off and nothing crashes —
the permission is re-checked before every show, never remembered as a fact.

With the permission absent, the app behaves exactly as it does today.

## Out of scope

- **A persistent floating player.** The user chose the brief flash; a window that stays would cover
  the map, which is the thing he is actually looking at.
- **Making the notification heads-up instead.** It was the alternative offered and not chosen: the
  system decides how a heads-up looks and when it is allowed, and on a maps-first screen it competes
  with navigation prompts.
- **Album art or controls in the overlay.** It is a label, not a player. Anything tappable over a
  navigation app is a hazard.

## How we know it works

Verified on the emulator, not by reading code:

- with another app in front, a shuffle shows the panel and it disappears on its own;
- with r4dio in front, nothing appears;
- a pause and a resume of the same station show nothing;
- with the permission never granted, the pill reads off and no overlay is attempted;
- with the permission revoked while the app runs, the next change shows nothing and does not crash.
