# Site downloads, the macOS app, and honest release notes — plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to
> implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Spec:** `docs/superpowers/specs/2026-08-09-site-downloads-and-macos.md`

**Goal:** make downloads work at all, put the macOS app where people can find it, and stop telling
users every release is "maintenance".

**Three repositories, deployed separately** — this is the thing to keep straight:

| Repo | Holds | Deployed by |
|---|---|---|
| `ops` | the site, nginx, the deploy workflow | its own workflow, on push to main |
| `radio` | the app, and the CI that builds releases | `dev`→`main` PR, admin-merge |
| `stat` | the stats service | pushed and deployed manually |

Editing `radio/ops/...` does nothing — `ops` is a **separate checkout** at
`/Users/vchub/dev/projects/world-radio/ops`. Same for `stat`. See [[deploy-architecture]].

## Global Constraints

- English code/comments/copy; comments **lowercase**, WHY not WHAT.
- **No AI/assistant mention anywhere** — code, comments, commits, and especially not the site.
- **No `else if`**.
- Commit subjects are the public changelog — and Task 3 makes them visible on the site, so they
  matter more than usual here.
- Amber CRT palette; the site's existing type scale and spacing.
- **Never launch r4dio without warning the user — it plays audio out loud on their machine.**
  Task 2 needs it for a screenshot; that is the only step that does.

---

### Task 1: Make downloads work again

**Repo: `ops`.** The urgent one — every download button on r4dio.net 404s today, on every platform.

**Files:** `ops/site/index.html` (the JS at ~line 656-675).

- [ ] **Step 1: Confirm the failure and the fix target**

```bash
curl -s -o /dev/null -w "site:   %{http_code}\n" "https://r4dio.net/releases/r4dio-1.14.0-aarch64-apple-darwin.tar.gz"
curl -s -o /dev/null -w "github: %{http_code}\n" -L "https://github.com/Chubik/world-radio/releases/download/v1.14.0/r4dio-1.14.0-aarch64-apple-darwin.tar.gz"
```

Expect `site: 404` and `github: 200`. If the site now returns 200, someone populated the volume —
stop and report rather than "fixing" a bug that no longer exists.

- [ ] **Step 2: Point the buttons at GitHub**

`index.html:671` currently does:

```js
if (hrefEl) { hrefEl.href = `/releases/${file}`; hrefEl.setAttribute('download', ''); }
```

Replace the local path with the release URL for the **exact version**, not `latest` — `appVer` is
already in scope at line 656:

```js
`https://github.com/Chubik/world-radio/releases/download/v${appVer}/${file}`
```

Drop the `download` attribute: it does nothing cross-origin and misleads whoever reads this next.
Leave a comment saying why the local path is gone, so nobody reinstates it.

**Do not delete the nginx `/releases` blocks or the volume in this task.** They are harmless, and
removing infrastructure is a separate decision from fixing a user-facing 404.

- [ ] **Step 3: Check every card, not just the one you edited**

The same loop builds Linux, macOS arm64 and macOS Intel. Confirm all three hrefs, and that the
SHA256 lines still render — they come from a different source (`cli-info`, then `SHA256SUMS`) and
must not regress.

- [ ] **Step 4: Verify after deploy**

This is `ops`, so it deploys on push to main. After it lands, fetch the real page and confirm the
hrefs — and then **actually request one** and expect 200. A page that merely contains a URL is not
a working download.

- [ ] **Step 5: Commit** — `fix the download buttons on the site`

---

### Task 2: Put the macOS app on the site

**Repo: `ops`**, plus a screenshot that has to be taken from the running app.

**Files:** `ops/site/index.html`, `ops/site/img/` (new screenshot).

- [ ] **Step 1: Take the screenshot**

**Ask the user before launching** — it plays audio. Then open the main window on Favourites, which
is the section that shows the app is a real player rather than a menubar toy.

Capture the window alone, not the whole desktop. Save as `ops/site/img/macos-app.png`. Match the
Android shot's treatment (`ops/site/img/android-home.png`, 360×800) — same visual weight, so the two
sections sit together rather than competing.

The user asked for the two to look symmetric. That is the acceptance test.

- [ ] **Step 2: Add a download entry**

A card for the app, clearly distinct from the CLI tarballs — someone looking for "the app" must not
land on a terminal binary. It is **Universal** (one file, Intel and Apple Silicon), which is worth
saying since the CLI is split into two.

File name pattern: `r4dio-<version>-macos.dmg` once Task 5 lands, with the `.zip` as the secondary.
Wire its SHA256 the way the tarballs already do.

- [ ] **Step 3: Add the section**

Symmetric with `#android` (`index.html:378-403`): kicker, title, lead, screenshot in `.shot` with a
caption, and a `.feat-list` of four. Suggested four, mirroring Android's:

- **Menubar** — what is playing, one click away, without a dock icon.
- **Shuffle anywhere** — ⌥⇧R changes station without opening anything, even over a fullscreen app.
- **Favourites & filters** — star, block, and hide whole countries from the window.
- **Sync** — pair with a key or QR; favourites follow you from the phone.

Every claim must be true of v1.14.0. Do not promise signing, notarisation, or Windows.

- [ ] **Step 4: Verify the two sections together**

Load the page and look at the Android and macOS sections one after the other. They must read as a
pair. Report what you saw; if they do not balance, say so rather than shipping it.

