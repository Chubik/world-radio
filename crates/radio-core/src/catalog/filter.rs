use super::station::Station;

/// the one rule every surface picks by. `included` empty means unrestricted;
/// hidden countries and blocked stations always outrank it.
pub fn allowed_station(
    station: &Station,
    excluded_countries: &[String],
    blocked: &[String],
    included_countries: &[String],
) -> bool {
    let country = station.countrycode.to_uppercase();
    if excluded_countries
        .iter()
        .any(|c| c.to_uppercase() == country)
    {
        return false;
    }
    if blocked.iter().any(|b| b == &station.stationuuid) {
        return false;
    }
    match included_countries.is_empty() {
        true => true,
        false => included_countries
            .iter()
            .any(|c| c.to_uppercase() == country),
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SearchQuery {
    pub name: Option<String>,
    pub countrycode: Option<String>,
    pub countrycodes: Vec<String>,
    pub language: Option<String>,
    pub tag: Option<String>,
    pub codec: Option<String>,
    pub bitrate_min: Option<u32>,
}

impl SearchQuery {
    pub fn to_params(&self) -> Vec<(&'static str, String)> {
        let mut p = Vec::new();
        if let Some(v) = &self.name {
            p.push(("name", v.clone()));
        }
        if let Some(v) = &self.countrycode {
            p.push(("countrycode", v.clone()));
        }
        if let Some(v) = &self.language {
            p.push(("language", v.clone()));
        }
        if let Some(v) = &self.tag {
            p.push(("tag", v.clone()));
        }
        if let Some(v) = &self.codec {
            p.push(("codec", v.clone()));
        }
        if let Some(v) = &self.bitrate_min {
            p.push(("bitrateMin", v.to_string()));
        }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // `Station` does not derive Default, so every field is named here.
    fn st(uuid: &str, country: &str) -> Station {
        Station {
            stationuuid: uuid.into(),
            name: "n".into(),
            url_resolved: "u".into(),
            countrycode: country.into(),
            language: String::new(),
            tags: String::new(),
            codec: String::new(),
            bitrate: 0,
            votes: 0,
            geo_lat: None,
            geo_long: None,
            lastcheckok: 1,
            lastchecktime_iso8601: String::new(),
        }
    }

    #[test]
    fn an_empty_include_set_allows_every_country() {
        assert!(allowed_station(&st("a", "PL"), &[], &[], &[]));
    }

    #[test]
    fn an_include_set_admits_only_its_countries() {
        let inc = vec!["UA".to_string(), "US".to_string()];
        assert!(allowed_station(&st("a", "UA"), &[], &[], &inc));
        assert!(allowed_station(&st("b", "US"), &[], &[], &inc));
        assert!(!allowed_station(&st("c", "PL"), &[], &[], &inc));
    }

    #[test]
    fn country_matching_ignores_case() {
        let inc = vec!["ua".to_string()];
        assert!(allowed_station(&st("a", "UA"), &[], &[], &inc));
    }

    // a blocked station stays blocked even inside the filter, and a hidden
    // country stays hidden even when the filter names it.
    #[test]
    fn blocked_and_excluded_outrank_the_include_set() {
        let inc = vec!["UA".to_string()];
        let blocked = vec!["a".to_string()];
        assert!(!allowed_station(&st("a", "UA"), &[], &blocked, &inc));
        let excluded = vec!["UA".to_string()];
        assert!(!allowed_station(&st("b", "UA"), &excluded, &[], &inc));
    }

    #[test]
    fn builds_params_for_set_fields_only() {
        let q = SearchQuery {
            name: Some("jazz".into()),
            countrycode: Some("FR".into()),
            bitrate_min: Some(128),
            ..Default::default()
        };
        let p = q.to_params();
        assert!(p.contains(&("name", "jazz".to_string())));
        assert!(p.contains(&("countrycode", "FR".to_string())));
        assert!(p.contains(&("bitrateMin", "128".to_string())));
        assert_eq!(p.len(), 3);
    }

    #[test]
    fn empty_query_produces_no_params() {
        assert!(SearchQuery::default().to_params().is_empty());
    }

    #[test]
    fn builds_params_for_language_tag_codec() {
        let q = SearchQuery {
            language: Some("french".into()),
            tag: Some("jazz".into()),
            codec: Some("MP3".into()),
            ..Default::default()
        };
        let p = q.to_params();
        assert!(p.contains(&("language", "french".to_string())));
        assert!(p.contains(&("tag", "jazz".to_string())));
        assert!(p.contains(&("codec", "MP3".to_string())));
        assert_eq!(p.len(), 3);
    }
}
