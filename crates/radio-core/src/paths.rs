use std::path::{Path, PathBuf};

const MIGRATE_FILES: &[&str] = &[
    "stations.db",
    "favorites.json",
    "blacklist.json",
    "history.json",
    "station_health.json",
    "config.toml",
];

pub fn data_dir() -> PathBuf {
    match directories::ProjectDirs::from("net", "vchub", "r4dio") {
        Some(dirs) => dirs.data_dir().to_path_buf(),
        None => legacy_data_dir().unwrap_or_else(|| PathBuf::from("data")),
    }
}

pub fn legacy_data_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.join("data"))
}

pub fn ensure_data_dir() -> std::io::Result<PathBuf> {
    let d = data_dir();
    std::fs::create_dir_all(&d)?;
    if let Some(legacy) = legacy_data_dir() {
        migrate_into(&legacy, &d)?;
    }
    Ok(d)
}

pub fn migrate_legacy() -> std::io::Result<()> {
    let d = data_dir();
    std::fs::create_dir_all(&d)?;
    if let Some(legacy) = legacy_data_dir() {
        migrate_into(&legacy, &d)?;
    }
    Ok(())
}

fn migrate_into(legacy: &Path, dest: &Path) -> std::io::Result<()> {
    // skip if dest already populated or legacy is absent / same dir
    if dest.join("stations.db").exists() {
        return Ok(());
    }
    if !legacy.exists() || legacy == dest {
        return Ok(());
    }
    let mut moved_any = false;
    for name in MIGRATE_FILES {
        let src = legacy.join(name);
        if src.exists() {
            let dst = dest.join(name);
            match std::fs::rename(&src, &dst) {
                Ok(_) => moved_any = true,
                Err(_) => {
                    std::fs::copy(&src, &dst)?;
                    let _ = std::fs::remove_file(&src);
                    moved_any = true;
                }
            }
        }
    }
    // only delete the legacy dir if we actually took ownership of its data
    if moved_any {
        let _ = std::fs::remove_dir_all(legacy);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_moves_files_and_removes_legacy() {
        let tmp = tempfile::tempdir().unwrap();
        let legacy = tmp.path().join("legacy");
        let dest = tmp.path().join("dest");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(legacy.join("favorites.json"), b"[\"a\"]").unwrap();
        std::fs::write(legacy.join("stations.db"), b"db").unwrap();

        migrate_into(&legacy, &dest).unwrap();

        assert!(dest.join("favorites.json").exists());
        assert!(dest.join("stations.db").exists());
        assert_eq!(
            std::fs::read(dest.join("favorites.json")).unwrap(),
            b"[\"a\"]"
        );
        assert!(!legacy.exists(), "legacy dir should be removed");
    }

    #[test]
    fn migrate_noop_when_dest_already_has_db() {
        let tmp = tempfile::tempdir().unwrap();
        let legacy = tmp.path().join("legacy");
        let dest = tmp.path().join("dest");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(legacy.join("stations.db"), b"OLD").unwrap();
        std::fs::write(dest.join("stations.db"), b"NEW").unwrap();

        migrate_into(&legacy, &dest).unwrap();

        assert_eq!(std::fs::read(dest.join("stations.db")).unwrap(), b"NEW");
        assert!(
            legacy.exists(),
            "legacy left intact when dest already populated"
        );
    }
}
