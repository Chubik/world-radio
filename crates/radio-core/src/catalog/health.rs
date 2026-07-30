use std::collections::HashMap;

const HIDE_THRESHOLD: u32 = 1;

#[derive(Debug, Default)]
pub struct Health {
    fails: HashMap<String, u32>,
}

impl Health {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_failure(&mut self, uuid: &str) {
        *self.fails.entry(uuid.to_string()).or_insert(0) += 1;
    }

    pub fn record_success(&mut self, uuid: &str) {
        self.fails.remove(uuid);
    }

    pub fn is_hidden(&self, uuid: &str) -> bool {
        self.fails.get(uuid).copied().unwrap_or(0) >= HIDE_THRESHOLD
    }

    pub fn hidden_ids(&self) -> Vec<String> {
        self.fails
            .iter()
            .filter(|(_, &n)| n >= HIDE_THRESHOLD)
            .map(|(uuid, _)| uuid.clone())
            .collect()
    }

    pub fn clear(&mut self, uuid: &str) {
        self.fails.remove(uuid);
    }

    pub fn clear_all(&mut self) {
        self.fails.clear();
    }
}

impl Health {
    pub fn load(path: &std::path::Path) -> Self {
        let Ok(bytes) = std::fs::read(path) else {
            return Self::new();
        };
        let fails: HashMap<String, u32> = serde_json::from_slice(&bytes).unwrap_or_default();
        Self { fails }
    }

    pub fn save(&self, path: &std::path::Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(&self.fails)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_resets_failures() {
        let mut h = Health::new();
        h.record_failure("u1");
        h.record_success("u1");
        assert!(!h.is_hidden("u1"));
        h.record_failure("u1");
        assert!(h.is_hidden("u1"));
    }

    #[test]
    fn hidden_ids_lists_only_hidden() {
        let mut h = Health::new();
        for _ in 0..HIDE_THRESHOLD {
            h.record_failure("dead");
        }
        let ids = h.hidden_ids();
        assert_eq!(ids, vec!["dead".to_string()]);
    }

    #[test]
    fn clear_unhides_station() {
        let mut h = Health::new();
        for _ in 0..HIDE_THRESHOLD {
            h.record_failure("u1");
        }
        assert!(h.is_hidden("u1"));
        h.clear("u1");
        assert!(!h.is_hidden("u1"));
    }

    #[test]
    fn clear_all_unhides_everything() {
        let mut h = Health::new();
        for _ in 0..HIDE_THRESHOLD {
            h.record_failure("u1");
            h.record_failure("u2");
        }
        h.clear_all();
        assert!(h.hidden_ids().is_empty());
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("health.json");
        let mut h = Health::new();
        h.record_failure("u1");
        h.save(&path).unwrap();
        let loaded = Health::load(&path);
        assert!(loaded.is_hidden("u1"));
    }

    #[test]
    fn one_failure_hides() {
        let mut h = Health::new();
        h.record_failure("u1");
        assert!(h.is_hidden("u1"));
    }

    #[test]
    fn success_clears_a_failure() {
        let mut h = Health::new();
        h.record_failure("u1");
        h.record_success("u1");
        assert!(!h.is_hidden("u1"));
    }

    #[test]
    fn untouched_station_is_not_hidden() {
        let h = Health::new();
        assert!(!h.is_hidden("u1"));
    }
}
