# Android: removals that stick, and changes that actually sync

**Goal:** unstarring a station on Android stays unstarred, and every state change made in the app reaches the other devices instead of waiting for an unrelated event.

**Architecture:** the server and the CLI already implement removal-aware sync — the server takes a `changed` list of `{id, gone}` tombstones alongside the plain `present` list, and the CLI tracks them in `radio-core/src/sync/pending.rs`. Android never got that half: it sends only the plain list, so the server (correctly, per its own contract) treats a missing id as "not mentioned" rather than "deleted", and `applyMerged` then overwrites local state with the server's answer. This plan ports the CLI's `Pending` design to Kotlin and wires a sync into the seven app-side actions that currently change state silently.

**Tech Stack:** Kotlin, Jetpack Compose, DataStore Preferences, kotlinx.serialization; the existing `SyncClient` / `Profile.outgoing` path.

**Spec:** none written — the diagnosis is in this plan's Context section, traced through all three layers on 2026-08-28.

## Context: what is actually broken

Two defects, one theme. Both were traced to source, not guessed.

**1. Removals do not survive a sync.**

`FavStore.toggleFav` (FavStore.kt:122) is correct and symmetric. The server is correct too — `sync/src/merge.rs:66` documents the contract:

> a plain listing only asserts existence; it must never outrank a tombstone, so an id already known is left exactly as it is.

and has a passing test, `a_tombstone_survives_a_later_push_that_still_lists_the_item`.

The gap is the Android client. `Profile.outgoing` (Profile.kt:91) builds `SyncData` with `favs = favs` — a plain list. Grepping `Profile.kt` and `SyncClient.kt` for `changed` / `Change` returns nothing. So:

1. user unstars → local set loses the uuid (correct)
2. `syncNow()` pushes a list that simply omits it
3. server sees a known id and no tombstone → keeps it
4. `applyMerged` (FavStore.kt:452) does `prefs[keyFavs] = favs` — a full overwrite from the server → the star is back

`applyMerged:459` does the same for `keyBlocked`, so **unblocking is broken identically**. Excluded countries go through `Lww` and are unaffected by this defect (but see defect 2 — they are affected by that one).

**2. Seven app-side actions change state and never sync.**

In `MainActivity.kt`, all of these call into `favStore` and stop there:

| line | action |
|---|---|
| 115 | `setTheme` |
| 117 | `setExcluded` (show country) |
| 121 | `clearPlayHistory` |
| 128 | `toggleBlocked` (block playing) |
| 134 | `toggleFav` (star) |
| 135 | `toggleBlocked` (block) |
| SyncActivity.kt:189 | `setExcluded` |

Compare `PlaybackService.kt:964`, where the notification's star does `toggleFav` **and then** `syncNow()`. The same star, tapped in the app, does not.

A comment at MainActivity.kt:113 says "the next sync carries it to the desktop without a command of its own" — but verified: there is **no periodic sync** and **no sync on `onPause`/`onDestroy`**. Changes sit locally until something unrelated happens (service start, scope change).

## Global Constraints

- **Kotlin/Android.** Match the surrounding style: `when` over if-chains where the codebase does, comments lower-case and explaining *why*.
- **The wire format already exists and must not change.** The server accepts `changed: { favs: [{id, gone}], blocked: [...], excluded_countries: [...] }`, all fields `#[serde(default)]`. Do not invent a new shape; do not touch the server.
- **Backwards compatibility is load-bearing.** An old Android build must keep working against the new server, and this new build must work against the current server. Both are already true if the format is respected — `changed` is optional on the wire.
- **Commits:** English, concise, changelog-style subject. NO AI/assistant mention, NO `Co-Authored-By`, NO "Generated with" trailer.
- **Branch:** `dev` in the `radio` repo. Never commit to `main`.
- **Verification:** `cd android && ./gradlew test` must pass. This project's gradle
  does NOT accept `--tests` (verified: "Unknown command-line option '--tests'"), so
  run the whole suite. Gradle caches hard — a 500ms "BUILD SUCCESSFUL" means nothing
  actually ran; confirm by reading the real counts out of
  `app/build/test-results/testDebugUnitTest/TEST-*.xml`. The emulator workflow is the project's normal way to prove Android behaviour — a unit test alone does not prove a sync round-trip.
- **Never touch the user's real data directory** when testing.

---

## File Structure

