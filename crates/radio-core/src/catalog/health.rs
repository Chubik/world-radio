use std::collections::HashMap;

const HIDE_THRESHOLD: u32 = 3;

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
    fn hides_after_threshold_failures() {
        let mut h = Health::new();
        for _ in 0..HIDE_THRESHOLD {
            h.record_failure("u1");
        }
        assert!(h.is_hidden("u1"));
    }

    #[test]
    fn below_threshold_not_hidden() {
        let mut h = Health::new();
        h.record_failure("u1");
        assert!(!h.is_hidden("u1"));
    }

    #[test]
    fn success_resets_failures() {
        let mut h = Health::new();
        h.record_failure("u1");
        h.record_failure("u1");
        h.record_success("u1");
        assert!(!h.is_hidden("u1"));
        h.record_failure("u1");
        assert!(!h.is_hidden("u1"));
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("health.json");
        let mut h = Health::new();
        h.record_failure("u1");
        h.record_failure("u1");
        h.save(&path).unwrap();
        let mut loaded = Health::load(&path);
        assert!(!loaded.is_hidden("u1"));
        loaded.record_failure("u1");
        assert!(loaded.is_hidden("u1"));
    }
}
