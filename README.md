# World Radio

A terminal radio player for the whole planet — browse and play 30,000+ live
internet stations without leaving your shell. Fast, keyboard-driven, no browser,
no mouse.

**Website:** [radio.vchub.net](https://radio.vchub.net)

## Features

- **30,000+ stations** from the community
  [radio-browser](https://www.radio-browser.info/) database, with a local SQLite +
  full-text cache. Search by name; filter (multi-select) by country, tag, codec
  and bitrate.
- **Real-time spectrum** — a live FFT analyzer in block glyphs (bars / mirror /
  dots / wave) plus a VU-style volume bar.
- **Crossfade** — the current station keeps playing while the next buffers, then a
  smooth swap (toggleable).
- **Favorites, history & blocklist**, all as filters — never separate screens.
- **14 retro themes**, switched live, and **remappable keybindings** with an in-app
  overlay.
- **Terminal-native** — truecolor / 256 / 16-color fallbacks, optional `--no-emoji`
  mode, clean reflow down to 80×24. Settings, filters and the last station persist
  between sessions.

## Build & run

Requires a stable [Rust toolchain](https://rustup.rs/).

```sh
git clone https://github.com/Chubik/world-radio.git
cd world-radio
cargo run --release -p radio-tui
```

Press `?` in the app for the full keybinding reference. State (cache, favorites,
history, config) lives in a `data/` directory next to the binary.

Prebuilt Linux binaries are on [radio.vchub.net](https://radio.vchub.net).

## Layout

A Cargo workspace:

- **`radio-core`** — the portable core: catalog (radio-browser API, SQLite + FTS
  cache, favorites/history/blocklist, station health) and pure audio logic (ICY
  metadata, retry, gain, resampling, crossfade).
- **`radio-tui`** — the `world-radio` binary: native audio (symphonia + cpal +
  ringbuf) and the ratatui terminal UI.

## License

[MIT](LICENSE)
