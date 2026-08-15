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
    /// display preferences that are not worth a field each — the equalizer's
    /// style and gain today. it travels as one json object under one stamp, so
    /// a new key needs no server change; the cost is that two devices changing
    /// different keys at once resolve to whichever wrote last.
    #[serde(default)]
    pub settings: serde_json::Value,
    #[serde(default)]
    pub settings_at: i64,
    /// set once **this device** has run the legacy migration, so a launch that
    /// has nothing left to adopt stops rewriting this file every time.
    ///
    /// it does not decide whether `config.toml` may drop its legacy keys —
    /// `migration_settled` answers that by comparing values, not stamps, because
    /// a sync stamps a field with *another* device's value and what protects the
    /// user is this machine's own value having landed somewhere. it never
    /// travels on the wire: it is a fact about this disk, not about the account.
    #[serde(default)]
    pub migrated: bool,
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

    /// merges one key into the settings bag. it is a merge rather than a
    /// replace because the bag is shared: writing the whole object would drop
    /// a key this build does not know about but another client set.
    pub fn set_setting(&mut self, key: &str, value: serde_json::Value, now: i64) {
        if !self.settings.is_object() {
            self.settings = serde_json::json!({});
        }
        let map = self.settings.as_object_mut().expect("just made an object");
        if map.get(key) == Some(&value) {
            return;
        }
        map.insert(key.to_string(), value);
        self.settings_at = now;
    }

    pub fn setting(&self, key: &str) -> Option<&serde_json::Value> {
        self.settings.as_object()?.get(key)
    }

    pub fn set_theme(&mut self, theme: &str, now: i64) {
        if self.theme == theme {
            return;
        }
        self.theme = theme.to_string();
        self.theme_at = now;
    }

    /// stamps settings this device already had before it ever wrote a profile,
    /// so an upgrade publishes them instead of looking like a device that has
    /// chosen nothing. a field whose stamp is already set was stamped by a real
    /// user change here or arrived from another device — adoption must never
    /// overwrite it. reports whether anything was taken up.
    pub fn adopt_existing(
        &mut self,
        countries: &[String],
        scope: &str,
        theme: &str,
        now: i64,
    ) -> bool {
        let mut adopted = false;
        if self.countries_at == 0 && !countries.is_empty() {
            self.countries = countries.to_vec();
            self.countries_at = now;
            adopted = true;
        }
        if self.scope_at == 0 && !scope.is_empty() {
            self.scope = scope.to_string();
            self.scope_at = now;
            adopted = true;
        }
        if self.theme_at == 0 && !theme.is_empty() {
            self.theme = theme.to_string();
            self.theme_at = now;
            adopted = true;
        }
        adopted
    }

    /// takes each field from `remote` when its stamp is newer than what's
    /// stored locally. reports per field, so the caller only pushes into the
    /// ui what actually moved — telling it "scope changed" when only the theme
    /// did would reset the user's current browse scope.
    pub fn apply_newer(
        &mut self,
        filter: Option<(Vec<String>, i64)>,
        scope: Option<(String, i64)>,
        theme: Option<(String, i64)>,
        settings: Option<(serde_json::Value, i64)>,
    ) -> ProfileChange {
        let mut changed = ProfileChange::default();
        if let Some((countries, at)) = filter {
            if at > self.countries_at {
                self.countries = countries;
                self.countries_at = at;
                changed.countries = true;
            }
        }
        if let Some((scope, at)) = scope {
            if at > self.scope_at {
                self.scope = scope;
                self.scope_at = at;
                changed.scope = true;
            }
        }
        if let Some((theme, at)) = theme {
            if at > self.theme_at {
                self.theme = theme;
                self.theme_at = at;
                changed.theme = true;
            }
        }
        if let Some((settings, at)) = settings {
            if at > self.settings_at {
                self.settings = settings;
                self.settings_at = at;
                changed.settings = true;
            }
        }
        changed
    }
}

/// which profile fields a sync actually moved.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ProfileChange {
    pub countries: bool,
    pub scope: bool,
    pub theme: bool,
    pub settings: bool,
}

