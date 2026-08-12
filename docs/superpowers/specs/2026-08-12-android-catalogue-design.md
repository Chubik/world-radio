# A catalogue worth filtering

## The complaint, and the measurement behind it

The country filter reached the phone correctly, and shuffle then felt like "maybe 3 stations" on a
UA filter. The filter was not broken — the catalogue was.

Android holds the **world top-1000 by clickcount** (`DEFAULT_TARGET = 1000`,
`order=clickcount&reverse=true`, `android/.../Catalog.kt:16,161-164`). Measured against the live API
on 2026-08-12:

| | |
|---|---|
| UA stations in the top-1000 | **7** |
| UA stations that actually exist | **351** |
| What the user was seeing | 2% of them |

The top-1000 skews hard to DE (111), FR (99), US (90), IN (66), GB (59), IT (53). A filter for
anywhere outside that handful has almost nothing to work with. The desktop, by contrast, holds the
whole ~58k catalogue, so the same account behaves completely differently on the two devices.

This is the second time the top-1000 has bitten: the user's own favourites were measured at 0/7
present in it.

## The correction that shapes the design

The obvious answer — "fetch the countries the user filtered to" — is not enough, and the user said
so: **at the start no filter is set at all.** A filter-triggered fetch helps nobody until the user
happens to turn one on, so a new user stays on the skewed top-1000 indefinitely.

So both mechanisms are needed, with distinct jobs:

- **the filter as a trigger** — selecting UA pulls all 351 UA stations promptly, because that is the
  moment the user's intent is explicit and narrow;
- **a passive background top-up** — the baseline for someone who never sets a filter.

## The filter-triggered fetch

When the synced filter changes to a non-empty set, each country in it is fetched by
`bycountrycodeexact` and merged into the cache. One request per country, so a three-country filter
costs three requests, not a crawl of the world.

It runs when the filter arrives, whether that is from a sync or from the phone itself, and it does
not wait for the staleness window — the user has just told us what they want and is presumably
about to press shuffle.

## The passive top-up

Without a filter there is no intent to follow, so the app widens the catalogue on its own, slowly,
under conditions that cost the user nothing:

- **only when it would not be noticed** — on unmetered network and while charging. This is what
  "без навантаження" has to mean in practice: not a smaller burst, but one that waits for a moment
  when neither battery nor data matters.
- **a page at a time**, by descending clickcount beyond the first 1000, so the catalogue grows
  towards the whole of it rather than re-fetching the same head.
- **stopping when there is nothing left to add.**

## Newly-arrived stations obey the active filter

Required by the user in the same breath: **"щоб фільтр вмикався на нових завантажених"**. Stations
that land mid-session are subject to the filter as they arrive, not only at the next full refresh.
Since the pick path already filters at pick time through the shared `allowedStation` rule, this
falls out of merging into the same cache the picks read — but it must be verified on the device, not
assumed from the code.

## A definite state

Also required: **"щоб був state певний"**. The catalogue is no longer either "the top-1000" or
"everything" — it is somewhere in between, and the user has to be able to see where. The home screen
already carries pills for the filter, hidden countries and scope; the catalogue count belongs
alongside them, showing what is held and, while a top-up is running, that it is growing.

Nothing about this may block playback: a partial catalogue plays, exactly as the current one does.

## Out of scope

- **Fetching the entire 58k catalogue eagerly.** It is what the desktop does, and it is wrong on a
  phone — the storage and the traffic are the user's, and the top-up reaches the same place without
  a single expensive moment.
- **Changing what the desktop holds.** It already has everything.
- **A user-facing "download the catalogue" button.** The whole point is that it happens without
  being asked for; a button invites the question of when to press it.
- **Relaxing the RU/BY ban.** It applies to every ingest path, including these new ones.

## How we know it works

On the emulator, against the real API — the measurement above is the baseline to beat:

- with a UA filter set, the catalogue holds substantially more than 7 UA stations, and ten shuffles
  land on more than a handful of distinct ones;
- with no filter ever set, the catalogue grows past 1000 over time under the top-up's conditions,
  and does not grow when those conditions are absent;
- a station that arrives during a session is subject to the active filter immediately;
- the home screen shows what is held, and playback works throughout.
