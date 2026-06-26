# Backlog

Planned work and ideas, roughly by area. Not a roadmap — priorities shift.

## Usage stats (privacy-first)

Goal: know if people actually use this — **without the app phoning home**. The app
sends nothing; everything is measured passively server-side. Decided model: passive
downloads only (no in-app telemetry, no active-user ping, no opt-in heartbeat — all
rejected on privacy grounds).

What already exists (server dashboard): downloads **by country** and **by version**
are tracked and graphed. The gaps are about presentation, not collection:

- **Own dashboard, not the shared one** — the World Radio panels currently live on the
  general dashboard alongside every other server. Move them to a dedicated World Radio
  view so the data is findable and not mixed with unrelated infra.
- **Consolidate to one graph, drop the text** — fold the by-version table into a chart
  and present downloads (over time, by country, by version) as a single consolidated
  graph. No text tables.
- **Explicitly NOT doing:** in-app analytics, unique-install IDs, active-user
  heartbeats, "who is using it right now". If we ever reconsider, it must be opt-in
  and anonymous, and documented plainly in the README.

## Distribution

- **Install script** — `curl -fsSL <host>/install | sh` into `~/.local/bin`
  (user-level, no system self-update, no apt repo). Add to README and site.
- **cargo install / crates.io** — publish so `cargo install world-radio` works.
- **apt / .deb / AUR** — later, on demand (signing, repo, GPG).
- **Per-platform builds** — Linux / macOS / Windows release artifacts.
- **Release integrity (gap)** — the README already tells users to verify downloads
  against `SHA256SUMS`, but no such file exists and CI builds no release artifacts.
  Need a release workflow that builds the binaries, emits `SHA256SUMS`, and publishes
  both (ideally signed). Until then the README's verify instruction is unbacked.

## Sync (new)

Sync config, favorites, blocklist and settings for one person across devices,
with no email and no password — Mullvad-account style.

- The app generates a single **sync key** (a random secret rendered as a short
  human-typable string, like a Mullvad account number; under the hood a key, not
  credentials).
- A small server stores the settings blob keyed by a hash of that key.
- Entering the same key on another device pulls the blob down and merges it.
- No accounts to manage. Lose the key, lose access — by design.
- Open questions: merge strategy on conflict, blob encryption at rest, key rotation.

## Companion apps (new, future)

- **Tray / menubar micro-radio** — a tiny applet that lives in the system tray
  (Linux/macOS/Windows) with shuffle: across all stations or across favorites.

## Sources

- **radio-browser** stays the source. Alternatives are paid (RadioAPI) or worse.
- **Curated overlay** — small static JSON of recommended stations served from the
  site, merged with radio-browser results. No server logic.
- **Aggregator service** — far future; only if there's a need radio-browser can't
  cover. Would issue per-client configs keyed by an id.

## Done (recent)

- **Mirror fallback** — `resolve()` probes every IP behind all.api.radio-browser.info
  with a health check (TLS pinned to the canonical host) and falls back cleanly.
- **Duplicate collapse** — catalog reads dedup by name + country + codec + bitrate.
- **Recheck dead stations** — `dead` status filter + `R` to clear failure counters.
- **Compact narrow filter** — single focused group with a tab row under 100 cols;
  on narrow screens it now sits as a content-sized panel below the station list.
