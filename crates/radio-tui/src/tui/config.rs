use crate::tui::keybind::Keymap;
use crate::tui::model::SpectrumStyle;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
/// machine-local settings only. `theme` and `[filters]` are deliberately absent:
/// `profile.json` owns them from this build on, and an old file that still
/// carries them is read by `radio_core::sync::legacy_settings` — the one reader
/// every surface shares — never from here.
pub struct Config {
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
        assert!(!cfg.no_emoji);
        assert!(cfg.crossfade);
    }

    // an old file still carries `theme` and `[filters]`. this struct must parse
    // straight past them rather than erroring out, or a machine due for the
    // migration would fall back to defaults for its real settings too.
    #[test]
    fn an_old_config_still_parses_its_machine_local_keys() {
        let cfg = Config::from_toml_str(
            "theme = \"cyber-neon\"\nno_emoji = true\n[filters]\nstatus = \"all\"\n",
        )
        .unwrap();
        assert!(cfg.no_emoji);
    }

    #[test]
    fn missing_file_yields_defaults() {
        let cfg = Config::load(std::path::Path::new("/no/such/config.toml"));
        assert!(!cfg.no_emoji);
        assert!(cfg.crossfade);
    }

    #[test]
    fn malformed_toml_falls_back_to_defaults() {
        assert!(Config::from_toml_str("not = [valid").is_err());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "not = [valid").unwrap();
        assert!(Config::load(&path).crossfade);
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
