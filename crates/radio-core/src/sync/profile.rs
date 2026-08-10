use serde::{Deserialize, Serialize};
use std::path::Path;

/// the local listening profile: filter countries, browse scope, and theme,
/// each stamped with the client time of the change so a sync round-trip can
/// take the newer side (last-write-wins) instead of blindly overwriting.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct Profile {
    #[serde(default)]
    pub countries: Vec<String>,
    #[serde(default)]
    pub countries_at: i64,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub scope_at: i64,
    #[serde(default)]
    pub theme: String,
    #[serde(default)]
    pub theme_at: i64,
}

impl Profile {
    pub fn load(path: &Path) -> Profile {
        let Ok(raw) = std::fs::read_to_string(path) else {
            return Profile::default();
        };
        match serde_json::from_str(&raw) {
            Ok(profile) => profile,
            Err(e) => {
                eprintln!(
                    "warning: profile at {} is corrupt, treating as default: {e}",
                    path.display()
                );
                Profile::default()
            }
        }
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let body = serde_json::to_string(self)?;
        std::fs::write(path, body)?;
        Ok(())
    }

    pub fn set_countries(&mut self, countries: Vec<String>, now: i64) {
        if self.countries == countries {
            return;
        }
        self.countries = countries;
        self.countries_at = now;
    }

    pub fn set_scope(&mut self, scope: &str, now: i64) {
        if self.scope == scope {
            return;
        }
        self.scope = scope.to_string();
        self.scope_at = now;
    }

    pub fn set_theme(&mut self, theme: &str, now: i64) {
        if self.theme == theme {
            return;
        }
        self.theme = theme.to_string();
        self.theme_at = now;
    }

    /// takes each field from `remote` when its stamp is newer than what's
    /// stored locally. returns true when anything changed, so the caller
    /// knows to re-render.
    pub fn apply_newer(
        &mut self,
        filter: Option<(Vec<String>, i64)>,
        scope: Option<(String, i64)>,
        theme: Option<(String, i64)>,
    ) -> bool {
        let mut changed = false;
        if let Some((countries, at)) = filter {
            if at > self.countries_at {
                self.countries = countries;
                self.countries_at = at;
                changed = true;
            }
        }
        if let Some((scope, at)) = scope {
            if at > self.scope_at {
                self.scope = scope;
                self.scope_at = at;
                changed = true;
            }
        }
        if let Some((theme, at)) = theme {
            if at > self.theme_at {
                self.theme = theme;
                self.theme_at = at;
                changed = true;
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_countries_stamps_the_change() {
        let mut p = Profile::default();
        p.set_countries(vec!["UA".into()], 100);
        assert_eq!(p.countries, vec!["UA".to_string()]);
        assert_eq!(p.countries_at, 100);
    }

    #[test]
    fn set_countries_is_a_no_op_on_equal_value() {
        let mut p = Profile::default();
        p.set_countries(vec!["UA".into()], 100);
        p.set_countries(vec!["UA".into()], 200);
        assert_eq!(
            p.countries_at, 100,
            "stamp must not move when value is unchanged"
        );
    }

    #[test]
    fn set_scope_stamps_the_change() {
        let mut p = Profile::default();
        p.set_scope("FAVS", 50);
        assert_eq!(p.scope, "FAVS");
        assert_eq!(p.scope_at, 50);
    }

    #[test]
    fn set_scope_is_a_no_op_on_equal_value() {
        let mut p = Profile::default();
        p.set_scope("FAVS", 50);
        p.set_scope("FAVS", 99);
        assert_eq!(p.scope_at, 50);
    }

    #[test]
    fn set_theme_stamps_the_change() {
        let mut p = Profile::default();
        p.set_theme("nord", 10);
        assert_eq!(p.theme, "nord");
        assert_eq!(p.theme_at, 10);
    }

    #[test]
    fn set_theme_is_a_no_op_on_equal_value() {
        let mut p = Profile::default();
        p.set_theme("nord", 10);
        p.set_theme("nord", 20);
        assert_eq!(p.theme_at, 10);
    }

    #[test]
    fn apply_newer_takes_a_newer_remote_filter() {
        let mut p = Profile::default();
        p.set_countries(vec!["UA".into()], 10);
        let changed = p.apply_newer(Some((vec!["PL".into()], 20)), None, None);
        assert!(changed);
        assert_eq!(p.countries, vec!["PL".to_string()]);
        assert_eq!(p.countries_at, 20);
    }

    #[test]
    fn apply_newer_rejects_an_older_remote_filter() {
        let mut p = Profile::default();
        p.set_countries(vec!["UA".into()], 20);
        let changed = p.apply_newer(Some((vec!["PL".into()], 10)), None, None);
        assert!(!changed);
        assert_eq!(p.countries, vec!["UA".to_string()]);
        assert_eq!(p.countries_at, 20);
    }

    #[test]
    fn apply_newer_takes_a_newer_remote_scope_and_theme() {
        let mut p = Profile::default();
        p.set_scope("ALL", 10);
        p.set_theme("amber-crt", 10);
        let changed = p.apply_newer(None, Some(("FAVS".into(), 20)), Some(("nord".into(), 20)));
        assert!(changed);
        assert_eq!(p.scope, "FAVS");
        assert_eq!(p.scope_at, 20);
        assert_eq!(p.theme, "nord");
        assert_eq!(p.theme_at, 20);
    }

    #[test]
    fn apply_newer_with_nothing_newer_reports_no_change() {
        let mut p = Profile::default();
        p.set_countries(vec!["UA".into()], 20);
        p.set_scope("ALL", 20);
        p.set_theme("amber-crt", 20);
        let changed = p.apply_newer(
            Some((vec!["PL".into()], 5)),
            Some(("FAVS".into(), 5)),
            Some(("nord".into(), 5)),
        );
        assert!(!changed);
    }

    #[test]
    fn apply_newer_with_all_none_is_a_no_op() {
        let mut p = Profile::default();
        p.set_countries(vec!["UA".into()], 20);
        assert!(!p.apply_newer(None, None, None));
    }

    #[test]
    fn load_of_a_missing_file_is_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profile.json");
        assert_eq!(Profile::load(&path), Profile::default());
    }

    #[test]
    fn load_of_a_corrupt_file_is_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profile.json");
        std::fs::write(&path, "not json").unwrap();
        assert_eq!(Profile::load(&path), Profile::default());
    }

    #[test]
    fn it_round_trips_through_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profile.json");
        let mut p = Profile::default();
        p.set_countries(vec!["UA".into(), "PL".into()], 100);
        p.set_scope("FAVS", 100);
        p.set_theme("nord", 100);
        p.save(&path).unwrap();
        assert_eq!(Profile::load(&path), p);
    }
}
