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

    pub fn load(path: &Path) -> Pending {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let body = serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string());
        std::fs::write(path, body)
    }
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
}
