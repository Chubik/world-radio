use crate::tui::message::Msg;
use crate::tui::model::{RowState, StationRow};
use radio_core::catalog::{api, Catalog, SearchQuery, Station};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};

pub enum WorkerReq {
    Search(SearchQuery, crate::tui::model::BrowseFilters),
    LoadFacets,
    ToggleFavorite(String),
    Blacklist(String),
    ToggleExcludedCountry(String),
    Recheck(String),
    RecheckAll,
    RecordHistory(String),
    MarkFailed(String),
    MirrorAnnounce {
        uuid: String,
        name: String,
        url: String,
    },
    ResolveAndPlay(String),
    SaveState,
    SyncCatalog,
    QuickTop,
    Sync,
    SyncCreate,
    SyncLogout,
    SyncDelete,
    CheckUpdate,
    Update(radio_core::update::Release),
    Shutdown,
}

pub struct WorkerPaths {
    pub fav: PathBuf,
    pub hist: PathBuf,
    pub health: PathBuf,
    pub blacklist: PathBuf,
    pub excluded: PathBuf,
}

pub fn station_to_row(s: &Station, favorite: bool, hidden: bool) -> StationRow {
    let state = if hidden {
        RowState::Disabled
    } else {
        RowState::Normal
    };
    StationRow {
        uuid: s.stationuuid.clone(),
        name: s.name.clone(),
        url: s.url_resolved.clone(),
        country: s.countrycode.clone(),
        tags: s.tags.clone(),
        bitrate: s.bitrate,
        codec: s.codec.clone(),
        favorite,
        state,
    }
}

pub fn spawn(
    mut catalog: Catalog,
    paths: WorkerPaths,
    req_rx: Receiver<WorkerReq>,
    msg_tx: Sender<Msg>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        while let Ok(first) = req_rx.recv() {
            let mut batch = vec![first];
            while let Ok(more) = req_rx.try_recv() {
                batch.push(more);
            }
            let (others, last_search) = coalesce(batch);
            let mut shutdown = false;
            for req in others {
                if handle_req(req, &mut catalog, &paths, &msg_tx) {
                    shutdown = true;
                    break;
                }
            }
            if shutdown {
                break;
            }
            if let Some(search) = last_search {
                handle_req(search, &mut catalog, &paths, &msg_tx);
            }
        }
        save_all(&catalog, &paths);
    })
}

