# Contributing

Thanks for your interest in World Radio. Contributions are welcome — bug fixes,
features, docs, and station/playback issues alike.

## Workflow

`main` is protected: it never takes direct pushes. Every change lands through a
pull request that passes CI.

1. Fork the repo and create a branch from `main`.
2. Make your change. Keep commits focused and the message in the imperative mood
   ("fix crossfade gap", not "fixed stuff").
3. Open a pull request against `main` with a short description of what and why.
4. CI (`fmt` · `clippy` · `test`) must be green before a PR can merge. The
   maintainer reviews and merges.

## Before you push

Run the same checks CI runs:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

All three must pass. The build needs ALSA headers on Linux
(`sudo apt-get install libasound2-dev`).

## Code style

- Rust 2021, formatted with `rustfmt`, lint-clean under `clippy -D warnings`.
- Prefer small, focused files and functions.
- Comments only where they explain a non-obvious *why* — the code should otherwise
  speak for itself.
- The workspace splits into `radio-core` (portable: catalog + pure audio logic)
  and `radio-tui` (the binary: native audio + the ratatui UI). Keep platform and
  I/O concerns in `radio-tui`; keep `radio-core` portable and side-effect-light.

## Tests

Pure logic (catalog, filters, audio math, keymap, model) is unit-tested; add tests
for new pure behavior. Rendering and live audio are verified by running the app.

## Reporting issues

Open an issue with your OS, terminal, steps to reproduce, and — for playback
problems — the station URL or name if you can share it.

By contributing you agree your work is licensed under the project's
[MIT license](LICENSE).
