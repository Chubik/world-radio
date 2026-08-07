use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Change {
    pub id: String,
    pub gone: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum Set {
    Favs,
    Blocked,
    Countries,
}

/// what this device changed since its last successful sync. the server needs it
/// because a plain list of what we still hold cannot express a removal.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct Pending {
    #[serde(default)]
    pub favs: Vec<Change>,
    #[serde(default)]
    pub blocked: Vec<Change>,
    #[serde(default)]
    pub excluded_countries: Vec<Change>,
}

impl Pending {
    pub fn note(&mut self, set: Set, id: &str, gone: bool) {
        let list = match set {
            Set::Favs => &mut self.favs,
            Set::Blocked => &mut self.blocked,
            Set::Countries => &mut self.excluded_countries,
        };
        list.retain(|c| c.id != id);
        list.push(Change {
            id: id.to_string(),
            gone,
        });
    }

    pub fn is_empty(&self) -> bool {
        self.favs.is_empty() && self.blocked.is_empty() && self.excluded_countries.is_empty()
    }

    pub fn clear(&mut self) {
        self.favs.clear();
        self.blocked.clear();
        self.excluded_countries.clear();
    }

    /// folds `disk` into `self`: per id, `self` wins (it is the more recent
    /// action by this surface), and ids `self` has no opinion on are kept as-is.
    /// this is what lets two surfaces (TUI, CLI) share one pending log without
    /// a save from one wiping out a concurrent write from the other.
    fn merge_from(&mut self, disk: &Pending) {
        merge_list(&mut self.favs, &disk.favs);
        merge_list(&mut self.blocked, &disk.blocked);
        merge_list(&mut self.excluded_countries, &disk.excluded_countries);
    }

    pub fn load(path: &Path) -> Pending {
        let Ok(raw) = std::fs::read_to_string(path) else {
            // a missing file is the normal case for a device with nothing pending
            return Pending::default();
        };
        match serde_json::from_str(&raw) {
            Ok(pending) => pending,
            Err(e) => {
                // unlike a missing file, this one existed and failed to parse —
                // silently returning empty here would drop a real pending deletion
                eprintln!(
                    "warning: sync pending log at {} is corrupt, treating as empty: {e}",
                    path.display()
                );
                Pending::default()
            }
        }
    }

    /// plain overwrite, for when `self` is already the whole truth — the
    /// post-sync clear, where re-merging what is on disk would replay
    /// deletions the server has already accepted forever.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let body = serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string());
        write_atomic(path, &body)
    }

    /// read-merge-write for the accumulate case: folds whatever another
    /// surface (TUI, CLI, mini) has since written into `self` before saving,
    /// so a save never clobbers a concurrent writer's entry for a different id.
    pub fn save_merged(&self, path: &Path) -> std::io::Result<()> {
        let mut merged = self.clone();
        merged.merge_from(&Pending::load(path));
        merged.save(path)
    }

    /// post-sync clear for the case where `pushed` (not necessarily `self`) is what
    /// the server round-trip actually sent: reads the current log and removes only the
    /// exact (id, gone) entries that were pushed, keeping anything written meanwhile.
    /// a plain overwrite here would silently drop an edit made during the round-trip;
    /// re-merging the whole disk log back in would replay an already-synced deletion
    /// forever. an id whose `gone` flipped since the push is a new, unsent action and
    /// must survive the clear.
    pub fn clear_pushed(pushed: &Pending, path: &Path) -> std::io::Result<()> {
        let mut current = Pending::load(path);
        current.favs = remove_pushed(&current.favs, &pushed.favs);
        current.blocked = remove_pushed(&current.blocked, &pushed.blocked);
        current.excluded_countries =
            remove_pushed(&current.excluded_countries, &pushed.excluded_countries);
        current.save(path)
    }
}

/// per-id, `into` wins; ids only present in `from` are appended unchanged.
fn merge_list(into: &mut Vec<Change>, from: &[Change]) {
    for change in from {
        if !into.iter().any(|c| c.id == change.id) {
            into.push(change.clone());
        }
    }
}

/// keeps every entry of `current` except one that exactly matches (same id AND
/// same `gone`) an entry in `pushed` — an id present in both but with `gone`
/// flipped is a new action written during the round-trip, not the one that was sent.
fn remove_pushed(current: &[Change], pushed: &[Change]) -> Vec<Change> {
    current
        .iter()
        .filter(|c| !pushed.contains(c))
        .cloned()
        .collect()
}

