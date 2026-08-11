use crate::catalog_src;
use crate::state::{MiniState, Phase, Scope, StationPick};
use radio_audio::AudioEngine;
use radio_core::catalog::{Cache, Catalog, Health};
use std::path::PathBuf;

pub struct Backend {
    pub state: MiniState,
    engine: Option<AudioEngine>,
    catalog: Catalog,
    fav_path: PathBuf,
    hist_path: PathBuf,
    blacklist_path: PathBuf,
    excluded_path: PathBuf,
    pending_path: PathBuf,
    profile_path: PathBuf,
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// this app's shuffle scope is only all-or-favourites. recent/blocked/dead are
/// real synced scopes it has no equivalent for, so they leave the panel where
/// it is instead of being collapsed into `All` — the profile on disk still
/// carries them for the surfaces that do.
fn scope_from_wire(wire: &str) -> Option<Scope> {
    match radio_core::sync::Scope::from_wire(wire)? {
        radio_core::sync::Scope::All => Some(Scope::All),
        radio_core::sync::Scope::Favorites => Some(Scope::Favorites),
        _ => None,
    }
}

impl Backend {
    pub fn new() -> anyhow::Result<Backend> {
        let data = radio_core::paths::ensure_data_dir()?;
        let cache = Cache::open(&data.join("stations.db"))?;
        let health = Health::load(&data.join("station_health.json"));
        let fav_path = data.join("favorites.json");
        let hist_path = data.join("history.json");
        let blacklist_path = data.join("blacklist.json");
        let excluded_path = data.join("excluded_countries.json");
        let pending_path = data.join("sync_pending.json");
        let profile_path = data.join("profile.json");
        let mut catalog = Catalog::load(
            cache,
            health,
            &fav_path,
            &hist_path,
            &blacklist_path,
            &excluded_path,
        );
        // a prior session may have quit or gone offline before its sync landed;
        // pick that delta back up rather than starting the log over.
        catalog.pending = radio_core::sync::Pending::load(&pending_path);

        let all = catalog_src::all_stations(&catalog)?;
        let favorites = catalog_src::favorite_stations(&catalog)?;

        let mut state = MiniState::new();
        state.load_stations(all, favorites);
        // a scope synced from another device is on disk before this app starts;
        // reading it here is what makes the panel open on the same scope.
        if let Some(scope) = scope_from_wire(&radio_core::sync::Profile::load(&profile_path).scope)
        {
            state.set_scope(scope);
        }

        let engine = AudioEngine::spawn().ok();
        if let Some(engine) = &engine {
            engine.set_volume(state.volume);
        }

        Ok(Backend {
            state,
            engine,
            catalog,
            fav_path,
            hist_path,
            blacklist_path,
            excluded_path,
            pending_path,
            profile_path,
        })
    }

    fn play_pick(&mut self, pick: StationPick) {
        if let Some(engine) = &self.engine {
            engine.play(&pick.url);
        }
        self.catalog.record_history(&pick.uuid);
        if let Err(e) = self.catalog.save_state(
            &self.fav_path,
            &self.hist_path,
            &self.blacklist_path,
            &self.excluded_path,
        ) {
            eprintln!("save history failed: {e}");
        }
        self.state.begin_play(pick);
    }

    pub fn shuffle(&mut self) {
        if let Some(pick) = self.state.pick_shuffle() {
            self.play_pick(pick);
        }
    }

    pub fn play_last(&mut self) {
        match catalog_src::last_played(&self.catalog) {
            Ok(Some(pick)) => self.play_pick(pick),
            Ok(None) => self.shuffle(),
            Err(e) => {
                eprintln!("load last station failed: {e}");
                self.shuffle();
            }
        }
    }

    pub fn resume(&mut self) {
        match self.state.now.clone() {
            Some(pick) => self.play_pick(pick),
            None => self.shuffle(),
        }
    }

    pub fn stop(&mut self) {
        self.state.stop();
        if let Some(engine) = &self.engine {
            engine.stop();
        }
    }