fn handle_req(
    req: WorkerReq,
    catalog: &mut Catalog,
    paths: &WorkerPaths,
    msg_tx: &Sender<Msg>,
) -> bool {
    match req {
        WorkerReq::Shutdown => return true,
        WorkerReq::Search(q, filters) => handle_search(catalog, &q, &filters, msg_tx),
        WorkerReq::LoadFacets => handle_load_facets(catalog, msg_tx),
        WorkerReq::Blacklist(uuid) => {
            catalog.toggle_blacklist(&uuid);
            handle_sync(catalog, paths, msg_tx, false);
        }
        WorkerReq::ToggleExcludedCountry(code) => {
            catalog.toggle_excluded_country(&code);
            save_all(catalog, paths);
            let _ = msg_tx.send(Msg::ExcludedCountriesChanged(
                catalog.excluded_country_ids().to_vec(),
            ));
        }
        WorkerReq::Recheck(uuid) => {
            catalog.clear_health(&uuid);
            if let Err(e) = catalog.save_health(&paths.health) {
                crate::log_warn!("worker: failed to save health: {e}");
            }
        }
        WorkerReq::RecheckAll => {
            catalog.clear_all_health();
            if let Err(e) = catalog.save_health(&paths.health) {
                crate::log_warn!("worker: failed to save health: {e}");
            }
        }
        WorkerReq::ToggleFavorite(uuid) => {
            catalog.toggle_favorite(&uuid);
            handle_sync(catalog, paths, msg_tx, false);
        }
        WorkerReq::RecordHistory(uuid) => catalog.record_history(&uuid),
        WorkerReq::MarkFailed(uuid) => {
            catalog.note_play_failure(&uuid);
            if let Err(e) = catalog.save_health(&paths.health) {
                crate::log_warn!("worker: failed to save health: {e}");
            }
        }
        WorkerReq::MirrorAnnounce { uuid, name, url } => {
            if let Some(key) = radio_core::sync::load_key() {
                let client = radio_core::mirror::MirrorClient::new("https://r4dio.net");
                let origin = radio_core::mirror::device_id();
                if let Err(e) = client.play(&key, &uuid, &name, &url, &origin) {
                    crate::log_warn!("worker: mirror announce failed: {e}");
                }
            }
        }
        WorkerReq::ResolveAndPlay(uuid) => handle_resolve_and_play(catalog, &uuid, msg_tx),
        WorkerReq::SaveState => save_all(catalog, paths),
        WorkerReq::SyncCatalog => handle_sync_catalog(catalog, msg_tx),
        WorkerReq::QuickTop => handle_quick_top(catalog, msg_tx),
        WorkerReq::Sync => {
            handle_sync(catalog, paths, msg_tx, true);
        }
        WorkerReq::SyncCreate => {
            match radio_core::sync::SyncClient::new("https://r4dio.net").create_account() {
                Ok(key) => {
                    if let Err(e) = radio_core::sync::store_key(&key) {
                        crate::log_warn!("worker: store key failed: {e}");
                    }
                    let _ = msg_tx.send(Msg::SyncKeyChanged(Some(key)));
                    let _ = msg_tx.send(Msg::Notice("account created and linked".into()));
                    handle_sync(catalog, paths, msg_tx, false);
                }
                Err(e) => {
                    crate::log_warn!("worker: create account failed: {e}");
                    let _ = msg_tx.send(Msg::Notice("could not create account".into()));
                }
            }
        }
        WorkerReq::SyncLogout => {
            let _ = radio_core::sync::clear_key();
            let _ = msg_tx.send(Msg::SyncKeyChanged(None));
            let _ = msg_tx.send(Msg::Notice("logged out (favourites kept)".into()));
        }
        WorkerReq::SyncDelete => {
            if let Some(key) = radio_core::sync::load_key() {
                let _ = radio_core::sync::SyncClient::new("https://r4dio.net").delete(&key);
            }
            let _ = radio_core::sync::clear_key();
            let _ = msg_tx.send(Msg::SyncKeyChanged(None));
            let _ = msg_tx.send(Msg::Notice("account deleted".into()));
        }
        WorkerReq::CheckUpdate => {
            // a fresh check so pressing U picks up a release published
            // after this session started; downloads immediately if newer.
            match radio_core::update::fetch_latest() {
                Ok(Some(rel)) => {
                    let _ = msg_tx.send(Msg::UpdateFound(rel));
                }
                Ok(None) => {
                    let _ = msg_tx.send(Msg::UpdateUpToDate);
                }
                Err(e) => {
                    let _ = msg_tx.send(Msg::Notice(format!("update check failed: {e}")));
                }
            }
        }
        WorkerReq::Update(rel) => match radio_core::update::apply(&rel) {
            Ok(()) => {
                let _ = msg_tx.send(Msg::UpdateApplied(rel.version.clone()));
            }
            Err(e) => {
                crate::log_warn!("worker: update failed: {e}");
                let _ = msg_tx.send(Msg::Notice(format!("update failed: {e}")));
            }
        },
    }
    false
}

fn coalesce(pending: Vec<WorkerReq>) -> (Vec<WorkerReq>, Option<WorkerReq>) {
    let mut others = Vec::new();
    let mut last_search = None;
    for req in pending {
        match req {
            WorkerReq::Search(..) => last_search = Some(req),
            other => others.push(other),
        }
    }
    (others, last_search)
}

fn save_all(catalog: &Catalog, paths: &WorkerPaths) {
    if let Err(e) = catalog.save_state(&paths.fav, &paths.hist, &paths.blacklist, &paths.excluded) {
        crate::log_warn!("worker: failed to save favorites/history/blacklist: {e}");
    }
    if let Err(e) = catalog.save_health(&paths.health) {
        crate::log_warn!("worker: failed to save health: {e}");
    }
}