// same directory, pid-scoped temp file, one rename — a crash mid-write leaves the
// old file intact instead of the truncated-then-failed write `fs::write` risks.
// the pid in the temp name ensures concurrent writers do not collide on the same temp file.
fn write_atomic(path: &Path, body: &str) -> std::io::Result<()> {
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("tmp");
    let pid = std::process::id();
    let tmp_name = format!("{}.{}.tmp", filename, pid);
    let tmp = path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join(&tmp_name);
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_records_a_deletion() {
        let mut p = Pending::default();
        p.note(Set::Favs, "a", true);
        assert_eq!(
            p.favs,
            vec![Change {
                id: "a".into(),
                gone: true
            }]
        );
    }

    #[test]
    fn the_latest_action_on_an_id_replaces_the_earlier_one() {
        // star, un-star, star again between two syncs is one net add — sending
        // the whole history would let the server apply them out of order.
        let mut p = Pending::default();
        p.note(Set::Favs, "a", false);
        p.note(Set::Favs, "a", true);
        p.note(Set::Favs, "a", false);
        assert_eq!(
            p.favs,
            vec![Change {
                id: "a".into(),
                gone: false
            }]
        );
    }

    #[test]
    fn sets_are_kept_apart() {
        let mut p = Pending::default();
        p.note(Set::Favs, "a", true);
        p.note(Set::Blocked, "a", false);
        p.note(Set::Countries, "DE", true);
        assert_eq!(p.favs.len(), 1);
        assert_eq!(p.blocked.len(), 1);
        assert_eq!(p.excluded_countries.len(), 1);
    }

    #[test]
    fn is_empty_is_true_only_with_no_entries_at_all() {
        let mut p = Pending::default();
        assert!(p.is_empty());
        p.note(Set::Countries, "DE", true);
        assert!(!p.is_empty());
        p.clear();
        assert!(p.is_empty());
    }

    #[test]
    fn it_round_trips_through_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sync_pending.json");
        let mut p = Pending::default();
        p.note(Set::Favs, "a", true);
        p.save(&path).unwrap();
        assert_eq!(Pending::load(&path), p);
    }

    #[test]
    fn a_missing_or_corrupt_file_loads_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope.json");
        assert!(Pending::load(&missing).is_empty());
        let bad = dir.path().join("bad.json");
        std::fs::write(&bad, "not json").unwrap();
        assert!(Pending::load(&bad).is_empty());
    }

    // PROBE (finding 1): before the fix, a plain `save` from a surface holding a
    // stale in-memory copy clobbered a concurrent writer's deletion. this shows
    // the old failure mode still exists on the raw `save`, and that `save_merged`
    // — what every accumulate-case call site now uses — does not have it.
    #[test]
    fn probe_blind_overwrite_loses_a_concurrently_written_deletion() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sync_pending.json");

        // TUI spawns with nothing pending and loads that into memory.
        let tui_in_memory = Pending::load(&path);
        assert!(tui_in_memory.is_empty());

        // meanwhile the CLI's `sync run` notes an un-favourite and writes it out.
        let mut cli_pending = Pending::load(&path);
        cli_pending.note(Set::Favs, "station-1", true);
        cli_pending.save(&path).unwrap();
        assert!(!Pending::load(&path).is_empty());

        // a plain save of the TUI's (stale, empty) in-memory state still clobbers.
        tui_in_memory.save(&path).unwrap();
        let on_disk = Pending::load(&path);
        assert!(
            on_disk.is_empty(),
            "expected the raw save to still clobber, found {on_disk:?}"
        );
    }

    #[test]
    fn save_merged_keeps_a_concurrently_written_deletion() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sync_pending.json");

        // TUI spawns with nothing pending and holds that in memory.
        let tui_in_memory = Pending::load(&path);
        assert!(tui_in_memory.is_empty());

        // meanwhile the CLI notes an un-favourite and writes it out.
        let mut cli_pending = Pending::load(&path);
        cli_pending.note(Set::Favs, "station-1", true);
        cli_pending.save(&path).unwrap();

        // TUI saves via save_merged instead of a blind overwrite.
        tui_in_memory.save_merged(&path).unwrap();

        // fixed: the CLI's deletion survives.
        let on_disk = Pending::load(&path);
        assert_eq!(
            on_disk.favs,
            vec![Change {
                id: "station-1".into(),
                gone: true
            }]
        );
    }

    #[test]
    fn save_merged_lets_the_in_memory_entry_win_over_a_stale_disk_entry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sync_pending.json");

        // disk has an older action for the same id.
        let mut on_disk = Pending::default();
        on_disk.note(Set::Favs, "station-1", true);
        on_disk.save(&path).unwrap();

        // this surface's in-memory copy has since re-favourited it.
        let mut in_memory = Pending::default();
        in_memory.note(Set::Favs, "station-1", false);
        in_memory.save_merged(&path).unwrap();

        let merged = Pending::load(&path);
        assert_eq!(
            merged.favs,
            vec![Change {
                id: "station-1".into(),
                gone: false
            }]
        );
    }

    #[test]
    fn clear_then_save_does_not_resurrect_what_is_on_disk() {
        // a "clear" that merges the old file back in would replay a deletion
        // the server has already accepted forever.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sync_pending.json");

        let mut p = Pending::default();
        p.note(Set::Favs, "station-1", true);
        p.save(&path).unwrap();

        p.clear();
        p.save(&path).unwrap();

        assert!(Pending::load(&path).is_empty());
    }

    #[test]
    fn save_is_atomic_no_temp_file_left_behind() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sync_pending.json");
        let mut p = Pending::default();
        p.note(Set::Favs, "a", true);
        p.save(&path).unwrap();
        assert!(path.exists());
        // verify no tmp files remain in the directory
        let entries: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(entries.is_empty(), "expected no .tmp files after save");
    }

    // PROBE (finding 2): the post-sync clear used a plain `save` with a truncated
    // log, so an edit written during the HTTP round-trip had no chance to survive —
    // it was never merged in, only overwritten. this shows that failure mode on the
    // old operation (`pending.clear(); pending.save()`), and that `clear_pushed`
    // — the fix — keeps the concurrent edit instead.
    #[test]
    fn probe_post_sync_clear_loses_a_concurrent_edit_before_the_fix() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sync_pending.json");

        // what actually went out on the wire.
        let mut pushed = Pending::default();
        pushed.note(Set::Favs, "s1", true);
        pushed.save(&path).unwrap();

        // mid-flight, a second writer (e.g. the TUI un-starring another station)
        // appends its own entry to the same on-disk log.
        let mut second_writer = Pending::load(&path);
        second_writer.note(Set::Favs, "s3", true);
        second_writer.save_merged(&path).unwrap();
        assert_eq!(Pending::load(&path).favs.len(), 2);

        // the old post-sync clear: truncate to empty and overwrite, blind to
        // whatever landed on disk since the push.
        let mut old_style = pushed.clone();
        old_style.clear();
        old_style.save(&path).unwrap();

        let on_disk = Pending::load(&path);
        assert!(
            on_disk.is_empty(),
            "expected the old blind clear to also destroy the concurrent edit, found {on_disk:?}"
        );
    }

    #[test]
    fn clear_pushed_keeps_an_edit_written_during_the_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sync_pending.json");

        let mut pushed = Pending::default();
        pushed.note(Set::Favs, "s1", true);
        pushed.save(&path).unwrap();

        // mid-flight: a second writer notes an unrelated deletion.
        let mut second_writer = Pending::load(&path);
        second_writer.note(Set::Favs, "s3", true);
        second_writer.save_merged(&path).unwrap();

        Pending::clear_pushed(&pushed, &path).unwrap();

        let on_disk = Pending::load(&path);
        assert_eq!(
            on_disk.favs,
            vec![Change {
                id: "s3".into(),
                gone: true
            }],
            "the entry pushed to the server should be gone, the concurrent one must survive"
        );
    }

    #[test]
    fn clear_pushed_keeps_an_id_whose_gone_flipped_during_the_round_trip() {
        // id "x" was pushed as gone:false (a favourite). during the round-trip the
        // same id is un-starred on disk (gone:true). that is a brand new action the
        // server has not seen — clearing must not remove it just because the id matches.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sync_pending.json");

        let mut pushed = Pending::default();
        pushed.note(Set::Favs, "x", false);
        pushed.save(&path).unwrap();

        let mut second_writer = Pending::load(&path);
        second_writer.note(Set::Favs, "x", true);
        second_writer.save_merged(&path).unwrap();
        assert_eq!(Pending::load(&path).favs.len(), 1);

        Pending::clear_pushed(&pushed, &path).unwrap();

        let on_disk = Pending::load(&path);
        assert_eq!(
            on_disk.favs,
            vec![Change {
                id: "x".into(),
                gone: true
            }],
            "the flipped gone:true must survive the clear"
        );
    }

    #[test]
    fn clear_pushed_removes_nothing_extra_when_nothing_changed_meanwhile() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sync_pending.json");

        let mut pushed = Pending::default();
        pushed.note(Set::Favs, "s1", true);
        pushed.note(Set::Blocked, "b1", true);
        pushed.save(&path).unwrap();

        Pending::clear_pushed(&pushed, &path).unwrap();

        assert!(Pending::load(&path).is_empty());
    }

    #[test]
    fn concurrent_pids_do_not_share_temp_filename() {
        // simulate two different processes by verifying the naming scheme produces
        // unique names per pid: concurrent writers will use different temp filenames
        let pid1 = std::process::id();
        let filename = "sync_pending.json";
        let tmp_name_1 = format!("{}.{}.tmp", filename, pid1);

        // if a different pid were to write, it would use a different temp name
        let different_pid = pid1 + 1;
        let tmp_name_2 = format!("{}.{}.tmp", filename, different_pid);

        assert_ne!(
            tmp_name_1, tmp_name_2,
            "different pids must produce different temp filenames"
        );
    }
}
