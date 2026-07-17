use crate::catalog::favorites::{Favorites, History};
use crate::catalog::{Cache, Facets, Health, SearchQuery, Station};

pub struct Catalog {
    cache: Cache,
    health: Health,
    favorites: Favorites,
    history: History,
    blacklist: Favorites,
    excluded_countries: Favorites,
}

impl Catalog {
    pub fn new(cache: Cache, health: Health) -> Self {
        Self {
            cache,
            health,
            favorites: Favorites::new(),
            history: History::new(),
            blacklist: Favorites::new(),
            excluded_countries: Favorites::new(),
        }
    }

    pub fn ingest(&self, stations: &[Station]) -> anyhow::Result<()> {
        self.cache.upsert(stations)
    }

    pub fn list_by_popularity(
        &self,
        favourites: &[String],
        limit: usize,
    ) -> anyhow::Result<Vec<Station>> {
        self.cache
            .list_by_popularity(favourites, limit, self.excluded_country_ids())
    }

    pub fn excluded_country_ids(&self) -> &[String] {
        self.excluded_countries.ids()
    }

    pub fn set_excluded_countries(&mut self, codes: Vec<String>) {
        let mut f = Favorites::new();
        for code in codes {
            let up = code.to_uppercase();
            if !f.contains(&up) {
                f.toggle(&up);
            }
        }
        self.excluded_countries = f;
    }

    pub fn toggle_excluded_country(&mut self, code: &str) -> bool {
        self.excluded_countries.toggle(&code.to_uppercase())
    }

    pub fn last_sync(&self) -> anyhow::Result<Option<i64>> {
        self.cache.last_sync()
    }

    pub fn set_last_sync(&self, secs: i64) -> anyhow::Result<()> {
        self.cache.set_last_sync(secs)
    }

    pub fn catalog_count(&self) -> anyhow::Result<usize> {
        self.cache.count()
    }

    pub fn replace_catalog(&self, stations: &[Station]) -> anyhow::Result<usize> {
        self.cache.replace_all(stations)
    }

    pub fn search_offline(&self, term: &str) -> anyhow::Result<Vec<Station>> {
        let stations = match term.trim().is_empty() {
            true => self.cache.list_all(self.excluded_country_ids())?,
            false => {
                let sanitized = term.replace('*', "");
                let escaped = sanitized.replace('"', "\"\"");
                let phrase = format!("\"{escaped}\"");
                self.cache
                    .search_name(&phrase, self.excluded_country_ids())?
            }
        };
        Ok(stations
            .into_iter()
            .filter(|s| !self.health.is_hidden(&s.stationuuid))
            .collect())
    }

    pub fn search_offline_filtered(&self, q: &SearchQuery) -> anyhow::Result<Vec<Station>> {
        self.cache.search(q, self.excluded_country_ids())
    }

    pub fn is_hidden(&self, uuid: &str) -> bool {
        self.health.is_hidden(uuid) || self.blacklist.contains(uuid)
    }

    pub fn toggle_blacklist(&mut self, uuid: &str) -> bool {
        self.blacklist.toggle(uuid)
    }

    pub fn is_blacklisted(&self, uuid: &str) -> bool {
        self.blacklist.contains(uuid)
    }

    pub fn blacklist_ids(&self) -> &[String] {
        self.blacklist.ids()
    }

    pub fn facets(&self, limit: usize) -> anyhow::Result<Facets> {
        self.cache.facets(limit)
    }

    pub fn note_play_failure(&mut self, uuid: &str) {
        self.health.record_failure(uuid);
    }

    pub fn note_play_success(&mut self, uuid: &str) {
        self.health.record_success(uuid);
    }

    pub fn hidden_ids(&self) -> Vec<String> {
        self.health.hidden_ids()
    }

    pub fn clear_health(&mut self, uuid: &str) {
        self.health.clear(uuid);
    }

    pub fn clear_all_health(&mut self) {
        self.health.clear_all();
    }