fn handle_sync(catalog: &mut Catalog, paths: &WorkerPaths, msg_tx: &Sender<Msg>, announce: bool) {
    use radio_core::sync::{self, SyncClient, SyncData};

    let Some(key) = sync::load_key() else {
        if announce {
            let _ = msg_tx.send(Msg::Notice(
                "not linked — run: world-radio sync login".into(),
            ));
        }
        return;
    };
    let local = SyncData {
        favs: catalog.favorite_ids().to_vec(),
        blocked: catalog.blacklist_ids().to_vec(),
        excluded_countries: catalog.excluded_country_ids().to_vec(),
    };
    let client = SyncClient::new("https://r4dio.net");
    let merged = match client.push(&key, &local) {
        Ok(m) => m,
        Err(e) => {
            crate::log_warn!("worker: sync failed: {e}");
            if announce {
                let _ = msg_tx.send(Msg::Notice("sync failed — check connection".into()));
            }
            return;
        }
    };
    catalog.set_favorites(merged.favs.clone());
    catalog.set_blacklist(merged.blocked.clone());
    catalog.set_excluded_countries(merged.excluded_countries.clone());
    save_all(catalog, paths);
    let _ = msg_tx.send(Msg::ExcludedCountriesChanged(
        catalog.excluded_country_ids().to_vec(),
    ));
    if announce {
        let _ = msg_tx.send(Msg::Notice(format!(
            "synced: {} favourites, {} blocked, {} excluded countries",
            merged.favs.len(),
            merged.blocked.len(),
            merged.excluded_countries.len()
        )));
    }
}

fn matches_filters(row: &StationRow, f: &crate::tui::model::BrowseFilters) -> bool {
    let country_ok = f.countries.is_empty()
        || f.countries
            .iter()
            .any(|c| row.country.eq_ignore_ascii_case(c));
    let codec_ok =
        f.codecs.is_empty() || f.codecs.iter().any(|c| row.codec.eq_ignore_ascii_case(c));
    let bitrate_ok = match f.bitrate_min {
        Some(min) => row.bitrate >= min,
        None => true,
    };
    let row_tags: Vec<String> = row
        .tags
        .to_lowercase()
        .split(',')
        .map(|x| x.trim().to_string())
        .collect();
    let tag_ok = f.tags.is_empty()
        || f.tags
            .iter()
            .any(|t| row_tags.iter().any(|rt| rt == &t.to_lowercase()));
    country_ok && codec_ok && bitrate_ok && tag_ok
}

fn handle_search(
    catalog: &Catalog,
    q: &SearchQuery,
    filters: &crate::tui::model::BrowseFilters,
    msg_tx: &Sender<Msg>,
) {
    use crate::tui::model::StatusFilter;
    let mut offline = false;
    let result = match filters.status {
        StatusFilter::All => {
            let (msg, off) = search_all(catalog, q);
            offline = off;
            narrow_msg(msg, filters)
        }
        StatusFilter::Favorites => Msg::SearchResults(narrow(
            resolve(catalog, catalog.favorite_ids(), true),
            filters,
        )),
        StatusFilter::Recent => Msg::SearchResults(narrow(
            resolve(catalog, catalog.history_ids(), false),
            filters,
        )),
        StatusFilter::Blocked => Msg::SearchResults(narrow(
            resolve(catalog, catalog.blacklist_ids(), false),
            filters,
        )),
        StatusFilter::Dead => Msg::SearchResults(narrow(
            resolve_visible(catalog, &catalog.hidden_ids()),
            filters,
        )),
    };
    let _ = msg_tx.send(Msg::SetOffline(offline));
    let _ = msg_tx.send(drop_unplayable(result, filters.hide_unplayable));
}

fn drop_unplayable(msg: Msg, hide: bool) -> Msg {
    if !hide {
        return msg;
    }
    match msg {
        Msg::SearchResults(rows) => Msg::SearchResults(
            rows.into_iter()
                .filter(|r| r.state != RowState::Disabled && !r.unstable())
                .collect(),
        ),
        other => other,
    }
}