    pub fn set_volume(&mut self, v: f32) {
        self.state.set_volume(v);
        if let Some(engine) = &self.engine {
            engine.set_volume(self.state.volume);
        }
    }

    // the stamp is taken here, at the moment the user changes the scope, never
    // at sync time — a sync-time stamp would always outrank the other device.
    pub fn set_scope(&mut self, scope: Scope) {
        self.state.set_scope(scope);
        let wire = match scope {
            Scope::All => radio_core::sync::Scope::All,
            Scope::Favorites => radio_core::sync::Scope::Favorites,
        };
        let mut profile = radio_core::sync::Profile::load(&self.profile_path);
        profile.set_scope(wire.as_wire(), now_secs());
        if let Err(e) = profile.save(&self.profile_path) {
            eprintln!("save profile failed: {e}");
        }
    }

    pub fn now_is_favorite(&self) -> bool {
        match &self.state.now {
            Some(pick) => self.catalog.is_favorite(&pick.uuid),
            None => false,
        }
    }

    pub fn toggle_favorite(&mut self) {
        let Some(pick) = self.state.now.clone() else {
            return;
        };
        match catalog_src::toggle_and_reload(&mut self.catalog, &pick.uuid) {
            Ok(favorites) => self.state.set_favorites(favorites),
            Err(e) => eprintln!("toggle favorite failed: {e}"),
        }
        self.persist();
    }

    pub fn poll_engine(&mut self) {
        if let Some(engine) = &self.engine {
            while let Some(status) = engine.poll_status() {
                self.state.apply_status(status);
            }
        }
    }

    pub fn read_spectrum(&self, bars: usize) -> Vec<f32> {
        let _ = bars;
        crate::state::spectrum_bars(bars)
    }

    pub fn phase(&self) -> Phase {
        self.state.phase
    }

    pub fn favourite_count(&self) -> u32 {
        self.catalog.favorite_ids().len() as u32
    }

