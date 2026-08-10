# macOS In-App Updater Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The macOS app checks GitHub releases at startup and, when a newer version exists, offers a one-click "Update to v…" in the tray menu that downloads, installs, and relaunches — no dmg re-download, no second Gatekeeper trip.

**Architecture:** The official `tauri-plugin-updater` (v2) does the transport, signature check, and install; CI signs the update archive with our minisign key and publishes a `latest.json` manifest on every release; the app's only new logic is a silent startup check, one tray-menu row inserted when an update exists, and a click handler. All updater logic is Rust-side.

**Tech Stack:** Tauri 2, tauri-plugin-updater 2, minisign (via `cargo tauri signer`), GitHub Actions + `gh` CLI, jq.

**Spec:** `docs/superpowers/specs/2026-08-10-macos-in-app-updater.md`

## Global Constraints

- No code comments unless they state a constraint the code cannot show; lowercase-first. No `else if` — use `when`/`match`.
- All strings/logs English, lowercase-first. Commit subjects are the public changelog.
- The private key NEVER enters the repo, the plan, a commit, or any log output. Only the public key is committed.
- Endpoint URL exactly: `https://github.com/Chubik/world-radio/releases/latest/download/latest.json`
- Update asset name exactly: `r4dio-<version>-macos-update.tar.gz` (+ `.sig`).
- Check errors at startup are silent (log only). No dialogs anywhere.
- The update menu row is absent when no update exists — never a disabled row.

---

### Task 1: Generate the signing key and store the secrets

Operational task — no repo files change. Work happens in the shell and the GitHub repo settings.

**Files:**
- Create (outside the repo): `~/.r4dio-updater-key` (private key), `~/.r4dio-updater-key.password`, `~/.r4dio-updater-key.pub` (public key — Task 2 reads this)

