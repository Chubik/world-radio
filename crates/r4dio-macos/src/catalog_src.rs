use crate::state::StationPick;
use radio_core::catalog::Catalog;

fn to_pick(s: &radio_core::catalog::Station) -> StationPick {
    StationPick {
        uuid: s.stationuuid.clone(),
        name: s.name.clone(),
        url: s.url_resolved.clone(),
        country: s.countrycode.clone(),
        codec: s.codec.clone(),
        bitrate: s.bitrate,
    }
}

pub fn all_stations(catalog: &Catalog) -> anyhow::Result<Vec<StationPick>> {
    let stations = catalog.search_offline("")?;
    Ok(stations.iter().map(to_pick).collect())
}

pub fn last_played(catalog: &Catalog) -> anyhow::Result<Option<StationPick>> {
    let Some(uuid) = catalog.history_ids().first() else {
        return Ok(None);
    };
    let station = catalog.station_by_uuid(uuid)?;
    Ok(station.as_ref().map(to_pick))
}

pub fn toggle_and_reload(catalog: &mut Catalog, uuid: &str) -> anyhow::Result<Vec<StationPick>> {
    catalog.toggle_favorite(uuid);
    favorite_stations(catalog)
}

/// the window's remove button, which must only ever remove: toggling a uuid that
/// is not a favourite would add it, turning ★ into an add button on a stale row.
pub fn unfavorite_and_reload(
    catalog: &mut Catalog,
    uuid: &str,
) -> anyhow::Result<Vec<StationPick>> {
    if catalog.is_favorite(uuid) {
        catalog.toggle_favorite(uuid);
    }
    favorite_stations(catalog)
}

pub fn station_pick(catalog: &Catalog, uuid: &str) -> anyhow::Result<Option<StationPick>> {
    Ok(catalog.station_by_uuid(uuid)?.as_ref().map(to_pick))
}

pub fn favorite_stations(catalog: &Catalog) -> anyhow::Result<Vec<StationPick>> {
    let mut out = Vec::new();
    for uuid in catalog.favorite_ids() {
        if let Some(s) = catalog.station_by_uuid(uuid)? {
            out.push(to_pick(&s));
        }
    }
    Ok(out)
}

/// a blocked uuid the catalogue can no longer resolve still has to appear: it is
/// the only row the user can press to unblock it, and dropping it would strand
/// the entry in their account with no way to reach it.
pub fn blocked_stations(catalog: &Catalog) -> anyhow::Result<Vec<StationPick>> {
    let mut out = Vec::new();
    for uuid in catalog.blacklist_ids() {
        match catalog.station_by_uuid(uuid)? {
            Some(s) => out.push(to_pick(&s)),
            None => out.push(StationPick {
                uuid: uuid.clone(),
                name: String::new(),
                url: String::new(),
                country: String::new(),
                codec: String::new(),
                bitrate: 0,
            }),
        }
    }
    Ok(out)
}

/// the window's unblock button, which must only ever unblock: toggling a uuid
/// that is not blocked would block it, so a stale row would silently ban a
/// station instead of freeing one.
pub fn unblock(catalog: &mut Catalog, uuid: &str) {
    if catalog.is_blacklisted(uuid) {
        catalog.toggle_blacklist(uuid);
    }
}

/// RU and BY are hard-filtered from the catalogue itself, so they cannot reach a
/// user-facing filter list. this repeats the check at the boundary because the
/// rule must not depend on a cache that a future migration could refill.
fn is_hidden_country(code: &str) -> bool {
    const HIDDEN: &[&str] = &["RU", "BY"];
    HIDDEN.iter().any(|h| code.eq_ignore_ascii_case(h))
}

/// the window never draws an RU/BY row, so a list built from what it drew cannot
/// mention them. carrying any already-stored entry through keeps a wholesale
/// replace from logging a deletion the user never asked for and did not see.
pub fn merge_hidden_exclusions(catalog: &Catalog, mut codes: Vec<String>) -> Vec<String> {
    codes.retain(|c| !is_hidden_country(c));
    for stored in catalog.excluded_country_ids() {
        if is_hidden_country(stored) {
            codes.push(stored.clone());
        }
    }
    codes
}

pub struct CountryFacet {
    pub code: String,
    pub count: u32,
    pub excluded: bool,
}

const TAGS_UNUSED: usize = 1;

