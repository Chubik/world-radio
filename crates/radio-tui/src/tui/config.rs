use crate::tui::keybind::Keymap;
use crate::tui::model::{BrowseFilters, SpectrumStyle};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    #[serde(default = "default_theme")]
    pub theme: String,
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
    #[serde(default)]
    pub filters: BrowseFilters,
}

fn default_true() -> bool {
    true
}

fn default_theme() -> String {
    "amber-crt".to_string()
}

fn default_divisor() -> f32 {
    12.0
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            no_emoji: false,
            last_station: None,
            query: String::new(),
            fft_divisor: default_divisor(),
            crossfade: true,
            spectrum_style: SpectrumStyle::default(),
            keybindings: Keymap::default(),
            filters: BrowseFilters::default(),
        }
    }
}

impl Config {
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
        assert_eq!(cfg.theme, "amber-crt");
        assert!(!cfg.no_emoji);
    }

    #[test]
    fn parse_reads_theme_and_no_emoji() {
        let cfg = Config::from_toml_str("theme = \"cyber-neon\"\nno_emoji = true\n").unwrap();
        assert_eq!(cfg.theme, "cyber-neon");
        assert!(cfg.no_emoji);
    }

    #[test]
    fn missing_file_yields_defaults() {
        let cfg = Config::load(std::path::Path::new("/no/such/config.toml"));
        assert_eq!(cfg.theme, "amber-crt");
    }

    #[test]
    fn malformed_toml_falls_back_to_defaults() {
        assert!(Config::from_toml_str("not = [valid").is_err());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "not = [valid").unwrap();
        let cfg = Config::load(&path);
        assert_eq!(cfg.theme, "amber-crt");
    }

    #[test]
    fn config_roundtrips_theme_and_no_emoji() {
        let cfg = Config {
            theme: "cyber-neon".into(),
            no_emoji: true,
            last_station: Some("uuid-123".into()),
            ..Default::default()
        };
        let s = cfg.to_toml_string();
        let back = Config::from_toml_str(&s).unwrap();
        assert_eq!(back.theme, "cyber-neon");
        assert!(back.no_emoji);
        assert_eq!(back.last_station.as_deref(), Some("uuid-123"));
    }

    #[test]
    fn config_roundtrips_query_and_filters() {
        use crate::tui::model::{BrowseFilters, SpectrumStyle, StatusFilter};
        let cfg = Config {
            query: "80".into(),
            fft_divisor: 4.0,
            crossfade: false,
            spectrum_style: SpectrumStyle::Wave,
            filters: BrowseFilters {
                status: StatusFilter::Favorites,
                tags: vec!["jazz".into(), "80s".into()],
                hide_unplayable: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let s = cfg.to_toml_string();
        let back = Config::from_toml_str(&s).unwrap();
        assert_eq!(back.query, "80");
        assert_eq!(back.fft_divisor, 4.0);
        assert!(!back.crossfade);
        assert_eq!(back.spectrum_style, SpectrumStyle::Wave);
        assert_eq!(back.filters.status, StatusFilter::Favorites);
        assert_eq!(
            back.filters.tags,
            vec!["jazz".to_string(), "80s".to_string()]
        );
        assert!(back.filters.hide_unplayable);
    }
}