impl ProfileChange {
    pub fn any(&self) -> bool {
        self.countries || self.scope || self.theme || self.settings
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
    fn set_setting_merges_rather_than_replaces() {
        // the bag is shared with clients this build knows nothing about, so
        // writing one key must leave the others exactly where they were.
        let mut p = Profile::default();
        p.settings = serde_json::json!({"theme_variant": "high-contrast"});
        p.set_setting("eq_style", serde_json::json!("wave"), 10);

        assert_eq!(p.setting("eq_style").unwrap(), "wave");
        assert_eq!(p.setting("theme_variant").unwrap(), "high-contrast");
        assert_eq!(p.settings_at, 10);
    }

    #[test]
    fn setting_the_same_value_does_not_move_the_stamp() {
        // an unchanged write must not win a merge against another device's real
        // change — that is how a no-op reverts someone else's edit.
        let mut p = Profile::default();
        p.set_setting("eq_style", serde_json::json!("bars"), 10);
        p.set_setting("eq_style", serde_json::json!("bars"), 50);
        assert_eq!(p.settings_at, 10);
    }

    #[test]
    fn a_newer_remote_settings_bag_wins() {
        let mut p = Profile::default();
        p.set_setting("eq_style", serde_json::json!("bars"), 10);

        let changed = p.apply_newer(
            None,
            None,
            None,
            Some((serde_json::json!({"eq_style": "dots"}), 20)),
        );

        assert!(changed.settings);
        assert_eq!(p.setting("eq_style").unwrap(), "dots");
    }

    #[test]
    fn an_older_remote_settings_bag_is_ignored() {
        let mut p = Profile::default();
        p.set_setting("eq_style", serde_json::json!("mirror"), 30);

        let changed = p.apply_newer(
            None,
            None,
            None,
            Some((serde_json::json!({"eq_style": "bars"}), 10)),
        );

        assert!(!changed.settings);
        assert_eq!(p.setting("eq_style").unwrap(), "mirror");
    }

    #[test]
    fn apply_newer_takes_a_newer_remote_filter() {
        let mut p = Profile::default();
        p.set_countries(vec!["UA".into()], 10);
        let changed = p.apply_newer(Some((vec!["PL".into()], 20)), None, None, None);
        assert!(changed.any());
        assert!(changed.countries);
        assert_eq!(p.countries, vec!["PL".to_string()]);
        assert_eq!(p.countries_at, 20);
    }

    #[test]
    fn apply_newer_rejects_an_older_remote_filter() {
        let mut p = Profile::default();
        p.set_countries(vec!["UA".into()], 20);
        let changed = p.apply_newer(Some((vec!["PL".into()], 10)), None, None, None);
        assert!(!changed.any());
        assert_eq!(p.countries, vec!["UA".to_string()]);
        assert_eq!(p.countries_at, 20);
    }

    #[test]
    fn apply_newer_takes_a_newer_remote_scope_and_theme() {
        let mut p = Profile::default();
        p.set_scope("ALL", 10);
        p.set_theme("amber-crt", 10);
        let changed = p.apply_newer(None, Some(("FAVS".into(), 20)), Some(("nord".into(), 20)), None);
        assert!(changed.scope);
        assert!(changed.theme);
        assert!(!changed.countries);
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
            None,
        );
        assert!(!changed.any());
    }

    #[test]
    fn apply_newer_with_all_none_is_a_no_op() {
        let mut p = Profile::default();
        p.set_countries(vec!["UA".into()], 20);
        assert!(!p.apply_newer(None, None, None, None).any());
    }

    #[test]
    fn apply_newer_reports_only_the_field_that_moved() {
        // a newer remote theme must not be reported as a scope change: the
        // caller would reset the user's current browse scope from it.
        let mut p = Profile::default();
        p.set_countries(vec!["UA".into()], 100);
        p.set_scope("FAVS", 100);
        p.set_theme("amber-crt", 100);
        let changed = p.apply_newer(
            Some((vec!["UA".into()], 50)),
            Some(("FAVS".into(), 50)),
            Some(("nord".into(), 200)),
            None,
        );
        assert!(changed.theme);
        assert!(!changed.scope);
        assert!(!changed.countries);
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

    // a device that had settings before this feature existed has never stamped
    // them, so they would never be published and the account would look empty to
    // every other device.
    #[test]
    fn settings_that_predate_the_profile_get_adopted() {
        let mut p = Profile::default();
        let adopted = p.adopt_existing(&["UA".to_string()], "favorites", "monokai", 500);
        assert!(adopted);
        assert_eq!(p.countries, vec!["UA".to_string()]);
        assert_eq!(p.countries_at, 500);
        assert_eq!(p.scope, "favorites");
        assert_eq!(p.theme, "monokai");
    }

    // adoption is a one-off rescue of unstamped settings, never a writer that
    // outranks a value another device stamped deliberately.
    #[test]
    fn adoption_never_overrides_an_already_stamped_field() {
        let mut p = Profile::default();
        p.set_countries(vec!["PL".to_string()], 100);
        let adopted = p.adopt_existing(&["UA".to_string()], "all", "nord", 500);
        assert!(
            adopted,
            "the unstamped theme and scope still needed adopting"
        );
        assert_eq!(
            p.countries,
            vec!["PL".to_string()],
            "stamped value was lost"
        );
        assert_eq!(p.countries_at, 100, "stamp moved");
    }

    #[test]
    fn adopting_nothing_reports_no_change() {
        let mut p = Profile::default();
        assert!(!p.adopt_existing(&[], "", "", 500));
    }
}
