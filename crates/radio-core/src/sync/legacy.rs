use super::{Profile, Scope};
use serde::Deserialize;
use std::path::Path;

/// what an upgrade can still rescue out of a pre-profile `config.toml`. every
/// field is optional because "absent" and "set to the default" must stay
/// distinguishable: adoption may only stamp what the user actually chose.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LegacySettings {
    pub countries: Vec<String>,
    pub scope: Option<String>,
    pub theme: Option<String>,
}

impl LegacySettings {
    pub fn is_empty(&self) -> bool {
        self.countries.is_empty() && self.scope.is_none() && self.theme.is_none()
    }
}

#[derive(Deserialize, Default)]
struct LegacyFilters {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    countries: Vec<String>,
}

#[derive(Deserialize, Default)]
struct LegacyConfig {
    #[serde(default)]
    theme: Option<String>,
    #[serde(default)]
    filters: Option<LegacyFilters>,
}

/// reads the two migration carriers out of a `config.toml` without knowing
/// anything else about it. this lives in the core, not in the tui, because
/// every surface that syncs must adopt before it pushes — a surface that
/// cannot read the legacy keys would publish an empty profile and let another
/// device's choice overwrite settings this machine has held all along.
pub fn legacy_settings(path: &Path) -> LegacySettings {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return LegacySettings::default();
    };
    legacy_settings_from_toml(&raw)
}

pub fn legacy_settings_from_toml(raw: &str) -> LegacySettings {
    let Ok(cfg) = toml::from_str::<LegacyConfig>(raw) else {
        return LegacySettings::default();
    };
    let filters = cfg.filters.as_ref();
    LegacySettings {
        countries: filters.map(|f| f.countries.clone()).unwrap_or_default(),
        // a `[filters]` table with no `status` still meant "all" to the build
        // that wrote it, so the scope is claimed whenever the table exists.
        scope: filters.map(|f| legacy_scope(f.status.as_deref())),
        theme: cfg.theme.clone(),
    }
}

fn legacy_scope(status: Option<&str>) -> String {
    let raw = status.unwrap_or("all");
    Scope::from_wire(raw)
        .unwrap_or_default()
        .as_wire()
        .to_string()
}

/// runs this device's legacy migration and records that it ran.
///
/// the marker is set whether or not anything was actually taken up: an
/// already-stamped field is a field this device has nothing left to say about,
/// and a fresh install has nothing to rescue at all. what matters is that the
/// migration *reached* this disk, which is precisely what the old
/// `adopted == false` shortcut could not tell apart from "a sync stamped
/// everything from another device before we ever looked".
///
/// reports whether the result is still unwritten — the legacy values live
/// nowhere else, so a failed save must leave `config.toml` exactly as it is.
pub fn adopt_legacy(profile: &mut Profile, legacy: &LegacySettings, profile_path: &Path) -> bool {
    adopt_legacy_at(profile, legacy, profile_path, crate::sync::now_secs())
}

pub fn adopt_legacy_at(
    profile: &mut Profile,
    legacy: &LegacySettings,
    profile_path: &Path,
    now: i64,
) -> bool {
    let adopted = profile.adopt_existing(
        &legacy.countries,
        legacy.scope.as_deref().unwrap_or(""),
        legacy.theme.as_deref().unwrap_or(""),
        now,
    );
    if profile.migrated && !adopted {
        return false;
    }
    profile.migrated = true;
    profile.save(profile_path).is_err()
}