    pub fn save_health(&self, path: &std::path::Path) -> anyhow::Result<()> {
        self.health.save(path)
    }

    pub fn toggle_favorite(&mut self, uuid: &str) -> bool {
        self.favorites.toggle(uuid)
    }

    pub fn is_favorite(&self, uuid: &str) -> bool {
        self.favorites.contains(uuid)
    }

    pub fn favorite_ids(&self) -> &[String] {
        self.favorites.ids()
    }

    pub fn set_favorites(&mut self, ids: Vec<String>) {
        self.favorites.set_from(ids);
    }

    pub fn set_blacklist(&mut self, ids: Vec<String>) {
        self.blacklist.set_from(ids);
    }

    pub fn record_history(&mut self, uuid: &str) {
        self.history.record(uuid);
    }

    pub fn history_ids(&self) -> &[String] {
        self.history.ids()
    }

    pub fn load(
        cache: Cache,
        health: Health,
        fav_path: &std::path::Path,
        hist_path: &std::path::Path,
        blacklist_path: &std::path::Path,
        excluded_path: &std::path::Path,
    ) -> Self {
        Self {
            cache,
            health,
            favorites: Favorites::load(fav_path),
            history: History::load(hist_path),
            blacklist: Favorites::load(blacklist_path),
            excluded_countries: Favorites::load(excluded_path),
        }
    }

    pub fn save_state(
        &self,
        fav_path: &std::path::Path,
        hist_path: &std::path::Path,
        blacklist_path: &std::path::Path,
        excluded_path: &std::path::Path,
    ) -> anyhow::Result<()> {
        self.favorites.save(fav_path)?;
        self.history.save(hist_path)?;
        self.blacklist.save(blacklist_path)?;
        self.excluded_countries.save(excluded_path)?;
        Ok(())
    }