**Interfaces:**
- Produces: repo secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` on `Chubik/world-radio`; the public-key string in `~/.r4dio-updater-key.pub`.

- [ ] **Step 1: Ensure tauri-cli is available**

Run: `cargo tauri --version || cargo install tauri-cli --version '^2' --locked`
Expected: a 2.x version prints (install takes a few minutes if missing).

- [ ] **Step 2: Generate the keypair with a random password**

```bash
pw="$(openssl rand -hex 24)"
printf '%s' "$pw" > ~/.r4dio-updater-key.password
chmod 600 ~/.r4dio-updater-key.password
cargo tauri signer generate -w ~/.r4dio-updater-key --password "$pw"
```

The command writes `~/.r4dio-updater-key` (private) and `~/.r4dio-updater-key.pub` (public), and prints the public key. Do NOT print or cat the private key.

- [ ] **Step 3: Store both secrets on the repo**

```bash
gh secret set TAURI_SIGNING_PRIVATE_KEY --repo Chubik/world-radio < ~/.r4dio-updater-key
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo Chubik/world-radio < ~/.r4dio-updater-key.password
```

- [ ] **Step 4: Verify**

Run: `gh secret list --repo Chubik/world-radio | grep TAURI_SIGNING`
Expected: both secret names listed. Also `test -s ~/.r4dio-updater-key.pub && echo pub-ok`.

No commit — nothing in the repo changed. Report the public key path in your report (never the private key).

---

### Task 2: Plugin, config, tray row, and wiring in the app

**Files:**
- Modify: `crates/r4dio-macos/Cargo.toml` (dependencies, near `tauri-plugin-global-shortcut` at :30)
- Modify: `crates/r4dio-macos/tauri.conf.json` (bundle + new plugins section)
- Modify: `crates/r4dio-macos/src/tray.rs` (label + test)
- Modify: `crates/r4dio-macos/src/main.rs` (`run()` at :187-380: plugin registration, startup check, menu insert, click handler)

**Interfaces:**
- Consumes: the public key from `~/.r4dio-updater-key.pub` (Task 1); existing menu construction in `main.rs:199-250`; `on_menu_event` at `main.rs:270-288`.
- Produces: `tray::update_label(version: &str) -> String`; menu item id `"update"`. Task 3 does not consume code from this task.

- [ ] **Step 1: Write the failing label test**

Add to `crates/r4dio-macos/src/tray.rs` tests:

```rust
    #[test]
    fn update_label_names_the_version_it_installs() {
        assert_eq!(update_label("1.17.0"), "Update to v1.17.0");
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p r4dio-macos update_label`
Expected: compile FAIL — `update_label` does not exist.

- [ ] **Step 3: Implement the label**

Add to `crates/r4dio-macos/src/tray.rs` (near the other labels):

```rust
pub fn update_label(version: &str) -> String {
    format!("Update to v{version}")
}
```

Run: `cargo test -p r4dio-macos update_label` — expected PASS.

- [ ] **Step 4: Add the dependency and config**

`Cargo.toml` dependencies:

```toml
tauri-plugin-updater = "2"
```

`tauri.conf.json`:
- inside `"bundle"`, add `"createUpdaterArtifacts": true`
- add a top-level section (pubkey = the exact contents of `~/.r4dio-updater-key.pub`):

```json
"plugins": {
  "updater": {
    "pubkey": "<contents of ~/.r4dio-updater-key.pub>",
    "endpoints": ["https://github.com/Chubik/world-radio/releases/latest/download/latest.json"]
  }
}
```

- [ ] **Step 5: Wire the app**

In `crates/r4dio-macos/src/main.rs`. First check the exact `tauri_plugin_updater` v2 API with the context7/Ref docs tools (`Update::download_and_install` signature and whether `Update` is `Clone`); the code below is the intended shape — adjust call signatures to the real API, not the design.

a. Register the plugin on the builder (next to positioner):

```rust
        .plugin(tauri_plugin_updater::Builder::new().build())
```

b. In `setup`, after the tray is built (after `.build(app)?` at :319), spawn the silent check. The menu row is created up-front but NOT added to the menu; it is inserted only when a check succeeds — the row is therefore absent, not disabled, when there is no update:

```rust
            let update_item = MenuItem::with_id(app, "update", "", true, None::<&str>)?;
            app.manage(Mutex::new(None::<tauri_plugin_updater::Update>));
            {
                use tauri_plugin_updater::UpdaterExt;
                let handle = app.handle().clone();
                let menu_handle = menu.clone();
                let row = update_item.clone();
                tauri::async_runtime::spawn(async move {
                    let updater = match handle.updater() {
                        Ok(u) => u,
                        Err(e) => {
                            eprintln!("update check unavailable: {e}");
                            return;
                        }
                    };
                    match updater.check().await {
                        Ok(Some(update)) => {
                            let _ = row.set_text(tray::update_label(&update.version));
                            // insert above the final separator, next to the account row
                            let _ = menu_handle.insert(&row, 8);
                            let pending = handle.state::<Mutex<Option<tauri_plugin_updater::Update>>>();
                            *pending.lock().unwrap() = Some(update);
                        }
                        Ok(None) => {}
                        Err(e) => eprintln!("update check failed: {e}"),
                    }
                });
            }
```

(`menu` must stay bound in `setup` — it already is, at :236. If `Menu::insert` is not `Send`-safe off the main thread, wrap the insert in `handle.run_on_main_thread(...)`.)

c. In `on_menu_event` (the `match` at :272), add an arm:

```rust
                        "update" => {
                            let app = app.clone();
                            tauri::async_runtime::spawn(async move {
                                let update = {
                                    let pending =
                                        app.state::<Mutex<Option<tauri_plugin_updater::Update>>>();
                                    let guard = pending.lock().unwrap();
                                    guard.clone()
                                };
                                let Some(update) = update else { return };
                                match update.download_and_install(|_, _| {}, || {}).await {
                                    Ok(()) => app.restart(),
                                    Err(e) => eprintln!("update install failed: {e}"),
                                }
                            });
                        }
```

On install failure the row stays in the menu, so the click can be retried — exactly the spec's "logs and leaves the menu item in place".

- [ ] **Step 6: Compile and test**

Run: `cargo clippy -p r4dio-macos --all-targets 2>&1 | tail -5 && cargo test -p r4dio-macos 2>&1 | tail -3`
Expected: clippy clean, tests green. (clippy on this crate builds for the host mac — fine.)

- [ ] **Step 7: Verify the signed updater artifact builds locally**

```bash
cd crates/r4dio-macos
TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.r4dio-updater-key)" \
TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat ~/.r4dio-updater-key.password)" \
cargo tauri build --target aarch64-apple-darwin 2>&1 | tail -5
find ../../target/aarch64-apple-darwin/release/bundle -name '*.app.tar.gz*'
```

Expected: both `r4dio.app.tar.gz` and `r4dio.app.tar.gz.sig` exist. (Single-arch build — faster than universal; CI does universal.)

- [ ] **Step 8: Commit**

```bash
git add crates/r4dio-macos/Cargo.toml crates/r4dio-macos/tauri.conf.json \
        crates/r4dio-macos/src/tray.rs crates/r4dio-macos/src/main.rs Cargo.lock
