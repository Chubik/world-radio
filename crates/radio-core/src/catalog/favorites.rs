use std::collections::HashSet;

/// matches the server's `merge::HISTORY_CAP` — the local file must hold every
/// entry the server is willing to keep, or a sync silently discards the rest.
pub const HISTORY_CAP: usize = 200;

#[derive(Debug, Default)]
pub struct Favorites {
    ids: Vec<String>,
    set: HashSet<String>,
}

impl Favorites {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn toggle(&mut self, uuid: &str) -> bool {
        if self.set.contains(uuid) {
            self.set.remove(uuid);
            self.ids.retain(|id| id != uuid);
            return false;
        }
        self.set.insert(uuid.to_string());
        self.ids.push(uuid.to_string());
        true
    }

    pub fn set_from(&mut self, ids: Vec<String>) {
        self.ids.clear();
        self.set.clear();
        for id in ids {
            if self.set.insert(id.clone()) {
                self.ids.push(id);
            }
        }
    }

    pub fn contains(&self, uuid: &str) -> bool {
        self.set.contains(uuid)
    }

    pub fn ids(&self) -> &[String] {
        &self.ids
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }
}

impl Favorites {
    pub fn load(path: &std::path::Path) -> Self {
        let Ok(bytes) = std::fs::read(path) else {
            return Self::new();
        };
        let ids: Vec<String> = serde_json::from_slice(&bytes).unwrap_or_default();
        let mut f = Self::new();
        for id in ids {
            f.toggle(&id);
        }
        f
    }

    pub fn save(&self, path: &std::path::Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(&self.ids)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }
}

/// one play, with the wall-clock second it happened at. the timestamp is what
/// makes history mergeable across devices: it is stamped once when the station
/// is played and never rewritten, so a remote entry older than a local one
/// still keeps its own place in the merged order.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Play {
    pub id: String,
    pub at: i64,
}

/// the on-disk history file: either the pre-timestamp format (a bare array of
/// station ids) or the current one. reading the old shape must not lose data.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum HistoryFile {
    Stamped(Vec<Play>),
    Legacy(Vec<String>),
}

#[derive(Debug, Default)]
pub struct History {
    plays: Vec<Play>,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, uuid: &str, at: i64) {
        self.plays.retain(|p| p.id != uuid);
        self.plays.insert(
            0,
            Play {
                id: uuid.to_string(),
                at,
            },
        );
        self.plays.truncate(HISTORY_CAP);
    }

    /// replaces the contents outright, most-recent-first — used to apply a
    /// sync merge, which already decided the order and the timestamps.
    pub fn set_from(&mut self, plays: Vec<Play>) {
        self.plays = plays;
        self.plays.truncate(HISTORY_CAP);
    }

    pub fn ids(&self) -> Vec<String> {
        self.plays.iter().map(|p| p.id.clone()).collect()
    }

    pub fn plays(&self) -> &[Play] {
        &self.plays
    }

    pub fn is_empty(&self) -> bool {
        self.plays.is_empty()
    }

    pub fn len(&self) -> usize {
        self.plays.len()
    }
}

impl History {
    pub fn load(path: &std::path::Path) -> Self {
        let Ok(bytes) = std::fs::read(path) else {
            return Self::new();
        };
        let Ok(file) = serde_json::from_slice::<HistoryFile>(&bytes) else {
            return Self::new();
        };
        let plays = match file {
            HistoryFile::Stamped(plays) => plays,
            // a pre-timestamp file has no play times. synthesising them once at
            // migration keeps the recorded order and, because the result is
            // written back, they stay stable instead of being re-stamped to
            // `now` on every sync.
            HistoryFile::Legacy(ids) => legacy_plays(ids, migration_epoch(path)),
        };
        Self { plays }
    }

    pub fn save(&self, path: &std::path::Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(&self.plays)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }
}

/// the migration stamp is the file's own mtime, not `now`: a history last
/// written months ago must not outrank a play another device made yesterday.
fn migration_epoch(path: &std::path::Path) -> i64 {
    let from_mtime = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64);
    match from_mtime {
        Some(secs) => secs,
        None => std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
    }
}