pub fn country_facets(catalog: &Catalog) -> anyhow::Result<Vec<CountryFacet>> {
    let excluded: Vec<String> = catalog
        .excluded_country_ids()
        .iter()
        .map(|c| c.to_uppercase())
        .collect();
    // the argument bounds tags, which this list does not use; countries always
    // come back whole, so a small number here costs nothing.
    let facets = catalog.facets(TAGS_UNUSED)?;
    Ok(facets
        .countries
        .into_iter()
        .filter(|(code, _)| !code.trim().is_empty() && !is_hidden_country(code))
        .map(|(code, count)| {
            let up = code.to_uppercase();
            CountryFacet {
                excluded: excluded.contains(&up),
                code: up,
                count,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use radio_core::catalog::{Cache, Catalog, Health, Station};

    fn station_in(uuid: &str, country: &str) -> Station {
        Station {
            countrycode: country.into(),
            ..station(uuid, "http://x")
        }
    }

    fn station(uuid: &str, url: &str) -> Station {
        Station {
            stationuuid: uuid.into(),
            name: uuid.into(),
            url_resolved: url.into(),
            countrycode: String::new(),
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

    fn catalog() -> Catalog {
        let cache = Cache::open_in_memory().unwrap();
        let mut cat = Catalog::new(cache, Health::new());
        cat.ingest(&[station("u1", "http://one"), station("u2", "http://two")])
            .unwrap();
        cat
    }

    #[test]
    fn all_stations_lists_cached() {
        let cat = catalog();
        let picks = all_stations(&cat).unwrap();
        assert_eq!(picks.len(), 2);
        assert!(picks.iter().all(|p| !p.url.is_empty()));
    }

    #[test]
    fn favorite_stations_resolves_marked() {
        let mut cat = catalog();
        cat.toggle_favorite("u2");
        let picks = favorite_stations(&cat).unwrap();
        assert_eq!(picks.len(), 1);
        assert_eq!(picks[0].uuid, "u2");
    }

    #[test]
    fn unfavorite_removes_a_favourite() {
        let mut cat = catalog();
        cat.toggle_favorite("u1");
        let favs = unfavorite_and_reload(&mut cat, "u1").unwrap();
        assert!(favs.is_empty());
    }

    #[test]
    fn unfavorite_never_adds_one() {
        // the window's ★ is a remove button; a stale row must not re-add the
        // station a toggle would have flipped back on.
        let mut cat = catalog();
        let favs = unfavorite_and_reload(&mut cat, "u1").unwrap();
        assert!(favs.is_empty());
        assert!(!cat.is_favorite("u1"));
    }

    #[test]
    fn station_pick_resolves_by_uuid_and_misses_cleanly() {
        let cat = catalog();
        let pick = station_pick(&cat, "u1").unwrap().unwrap();
        assert_eq!(pick.uuid, "u1");
        assert_eq!(pick.url, "http://one");
        assert!(station_pick(&cat, "nope").unwrap().is_none());
    }

    #[test]
    fn blocked_stations_resolves_marked() {
        let mut cat = catalog();
        cat.toggle_blacklist("u2");
        let rows = blocked_stations(&cat).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].uuid, "u2");
        assert_eq!(rows[0].name, "u2");
    }

    #[test]
    fn blocked_stations_keeps_a_uuid_the_catalogue_dropped() {
        // otherwise the only row that can unblock it disappears, and the entry is
        // stranded in the account forever.
        let mut cat = catalog();
        cat.toggle_blacklist("gone");
        let rows = blocked_stations(&cat).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].uuid, "gone");
        assert!(rows[0].name.is_empty());
    }

    #[test]
    fn unblock_removes_a_block() {
        let mut cat = catalog();
        cat.toggle_blacklist("u1");
        unblock(&mut cat, "u1");
        assert!(!cat.is_blacklisted("u1"));
        assert!(blocked_stations(&cat).unwrap().is_empty());
    }

    #[test]
    fn unblock_never_adds_one() {
        // a stale row must not turn the unblock button into a block button.
        let mut cat = catalog();
        unblock(&mut cat, "u1");
        assert!(!cat.is_blacklisted("u1"));
    }

    #[test]
    fn country_facets_marks_the_excluded_ones() {
        let cache = Cache::open_in_memory().unwrap();
        let mut cat = Catalog::new(cache, Health::new());
        cat.ingest(&[station_in("a", "UA"), station_in("b", "PL")])
            .unwrap();
        cat.set_excluded_countries(vec!["pl".into()]);

        let rows = country_facets(&cat).unwrap();
        let pl = rows.iter().find(|r| r.code == "PL").unwrap();
        let ua = rows.iter().find(|r| r.code == "UA").unwrap();
        // a lowercase stored code must still match the uppercase facet
        assert!(pl.excluded);
        assert!(!ua.excluded);
        assert_eq!(ua.count, 1);
    }

    #[test]
    fn the_hidden_countries_are_ru_and_by_in_any_case() {
        // the boundary filter itself, pinned directly: the ingest-level tests
        // below cannot fail it, because the cache already drops those rows before
        // facets() ever sees them. this is what stops the list regrowing a row if
        // that ever stops being true.
        assert!(is_hidden_country("RU"));
        assert!(is_hidden_country("BY"));
        assert!(is_hidden_country("ru"));
        assert!(is_hidden_country("by"));
        assert!(!is_hidden_country("UA"));
        assert!(!is_hidden_country("RO"));
        assert!(!is_hidden_country("BE"));
    }

    #[test]
    fn country_facets_never_lists_ru_or_by() {
        // the binding rule: RU/BY are hard-filtered everywhere and must never
        // surface as a row the user could see, ticked or unticked.
        let cache = Cache::open_in_memory().unwrap();
        let mut cat = Catalog::new(cache, Health::new());
        cat.ingest(&[
            station_in("a", "UA"),
            station_in("b", "RU"),
            station_in("c", "BY"),
            station_in("d", "ru"),
            station_in("e", "by"),
        ])
        .unwrap();

        let rows = country_facets(&cat).unwrap();
        assert!(rows.iter().any(|r| r.code == "UA"));
        assert!(!rows
            .iter()
            .any(|r| r.code.eq_ignore_ascii_case("RU") || r.code.eq_ignore_ascii_case("BY")));
    }

    #[test]
    fn country_facets_never_lists_ru_or_by_even_when_excluded() {
        // an excluded-countries file carried over from another device could name
        // them; that must not conjure a row.
        let cache = Cache::open_in_memory().unwrap();
        let mut cat = Catalog::new(cache, Health::new());
        cat.ingest(&[station_in("a", "UA")]).unwrap();
        cat.set_excluded_countries(vec!["RU".into(), "BY".into()]);

        let rows = country_facets(&cat).unwrap();
        assert!(!rows
            .iter()
            .any(|r| r.code.eq_ignore_ascii_case("RU") || r.code.eq_ignore_ascii_case("BY")));
    }

    #[test]
    fn saving_the_filter_keeps_a_stored_ru_entry_the_window_never_showed() {
        // the window cannot list RU/BY, so a save built from its rows would drop
        // them — a deletion the user never made, pushed to every other device.
        let cache = Cache::open_in_memory().unwrap();
        let mut cat = Catalog::new(cache, Health::new());
        cat.set_excluded_countries(vec!["RU".into(), "PL".into()]);

        let merged = merge_hidden_exclusions(&cat, vec!["PL".into(), "UA".into()]);
        assert!(merged.contains(&"RU".to_string()));
        assert!(merged.contains(&"PL".to_string()));
        assert!(merged.contains(&"UA".to_string()));
    }

    #[test]
    fn saving_the_filter_refuses_an_ru_code_the_window_should_never_send() {
        let cache = Cache::open_in_memory().unwrap();
        let cat = Catalog::new(cache, Health::new());
        let merged = merge_hidden_exclusions(&cat, vec!["ru".into(), "UA".into()]);
        assert_eq!(merged, vec!["UA".to_string()]);
    }

    #[test]
    fn country_facets_skips_stations_with_no_country() {
        // radio-browser leaves the field empty often; a blank row is not a country
        let cache = Cache::open_in_memory().unwrap();
        let mut cat = Catalog::new(cache, Health::new());
        cat.ingest(&[station_in("a", "UA"), station("b", "http://b")])
            .unwrap();
        let rows = country_facets(&cat).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].code, "UA");
    }

    #[test]
    fn toggle_and_reload_reflects_change() {
        let mut cat = catalog();
        let favs = toggle_and_reload(&mut cat, "u1").unwrap();
        assert_eq!(favs.len(), 1);
        assert_eq!(favs[0].uuid, "u1");
        let favs = toggle_and_reload(&mut cat, "u1").unwrap();
        assert!(favs.is_empty());
    }
}
