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
        let catalog = Catalog::load(
            cache,
            health,
            &fav_path,
            &hist_path,
            &blacklist_path,
            &excluded_path,
        );

        let all = catalog_src::all_stations(&catalog)?;
        let favorites = catalog_src::favorite_stations(&catalog)?;

        let mut state = MiniState::new();
        state.load_stations(all, favorites);

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

    pub fn set_scope(&mut self, scope: Scope) {
        self.state.set_scope(scope);
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
        if let Err(e) = self.catalog.save_state(
            &self.fav_path,
            &self.hist_path,
            &self.blacklist_path,
            &self.excluded_path,
        ) {
            eprintln!("save favorites failed: {e}");
        }
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

    pub fn sync(&mut self) -> anyhow::Result<()> {
        let Some(key) = radio_core::sync::load_key() else {
            return Ok(());
        };
        let local = radio_core::sync::SyncData {
            favs: self.catalog.favorite_ids().to_vec(),
            blocked: self.catalog.blacklist_ids().to_vec(),
            excluded_countries: self.catalog.excluded_country_ids().to_vec(),
        };
        let client = radio_core::sync::SyncClient::new("https://r4dio.net");
        let merged = client.push(&key, &local)?;
        for uuid in &merged.favs {
            if !self.catalog.is_favorite(uuid) {
                self.catalog.toggle_favorite(uuid);
            }
        }
        for uuid in &merged.blocked {
            if !self.catalog.is_blacklisted(uuid) {
                self.catalog.toggle_blacklist(uuid);
            }
        }
        self.catalog
            .set_excluded_countries(merged.excluded_countries.clone());
        self.catalog.save_state(
            &self.fav_path,
            &self.hist_path,
            &self.blacklist_path,
            &self.excluded_path,
        )?;
        let all = catalog_src::all_stations(&self.catalog)?;
        let favorites = catalog_src::favorite_stations(&self.catalog)?;
        self.state.load_stations(all, favorites);
        Ok(())
    }
}