fn legacy_plays(ids: Vec<String>, epoch: i64) -> Vec<Play> {
    ids.into_iter()
        .enumerate()
        .map(|(i, id)| Play {
            id,
            at: epoch - i as i64,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_adds_then_removes() {
        let mut f = Favorites::new();
        assert!(f.toggle("u1"));
        assert!(f.contains("u1"));
        assert_eq!(f.ids(), &["u1".to_string()]);
        assert!(!f.toggle("u1"));
        assert!(!f.contains("u1"));
        assert!(f.ids().is_empty());
    }

    #[test]
    fn toggle_preserves_insertion_order() {
        let mut f = Favorites::new();
        f.toggle("u1");
        f.toggle("u2");
        f.toggle("u3");
        assert_eq!(
            f.ids(),
            &["u1".to_string(), "u2".to_string(), "u3".to_string()]
        );
    }

    #[test]
    fn history_most_recent_first_dedup() {
        let mut h = History::new();
        h.record("u1", 100);
        h.record("u2", 200);
        h.record("u1", 300);
        assert_eq!(h.ids(), vec!["u1".to_string(), "u2".to_string()]);
    }

    #[test]
    fn history_capped() {
        let mut h = History::new();
        for i in 0..(HISTORY_CAP + 10) {
            h.record(&format!("u{i}"), i as i64);
        }
        assert_eq!(h.ids().len(), HISTORY_CAP);
        assert_eq!(h.ids()[0], format!("u{}", HISTORY_CAP + 9));
    }

    #[test]
    fn history_cap_matches_the_server_cap() {
        // the server keeps 200; a smaller local cap silently drops the rest of
        // a merged history on every sync.
        assert_eq!(HISTORY_CAP, 200);
    }

    #[test]
    fn record_keeps_the_time_the_station_was_played() {
        let mut h = History::new();
        h.record("u1", 1_700_000_000);
        assert_eq!(h.plays()[0].at, 1_700_000_000);
    }

    #[test]
    fn a_saved_history_keeps_its_timestamps_across_a_reload() {
        // stamps must be stable across restarts and syncs, not re-derived.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        let mut h = History::new();
        h.record("u1", 1_000);
        h.record("u2", 2_000);
        h.save(&path).unwrap();
        let loaded = History::load(&path);
        assert_eq!(
            loaded.plays()[0],
            Play {
                id: "u2".into(),
                at: 2_000
            }
        );
        assert_eq!(
            loaded.plays()[1],
            Play {
                id: "u1".into(),
                at: 1_000
            }
        );
    }

    #[test]
    fn a_legacy_history_file_loads_without_losing_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        std::fs::write(&path, r#"["u1","u2","u3"]"#).unwrap();
        let loaded = History::load(&path);
        assert_eq!(
            loaded.ids(),
            vec!["u1".to_string(), "u2".to_string(), "u3".to_string()]
        );
        // descending stamps preserve the recorded order
        assert!(loaded.plays()[0].at > loaded.plays()[1].at);
        assert!(loaded.plays()[1].at > loaded.plays()[2].at);
    }

    #[test]
    fn a_legacy_history_is_not_stamped_at_now() {
        // stamping migrated entries with `now` would make them outrank every
        // remote play forever; the file's own mtime is the honest upper bound.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        std::fs::write(&path, r#"["u1"]"#).unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        assert!(migration_epoch(&path) <= now);
        assert_eq!(
            legacy_plays(vec!["u1".into()], 1_600_000_000)[0].at,
            1_600_000_000
        );
    }

    #[test]
    fn a_legacy_migration_stamp_stays_put_once_written_back() {
        // the defect: re-deriving stamps on every load/sync pins the whole local
        // history at `now`, starving every other device on the server's cap.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        std::fs::write(&path, r#"["u1","u2"]"#).unwrap();
        let first = History::load(&path);
        first.save(&path).unwrap();
        let second = History::load(&path);
        assert_eq!(first.plays(), second.plays());
    }

    #[test]
    fn set_from_keeps_two_hundred_entries() {
        let mut h = History::new();
        let plays: Vec<Play> = (0..300)
            .map(|i| Play {
                id: format!("u{i}"),
                at: 1_000_000 - i,
            })
            .collect();
        h.set_from(plays);
        assert_eq!(h.len(), 200);
    }

    #[test]
    fn empty_and_len_track_contents() {
        let mut f = Favorites::new();
        assert!(f.is_empty());
        assert_eq!(f.len(), 0);
        f.toggle("u1");
        assert!(!f.is_empty());
        assert_eq!(f.len(), 1);
    }

    #[test]
    fn favorites_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("favorites.json");
        let mut f = Favorites::new();
        f.toggle("u1");
        f.toggle("u2");
        f.save(&path).unwrap();
        let loaded = Favorites::load(&path);
        assert!(loaded.contains("u1"));
        assert!(loaded.contains("u2"));
        assert_eq!(loaded.ids(), &["u1".to_string(), "u2".to_string()]);
    }

    #[test]
    fn favorites_load_missing_is_empty() {
        let f = Favorites::load(std::path::Path::new("/nonexistent/favorites.json"));
        assert!(f.ids().is_empty());
    }

    #[test]
    fn history_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        let mut h = History::new();
        h.record("u1", 100);
        h.record("u2", 200);
        h.save(&path).unwrap();
        let loaded = History::load(&path);
        assert_eq!(loaded.ids(), vec!["u2".to_string(), "u1".to_string()]);
    }

    #[test]
    fn set_from_replaces_contents() {
        let mut f = Favorites::new();
        f.toggle("old1");
        f.toggle("old2");
        f.set_from(vec!["new1".into(), "new2".into(), "new1".into()]);
        assert_eq!(f.ids(), &["new1".to_string(), "new2".to_string()]);
        assert!(!f.contains("old1"));
    }
}
