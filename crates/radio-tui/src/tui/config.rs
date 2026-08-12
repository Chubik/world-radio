use crate::tui::keybind::Keymap;
use crate::tui::model::{BrowseFilters, SpectrumStyle};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    // read-only migration carriers: an old config still holds these, a new one
    // never writes them. profile.json owns both from this build on.
    #[serde(default, skip_serializing, rename = "theme")]
    pub legacy_theme: Option<String>,
    #[serde(default, skip_serializing, rename = "filters")]
    pub legacy_filters: Option<BrowseFilters>,
    #[serde(default)]
    pub no_emoji: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_station: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub query: String,
    #[serde(default = "default_divisor")]
    pub fft_divisor: f32,
    #[serde(default = "default_true")]
    pub crossfade: bool,
    #[serde(default)]
    pub spectrum_style: SpectrumStyle,
    #[serde(default)]
    pub keybindings: Keymap,
}

fn default_true() -> bool {
    true
}

fn default_divisor() -> f32 {
    12.0
}

impl Default for Config {
    fn default() -> Self {
        Self {
            legacy_theme: None,
            legacy_filters: None,
            no_emoji: false,
            last_station: None,
            query: String::new(),
            fft_divisor: default_divisor(),
            crossfade: true,
            spectrum_style: SpectrumStyle::default(),
            keybindings: Keymap::default(),
        }
    }
}

impl Config {
    /// what this config can still hand to an unstamped profile, once. a value
    /// the file never carried stays `None` so adoption cannot stamp a default
    /// over a choice another device made.
    pub fn legacy_settings(&self) -> LegacySettings {
        let filters = self.legacy_filters.as_ref();
        LegacySettings {
            countries: filters.map(|f| f.countries.clone()).unwrap_or_default(),
            scope: filters
                .map(|f| crate::tui::update::status_filter_to_scope(f.status).to_string()),
            theme: self.legacy_theme.clone(),
        }
    }

    pub fn from_toml_str(s: &str) -> anyhow::Result<Config> {
        let cfg: Config = toml::from_str(s)?;
        Ok(cfg)
    }

    pub fn to_toml_string(&self) -> String {
        toml::to_string(self).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) {
        if let Err(e) = std::fs::write(path, self.to_toml_string()) {
            crate::log_warn!("warning: failed to save config.toml: {e}");
        }
    }

    pub fn load(path: &Path) -> Config {
        match std::fs::read_to_string(path) {
            Err(_) => Config::default(),
            Ok(s) => match Config::from_toml_str(&s) {
                Ok(cfg) => cfg,
                Err(e) => {
                    crate::log_warn!("warning: config.toml is invalid ({e}), using defaults");
                    Config::default()
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_defaults_when_missing_fields() {
        let cfg = Config::from_toml_str("").unwrap();
        assert_eq!(cfg.legacy_settings(), LegacySettings::default());
        assert!(!cfg.no_emoji);
    }

    #[test]
    fn parse_reads_no_emoji() {
        let cfg = Config::from_toml_str("theme = \"cyber-neon\"\nno_emoji = true\n").unwrap();
        assert_eq!(cfg.legacy_theme.as_deref(), Some("cyber-neon"));
        assert!(cfg.no_emoji);
    }

    #[test]
    fn missing_file_yields_defaults() {
        let cfg = Config::load(std::path::Path::new("/no/such/config.toml"));
        assert_eq!(cfg.legacy_settings(), LegacySettings::default());
        assert!(cfg.crossfade);
    }

    #[test]
    fn malformed_toml_falls_back_to_defaults() {
        assert!(Config::from_toml_str("not = [valid").is_err());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "not = [valid").unwrap();
        let cfg = Config::load(&path);
        assert_eq!(cfg.legacy_settings(), LegacySettings::default());
    }

    #[test]
    fn config_roundtrips_no_emoji_and_last_station() {
        let cfg = Config {
            no_emoji: true,
            last_station: Some("uuid-123".into()),
            ..Default::default()
        };
        let s = cfg.to_toml_string();
        let back = Config::from_toml_str(&s).unwrap();
        assert!(back.no_emoji);
        assert_eq!(back.last_station.as_deref(), Some("uuid-123"));
    }

    #[test]
    fn config_roundtrips_query_and_view_settings() {
        use crate::tui::model::SpectrumStyle;
        let cfg = Config {
            query: "80".into(),
            fft_divisor: 4.0,
            crossfade: false,
            spectrum_style: SpectrumStyle::Wave,
            ..Default::default()
        };
        let s = cfg.to_toml_string();
        let back = Config::from_toml_str(&s).unwrap();
        assert_eq!(back.query, "80");
        assert_eq!(back.fft_divisor, 4.0);
        assert!(!back.crossfade);
        assert_eq!(back.spectrum_style, SpectrumStyle::Wave);
    }

    // an old config still carrying filters and a theme must hand them over
    // exactly once, and must not resurrect them afterwards.
    #[test]
    fn an_old_config_hands_its_settings_to_the_profile() {
        let raw = r#"
theme = "monokai"
[filters]
status = "all"
countries = ["UA"]
"#;
        let cfg: Config = toml::from_str(raw).unwrap();
        let legacy = cfg.legacy_settings();
        assert_eq!(legacy.countries, vec!["UA".to_string()]);
        assert_eq!(legacy.theme.as_deref(), Some("monokai"));
    }

    #[test]
    fn an_old_config_hands_over_its_scope_too() {
        let raw = r#"
[filters]
status = "favorites"
"#;
        let cfg: Config = toml::from_str(raw).unwrap();
        assert_eq!(cfg.legacy_settings().scope.as_deref(), Some("favorites"));
    }

    // a config that never had a [filters] table must not claim a scope, or
    // adoption would stamp "all" over a scope another device chose.
    #[test]
    fn a_config_without_filters_claims_no_scope() {
        let cfg: Config = toml::from_str("theme = \"nord\"\n").unwrap();
        let legacy = cfg.legacy_settings();
        assert!(legacy.countries.is_empty());
        assert_eq!(legacy.scope, None);
        assert_eq!(legacy.theme.as_deref(), Some("nord"));
    }

    #[test]
    fn a_new_config_writes_no_filters_or_theme() {
        let cfg = Config::default();
        let out = toml::to_string(&cfg).unwrap();
        assert!(
            !out.contains("[filters]"),
            "filters must not be written: {out}"
        );
        assert!(!out.contains("theme"), "theme must not be written: {out}");
    }

    // the migration is one-way: once the values are read back out of an old
    // file they must not be written again, or the next launch re-adopts them.
    #[test]
    fn a_loaded_old_config_writes_neither_back() {
        let raw = "theme = \"monokai\"\n[filters]\nstatus = \"favorites\"\ncountries = [\"UA\"]\n";
        let cfg = Config::from_toml_str(raw).unwrap();
        let out = cfg.to_toml_string();
        assert!(!out.contains("[filters]"), "filters came back: {out}");
        assert!(!out.contains("monokai"), "theme came back: {out}");
    }
}
