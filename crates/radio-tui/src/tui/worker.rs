use crate::tui::message::Msg;
use crate::tui::model::{RowState, StationRow};
use radio_core::catalog::{api, Catalog, SearchQuery, Station};
use radio_core::sync::ProfileChange;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

// wall-clock hour between checks; the TTL inside should_refresh is what
// actually decides, this only bounds how often we ask
const REFRESH_CHECK_SECS: i64 = 3600;
const CATALOG_TTL_SECS: i64 = 86_400;

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
    MarkSuccess(String),
    MirrorAnnounce {
        uuid: String,
        name: String,
        url: String,
    },
    ResolveAndPlay(String),
    SaveState,
    SaveProfile(radio_core::sync::Profile),
    SyncCatalog,
    QuickTop,
    PopularSeed,
    Sync,
    // a sync the user did not ask for, so it stays silent; see the doorbell.
    SyncQuiet,
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
    pub pending: PathBuf,
    pub profile: PathBuf,
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
    resync_queued: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> std::thread::JoinHandle<()> {
    // a prior session may have quit or gone offline before its sync landed;
    // pick that delta back up rather than starting the log over.
    catalog.pending = radio_core::sync::Pending::load(&paths.pending);
    std::thread::spawn(move || {
        let mut last_check = now_secs();
        loop {
            let first = match req_rx.recv_timeout(Duration::from_secs(60)) {
                Ok(req) => req,
                Err(RecvTimeoutError::Timeout) => {
                    // periodic housekeeping; no request to process this round
                    maybe_refresh(&mut catalog, &msg_tx, &mut last_check);
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => break,
            };
            let mut batch = vec![first];
            while let Ok(more) = req_rx.try_recv() {
                batch.push(more);
            }
            let (others, last_search) = coalesce(batch);
            let mut shutdown = false;
            for req in others {
                if handle_req(req, &mut catalog, &paths, &msg_tx, &resync_queued) {
                    shutdown = true;
                    break;
                }
            }
            if shutdown {
                break;
            }
            if let Some(search) = last_search {
                handle_req(search, &mut catalog, &paths, &msg_tx, &resync_queued);
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
    resync_queued: &std::sync::atomic::AtomicBool,
) -> bool {
    match req {
        WorkerReq::Shutdown => return true,
        WorkerReq::Search(q, filters) => handle_search(catalog, &q, &filters, paths, msg_tx),
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
        WorkerReq::MarkSuccess(uuid) => {
            catalog.note_play_success(&uuid);
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
        WorkerReq::SaveProfile(profile) => {
            if let Err(e) = profile.save(&paths.profile) {
                crate::log_warn!("worker: failed to save profile: {e}");
            }
        }
        WorkerReq::SyncCatalog => handle_sync_catalog(catalog, msg_tx),
        WorkerReq::QuickTop => handle_quick_top(catalog, paths, msg_tx),
        WorkerReq::PopularSeed => handle_popular_seed(catalog, msg_tx),
        WorkerReq::Sync => {
            handle_sync(catalog, paths, msg_tx, true);
        }
        // triggered by the server's doorbell, not by the user: it must not
        // announce itself. the flag is cleared only after the sync has run, so
        // events arriving while it is in flight collapse into one re-sync.
        WorkerReq::SyncQuiet => {
            handle_sync(catalog, paths, msg_tx, false);
            resync_queued.store(false, std::sync::atomic::Ordering::SeqCst);
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
    save_state_and_health(catalog, paths);
    // sync_pending.json is shared with the CLI, which may run concurrently
    // (main.rs returns before single_instance::take_over) — merge rather than
    // overwrite so this save can't erase a deletion the CLI just persisted.
    if let Err(e) = catalog.pending.save_merged(&paths.pending) {
        crate::log_warn!("worker: failed to save pending sync log: {e}");
    }
}

/// only right after a successful push: `pushed` is the exact `Pending` sent in
/// that request, so removing just those entries (not overwriting the log) keeps
/// anything the CLI or another surface wrote to it during the round-trip.
fn save_all_after_clear(
    catalog: &Catalog,
    paths: &WorkerPaths,
    pushed: &radio_core::sync::Pending,
) {
    save_state_and_health(catalog, paths);
    if let Err(e) = radio_core::sync::Pending::clear_pushed(pushed, &paths.pending) {
        crate::log_warn!("worker: failed to save pending sync log: {e}");
    }
}

fn save_state_and_health(catalog: &Catalog, paths: &WorkerPaths) {
    if let Err(e) = catalog.save_state(&paths.fav, &paths.hist, &paths.blacklist, &paths.excluded) {
        crate::log_warn!("worker: failed to save favorites/history/blacklist: {e}");
    }
    if let Err(e) = catalog.save_health(&paths.health) {
        crate::log_warn!("worker: failed to save health: {e}");
    }
}

fn handle_sync(catalog: &mut Catalog, paths: &WorkerPaths, msg_tx: &Sender<Msg>, announce: bool) {
    use radio_core::sync::{self, session, Profile, SyncClient};

    let Some(key) = sync::load_key() else {
        if announce {
            let _ = msg_tx.send(Msg::Notice(
                "not linked — run: world-radio sync login".into(),
            ));
        }
        return;
    };
    let profile = Profile::load(&paths.profile);
    let local = session::outgoing(session::LocalState {
        favs: catalog.favorite_ids().to_vec(),
        blocked: catalog.blacklist_ids().to_vec(),
        excluded_countries: catalog.excluded_country_ids().to_vec(),
        changed: catalog.pending.clone(),
        profile: &profile,
        plays: catalog.history_plays(),
    });
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
    // only now: the server has the delta, so replaying it would be wrong.
    catalog.pending.clear();
    catalog.apply_synced_favorites(merged.favs.clone());
    catalog.apply_synced_blacklist(merged.blocked.clone());
    catalog.apply_synced_excluded_countries(merged.excluded_countries.clone());

    let mut profile = profile;
    let changed = session::apply_remote_profile(
        &mut profile,
        &merged.shuffle_filter,
        &merged.scope,
        &merged.theme,
        &merged.settings,
    );
    if changed.any() {
        if let Err(e) = profile.save(&paths.profile) {
            crate::log_warn!("worker: failed to save profile: {e}");
        }
    }
    if let Some(plays) = session::merge_history(catalog.history_plays(), &merged.history) {
        catalog.apply_synced_history(plays);
    }

    save_all_after_clear(catalog, paths, &local.changed);
    let _ = msg_tx.send(Msg::ExcludedCountriesChanged(
        catalog.excluded_country_ids().to_vec(),
    ));
    if changed.any() {
        let _ = msg_tx.send(profile_synced_msg(&profile, changed));
    }
    if announce {
        let _ = msg_tx.send(Msg::Notice(format!(
            "synced: {} favourites, {} blocked, {} excluded countries",
            merged.favs.len(),
            merged.blocked.len(),
            merged.excluded_countries.len()
        )));
    }
}

// only the fields the merge actually moved are sent: reporting an unchanged
// scope would reset the user's current browse scope on the receiving side.
fn profile_synced_msg(profile: &radio_core::sync::Profile, changed: ProfileChange) -> Msg {
    Msg::ProfileSynced {
        profile: profile.clone(),
        countries: changed.countries.then(|| profile.countries.clone()),
        scope: changed.scope.then(|| profile.scope.clone()),
        theme: changed.theme.then(|| profile.theme.clone()),
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
    catalog: &mut Catalog,
    q: &SearchQuery,
    filters: &crate::tui::model::BrowseFilters,
    paths: &WorkerPaths,
    msg_tx: &Sender<Msg>,
) {
    use crate::tui::model::StatusFilter;
    let StatusFilter::All = filters.status else {
        let result = match filters.status {
            StatusFilter::Favorites => Msg::SearchResults(narrow(
                resolve(catalog, catalog.favorite_ids(), true),
                filters,
            )),
            StatusFilter::Recent => Msg::SearchResults(narrow(
                resolve(catalog, &catalog.history_ids(), false),
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
            StatusFilter::All => unreachable!(),
        };
        let _ = msg_tx.send(Msg::SetOffline(false));
        let _ = msg_tx.send(drop_unplayable(result, filters.hide_unplayable));
        return;
    };
    let _ = msg_tx.send(Msg::SetOffline(false));
    // local first — instant, never blocks
    let (local, _) = search_local(catalog, q);
    let _ = msg_tx.send(drop_unplayable(
        narrow_msg(local, filters),
        filters.hide_unplayable,
    ));
    if should_search_online(q) {
        match online_search_bounded(catalog, q, &paths.health) {
            Ok(rows) => {
                let _ = msg_tx.send(drop_unplayable(
                    narrow_msg(Msg::SearchResults(rows), filters),
                    filters.hide_unplayable,
                ));
            }
            Err(e) => {
                crate::log_warn!("worker: online search failed ({e}), keeping local results");
                let _ = msg_tx.send(Msg::SetOffline(true));
            }
        }
    }
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

fn search_local(catalog: &Catalog, q: &SearchQuery) -> (Msg, bool) {
    let msg = match catalog.search_offline_filtered(q) {
        Ok(stations) => Msg::SearchResults(rows_from(catalog, &stations)),
        Err(e) => Msg::SearchFailed(e.to_string()),
    };
    (msg, false)
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

/// ingest carries dead-on-arrival health writes in-memory only; persist them here
/// rather than relying on the incidental save_all at worker shutdown, since a
/// search or quick-top ingest can happen any time in a long session.
fn ingest_and_persist(
    catalog: &mut Catalog,
    stations: &[Station],
    health_path: &std::path::Path,
) -> anyhow::Result<()> {
    catalog.ingest(stations)?;
    catalog.save_health(health_path)
}

/// the remote api takes one country per call, so a multi-country filter has to
/// be asked for country by country — one call would answer the first country
/// only and the rest would never reach the catalogue.
fn fanned_queries(q: &SearchQuery) -> Vec<SearchQuery> {
    match q.countrycodes.len() > 1 {
        false => vec![q.clone()],
        true => q
            .countrycodes
            .iter()
            .map(|c| SearchQuery {
                countrycodes: vec![c.clone()],
                ..q.clone()
            })
            .collect(),
    }
}

fn merge_unique(batches: Vec<Vec<Station>>) -> Vec<Station> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for batch in batches {
        for s in batch {
            if seen.insert(s.stationuuid.clone()) {
                out.push(s);
            }
        }
    }
    out
}

fn online_search_bounded(
    catalog: &mut Catalog,
    q: &SearchQuery,
    health_path: &std::path::Path,
) -> anyhow::Result<Vec<StationRow>> {
    let rb = api::resolve_with_timeout(4);
    let mut batches = Vec::new();
    for sub in fanned_queries(q) {
        batches.push(rb.search(&sub)?);
    }
    let stations = merge_unique(batches);
    ingest_and_persist(catalog, &stations, health_path)?;
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

fn maybe_refresh(catalog: &mut Catalog, msg_tx: &Sender<Msg>, last_check: &mut i64) {
    let now = now_secs();
    if now - *last_check < REFRESH_CHECK_SECS {
        return;
    }
    *last_check = now;
    let last_sync = catalog.last_sync().ok().flatten();
    if radio_core::catalog::should_refresh(last_sync, now, CATALOG_TTL_SECS) {
        handle_sync_catalog(catalog, msg_tx);
    }
}

fn handle_sync_catalog(catalog: &Catalog, msg_tx: &Sender<Msg>) {
    let rb = api::resolve();
    match rb.fetch_all() {
        Ok(stations) => match catalog.replace_catalog(&stations) {
            Ok(count) => {
                let _ = catalog.set_last_sync(now_secs());
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

fn handle_popular_seed(catalog: &Catalog, msg_tx: &Sender<Msg>) {
    let fav_ids = catalog.favorite_ids().to_vec();
    match catalog.list_by_popularity(&fav_ids, 200) {
        Ok(stations) => {
            let rows: Vec<StationRow> = stations
                .iter()
                .map(|s| {
                    let uuid = &s.stationuuid;
                    station_to_row(s, catalog.is_favorite(uuid), catalog.is_hidden(uuid))
                })
                .collect();
            if !rows.is_empty() {
                let _ = msg_tx.send(Msg::SearchResults(rows));
            }
        }
        Err(e) => {
            crate::log_warn!("worker: popular seed failed: {e}");
        }
    }
}

fn handle_quick_top(catalog: &mut Catalog, paths: &WorkerPaths, msg_tx: &Sender<Msg>) {
    let rb = api::resolve();
    match rb.fetch_top(200) {
        Ok(stations) => {
            if let Err(e) = ingest_and_persist(catalog, &stations, &paths.health) {
                crate::log_warn!("worker: quick-top ingest failed: {e}");
                return;
            }
            let count = stations.len();
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
            lastcheckok: 1,
            lastchecktime_iso8601: String::new(),
        }
    }

    fn q_with(name: Option<&str>, country: Option<&str>) -> SearchQuery {
        SearchQuery {
            name: name.map(str::to_string),
            countrycodes: country.map(str::to_string).into_iter().collect(),
            language: None,
            tags: Vec::new(),
            codecs: Vec::new(),
            bitrate_min: None,
        }
    }

    #[test]
    fn two_countries_fan_out_into_one_call_each() {
        let q = SearchQuery {
            name: Some("jazz".into()),
            countrycodes: vec!["UA".into(), "US".into()],
            ..Default::default()
        };
        let fanned = fanned_queries(&q);
        assert_eq!(fanned.len(), 2);
        assert_eq!(fanned[0].countrycodes, vec!["UA".to_string()]);
        assert_eq!(fanned[1].countrycodes, vec!["US".to_string()]);
        // the rest of the filter rides along on every call
        assert_eq!(fanned[1].name.as_deref(), Some("jazz"));
    }

    #[test]
    fn one_country_or_none_stays_a_single_call() {
        let one = SearchQuery {
            countrycodes: vec!["UA".into()],
            ..Default::default()
        };
        assert_eq!(fanned_queries(&one).len(), 1);
        assert_eq!(fanned_queries(&SearchQuery::default()).len(), 1);
    }

    #[test]
    fn merged_results_hold_no_duplicate_uuids() {
        let ua = vec![station("shared"), station("ua-only")];
        let us = vec![station("shared"), station("us-only")];
        let merged = merge_unique(vec![ua, us]);
        let ids: Vec<&str> = merged.iter().map(|s| s.stationuuid.as_str()).collect();
        assert_eq!(ids, vec!["shared", "ua-only", "us-only"]);
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
                countrycodes: Vec::new(),
                language: None,
                tags: Vec::new(),
                codecs: Vec::new(),
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

    #[test]
    fn profile_synced_msg_reports_only_the_field_that_moved() {
        // sending an unchanged scope resets the receiving side's browse scope.
        let mut p = radio_core::sync::Profile::default();
        p.set_countries(vec!["UA".into()], 100);
        p.set_scope("favorites", 100);
        p.set_theme("nord", 200);
        let changed = ProfileChange {
            theme: true,
            ..Default::default()
        };
        match profile_synced_msg(&p, changed) {
            Msg::ProfileSynced {
                countries,
                scope,
                theme,
                ..
            } => {
                assert_eq!(countries, None);
                assert_eq!(scope, None);
                assert_eq!(theme, Some("nord".to_string()));
            }
            _ => panic!("expected ProfileSynced"),
        }
    }

    #[test]
    fn profile_synced_msg_carries_the_whole_profile_it_just_saved() {
        let mut p = radio_core::sync::Profile::default();
        p.set_countries(vec!["PL".into()], 300);
        match profile_synced_msg(
            &p,
            ProfileChange {
                countries: true,
                ..Default::default()
            },
        ) {
            Msg::ProfileSynced { profile, .. } => assert_eq!(profile, p),
            _ => panic!("expected ProfileSynced"),
        }
    }

    #[test]
    fn ingest_and_persist_saves_health_deterministically() {
        // ingest-time health writes (a server-reported dead station) must not rely
        // on the incidental save_all at worker shutdown — a search or quick-top
        // ingest can happen anywhere in a long session, and search_cli exits right
        // after ingesting, losing the write entirely without this.
        use radio_core::catalog::{Cache, Catalog, Health};
        let dir = tempfile::tempdir().unwrap();
        let health_path = dir.path().join("station_health.json");
        let mut catalog = Catalog::new(Cache::open_in_memory().unwrap(), Health::new());
        let mut dead = station("dead1");
        dead.lastcheckok = 0;
        ingest_and_persist(&mut catalog, &[dead], &health_path).unwrap();
        let reloaded = Health::load(&health_path);
        assert!(reloaded.is_hidden("dead1"));
    }
}