    pub fn favourite_rows(&mut self) -> Vec<crate::commands::StationRow> {
        // the star reflects what is on screen, so the rows are read from the
        // catalog rather than from state.favorites, which only tracks shuffle scope.
        let favorites = match catalog_src::favorite_stations(&self.catalog) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("load favourites failed: {e}");
                return Vec::new();
            }
        };
        let now = self.state.now.as_ref().map(|n| n.uuid.clone());
        favorites
            .into_iter()
            .map(|s| crate::commands::StationRow {
                is_playing: now.as_deref() == Some(s.uuid.as_str())
                    && self.state.phase != Phase::Idle,
                uuid: s.uuid,
                name: s.name,
                country: s.country,
                codec: s.codec,
                bitrate: s.bitrate,
            })
            .collect()
    }

    pub fn play_uuid(&mut self, uuid: &str) {
        // a favourite is usually absent from the top-1000 cache, so the row is
        // resolved through the catalog rather than looked up in the loaded lists.
        match catalog_src::station_pick(&self.catalog, uuid) {
            Ok(Some(pick)) => self.play_pick(pick),
            Ok(None) => eprintln!("station {uuid} is not in the catalog"),
            Err(e) => eprintln!("resolve station failed: {e}"),
        }
    }

    pub fn remove_favourite(&mut self, uuid: &str) -> Vec<crate::commands::StationRow> {
        match catalog_src::unfavorite_and_reload(&mut self.catalog, uuid) {
            Ok(favorites) => self.state.set_favorites(favorites),
            Err(e) => eprintln!("remove favourite failed: {e}"),
        }
        self.persist();
        self.favourite_rows()
    }

    pub fn shuffle_favourites(&mut self) {
        if let Some(pick) = crate::state::pick_random(self.state.favorites()) {
            self.play_pick(pick);
        }
    }

    pub fn blocked_rows(&mut self) -> Vec<crate::commands::StationRow> {
        let blocked = match catalog_src::blocked_stations(&self.catalog) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("load blocked failed: {e}");
                return Vec::new();
            }
        };
        blocked
            .into_iter()
            .map(|s| crate::commands::StationRow {
                is_playing: false,
                uuid: s.uuid,
                name: s.name,
                country: s.country,
                codec: s.codec,
                bitrate: s.bitrate,
            })
            .collect()
    }

    pub fn unblock(&mut self, uuid: &str) -> Vec<crate::commands::StationRow> {
        catalog_src::unblock(&mut self.catalog, uuid);
        self.persist();
        self.blocked_rows()
    }

    pub fn country_rows(&self) -> Vec<crate::commands::CountryRow> {
        match catalog_src::country_facets(&self.catalog) {
            Ok(rows) => rows
                .into_iter()
                .map(|c| crate::commands::CountryRow {
                    code: c.code,
                    count: c.count,
                    excluded: c.excluded,
                })
                .collect(),
            Err(e) => {
                eprintln!("load countries failed: {e}");
                Vec::new()
            }
        }
    }

    pub fn set_excluded(&mut self, codes: Vec<String>) -> Vec<crate::commands::CountryRow> {
        self.catalog
            .set_excluded_countries(catalog_src::merge_hidden_exclusions(&self.catalog, codes));
        self.persist();
        // the excluded set changes which stations shuffle may reach, so the
        // loaded lists are stale the moment it is written.
        match catalog_src::all_stations(&self.catalog) {
            Ok(all) => self.state.set_all(all),
            Err(e) => eprintln!("reload stations failed: {e}"),
        }
        self.country_rows()
    }

    fn to_page(&self, page: catalog_src::StationPage) -> crate::commands::StationPage {
        let now = self.state.now.as_ref().map(|n| n.uuid.clone());
        crate::commands::StationPage {
            stations: page
                .stations
                .into_iter()
                .map(|s| crate::commands::StationRow {
                    is_playing: now.as_deref() == Some(s.uuid.as_str())
                        && self.state.phase != Phase::Idle,
                    uuid: s.uuid,
                    name: s.name,
                    country: s.country,
                    codec: s.codec,
                    bitrate: s.bitrate,
                })
                .collect(),
            capped: page.capped,
        }
    }

    fn empty_page() -> crate::commands::StationPage {
        crate::commands::StationPage {
            stations: Vec::new(),
            capped: false,
        }
    }

    pub fn search(&self, name: &str) -> crate::commands::StationPage {
        match catalog_src::search_by_name(&self.catalog, name) {
            Ok(page) => self.to_page(page),
            Err(e) => {
                eprintln!("search failed: {e}");
                Self::empty_page()
            }
        }
    }

    pub fn stations_in(&self, country: &str) -> crate::commands::StationPage {
        match catalog_src::stations_in_country(&self.catalog, country) {
            Ok(page) => self.to_page(page),
            Err(e) => {
                eprintln!("load country stations failed: {e}");
                Self::empty_page()
            }
        }
    }

    /// browse marks its rows from this list rather than re-reading a full page,
    /// so starring a station updates every row that shows it without a refetch.
    pub fn favourite_ids(&self) -> Vec<String> {
        self.catalog.favorite_ids().to_vec()
    }

    pub fn add_favourite(&mut self, uuid: &str) -> Vec<String> {
        match catalog_src::favorite_and_reload(&mut self.catalog, uuid) {
            Ok(favorites) => self.state.set_favorites(favorites),
            Err(e) => eprintln!("add favourite failed: {e}"),
        }
        self.persist();
        self.favourite_ids()
    }

    pub fn filter_counts(&self) -> crate::commands::FilterCounts {
        crate::commands::FilterCounts {
            excluded: self.catalog.excluded_country_ids().len() as u32,
            blocked: self.catalog.blacklist_ids().len() as u32,
        }
    }

    // favourites and the sync log are written together: a removal that reached
    // disk but not the log would be re-added by the next sync.
    fn persist(&mut self) {
        if let Err(e) = self.catalog.save_state(
            &self.fav_path,
            &self.hist_path,
            &self.blacklist_path,
            &self.excluded_path,
        ) {
            eprintln!("save favorites failed: {e}");
        }
        // sync_pending.json is shared with the TUI/CLI, which may run concurrently —
        // merge rather than overwrite so this save can't erase their deletion.
        if let Err(e) = self.catalog.pending.save_merged(&self.pending_path) {
            eprintln!("save pending sync log failed: {e}");
        }
    }

    pub fn sync(&mut self) -> anyhow::Result<()> {
        use radio_core::sync::session;

        let Some(key) = radio_core::sync::load_key() else {
            return Ok(());
        };
        let profile = radio_core::sync::Profile::load(&self.profile_path);
        let local = session::outgoing(session::LocalState {
            favs: self.catalog.favorite_ids().to_vec(),
            blocked: self.catalog.blacklist_ids().to_vec(),
            excluded_countries: self.catalog.excluded_country_ids().to_vec(),
            changed: self.catalog.pending.clone(),
            profile: &profile,
            plays: self.catalog.history_plays(),
        });
        let client = radio_core::sync::SyncClient::new("https://r4dio.net");
        let merged = client.push(&key, &local)?;
        // only now: the server has the delta, so replaying it would be wrong.
        let pushed = self.catalog.pending.clone();
        self.catalog.pending.clear();
        self.catalog.apply_synced_favorites(merged.favs.clone());
        self.catalog.apply_synced_blacklist(merged.blocked.clone());
        self.catalog
            .apply_synced_excluded_countries(merged.excluded_countries.clone());

        let mut profile = profile;
        let changed = session::apply_remote_profile(
            &mut profile,
            &merged.shuffle_filter,
            &merged.scope,
            &merged.theme,
        );
        if changed.any() {
            profile.save(&self.profile_path)?;
        }
        if let Some(plays) = session::merge_history(self.catalog.history_plays(), &merged.history) {
            self.catalog.apply_synced_history(plays);
        }
        // history is written by save_state, so the merge above must land before it.
        self.catalog.save_state(
            &self.fav_path,
            &self.hist_path,
            &self.blacklist_path,
            &self.excluded_path,
        )?;
        // the synced scope is the only profile field this app has live state
        // for; without this the merged value would sit on disk unread.
        if changed.scope {
            if let Some(scope) = scope_from_wire(&profile.scope) {
                self.state.set_scope(scope);
            }
        }
        // remove exactly what we just sent; keep anything written to the log
        // during the round-trip (a plain overwrite would destroy it).
        radio_core::sync::Pending::clear_pushed(&pushed, &self.pending_path)?;
        let all = catalog_src::all_stations(&self.catalog)?;
        let favorites = catalog_src::favorite_stations(&self.catalog)?;
        self.state.load_stations(all, favorites);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_scopes_this_app_has_come_across_the_wire() {
        assert_eq!(scope_from_wire("all"), Some(Scope::All));
        assert_eq!(scope_from_wire("favorites"), Some(Scope::Favorites));
    }

    #[test]
    fn legacy_wire_scopes_still_map() {
        assert_eq!(scope_from_wire("ALL"), Some(Scope::All));
        assert_eq!(scope_from_wire("FAVS"), Some(Scope::Favorites));
    }

    #[test]
    fn a_scope_this_app_cannot_show_leaves_the_panel_alone() {
        // collapsing these into All would move the panel off favourites just
        // because another device opened its blocked list.
        assert_eq!(scope_from_wire("recent"), None);
        assert_eq!(scope_from_wire("blocked"), None);
        assert_eq!(scope_from_wire("dead"), None);
    }

    #[test]
    fn an_unknown_scope_leaves_the_panel_alone() {
        assert_eq!(scope_from_wire("something-new"), None);
        assert_eq!(scope_from_wire(""), None);
    }
}
