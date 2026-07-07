mod client;
pub mod key;

pub use client::{SyncClient, SyncData};
pub use key::is_valid_format;

use std::path::{Path, PathBuf};

pub fn key_path() -> PathBuf {
    crate::paths::data_dir().join("sync_key")
}

pub fn load_key() -> Option<String> {
    load_key_at(&key_path())
}

pub fn store_key(key: &str) -> anyhow::Result<()> {
    crate::paths::ensure_data_dir()?;
    store_key_at(&key_path(), key)
}

pub fn clear_key() -> anyhow::Result<()> {
    clear_key_at(&key_path())
}

fn load_key_at(path: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(path).ok()?;
    let trimmed = raw.trim();
    match trimmed.is_empty() {
        true => None,
        false => Some(trimmed.to_string()),
    }
}

fn store_key_at(path: &Path, key: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, key)?;
    Ok(())
}

fn clear_key_at(path: &Path) -> anyhow::Result<()> {
    match path.exists() {
        true => {
            std::fs::remove_file(path)?;
            Ok(())
        }
        false => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_then_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sync_key");
        store_key_at(&p, "r4-xyz").unwrap();
        assert_eq!(load_key_at(&p), Some("r4-xyz".to_string()));
    }

    #[test]
    fn load_absent_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("nope");
        assert_eq!(load_key_at(&p), None);
    }

    #[test]
    fn clear_removes_key() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sync_key");
        store_key_at(&p, "r4-xyz").unwrap();
        clear_key_at(&p).unwrap();
        assert_eq!(load_key_at(&p), None);
    }
}