fn search_all(catalog: &Catalog, q: &SearchQuery) -> (Msg, bool) {
    if !should_search_online(q) {
        let msg = match catalog.search_offline_filtered(q) {
            Ok(stations) => Msg::SearchResults(rows_from(catalog, &stations)),
            Err(e) => Msg::SearchFailed(e.to_string()),
        };
        return (msg, false);
    }
    match online_search(catalog, q) {
        Ok(rows) => (Msg::SearchResults(rows), false),
        Err(e) => {
            crate::log_warn!("worker: online search failed ({e}), falling back to offline");
            let msg = match catalog.search_offline_filtered(q) {
                Ok(stations) => Msg::SearchResults(rows_from(catalog, &stations)),
                Err(e) => Msg::SearchFailed(e.to_string()),
            };
            (msg, true)
        }
    }
}

fn narrow(rows: Vec<StationRow>, filters: &crate::tui::model::BrowseFilters) -> Vec<StationRow> {
    rows.into_iter()
        .filter(|r| matches_filters(r, filters))
        .collect()
}

fn narrow_msg(msg: Msg, filters: &crate::tui::model::BrowseFilters) -> Msg {
    match msg {
        Msg::SearchResults(rows) => Msg::SearchResults(narrow(rows, filters)),
        other => other,
    }
}

fn should_search_online(q: &SearchQuery) -> bool {
    !q.name.as_deref().map(str::trim).unwrap_or("").is_empty()
}

fn online_search(catalog: &Catalog, q: &SearchQuery) -> anyhow::Result<Vec<StationRow>> {
    let rb = api::resolve();
    let stations = rb.search(q)?;
    catalog.ingest(&stations)?;
    let filtered = catalog.search_offline_filtered(q)?;
    Ok(rows_from(catalog, &filtered))
}

fn rows_from(catalog: &Catalog, stations: &[Station]) -> Vec<StationRow> {
    stations
        .iter()
        .map(|s| {
            let uuid = &s.stationuuid;
            station_to_row(s, catalog.is_favorite(uuid), catalog.is_hidden(uuid))
        })
        .collect()
}

fn resolve(catalog: &Catalog, ids: &[String], favorite: bool) -> Vec<StationRow> {
    ids.iter()
        .filter_map(|uuid| match catalog.station_by_uuid(uuid) {
            Ok(Some(s)) => Some(station_to_row(
                &s,
                favorite || catalog.is_favorite(uuid),
                catalog.is_hidden(uuid),
            )),
            Ok(None) => None,
            Err(e) => {
                crate::log_warn!("worker: station_by_uuid({uuid}) failed: {e}");
                None
            }
        })
        .collect()
}

fn resolve_visible(catalog: &Catalog, ids: &[String]) -> Vec<StationRow> {
    ids.iter()
        .filter_map(|uuid| match catalog.station_by_uuid(uuid) {
            Ok(Some(s)) => Some(station_to_row(&s, catalog.is_favorite(uuid), false)),
            Ok(None) => None,
            Err(e) => {
                crate::log_warn!("worker: station_by_uuid({uuid}) failed: {e}");
                None
            }
        })
        .collect()
}

