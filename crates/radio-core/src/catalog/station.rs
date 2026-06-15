use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Station {
    pub stationuuid: String,
    pub name: String,
    #[serde(default)]
    pub url_resolved: String,
    #[serde(default)]
    pub countrycode: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub tags: String,
    #[serde(default)]
    pub codec: String,
    #[serde(default)]
    pub bitrate: u32,
    #[serde(default)]
    pub geo_lat: Option<f64>,
    #[serde(default)]
    pub geo_long: Option<f64>,
}

pub fn codec_is_unstable(codec: &str) -> bool {
    let c = codec.trim().to_ascii_lowercase();
    c.contains("aac+") || c.contains("aacp") || c.contains("he-aac") || c.contains("heaac")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unstable_detects_he_aac_variants() {
        assert!(codec_is_unstable("AAC+"));
        assert!(codec_is_unstable("aac+"));
        assert!(codec_is_unstable("AACP"));
        assert!(codec_is_unstable("HE-AAC"));
        assert!(codec_is_unstable(" he-aac "));
    }

    #[test]
    fn unstable_false_for_plain_codecs() {
        assert!(!codec_is_unstable("AAC"));
        assert!(!codec_is_unstable("MP3"));
        assert!(!codec_is_unstable("OGG"));
        assert!(!codec_is_unstable(""));
    }

    #[test]
    fn parses_radio_browser_json() {
        let json = r#"{
            "stationuuid":"abc-123",
            "name":"Jazz FM",
            "url_resolved":"http://stream.example/jazz",
            "countrycode":"FR",
            "language":"french",
            "tags":"jazz,smooth",
            "codec":"MP3",
            "bitrate":128,
            "geo_lat":48.85,
            "geo_long":2.35
        }"#;
        let s: Station = serde_json::from_str(json).unwrap();
        assert_eq!(s.stationuuid, "abc-123");
        assert_eq!(s.bitrate, 128);
        assert_eq!(s.geo_lat, Some(48.85));
    }

    #[test]
    fn parses_with_missing_optional_fields() {
        let json = r#"{"stationuuid":"x","name":"Bare"}"#;
        let s: Station = serde_json::from_str(json).unwrap();
        assert_eq!(s.name, "Bare");
        assert_eq!(s.bitrate, 0);
        assert_eq!(s.geo_lat, None);
    }
}
