use crate::state::StationPick;
use radio_core::catalog::Catalog;

fn to_pick(s: &radio_core::catalog::Station) -> StationPick {
    StationPick {
        uuid: s.stationuuid.clone(),
        name: s.name.clone(),
        url: s.url_resolved.clone(),
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

pub fn favorite_stations(catalog: &Catalog) -> anyhow::Result<Vec<StationPick>> {
    let mut out = Vec::new();
    for uuid in catalog.favorite_ids() {
        if let Some(s) = catalog.station_by_uuid(uuid)? {
            out.push(to_pick(&s));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use radio_core::catalog::{Cache, Catalog, Health, Station};

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
        }
    }

    fn catalog() -> Catalog {
        let cache = Cache::open_in_memory().unwrap();
        let cat = Catalog::new(cache, Health::new());
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
    fn toggle_and_reload_reflects_change() {
        let mut cat = catalog();
        let favs = toggle_and_reload(&mut cat, "u1").unwrap();
        assert_eq!(favs.len(), 1);
        assert_eq!(favs[0].uuid, "u1");
        let favs = toggle_and_reload(&mut cat, "u1").unwrap();
        assert!(favs.is_empty());
    }
}