    pub fn station_by_uuid(&self, uuid: &str) -> anyhow::Result<Option<Station>> {
        self.cache.get_by_uuid(uuid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_search_excludes_hidden_stations() {
        let cache = Cache::open_in_memory().unwrap();
        cache
            .upsert(&[
                Station {
                    stationuuid: "u1".into(),
                    name: "Jazz Live".into(),
                    url_resolved: String::new(),
                    countrycode: String::new(),
                    language: String::new(),
                    tags: String::new(),
                    codec: String::new(),
                    bitrate: 0,
                    votes: 0,
                    geo_lat: None,
                    geo_long: None,
                },
                Station {
                    stationuuid: "u2".into(),
                    name: "Jazz Dead".into(),
                    url_resolved: String::new(),
                    countrycode: String::new(),
                    language: String::new(),
                    tags: String::new(),
                    codec: String::new(),
                    bitrate: 0,
                    votes: 0,
                    geo_lat: None,
                    geo_long: None,
                },
            ])
            .unwrap();
        let mut health = Health::new();
        for _ in 0..3 {
            health.record_failure("u2");
        }
        let cat = Catalog::new(cache, health);
        let results = cat.search_offline("jazz").unwrap();
        let uuids: Vec<_> = results.iter().map(|s| s.stationuuid.as_str()).collect();
        assert!(uuids.contains(&"u1"));
        assert!(!uuids.contains(&"u2"));
    }

    #[test]
    fn offline_search_empty_term_lists_all_non_hidden() {
        let cache = Cache::open_in_memory().unwrap();
        cache
            .upsert(&[
                Station {
                    stationuuid: "u1".into(),
                    name: "Alpha".into(),
                    url_resolved: String::new(),
                    countrycode: String::new(),
                    language: String::new(),
                    tags: String::new(),
                    codec: String::new(),
                    bitrate: 0,
                    votes: 0,
                    geo_lat: None,
                    geo_long: None,
                },
                Station {
                    stationuuid: "u2".into(),
                    name: "Beta".into(),
                    url_resolved: String::new(),
                    countrycode: String::new(),
                    language: String::new(),
                    tags: String::new(),
                    codec: String::new(),
                    bitrate: 0,
                    votes: 0,
                    geo_lat: None,
                    geo_long: None,
                },
            ])
            .unwrap();
        let mut health = Health::new();
        for _ in 0..3 {
            health.record_failure("u2");
        }
        let cat = Catalog::new(cache, health);
        let all = cat.search_offline("").unwrap();
        let uuids: Vec<_> = all.iter().map(|s| s.stationuuid.as_str()).collect();
        assert!(uuids.contains(&"u1"));
        assert!(!uuids.contains(&"u2"));
    }

    #[test]
    fn offline_search_handles_terms_with_fts_special_chars() {
        let cache = Cache::open_in_memory().unwrap();
        cache
            .upsert(&[Station {
                stationuuid: "u1".into(),
                name: "Rock and Roll".into(),
                url_resolved: String::new(),
                countrycode: String::new(),
                language: String::new(),
                tags: String::new(),
                codec: String::new(),
                bitrate: 0,
                votes: 0,
                geo_lat: None,
                geo_long: None,
            }])
            .unwrap();
        let cat = Catalog::new(cache, Health::new());

        let quoted = cat.search_offline("rock\"").unwrap();
        assert_eq!(quoted.len(), 1);
        assert_eq!(quoted[0].stationuuid, "u1");

        let keyword = cat.search_offline("OR").unwrap();
        assert!(keyword.is_empty());
    }

    #[test]
    fn note_play_success_unhides_recovered_station() {
        let cache = Cache::open_in_memory().unwrap();
        cache
            .upsert(&[Station {
                stationuuid: "u1".into(),
                name: "Jazz".into(),
                url_resolved: String::new(),
                countrycode: String::new(),
                language: String::new(),
                tags: String::new(),
                codec: String::new(),
                bitrate: 0,
                votes: 0,
                geo_lat: None,
                geo_long: None,
            }])
            .unwrap();
        let mut health = Health::new();
        for _ in 0..3 {
            health.record_failure("u1");
        }
        let mut cat = Catalog::new(cache, health);
        assert!(cat.search_offline("jazz").unwrap().is_empty());
        cat.note_play_success("u1");
        assert_eq!(cat.search_offline("jazz").unwrap().len(), 1);
    }

    #[test]
    fn save_health_persists_to_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("health.json");
        let cache = Cache::open_in_memory().unwrap();
        let mut cat = Catalog::new(cache, Health::new());
        cat.note_play_failure("u1");
        cat.save_health(&path).unwrap();
        let reloaded = Health::load(&path);
        assert!(!reloaded.is_hidden("u1"));
    }

    #[test]
    fn catalog_toggle_and_query_favorite() {
        let cache = Cache::open_in_memory().unwrap();
        let mut cat = Catalog::new(cache, Health::new());
        assert!(!cat.is_favorite("u1"));
        assert!(cat.toggle_favorite("u1"));
        assert!(cat.is_favorite("u1"));
        assert_eq!(cat.favorite_ids(), &["u1".to_string()]);
        assert!(!cat.toggle_favorite("u1"));
        assert!(!cat.is_favorite("u1"));
    }

    #[test]
    fn catalog_records_history() {
        let cache = Cache::open_in_memory().unwrap();
        let mut cat = Catalog::new(cache, Health::new());
        cat.record_history("u1");
        cat.record_history("u2");
        assert_eq!(cat.history_ids(), &["u2".to_string(), "u1".to_string()]);
    }

    #[test]
    fn load_hydrates_favorites_history_blacklist_then_save_state_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let fav = dir.path().join("favorites.json");
        let hist = dir.path().join("history.json");
        let bl = dir.path().join("blacklist.json");
        let excl = dir.path().join("excluded_countries.json");

        {
            let cache = Cache::open_in_memory().unwrap();
            let mut cat = Catalog::new(cache, Health::new());
            cat.toggle_favorite("u1");
            cat.record_history("u9");
            cat.toggle_blacklist("ux");
            cat.save_state(&fav, &hist, &bl, &excl).unwrap();
        }

        let cache = Cache::open_in_memory().unwrap();
        let cat = Catalog::load(cache, Health::new(), &fav, &hist, &bl, &excl);
        assert_eq!(cat.favorite_ids(), &["u1".to_string()]);
        assert_eq!(cat.history_ids(), &["u9".to_string()]);
        assert!(cat.is_blacklisted("ux"));
        assert!(cat.is_hidden("ux"));
    }