| file | responsibility |
|---|---|
| `android/app/src/main/kotlin/net/vchub/r4dio/PendingChanges.kt` (create) | the Kotlin port of `Pending`: note a change, merge, clear-after-push |
| `FavStore.kt` (modify) | persist pending changes; record one on every mutation; expose them for the push; clear them after a successful push |
| `Profile.kt` (modify) | carry `changed` in the outgoing payload |
| `SyncClient.kt` (modify) | send it |
| `MainActivity.kt` (modify) | request a sync after each state-changing action |
| `SyncActivity.kt` (modify) | same, for its `setExcluded` |
| `PendingChangesTest.kt` (create) | unit tests for the port |

---

## Task 1: The Pending port

**Files:**
- Create: `android/app/src/main/kotlin/net/vchub/r4dio/PendingChanges.kt`
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/PendingChangesTest.kt`

**Interfaces:**
- Produces:
  - `enum class ChangeSet { FAVS, BLOCKED, COUNTRIES }`
  - `@Serializable data class Change(val id: String, val gone: Boolean)`
  - `@Serializable data class PendingChanges(favs, blocked, excluded_countries: List<Change> = emptyList())`
  - `PendingChanges.note(set, id, gone): PendingChanges` — returns a copy
  - `PendingChanges.clearPushed(pushed): PendingChanges`
  - `PendingChanges.isEmpty(): Boolean`

**Test location:** `android/app/src/test/kotlin/net/vchub/r4dio/` — the project already has unit tests there (`BackupFileTest.kt`, `CatalogCacheTest.kt` and others). Follow their style.

**Read first:** `crates/radio-core/src/sync/pending.rs` — this is a port of it, not a new design. Two behaviours in there are load-bearing and easy to miss:
- `note` **replaces** any earlier change for the same id (`list.retain(|c| c.id != id)` then push) — starring, unstarring and starring again must leave exactly one entry saying `gone: false`.
- `clear_pushed` removes only what was actually pushed, so an edit written **during** the round trip survives. There is a CLI test named `clear_pushed_keeps_an_edit_written_during_the_round_trip` — the Kotlin port needs the equivalent.

- [ ] **Step 1: Write the failing tests**

```kotlin
package net.vchub.r4dio

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class PendingChangesTest {
    @Test
    fun a_removal_is_recorded_as_gone() {
        val p = PendingChanges().note(ChangeSet.FAVS, "abc", gone = true)
        assertEquals(listOf(Change("abc", true)), p.favs)
    }

    @Test
    fun the_latest_change_for_an_id_replaces_the_earlier_one() {
        // star, unstar, star again must leave ONE entry, not three
        val p = PendingChanges()
            .note(ChangeSet.FAVS, "abc", gone = false)
            .note(ChangeSet.FAVS, "abc", gone = true)
            .note(ChangeSet.FAVS, "abc", gone = false)
        assertEquals(listOf(Change("abc", false)), p.favs)
    }

    @Test
    fun sets_do_not_bleed_into_each_other() {
        val p = PendingChanges()
            .note(ChangeSet.FAVS, "a", gone = true)
            .note(ChangeSet.BLOCKED, "b", gone = false)
        assertEquals(listOf(Change("a", true)), p.favs)
        assertEquals(listOf(Change("b", false)), p.blocked)
        assertTrue(p.excluded_countries.isEmpty())
    }

    @Test
    fun clearing_what_was_pushed_leaves_the_rest() {
        val pushed = PendingChanges().note(ChangeSet.FAVS, "a", gone = true)
        val current = pushed.note(ChangeSet.FAVS, "b", gone = true)
        assertEquals(listOf(Change("b", true)), current.clearPushed(pushed).favs)
    }

    @Test
    fun an_edit_written_during_the_round_trip_survives_the_clear() {
        // "a" was pushed as gone; while the request was in flight the user
        // starred it again. clearing the push must NOT drop that newer edit.
        val pushed = PendingChanges().note(ChangeSet.FAVS, "a", gone = true)
        val current = pushed.note(ChangeSet.FAVS, "a", gone = false)
        assertEquals(listOf(Change("a", false)), current.clearPushed(pushed).favs)
    }

    @Test
    fun empty_means_nothing_to_send() {
        assertTrue(PendingChanges().isEmpty())
        assertTrue(!PendingChanges().note(ChangeSet.FAVS, "a", gone = true).isEmpty())
    }
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cd android && ./gradlew test
```

Expected: compilation failure — `PendingChanges` does not exist.

- [ ] **Step 3: Implement**

Write `PendingChanges.kt`. The field names must be exactly `favs`, `blocked`, `excluded_countries` — they go on the wire and the server deserialises by name. Keep the snake_case name for the third even though Kotlin would prefer camelCase; add a comment saying it matches the wire format, or use `@SerialName`.

`clearPushed` is the subtle one: for each set, drop an entry only when the pushed list contains an entry with the same id **and the same `gone` value**. Same id with a different value means the user changed their mind mid-flight and the newer edit must stay.

- [ ] **Step 4: Run to verify it passes**

```bash
cd android && ./gradlew test
```

Expected: 6 passing.

- [ ] **Step 5: Commit**

```bash
cd /Users/vchub/dev/projects/world-radio/radio
git add android/app/src/main/kotlin/net/vchub/r4dio/PendingChanges.kt android/app/src/test/kotlin/net/vchub/r4dio/PendingChangesTest.kt
git commit -m "track what changed locally so a removal can be expressed"
```

---

## Task 2: Record a change on every mutation

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/FavStore.kt`
- Test: `android/app/src/test/kotlin/net/vchub/r4dio/` (add to an existing FavStore test file if one exists; otherwise extend `PendingChangesTest.kt` with store-level tests)

**Interfaces:**
- Consumes: `PendingChanges`, `ChangeSet` from Task 1
- Produces:
  - `FavStore.currentPending(): PendingChanges`
  - `FavStore.clearPushedPending(pushed: PendingChanges)`
  - every mutation records into the same DataStore transaction as the mutation itself

- [ ] **Step 1: Add the storage key and accessors**

A new `stringPreferencesKey("pending_changes")` holding the JSON. Read it with the same `runCatching { ... }.getOrDefault(...)` shape the file already uses for `keyCached` (see `cachedFavs`, FavStore.kt:115) — a corrupt value must degrade to "nothing pending", never crash.

- [ ] **Step 2: Record inside the existing transactions**

The mutations to cover, all in `FavStore.kt`:

| function | set | gone |
|---|---|---|
| `toggleFav` (:122) | `FAVS` | `!next.contains(uuid)` |
| `toggleBlocked` (:145) | `BLOCKED` | whether the uuid left the set |
| `setExcluded` | `COUNTRIES` | one entry per country added *and* removed |

**Record inside the same `store.edit { }` block as the mutation.** Not after it. A crash between two separate transactions would leave the sets and the pending list disagreeing, and the disagreement is silent.

For `setExcluded`, diff the previous and next sets: ids in `next - prev` are `gone = false`, ids in `prev - next` are `gone = true`.

- [ ] **Step 3: Write the failing test**

Test through the real store with a temp DataStore if the project has that harness; otherwise test the pure diff helper you extract for `setExcluded`. State in the report which you did and why.

```kotlin
    @Test
    fun unstarring_records_a_tombstone() {
        // after toggleFav removes a uuid, currentPending must contain
        // Change(uuid, gone = true) — that is the whole point of the fix
    }

    @Test
    fun changing_countries_records_both_directions() {
        // prev = {UA, PL}, next = {PL, DE}
        // => UA gone=true, DE gone=false, PL absent
    }
```

- [ ] **Step 4: Run, implement, run again**

```bash
cd android && ./gradlew test
```

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/FavStore.kt android/app/src/test/
git commit -m "record every favourite and block change as it happens"
```

---

## Task 3: Send the changes, and clear them only after they land

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/Profile.kt` (`outgoing`)
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/SyncClient.kt`
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/PlaybackService.kt` (`syncNow`, around :650-680)

**Interfaces:**
- Consumes: `FavStore.currentPending()`, `FavStore.clearPushedPending()`
- Produces: a payload carrying `changed`

- [ ] **Step 1: Add `changed` to the payload**

`SyncData` gains a `changed: PendingChanges` field. Confirm against the server's `store::ChangeSets` (sync/src/store.rs:41) that the JSON keys match exactly: `favs`, `blocked`, `excluded_countries`, each a list of `{id, gone}`.

- [ ] **Step 2: Read the pending set before the push, clear it after**

In `syncNow()` (PlaybackService.kt), the else-branch that pushes:

```kotlin
val pending = favStore.currentPending()
val local = profile.outgoing(
    favs = favStore.currentFavUuids().toList(),
    blocked = favStore.currentBlocked().toList(),
    excluded = favStore.currentExcluded().toList(),
    plays = plays,
    changed = pending,
)
val merged = withContext(Dispatchers.IO) { syncClient.push(key, local) } ?: return@launch
// only now: a failed push must leave the tombstones queued, or a removal is
// lost forever. clear exactly what was sent, so an edit made during the
// round trip survives.
favStore.clearPushedPending(pending)
favStore.applyMerged(...)
```

**The ordering is the whole point.** Clearing before the push, or clearing everything rather than what was pushed, reintroduces the bug in a subtler form.

- [ ] **Step 3: Verify the round trip against the real server**

This cannot be proven by a unit test — it is a protocol change. Use the emulator workflow the project already has (see the android-emulator-workflow notes): link a sync key, star a station, unstar it, force a sync, and confirm the star does not come back. Capture the actual request body if possible.

- [ ] **Step 4: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/
git commit -m "tell the server what was removed, not just what remains"
```

---

## Task 4: Sync when the app changes something

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/MainActivity.kt`
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/SyncActivity.kt`

**Interfaces:**
- Consumes: the existing `ACTION_SYNC_NOW` path (`PlaybackService.kt:268`)

- [ ] **Step 1: Add one helper, use it everywhere**

**Do not invent this — copy it.** `SyncActivity.kt:270` already has exactly the helper needed:

```kotlin
private fun triggerSync() {
    startService(
        android.content.Intent(this, PlaybackService::class.java)
            .setAction(ACTION_SYNC_NOW),
    )
}
```

Put the same helper in `MainActivity` and call it after each of the seven actions listed in the Context section.

Do **not** call `syncNow` directly from the activity — the service owns the sync and already debounces it (`PlaybackService.kt:96` describes collapsing concurrent syncs). Going through the existing action keeps that debounce.

- [ ] **Step 2: Handle the no-service case**

If the playback service is not running, starting it just to sync a theme change would be wrong. Check what `ACTION_SYNC_NOW` does when the service is stopped, and if it would start it, gate the call — say in the report what you found and what you chose.

- [ ] **Step 3: Remove the stale comment**

MainActivity.kt:113 claims "the next sync carries it to the desktop without a command of its own". That was never true — there is no periodic sync. Replace it with what the code now does.

- [ ] **Step 4: Verify on the emulator**

Change a theme in the app, then confirm it reaches the server without touching playback. Then unstar from the app screen (not the notification) and confirm it sticks.

- [ ] **Step 5: Commit**

```bash
git add android/app/src/main/kotlin/net/vchub/r4dio/
git commit -m "sync a change made in the app without waiting for playback"
```

---

## Task 5: The overlay permission failure is silent

**Files:**
- Modify: `android/app/src/main/kotlin/net/vchub/r4dio/MainActivity.kt` (`askOverlay`, :188)

This is the user's first reported symptom — "Android does not give the permission to draw over other apps". The permission is declared (`AndroidManifest.xml:10`) and the request path exists, but:

```kotlin
runCatching {
    startActivity(Intent(Settings.ACTION_MANAGE_OVERLAY_PERMISSION, ...))
}
```

`runCatching` with no failure branch swallows the error. Note that `SyncActivity.kt:266` in this same codebase does it properly — `runCatching { startActivity(intent) }.onFailure { toast(...) }` — so the house style is already to surface these, and `askOverlay` is the outlier. On a device where that intent does not resolve — some OEM ROMs do not expose it — the user sees a toast and nothing else happens, which is exactly the reported experience.

- [ ] **Step 1: Surface the failure**

Handle the failure case: log it, and tell the user what to do by hand (Settings → Apps → r4dio → Display over other apps). Do not leave a bare `runCatching`.

- [ ] **Step 2: Reproduce first, if you can**

Before changing behaviour, check on the emulator whether the intent resolves at all (`packageManager.resolveActivity`). If it resolves and the flow works, then the reported problem is elsewhere — say so rather than "fixing" a working path. **A previous investigation of this same overlay found the code was correct and the real defect was a mislabelled button; do not repeat that mistake by assuming.**

- [ ] **Step 3: Commit**

```bash
git commit -m "say why the overlay permission screen could not open"
```

---

## Self-review notes

**Coverage.** Defect 1 (removals) is Tasks 1-3; defect 2 (missing syncs) is Task 4; the overlay report is Task 5, deliberately last and deliberately sceptical.

**The risk worth naming.** Task 3 changes the sync protocol payload. It is additive and the server's fields are all `#[serde(default)]`, so old clients and the current server both keep working — but that claim should be *checked* against a real push, not assumed. Task 3 Step 3 exists for that.

**What this plan does not do.** It does not add a periodic background sync. That is a bigger design question (battery, Data Saver, WorkManager) and the seven explicit triggers in Task 4 solve the reported problem without it.