- [ ] **Step 5: Commit** — `show the macos app on the site`

---

### Task 3: Real release notes

**Repo: `radio`** (the CI that builds releases).

Every release reads "Maintenance and internal improvements." — including the one that shipped a
whole new application.

**Files:** `.github/workflows/ci.yml` (the notes step, ~line 89 in the vendored copy under `ops`;
find the live one in `radio`).

- [ ] **Step 1: Find where the notes are built and why the fallback always wins**

Read the step. Establish whether it looks for notes and finds none, or never looks. Report which —
the fix differs.

- [ ] **Step 2: Build notes from commit subjects**

The material exists: this project writes commit subjects as user-facing changelog entries
([[commit-subject-is-public-changelog]]). Collect the subjects between the previous tag and this
one.

**Exclude:** merge commits, `plan:`/`spec:`/`brief:` commits, and CI chores (`format ...`,
`stamp ...`). Those are the ones a user must never see.

Keep the fallback for a release that genuinely has nothing user-facing — but it must be the
exception, not the rule.

- [ ] **Step 3: Prove it against a real range**

Run the extraction over `v1.13.0..v1.14.0` locally and print what it would publish. v1.14.0 has 18
user-facing commits and 3 that must be filtered out. Paste the output in your report — that list is
the deliverable, not the code.

- [ ] **Step 4: Commit** — `tell people what actually changed in each release`

---

### Task 4: An APP column in the stats

**Repo: `stat`** — separate checkout, deployed manually ([[stat-service-separate-repo]]).

**Files:** `stat/src/github.rs`, `stat/src/render.rs`, `stat/src/main.rs`.

- [ ] **Step 1: Add the counter**

`github.rs:46` sums `.tar.gz` as CLI; `.apk` is Android (`github.rs:69,137`). Add the macOS app:
`.dmg` **and** `.zip`, since both ship and either may be downloaded.

- [ ] **Step 2: Render it**

`VERSION | CLI | APP | APK`. Zero must render as `0`, not as blank — a blank column reads as
"broken", and v1.14.0 legitimately has few downloads.

- [ ] **Step 3: Verify against the live API**

The counts come from GitHub ([[download-stats-come-from-github]]). Fetch the real release list and
confirm the APP number matches what GitHub reports for the zip and dmg assets.

- [ ] **Step 4: Commit and deploy** — `count macos app downloads`. Deployment is manual; say
  plainly in your report whether you deployed or only committed.

---

### Task 5: Ship a .dmg

**Repo: `radio`.**

**Files:** `crates/r4dio-macos/tauri.conf.json`, `.github/workflows/ci.yml`.

- [ ] **Step 1: Build both targets**

`"targets": ["app", "dmg"]`. Verify locally:

```bash
cd crates/r4dio-macos && cargo tauri build --target universal-apple-darwin
find ../../target/universal-apple-darwin/release/bundle -name "*.dmg"
```

Expect a dmg next to the `.app`. Report its real path — the bundle layout has surprised this project
before.

- [ ] **Step 2: Attach it, with a checksum**

**The user was explicit: the dmg must have a SHA256 like everything else.** The macos job already
zips the app and writes a `.sha`; do the same for the dmg so it reaches `SHA256SUMS` through the
existing `cat ./*.sha` in the publish step.

- [ ] **Step 3: Check the version stamp while you are here**

`tauri.conf.json`'s version was shipping as `1.4.5` because CI stamped the file and never committed
it. Fixed in `3f670ac`. Confirm the next release's dmg reports the real version:

```bash
plutil -extract CFBundleShortVersionString raw <bundle>/Contents/Info.plist
```

- [ ] **Step 4: Commit** — `offer the macos app as a dmg`

---

### Task 6: Release and verify for real

- [ ] **Step 1: Ship `radio`**

`dev`→`main` PR, admin-merge with `[minor]`. Two runs start; the merge 409s while either is going.

- [ ] **Step 2: Verify the artefacts, not the workflow's word**

Download the dmg **from the release**, check its checksum against `SHA256SUMS`, and confirm
`lipo -archs` reports both `x86_64` and `arm64`.

- [ ] **Step 3: Verify the site end to end**

Click a download on the real r4dio.net and confirm the file arrives. Confirm the changelog shows
real notes for the new version, and that the macOS section renders with its screenshot.

**This is the step that matters.** Every defect in this plan existed because something was assumed
to work rather than requested once.

---

## Self-Review

**Spec coverage:** broken downloads → Task 1 · macOS on the site → Task 2 · release notes → Task 3 ·
stats column → Task 4 · dmg with checksum → Task 5 · verification → Task 6.

**Ordered by user harm**, as the user chose: a 404 on every download outranks a missing screenshot.

**Three repos, three deploy paths** — called out at the top because getting this wrong means editing
files that will never ship. `radio/ops/` is a vendored copy and is not the deployed site.

**Two places this plan expects to be wrong, flagged rather than hidden:** the release-notes step may
never look for notes at all rather than looking and failing (Task 3 Step 1 says find out first), and
the dmg's bundle path is a guess — this project has been surprised by Tauri's bundle layout before
(Task 5 Step 1 says report the real one).

**Not built:** signing/notarisation, removing the unused `/releases` nginx blocks, or any rework of
how the site fetches version data.
