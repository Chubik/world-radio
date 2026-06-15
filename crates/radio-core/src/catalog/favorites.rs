use std::collections::HashSet;

const HISTORY_CAP: usize = 50;

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

#[derive(Debug, Default)]
pub struct History {
    ids: Vec<String>,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, uuid: &str) {
        self.ids.retain(|id| id != uuid);
        self.ids.insert(0, uuid.to_string());
        self.ids.truncate(HISTORY_CAP);
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

impl History {
    pub fn load(path: &std::path::Path) -> Self {
        let Ok(bytes) = std::fs::read(path) else {
            return Self::new();
        };
        let ids: Vec<String> = serde_json::from_slice(&bytes).unwrap_or_default();
        Self { ids }
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
        h.record("u1");
        h.record("u2");
        h.record("u1");
        assert_eq!(h.ids(), &["u1".to_string(), "u2".to_string()]);
    }

    #[test]
    fn history_capped() {
        let mut h = History::new();
        for i in 0..(HISTORY_CAP + 10) {
            h.record(&format!("u{i}"));
        }
        assert_eq!(h.ids().len(), HISTORY_CAP);
        assert_eq!(h.ids()[0], format!("u{}", HISTORY_CAP + 9));
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
        h.record("u1");
        h.record("u2");
        h.save(&path).unwrap();
        let loaded = History::load(&path);
        assert_eq!(loaded.ids(), &["u2".to_string(), "u1".to_string()]);
    }
}