/// whether the legacy keys may be dropped from `config.toml`.
///
/// the defect this replaces inferred "migrated" from "adopted nothing". that is
/// also true when a sync stamped the profile from *another* device before this
/// one ever adopted — adoption then correctly refuses to outrank it and returns
/// `false`, and the config was rewritten without `[filters]` or `theme`, so this
/// machine's own filter and theme were gone from both files with no way to
/// re-run the migration.
///
/// so the question is not "did the migration run", nor even "is the field
/// stamped" — a sync stamps it with *another device's* value — but "does the
/// profile now carry the very value the config was holding". a field the config
/// never carried needs nothing; a field it did carry keeps every legacy key
/// alive until that value is somewhere else, because until then the config is
/// its only copy.
pub fn migration_settled(profile: &Profile, legacy: &LegacySettings) -> bool {
    let countries_settled = legacy.countries.is_empty() || profile.countries == legacy.countries;
    let scope_settled = legacy
        .scope
        .as_ref()
        .is_none_or(|scope| &profile.scope == scope);
    let theme_settled = legacy
        .theme
        .as_ref()
        .is_none_or(|theme| &profile.theme == theme);
    countries_settled && scope_settled && theme_settled
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD_CONFIG: &str =
        "theme = \"monokai\"\n[filters]\nstatus = \"all\"\ncountries = [\"UA\"]\n";

    #[test]
    fn an_old_config_hands_its_settings_over() {
        let legacy = legacy_settings_from_toml(OLD_CONFIG);
        assert_eq!(legacy.countries, vec!["UA".to_string()]);
        assert_eq!(legacy.scope.as_deref(), Some("all"));
        assert_eq!(legacy.theme.as_deref(), Some("monokai"));
    }

    #[test]
    fn a_config_without_filters_claims_no_scope() {
        let legacy = legacy_settings_from_toml("theme = \"nord\"\n");
        assert!(legacy.countries.is_empty());
        assert_eq!(legacy.scope, None);
        assert_eq!(legacy.theme.as_deref(), Some("nord"));
    }

    #[test]
    fn every_legacy_scope_word_survives_the_read() {
        for wire in ["all", "favorites", "recent", "blocked", "dead"] {
            let raw = format!("[filters]\nstatus = \"{wire}\"\n");
            assert_eq!(
                legacy_settings_from_toml(&raw).scope.as_deref(),
                Some(wire),
                "{wire}"
            );
        }
    }

    #[test]
    fn a_filters_table_without_a_status_still_means_all() {
        let legacy = legacy_settings_from_toml("[filters]\ncountries = [\"UA\"]\n");
        assert_eq!(legacy.scope.as_deref(), Some("all"));
    }

    #[test]
    fn an_unreadable_or_broken_config_rescues_nothing() {
        assert!(legacy_settings_from_toml("not = [valid").is_empty());
        assert!(legacy_settings(Path::new("/no/such/config.toml")).is_empty());
    }

    #[test]
    fn it_reads_a_real_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, OLD_CONFIG).unwrap();
        assert_eq!(
            legacy_settings(&path).countries,
            vec!["UA".to_string()],
            "the file was not read"
        );
    }

    #[test]
    fn a_written_adoption_reports_no_pending_migration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profile.json");
        let mut profile = Profile::default();
        let legacy = legacy_settings_from_toml(OLD_CONFIG);

        assert!(!adopt_legacy_at(&mut profile, &legacy, &path, 500));
        assert_eq!(Profile::load(&path).countries, vec!["UA".to_string()]);
        assert!(migration_settled(&profile, &legacy));
    }

    #[test]
    fn an_unwritable_profile_leaves_the_migration_pending() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, "not a directory").unwrap();
        let mut profile = Profile::default();

        assert!(adopt_legacy_at(
            &mut profile,
            &legacy_settings_from_toml(OLD_CONFIG),
            &blocker.join("profile.json"),
            500,
        ));
    }

    // the fix. a sync landed another device's filter and theme before this
    // device ever adopted, so adoption takes nothing — but the legacy keys are
    // still the only copy of *this* machine's filter, and dropping them loses
    // it for good.
    #[test]
    fn a_profile_stamped_by_another_device_does_not_settle_this_ones_migration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profile.json");
        // a sync stamped the theme from another device before this one ever
        // adopted. the filter this machine has held all along is stamped by
        // nothing, so `config.toml` is still its only copy.
        let mut profile = Profile::default();
        profile.set_theme("nord", 900);
        let legacy = LegacySettings {
            countries: vec!["UA".into()],
            scope: None,
            theme: Some("monokai".into()),
        };

        // the migration ran, and it correctly refused to outrank the other
        // device's theme — the pre-fix code read that refusal as "migrated" and
        // let the rewrite drop the filter with it.
        adopt_legacy_at(&mut profile, &legacy, &path, 500);
        assert_eq!(
            profile.theme, "nord",
            "another device must not be outranked"
        );
        assert_eq!(
            profile.countries,
            vec!["UA".to_string()],
            "the unstamped filter is exactly what adoption is for"
        );

        // and a filter the profile still does not carry keeps the keys alive.
        let mut stranded = Profile::default();
        stranded.set_theme("nord", 900);
        assert!(
            !migration_settled(&stranded, &legacy),
            "the config would drop keys the profile does not carry"
        );
    }

    // once the profile carries what the config was holding, the config has
    // stopped being anyone's only copy and the keys go.
    #[test]
    fn a_profile_carrying_the_rescued_values_settles_the_migration() {
        let mut profile = Profile::default();
        profile.set_countries(vec!["UA".into()], 900);
        profile.set_scope("all", 900);
        profile.set_theme("monokai", 900);
        assert!(migration_settled(
            &profile,
            &legacy_settings_from_toml(OLD_CONFIG)
        ));
    }

    // a stamp is not enough on its own: a sync can stamp every field with
    // another device's values, and the config is then still the only place this
    // machine's own settings exist.
    #[test]
    fn another_devices_values_do_not_settle_the_migration() {
        let mut profile = Profile::default();
        profile.set_countries(vec!["PL".into()], 900);
        profile.set_scope("favorites", 900);
        profile.set_theme("nord", 900);
        assert!(!migration_settled(
            &profile,
            &legacy_settings_from_toml(OLD_CONFIG)
        ));
    }

    // a config with nothing to rescue must never block the rewrite, or a fresh
    // install would keep re-checking a migration it never had.
    #[test]
    fn a_config_with_nothing_to_rescue_is_always_settled() {
        assert!(migration_settled(
            &Profile::default(),
            &LegacySettings::default()
        ));
    }

    // each field settles on its own: a theme-only legacy config is settled by
    // its theme alone, with no filter or scope to wait for.
    #[test]
    fn a_field_the_config_never_carried_holds_nothing_back() {
        let mut profile = Profile::default();
        profile.set_theme("monokai", 900);
        let legacy = legacy_settings_from_toml("theme = \"monokai\"\n");
        assert!(migration_settled(&profile, &legacy));
    }
}
