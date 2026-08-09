# Site downloads, the macOS app, and honest release notes

**Spec. Written 2026-08-09.** Four defects found by the user within minutes of v1.14.0 shipping,
plus one addition they asked for. Ordered by user harm, which is the order they chose.

## 1. Every download on r4dio.net is broken — not just macOS

**This is the urgent one.** The user tried to download and Chrome reported *"File wasn't available
on site"*.

Measured:
- `https://r4dio.net/releases/r4dio-1.14.0-aarch64-apple-darwin.tar.gz` → **404**
- `https://r4dio.net/releases/` → 403
- The same file on GitHub → **200**

**Cause.** `ops/site/index.html:671` rewrites every download button at runtime:

```js
if (hrefEl) { hrefEl.href = `/releases/${file}`; hrefEl.setAttribute('download', ''); }
```

The static href in the markup points at GitHub releases and works. The JS replaces it with a
server-local path that has never held anything:

- `ops/deploy/docker-compose.prod.yml:18,21-22` declares and mounts a `releases` volume.
- `ops/deploy/nginx/world-radio.conf:38-50` proxies `/releases/*.tar.gz` and `/releases/SHA256SUMS`
  to the site container, with immutable caching.
- **Nothing populates the volume.** Not `ops/.github/workflows/deploy.yml`, not a Dockerfile, not a
  script. It has been empty since it was created.

So a "faster local download" was wired up and the file-copying half was never built. Every user who
clicked a download button since then got a 404.

**Required:** downloads work for every platform. GitHub already serves these files reliably and is
where the SHA256SUMS the site displays come from; the simplest correct fix is to stop rewriting the
href and let the static GitHub URL stand — with the version substituted so it points at the exact
release, not `latest`.

If the local copy is kept instead, the deploy must actually fetch the assets, and the fix is not
done until a real request returns 200.

**This also explains [[download-stats-come-from-github]]**: that note blames Cloudflare caching for
the nginx log missing "~all CLI downloads". The real reason is that there were no downloads through
the site to log.

## 2. The macOS app is not on the site at all

The site offers macOS **CLI tarballs** (`aarch64-apple-darwin`, `x86_64-apple-darwin`) — a terminal
program. The menubar app shipped in v1.13.0 and became a full player in v1.14.0, and there is no way
to get it from r4dio.net.

**Required:**
- A download entry for the app, distinct from the CLI tarballs so nobody confuses them.
- **A section with a screenshot, symmetric with the Android one** (`ops/site/index.html:378-403`:
  kicker, title, lead, a screenshot in `.shot` with a caption, and a `.feat-list` of four features).
  The user asked for this explicitly, and for the two to sit well together.
- Suggested feature list, mirroring Android's four: Menubar · Shuffle anywhere (⌥⇧R) ·
  Favourites & filters · Sync. Wording is the writer's call; the claims must be true of v1.14.0.

**A screenshot must be taken.** None exists. It needs the app running, which plays audio on the
user's machine — so it is taken deliberately, with the user's consent, not silently.

## 3. Release notes say nothing

Every release on the site's changelog reads **"Maintenance and internal improvements."** — including
v1.14.0, which shipped an entire new application.

**Cause:** `ops/vendor/world-radio/.github/workflows/ci.yml:89` falls back to that string when it
finds no notes, and the fallback is what always wins. The site renders the same fallback at
`ops/site/index.html:733`.

**Required:** real notes, built from the commit subjects. This project already writes commit
subjects as public changelog entries ([[commit-subject-is-public-changelog]]) precisely so this can
work — the material exists and is being thrown away. Merge-commit noise, `plan:`/`spec:` commits and
CI chores must not appear.

## 4. The stats cannot see the macOS app

`stat/src/github.rs:46` sums assets ending in `.tar.gz` as CLI, and `.apk` as Android
(`github.rs:69,137`). A `.zip` or `.dmg` matches neither, so the app's downloads are counted
nowhere. The user's stats page shows `VERSION | CLI | APK` with no column for it.

**Required:** an APP column counting the macOS app's assets, separate from CLI and APK. Note this
lives in the **`stat` repo**, which is deployed independently ([[stat-service-separate-repo]],
[[deploy-architecture]]).

## 5. Ship a .dmg as well as the .zip

**User decision, 2026-08-09.** Today the app ships as `r4dio-<v>-macos.zip`: unzip, drag to
Applications. A `.dmg` is the form macOS users expect. Tauri builds it from
`targets: ["app", "dmg"]` in `crates/r4dio-macos/tauri.conf.json`.

**Binding, stated by the user: the dmg must carry a SHA256 like every other artefact.** It goes into
`SHA256SUMS` alongside the tarballs and the zip.

Keep the zip as well — it is smaller and already referenced.

## Also found, fix while nearby

`crates/r4dio-macos/tauri.conf.json` still says `"version": "1.4.5"`, so the shipped app's about-box
names a version that does not exist. `scripts/bump-version.sh` stamps the file correctly, but
`.github/workflows/ci.yml:111` did not `git add` it, so the stamp was discarded before the tag.
**Already fixed on `dev` in `3f670ac`** — this spec records it so the next release is checked, not
assumed.

## Out of scope

- Signing and notarisation. The dmg will still warn on first open; that is unchanged and separate.
- Reworking how the site gets version data (`cli-info`, the stat poll). It works.
- Any change to the Android section beyond making the two sit symmetrically.

## Constraints

- Two repositories, deployed separately: **`ops`** (site, nginx, deploy) and **`stat`** (the stats
  service). `radio` holds the CI that builds releases. Changing one does not deploy another.
- Amber CRT palette; the site's existing type and spacing scale.
- No AI/assistant reference anywhere in the product or the site.
- Commit subjects are the public changelog — which item 3 finally makes visible, so they matter more
  than usual here.
- **Launching r4dio plays audio out loud on the user's machine.** The screenshot in item 2 is the
  only step that needs it; warn first, and stop it afterwards.
