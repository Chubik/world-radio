# World Radio

A terminal radio player for the whole planet. Browse and play 30,000+ internet
radio stations from the community [radio-browser](https://www.radio-browser.info/)
database — all in a single, keyboard-driven text UI.

## Features

- **30,000+ stations** — search by name, filter by country, tag, codec and bitrate
  (multi-select), backed by a local SQLite + full-text cache.
- **Real-time spectrum** — a live FFT analyzer rendered in block glyphs, with
  selectable styles (bars, mirror, dots, wave) and a VU-style volume bar.
- **Crossfade** — the previous station keeps playing while the next buffers, then a
  smooth crossfade swaps them (toggleable).
- **Favorites, history & blocklist** — star stations, revisit what you played, and
  hide dead or unwanted streams — all as filters, never separate screens.
- **14 retro themes** — amber CRT, tube glow, hi-fi paper, nord, gruvbox, dracula,
  catppuccin, monokai and more, switched live.
- **Configurable keybindings** — remap the main actions from an in-app overlay, with
  conflict detection and reset-to-defaults.
- **Keyboard-driven** — vim-style and arrow navigation, paged scrolling, instant
  search, and a built-in help overlay. No mouse assumed.
- **Terminal-native** — truecolor / 256 / 16-color fallbacks, optional
  `--no-emoji` ASCII mode, clean reflow down to 80×24.

Settings, search, filters and the last-played station persist between sessions.

## Build from source

Requires a [Rust toolchain](https://rustup.rs/) (stable).

```sh
git clone https://github.com/Chubik/world-radio.git
cd world-radio
cargo build --release -p radio-tui
```

The binary is produced at `target/release/world-radio`.

## Run

```sh
cargo run --release -p radio-tui
# or, after building:
./target/release/world-radio
```

Press `?` inside the app for the full keybinding reference. Data (cache,
favorites, history, config) is stored in a `data/` directory next to the binary.

## Architecture

A Cargo workspace:

- `radio-core` — portable library: catalog (radio-browser API, SQLite + FTS cache,
  facets, favorites/history/blocklist, station health) and pure audio logic
  (ICY metadata, retry, gain, resampling, crossfade).
- `radio-tui` — the `world-radio` binary: native audio (symphonia + cpal + ringbuf)
  and the ratatui terminal UI.

## License

[MIT](LICENSE) © Valentyn Chub