    #[test]
    fn search_offline_filtered_marks_hidden_via_is_hidden() {
        let cache = Cache::open_in_memory().unwrap();
        cache
            .upsert(&[
                Station {
                    stationuuid: "u1".into(),
                    name: "Jazz Live".into(),
                    url_resolved: String::new(),
                    countrycode: "GB".into(),
                    language: String::new(),
                    tags: String::new(),
                    codec: "MP3".into(),
                    bitrate: 128,
                    votes: 0,
                    geo_lat: None,
                    geo_long: None,
                },
                Station {
                    stationuuid: "u2".into(),
                    name: "Jazz Dead".into(),
                    url_resolved: String::new(),
                    countrycode: "GB".into(),
                    language: String::new(),
                    tags: String::new(),
                    codec: "MP3".into(),
                    bitrate: 128,
                    votes: 0,
                    geo_lat: None,
                    geo_long: None,
                },
            ])
            .unwrap();
        let mut health = Health::new();
        for _ in 0..3 {
            health.record_failure("u2");
        }
        let cat = Catalog::new(cache, health);
        let q = SearchQuery {
            countrycode: Some("GB".into()),
            ..Default::default()
        };
        let rows = cat.search_offline_filtered(&q).unwrap();
        let mut uuids: Vec<_> = rows.iter().map(|s| s.stationuuid.clone()).collect();
        uuids.sort();
        assert_eq!(uuids, vec!["u1", "u2"]);
        assert!(!cat.is_hidden("u1"));
        assert!(cat.is_hidden("u2"));
    }

    #[test]
    fn facets_passes_through_to_cache() {
        let cache = Cache::open_in_memory().unwrap();
        cache
            .upsert(&[
                Station {
                    stationuuid: "u1".into(),
                    name: "A".into(),
                    url_resolved: String::new(),
                    countrycode: "GB".into(),
                    language: String::new(),
                    tags: String::new(),
                    codec: "MP3".into(),
                    bitrate: 128,
                    votes: 0,
                    geo_lat: None,
                    geo_long: None,
                },
                Station {
                    stationuuid: "u2".into(),
                    name: "B".into(),
                    url_resolved: String::new(),
                    countrycode: "DE".into(),
                    language: String::new(),
                    tags: String::new(),
                    codec: "MP3".into(),
                    bitrate: 128,
                    votes: 0,
                    geo_lat: None,
                    geo_long: None,
                },
            ])
            .unwrap();
        let cat = Catalog::new(cache, Health::new());
        let f = cat.facets(10).unwrap();
        let countries: Vec<_> = f.countries.iter().map(|(c, _)| c.as_str()).collect();
        assert!(countries.contains(&"GB"));
        assert!(countries.contains(&"DE"));
    }

    #[test]
    fn station_by_uuid_resolves_ingested_station_and_none_for_missing() {
        let cache = Cache::open_in_memory().unwrap();
        let cat = Catalog::new(cache, Health::new());
        cat.ingest(&[Station {
            stationuuid: "u1".into(),
            name: "Jazz".into(),
            url_resolved: String::new(),
            countrycode: String::new(),
            language: String::new(),
            tags: String::new(),
            codec: String::new(),
            bitrate: 0,
            votes: 0,
            geo_lat: None,
            geo_long: None,
        }])
        .unwrap();

        let found = cat.station_by_uuid("u1").unwrap();
        assert_eq!(found.map(|s| s.name), Some("Jazz".to_string()));
        assert!(cat.station_by_uuid("missing").unwrap().is_none());
    }
}
