use crate::catalog_src;
use crate::state::{MiniState, Phase, Scope};
use crate::theme::Theme;
use crate::tray::Tray;
use eframe::egui;
use radio_audio::AudioEngine;
use radio_core::catalog::{Cache, Catalog, Health};
use std::path::PathBuf;

pub struct MiniApp {
    state: MiniState,
    theme: Theme,
    engine: Option<AudioEngine>,
    tray: Option<Tray>,
    tray_ready: bool,
    catalog: Catalog,
    fav_path: PathBuf,
    hist_path: PathBuf,
    blacklist_path: PathBuf,
}

impl MiniApp {
    pub fn new() -> anyhow::Result<Self> {
        let data = radio_core::paths::ensure_data_dir()?;
        let cache = Cache::open(&data.join("stations.db"))?;
        let health = Health::load(&data.join("station_health.json"));
        let fav_path = data.join("favorites.json");
        let hist_path = data.join("history.json");
        let blacklist_path = data.join("blacklist.json");
        let catalog = Catalog::load(cache, health, &fav_path, &hist_path, &blacklist_path);

        let all = catalog_src::all_stations(&catalog)?;
        let favorites = catalog_src::favorite_stations(&catalog)?;

        let mut state = MiniState::new();
        state.load_stations(all, favorites);

        let engine = AudioEngine::spawn().ok();
        if let Some(engine) = &engine {
            engine.set_volume(state.volume);
        }

        Ok(Self {
            state,
            theme: Theme::amber(),
            engine,
            tray: None,
            tray_ready: false,
            catalog,
            fav_path,
            hist_path,
            blacklist_path,
        })
    }

    fn shuffle(&mut self) {
        if let Some(pick) = self.state.shuffle() {
            if let Some(engine) = &self.engine {
                engine.play(&pick.url);
            }
        }
    }

    fn stop(&mut self) {
        self.state.stop();
        if let Some(engine) = &self.engine {
            engine.stop();
        }
    }

    fn toggle(&mut self) {
        let playing = self.state.phase == Phase::Playing || self.state.phase == Phase::Buffering;
        match playing {
            true => self.stop(),
            false => self.shuffle(),
        }
    }

    fn ensure_tray(&mut self) {
        if self.tray_ready {
            return;
        }
        self.tray_ready = true;
        self.tray = crate::tray::build()
            .map_err(|e| eprintln!("tray init failed: {e}"))
            .ok();
    }

    fn handle_tray_events(&mut self) {
        self.ensure_tray();
        let Some(tray) = &self.tray else {
            return;
        };
        let (shuffle_all, shuffle_fav, toggle, quit) = (
            tray.shuffle_all.clone(),
            tray.shuffle_fav.clone(),
            tray.toggle.clone(),
            tray.quit.clone(),
        );
        while let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
            match event.id {
                id if id == shuffle_all => {
                    self.state.set_scope(Scope::All);
                    self.shuffle();
                }
                id if id == shuffle_fav => {
                    self.state.set_scope(Scope::Favorites);
                    self.shuffle();
                }
                id if id == toggle => self.toggle(),
                id if id == quit => std::process::exit(0),
                _ => {}
            }
        }
    }

    fn set_volume(&mut self, v: f32) {
        self.state.set_volume(v);
        if let Some(engine) = &self.engine {
            engine.set_volume(self.state.volume);
        }
    }

    #[allow(dead_code)]
    fn now_is_favorite(&self) -> bool {
        match &self.state.now {
            Some(pick) => self.catalog.is_favorite(&pick.uuid),
            None => false,
        }
    }

    #[allow(dead_code)]
    fn toggle_favorite(&mut self) {
        let Some(pick) = self.state.now.clone() else {
            return;
        };
        match catalog_src::toggle_and_reload(&mut self.catalog, &pick.uuid) {
            Ok(favorites) => self.state.set_favorites(favorites),
            Err(e) => eprintln!("toggle favorite failed: {e}"),
        }
        if let Err(e) =
            self.catalog
                .save_state(&self.fav_path, &self.hist_path, &self.blacklist_path)
        {
            eprintln!("save favorites failed: {e}");
        }
    }
}

impl eframe::App for MiniApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_tray_events();

        if let Some(engine) = &self.engine {
            while let Some(status) = engine.poll_status() {
                self.state.apply_status(status);
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(250));

        let t = self.theme;
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = t.bg;
        visuals.override_text_color = Some(t.fg);
        ctx.set_visuals(visuals);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let t = self.theme;
        let title = self
            .state
            .now
            .as_ref()
            .map(|n| n.name.clone())
            .unwrap_or_else(|| "Nothing playing".into());
        ui.colored_label(t.hi, title);

        let status = match self.state.phase {
            Phase::Idle => "idle",
            Phase::Buffering => "connecting…",
            Phase::Playing => "live",
            Phase::Error => "offline",
        };
        ui.colored_label(t.dim, status);

        ui.horizontal(|ui| {
            if ui.button("⤮ Shuffle").clicked() {
                self.shuffle();
            }
            let playing =
                self.state.phase == Phase::Playing || self.state.phase == Phase::Buffering;
            if ui
                .button(if playing { "⏸ Stop" } else { "▶ Play" })
                .clicked()
            {
                match playing {
                    true => self.stop(),
                    false => self.shuffle(),
                }
            }
        });

        ui.horizontal(|ui| {
            ui.label("vol");
            let mut v = self.state.volume;
            if ui
                .add(egui::Slider::new(&mut v, 0.0..=1.0).show_value(false))
                .changed()
            {
                self.set_volume(v);
            }
        });

        let scope_all = self.state.scope == Scope::All;
        ui.horizontal(|ui| {
            if ui.selectable_label(scope_all, "all").clicked() {
                self.state.set_scope(Scope::All);
            }
            if ui.selectable_label(!scope_all, "favorites").clicked() {
                self.state.set_scope(Scope::Favorites);
            }
        });
    }
}
