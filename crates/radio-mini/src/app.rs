use crate::catalog_src;
use crate::state::{MiniState, Phase, Scope};
use crate::theme::Theme;
use eframe::egui;
use radio_audio::AudioEngine;
use radio_core::catalog::{Cache, Catalog, Health};

pub struct MiniApp {
    state: MiniState,
    theme: Theme,
    engine: Option<AudioEngine>,
}

impl MiniApp {
    pub fn new() -> anyhow::Result<Self> {
        let data = radio_core::paths::ensure_data_dir()?;
        let cache = Cache::open(&data.join("stations.db"))?;
        let health = Health::load(&data.join("station_health.json"));
        let catalog = Catalog::load(
            cache,
            health,
            &data.join("favorites.json"),
            &data.join("history.json"),
            &data.join("blacklist.json"),
        );

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

    fn set_volume(&mut self, v: f32) {
        self.state.set_volume(v);
        if let Some(engine) = &self.engine {
            engine.set_volume(self.state.volume);
        }
    }
}

impl eframe::App for MiniApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(engine) = &self.engine {
            while let Some(status) = engine.poll_status() {
                self.state.apply_status(status);
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }

        let t = self.theme;
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = t.bg;
        visuals.override_text_color = Some(t.fg);
        ctx.set_visuals(visuals);

        egui::CentralPanel::default().show(ctx, |ui| {
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
                    if playing {
                        self.stop();
                    } else {
                        self.shuffle();
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
        });
    }
}