fn handle_resolve_and_play(catalog: &Catalog, uuid: &str, msg_tx: &Sender<Msg>) {
    match catalog.station_by_uuid(uuid) {
        Ok(Some(s)) if !catalog.is_hidden(uuid) => {
            let row = station_to_row(&s, catalog.is_favorite(uuid), false);
            let _ = msg_tx.send(Msg::AutoplayStation(row));
        }
        Ok(_) => {}
        Err(e) => crate::log_warn!("worker: resolve_and_play({uuid}) failed: {e}"),
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn seed_rows_by_popularity(catalog: &Catalog) -> Vec<StationRow> {
    let fav_ids: Vec<String> = catalog.favorite_ids().to_vec();
    match catalog.list_by_popularity(&fav_ids, 200) {
        Ok(stations) => rows_from(catalog, &stations),
        Err(e) => {
            crate::log_warn!("worker: list_by_popularity failed: {e}");
            Vec::new()
        }
    }
}

fn handle_sync_catalog(catalog: &Catalog, msg_tx: &Sender<Msg>) {
    let rb = api::resolve();
    match rb.fetch_all() {
        Ok(stations) => match catalog.replace_catalog(&stations) {
            Ok(count) => {
                let _ = catalog.set_last_sync(now_secs());
                let rows = seed_rows_by_popularity(catalog);
                if !rows.is_empty() {
                    let _ = msg_tx.send(Msg::SearchResults(rows));
                }
                let _ = msg_tx.send(Msg::CatalogSynced { count });
            }
            Err(e) => {
                crate::log_warn!("worker: replace_catalog failed: {e}");
                let _ = msg_tx.send(Msg::CatalogSyncFailed);
            }
        },
        Err(e) => {
            crate::log_warn!("worker: fetch_all failed: {e}");
            let _ = msg_tx.send(Msg::CatalogSyncFailed);
        }
    }
}

fn handle_quick_top(catalog: &Catalog, msg_tx: &Sender<Msg>) {
    let rb = api::resolve();
    match rb.fetch_top(200) {
        Ok(stations) => {
            if let Err(e) = catalog.ingest(&stations) {
                crate::log_warn!("worker: quick-top ingest failed: {e}");
                return;
            }
            let rows = seed_rows_by_popularity(catalog);
            let count = rows.len();
            if !rows.is_empty() {
                let _ = msg_tx.send(Msg::SearchResults(rows));
            }
            let _ = msg_tx.send(Msg::QuickTopReady { count });
        }
        Err(e) => {
            crate::log_warn!("worker: quick-top fetch failed: {e}");
        }
    }
}

fn handle_load_facets(catalog: &Catalog, msg_tx: &Sender<Msg>) {
    match catalog.facets(10) {
        Ok(f) => {
            let _ = msg_tx.send(Msg::FacetsLoaded(f));
        }
        Err(e) => {
            crate::log_warn!("worker: facets load failed: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radio_core::catalog::Station;

    fn station(uuid: &str) -> Station {
        Station {
            stationuuid: uuid.into(),
            name: "Name".into(),
            url_resolved: "http://x".into(),
            countrycode: "GB".into(),
            language: "english".into(),
            tags: "jazz".into(),
            codec: "MP3".into(),
            bitrate: 128,
            votes: 0,
            geo_lat: None,
            geo_long: None,
        }
    }

    fn q_with(name: Option<&str>, country: Option<&str>) -> SearchQuery {
        SearchQuery {
            name: name.map(str::to_string),
            countrycode: country.map(str::to_string),
            language: None,
            tag: None,
            codec: None,
            bitrate_min: None,
        }
    }

    #[test]
    fn online_only_when_there_is_a_text_query() {
        // a text name warrants hitting the network for fresh stations
        assert!(should_search_online(&q_with(Some("jazz"), None)));
        // filters alone (country/tag/codec/bitrate) resolve from the local catalog
        assert!(!should_search_online(&q_with(None, Some("GB"))));
        assert!(!should_search_online(&q_with(None, None)));
        // whitespace name is not a real query
        assert!(!should_search_online(&q_with(Some("   "), Some("GB"))));
    }

    #[test]
    fn station_to_row_maps_fields_and_favorite_flag() {
        let row = station_to_row(&station("u1"), true, false);
        assert_eq!(row.uuid, "u1");
        assert_eq!(row.url, "http://x");
        assert_eq!(row.country, "GB");
        assert_eq!(row.bitrate, 128);
        assert!(row.favorite);
        assert_eq!(row.state, crate::tui::model::RowState::Normal);
    }

    #[test]
    fn station_to_row_hidden_marks_disabled() {
        let row = station_to_row(&station("u1"), false, true);
        assert_eq!(row.state, crate::tui::model::RowState::Disabled);
    }

    fn r(name: &str, country: &str, codec: &str, bitrate: u32, tags: &str) -> StationRow {
        StationRow {
            uuid: name.into(),
            name: name.into(),
            url: format!("http://{name}"),
            country: country.into(),
            tags: tags.into(),
            bitrate,
            codec: codec.into(),
            favorite: false,
            state: crate::tui::model::RowState::Normal,
        }
    }

    #[test]
    fn matches_filters_passes_when_all_none() {
        let f = crate::tui::model::BrowseFilters::default();
        assert!(matches_filters(&r("a", "GB", "MP3", 128, "jazz"), &f));
    }

    #[test]
    fn matches_filters_country_codec_bitrate_tag() {
        let f = crate::tui::model::BrowseFilters {
            countries: vec!["GB".into()],
            codecs: vec!["MP3".into()],
            bitrate_min: Some(128),
            tags: vec!["jazz".into()],
            ..Default::default()
        };
        assert!(matches_filters(
            &r("a", "GB", "MP3", 192, "jazz,smooth"),
            &f
        ));
        assert!(!matches_filters(&r("b", "DE", "MP3", 192, "jazz"), &f));
        assert!(!matches_filters(&r("c", "GB", "AAC", 192, "jazz"), &f));
        assert!(!matches_filters(&r("d", "GB", "MP3", 96, "jazz"), &f));
        assert!(!matches_filters(&r("e", "GB", "MP3", 192, "rock"), &f));
    }

    #[test]
    fn matches_filters_or_within_group() {
        let f = crate::tui::model::BrowseFilters {
            countries: vec!["GB".into(), "DE".into()],
            tags: vec!["jazz".into(), "rock".into()],
            ..Default::default()
        };
        assert!(matches_filters(&r("a", "GB", "MP3", 128, "jazz"), &f));
        assert!(matches_filters(&r("b", "DE", "MP3", 128, "rock"), &f));
        assert!(!matches_filters(&r("c", "FR", "MP3", 128, "jazz"), &f));
        assert!(!matches_filters(&r("d", "GB", "MP3", 128, "pop"), &f));
    }

    fn dead(name: &str) -> StationRow {
        let mut row = r(name, "GB", "MP3", 128, "jazz");
        row.state = RowState::Disabled;
        row
    }

    #[test]
    fn drop_unplayable_off_keeps_all() {
        let msg = Msg::SearchResults(vec![
            r("ok", "GB", "MP3", 128, "jazz"),
            dead("x"),
            r("u", "GB", "AAC+", 64, "pop"),
        ]);
        let out = drop_unplayable(msg, false);
        assert!(matches!(out, Msg::SearchResults(rows) if rows.len() == 3));
    }

    #[test]
    fn drop_unplayable_on_removes_dead_and_unstable() {
        let msg = Msg::SearchResults(vec![
            r("ok", "GB", "MP3", 128, "jazz"),
            dead("x"),
            r("u", "GB", "AAC+", 64, "pop"),
        ]);
        let out = drop_unplayable(msg, true);
        match out {
            Msg::SearchResults(rows) => {
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].name, "ok");
            }
            _ => panic!("expected SearchResults"),
        }
    }

    fn search_req(name: &str) -> WorkerReq {
        WorkerReq::Search(
            SearchQuery {
                name: Some(name.into()),
                countrycode: None,
                language: None,
                tag: None,
                codec: None,
                bitrate_min: None,
            },
            crate::tui::model::BrowseFilters::default(),
        )
    }

    #[test]
    fn coalesce_keeps_only_last_search_and_preserves_other_reqs() {
        let batch = vec![
            search_req("a"),
            WorkerReq::SaveState,
            search_req("b"),
            WorkerReq::LoadFacets,
            search_req("c"),
        ];
        let (others, last) = coalesce(batch);
        assert!(matches!(
            others.as_slice(),
            [WorkerReq::SaveState, WorkerReq::LoadFacets]
        ));
        match last {
            Some(WorkerReq::Search(q, _)) => assert_eq!(q.name.as_deref(), Some("c")),
            _ => panic!("expected last search 'c'"),
        }
    }

    #[test]
    fn coalesce_no_search_returns_all_others_and_none() {
        let (others, last) = coalesce(vec![WorkerReq::SaveState, WorkerReq::LoadFacets]);
        assert_eq!(others.len(), 2);
        assert!(last.is_none());
    }
}
