use super::station::Station;

/// the one rule every surface picks by. `included` empty means unrestricted;
/// hidden countries and blocked stations always outrank it.
pub fn allowed_station(
    station: &Station,
    excluded_countries: &[String],
    blocked: &[String],
    included_countries: &[String],
) -> bool {
    allowed_row(
        &station.stationuuid,
        &station.countrycode,
        excluded_countries,
        blocked,
        included_countries,
    )
}

/// the same rule for a surface that carries its own row type rather than a
/// `Station` — macos picks from `StationPick`, and building a throwaway
/// `Station` per row would allocate over the whole catalogue on every shuffle.
pub fn allowed_row(
    uuid: &str,
    countrycode: &str,
    excluded_countries: &[String],
    blocked: &[String],
    included_countries: &[String],
) -> bool {
    let country = countrycode.to_uppercase();
    if excluded_countries
        .iter()
        .any(|c| c.to_uppercase() == country)
    {
        return false;
    }
    if blocked.iter().any(|b| b == uuid) {
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
    pub countrycodes: Vec<String>,
    pub language: Option<String>,
    pub tags: Vec<String>,
    pub codecs: Vec<String>,
    pub bitrate_min: Option<u32>,
    /// how the whole result is ordered. it belongs in the query rather than in
    /// the caller: a page is 200 rows out of ~58,000, so sorting after the cut
    /// would only order the arbitrary slice sqlite happened to return.
    pub sort: Sort,
}

/// the orders a station list can be read in. `Name` is the default because it
/// is the only one that is stable — two runs of the same search return the same
/// rows in the same places.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sort {
    #[default]
    Name,
    /// most-voted first: radio-browser's rough proxy for popularity.
    Popular,
    /// highest bitrate first, which is the closest thing to "best sounding".
    Bitrate,
    Country,
}

impl Sort {
    /// the ORDER BY body. every one ends in `name` so equal keys keep a stable
    /// order rather than shuffling between identical queries.
    pub fn clause(self) -> &'static str {
        match self {
            Sort::Name => "name",
            Sort::Popular => "votes DESC, name",
            Sort::Bitrate => "bitrate DESC, name",
            Sort::Country => "countrycode, name",
        }
    }

    pub fn from_wire(s: &str) -> Sort {
        match s {
            "popular" => Sort::Popular,
            "bitrate" => Sort::Bitrate,
            "country" => Sort::Country,
            _ => Sort::Name,
        }
    }
}

impl SearchQuery {
    /// the remote api takes one value per field, so the params carry the first
    /// of each list; callers that need the whole list query it country by
    /// country and merge (see `online_search_bounded`).
    pub fn to_params(&self) -> Vec<(&'static str, String)> {
        let mut p = Vec::new();
        if let Some(v) = &self.name {
            p.push(("name", v.clone()));
        }
        if let Some(v) = self.countrycodes.first() {
            p.push(("countrycode", v.clone()));
        }
        if let Some(v) = &self.language {
            p.push(("language", v.clone()));
        }
        if let Some(v) = self.tags.first() {
            p.push(("tag", v.clone()));
        }
        if let Some(v) = self.codecs.first() {
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

    // the row form is what macos picks through; if it ever stops agreeing with
    // the station form, the filter silently means two different things again.
    #[test]
    fn the_row_form_answers_exactly_as_the_station_form() {
        let excluded = vec!["DE".to_string()];
        let blocked = vec!["b".to_string()];
        let included = vec!["ua".to_string(), "US".to_string()];
        for (uuid, country) in [
            ("a", "UA"),
            ("b", "UA"),
            ("c", "PL"),
            ("d", "DE"),
            ("e", "us"),
            ("f", ""),
        ] {
            let s = st(uuid, country);
            assert_eq!(
                allowed_row(uuid, country, &excluded, &blocked, &included),
                allowed_station(&s, &excluded, &blocked, &included),
                "{uuid}/{country}"
            );
        }
        // and with no include set, where everything not excluded or blocked passes
        for (uuid, country) in [("a", "UA"), ("b", "UA"), ("d", "DE")] {
            let s = st(uuid, country);
            assert_eq!(
                allowed_row(uuid, country, &excluded, &blocked, &[]),
                allowed_station(&s, &excluded, &blocked, &[]),
                "{uuid}/{country}"
            );
        }
    }

    #[test]
    fn builds_params_for_set_fields_only() {
        let q = SearchQuery {
            name: Some("jazz".into()),
            countrycodes: vec!["FR".into()],
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
            tags: vec!["jazz".into()],
            codecs: vec!["MP3".into()],
            ..Default::default()
        };
        let p = q.to_params();
        assert!(p.contains(&("language", "french".to_string())));
        assert!(p.contains(&("tag", "jazz".to_string())));
        assert!(p.contains(&("codec", "MP3".to_string())));
        assert_eq!(p.len(), 3);
    }

    // the api takes a single value per field, so extra selections are dropped
    // here and re-queried by the caller instead.
    #[test]
    fn params_carry_the_first_of_each_list() {
        let q = SearchQuery {
            countrycodes: vec!["UA".into(), "US".into()],
            tags: vec!["jazz".into(), "rock".into()],
            ..Default::default()
        };
        let p = q.to_params();
        assert!(p.contains(&("countrycode", "UA".to_string())));
        assert!(p.contains(&("tag", "jazz".to_string())));
        assert_eq!(p.len(), 2);
    }
}