git commit -m "let the macos app update itself from the tray"
```

---

### Task 3: CI — sign the update archive and publish latest.json

**Files:**
- Modify: `.github/workflows/ci.yml` — the `macos-app` job (build step ~:283, package step ~:287-308, upload ~:310-318) and the release-assets list in the publish job (~:355-356).

**Interfaces:**
- Consumes: repo secrets `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (Task 1). Nothing from Task 2's code — but Task 2's config is what makes the build emit the artifacts.
- Produces: release assets `r4dio-<version>-macos-update.tar.gz`, `.sig`, `latest.json`.

- [ ] **Step 1: Pass the secrets to the build step**

On the `build the bundle` step add:

```yaml
        env:
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
```

- [ ] **Step 2: Extend the package step**

After the dmg block inside the `package` step, add:

```bash
          # updater artifacts: tauri emits a signed app.tar.gz when
          # createUpdaterArtifacts is on; a release without them would strand
          # every installed copy on its current version, so fail loudly.
          built_upd="$(find target/universal-apple-darwin/release/bundle -name '*.app.tar.gz' | head -1)"
          test -n "$built_upd" || { echo "no updater archive was built"; exit 1; }
          test -f "$built_upd.sig" || { echo "updater archive is unsigned"; exit 1; }
          upd="r4dio-${version}-macos-update.tar.gz"
          mv "$built_upd" "$upd"
          mv "$built_upd.sig" "$upd.sig"
          url="https://github.com/Chubik/world-radio/releases/download/v${version}/${upd}"
          jq -n --arg v "$version" --arg d "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
                --arg sig "$(cat "$upd.sig")" --arg url "$url" \
                '{version:$v, pub_date:$d, platforms:{"darwin-aarch64":{signature:$sig,url:$url},"darwin-x86_64":{signature:$sig,url:$url}}}' \
                > latest.json
          jq -e '.version != "" and (.platforms["darwin-aarch64"].signature|length>0) and (.platforms["darwin-x86_64"].signature|length>0)' latest.json >/dev/null
          echo "upd=$upd" >> "$GITHUB_OUTPUT"
```

- [ ] **Step 3: Upload the new artifacts**

Extend the `macos-app` upload paths:

```yaml
            ${{ steps.pkg.outputs.upd }}
            ${{ steps.pkg.outputs.upd }}.sig
            latest.json
```

- [ ] **Step 4: Attach them to the release**

In the publish job's release-files list (where `dist/*.zip` and `dist/*.dmg` are), add:

```yaml
            dist/*-update.tar.gz
            dist/*-update.tar.gz.sig
            dist/latest.json
```

- [ ] **Step 5: Sanity-check the yaml**

Run: `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/ci.yml'))" 2>/dev/null || ruby -ryaml -e "YAML.load_file('.github/workflows/ci.yml')" && echo yaml-ok`
Expected: `yaml-ok`.

- [ ] **Step 6: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "sign macos updates and publish the update manifest"
```

---

## Verification after all tasks

- `cargo clippy -p r4dio-macos --all-targets` clean, `cargo test -p r4dio-macos` green,
  local single-arch build emits `.app.tar.gz` + `.sig`.
- Push dev; on the NEXT release confirm the three new assets are on it and
  `curl -sL https://github.com/Chubik/world-radio/releases/latest/download/latest.json | jq .version`
  matches.
- **Owed at release N+1** (the one after the first updater release): a running copy of N shows
  "Update to vN+1" in the tray, installs on click, relaunches as N+1. Record this in the session
  memory as an outstanding verification.
- Remind the user to move `~/.r4dio-updater-key` + `.password` into their password manager.
