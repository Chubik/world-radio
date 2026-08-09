# Startup update visibility in the TUI

**Spec. Written 2026-08-09.** The user asked for an update prompt at startup. Investigation showed
the feature mostly exists already — this spec covers the two pieces that are actually missing.

## What already exists (do not rebuild)

- `tui/mod.rs:111-115` spawns a background thread at TUI start that calls
  `radio_core::update::fetch_latest()` and sends `Msg::UpdateAvailable(rel)`. The radio starts
  immediately; the check never blocks it.
- `view/header.rs:56-66` renders a persistent `↑ v<X> available` indicator once
  `model.pending_update` is set.
- The `U` key downloads, sha-verifies, swaps the binary, and execs the new one.
- The `r4dio update` subcommand does the same from the command line.

The user chose (2026-08-09, over a blocking y/N prompt): keep startup instant, make the existing
mechanism more visible, bound the network.

## 1. Timeouts in `radio-core/src/update.rs`

reqwest's blocking client defaults to a 30-second total timeout. Two problems:

- **The check is too slow to fail.** 30 s is far too long for an interactive `r4dio update` on a
  bad network. The check client (`check_from`) gets `connect_timeout(2s)` and a total
  `timeout(5s)`. Failures stay silent on the startup path (already the case — the thread drops
  errors) and keep their existing error message in `r4dio update`.
- **The download can be killed too early.** The same 30-second default applies to `apply`'s
  tarball download (~5 MB); a slow link can legitimately need longer. The apply client gets
  `connect_timeout(5s)` and **no total timeout**.

## 2. A startup notice in `tui/update.rs`

`Msg::UpdateAvailable` today only sets `model.pending_update`. It must additionally set

```
model.notice = Some(format!("new version v{} available — press U to update", rel.version))
```

`model.notice` renders as a third header line and persists until another event overwrites it —
exactly the visibility wanted. No auto-expiry work, no new keys, no new messages.

## Out of scope

A pre-TUI y/N prompt (rejected: delays music), changes to the `U` flow, changes to
`r4dio update`, once-a-day check caching.

## Verification

- Unit test: `Msg::UpdateAvailable` sets both `pending_update` and a notice containing "press U".
- Existing update tests stay green; `cargo clippy` clean.
- Manual: run the TUI with a mocked older version (or wait for the next real release) and see the
  notice line appear without the start being delayed.
