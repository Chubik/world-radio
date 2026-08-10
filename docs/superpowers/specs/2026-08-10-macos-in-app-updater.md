# In-app updater for the macOS app

**Spec. Written 2026-08-10.** Today every macOS update means re-downloading the dmg and passing
Gatekeeper again — the app is not notarized ($99/yr, declined for now), so each dmg install costs
the user a trip through System Settings. The Tauri updater closes that loop for free: updates are
signed with our own minisign key, installed by the running app itself, and files an app writes
itself carry no quarantine — Gatekeeper is paid exactly once, at the very first dmg.

## Decisions (user, 2026-08-10)

- **Notice + one click**, mirroring the CLI: silent check at startup; a tray-menu item appears
  when a newer version exists; clicking it downloads, installs, and relaunches. Nothing installs
  without the click.
- **Key custody:** private key + password live in GitHub Actions secrets; a local backup stays at
  `~/.r4dio-updater-key` until the user moves it into their password manager and says to delete
  it. Losing the key permanently breaks the update chain for shipped apps — treat it like the
  sync DB.

## Components

### 1. Plugin and config (`crates/r4dio-macos`)

- `Cargo.toml`: add `tauri-plugin-updater = "2"`.
- `main.rs` (or wherever the builder lives): register the plugin.
- `tauri.conf.json`:
  - `bundle.createUpdaterArtifacts: true`
  - `plugins.updater.pubkey`: the generated public key (committed — it is public).
  - `plugins.updater.endpoints`:
    `["https://github.com/Chubik/world-radio/releases/latest/download/latest.json"]`
- All updater logic is Rust-side; no JS capabilities are added.

### 2. One-time key generation (done during implementation, not in CI)

`cargo tauri signer generate -w ~/.r4dio-updater-key` →
- public key into `tauri.conf.json`,
- private key into repo secret `TAURI_SIGNING_PRIVATE_KEY`,
- its password into `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (via `gh secret set`, repo
  Chubik/world-radio),
- the key file stays at `~/.r4dio-updater-key` for the user to archive.

### 3. CI (`.github/workflows/ci.yml`, macos-app job)

- Export both secrets as env on the `cargo tauri build` step; with
  `createUpdaterArtifacts` on, Tauri emits `r4dio.app.tar.gz` + `r4dio.app.tar.gz.sig` next to
  the bundle.
- Package step additionally:
  - renames the pair to `r4dio-<version>-macos-update.tar.gz` (+ `.sig`),
  - writes `latest.json`:

    ```json
    {
      "version": "<version>",
      "pub_date": "<iso8601>",
      "platforms": {
        "darwin-aarch64": { "signature": "<contents of .sig>", "url": "https://github.com/Chubik/world-radio/releases/download/v<version>/r4dio-<version>-macos-update.tar.gz" },
        "darwin-x86_64":  { "signature": "<same>", "url": "<same>" }
      }
    }
    ```

    Both platform keys point at the one universal archive — the updater on each arch looks up
    its own key; the payload is the same fat binary.
  - fails loudly if the `.sig` is missing or `latest.json` fails a `jq` shape check
    (version non-empty, both signatures non-empty).
- The new pair + `latest.json` join the existing artifact upload and land on the GitHub release
  with the other assets. The `releases/latest/download/latest.json` endpoint then always serves
  the newest release's manifest with zero extra infrastructure.

### 4. App wiring

- On setup, spawn an async task: `app.updater()?.check().await`. Any error is silent (log only)
  — a radio that nags about update-check failures is worse than a missed notice. The check does
  not block startup; the plugin's HTTP client has its own timeout.
- When an update exists: store it in state and show a tray-menu item labeled by a pure function
  in `tray.rs` (same pattern as `playstop_label`):

  ```rust
  pub fn update_label(version: &str) -> String  // "Update to v1.17.0"
  ```

- Click → `update.download_and_install().await` → `app.restart()`. A download/install error
  logs and leaves the menu item in place for a retry; no dialogs.
- While no update exists the menu item is absent entirely — not a disabled row.

## Out of scope

Apple notarization, the App Store, auto-install without a click, update UI in the main window
(the tray item is the whole surface), Windows/Linux bundles.

## Verification

- Unit: `update_label` formats as "Update to v<version>".
- CI: the release contains `latest.json` and `r4dio-<version>-macos-update.tar.gz` + `.sig`;
  `jq` validates the manifest shape before publishing.
- Local: `cargo tauri build` with a locally-set signing env produces the updater artifacts.
- **The full loop — an installed old version discovers and installs the new one — is only
  provable once TWO updater-capable releases exist.** The first updater release (N) ships the
  machinery; on release N+1 the running copy of N must show "Update to v<N+1>", install it on
  click, and relaunch as N+1. That check is owed at release N+1 and recorded as such.
