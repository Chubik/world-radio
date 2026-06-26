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
    visible: bool,
    hidden_once: bool,
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

        let mut app = Self {
            state,
            theme: Theme::amber(),
            engine,
            tray: None,
            tray_ready: false,
            visible: false,
            hidden_once: false,
            catalog,
            fav_path,
            hist_path,
            blacklist_path,
        };
        app.play_last();
        Ok(app)
    }

    fn play_pick(&mut self, pick: crate::state::StationPick) {
        if let Some(engine) = &self.engine {
            engine.play(&pick.url);
        }
        self.catalog.record_history(&pick.uuid);
        if let Err(e) =
            self.catalog
                .save_state(&self.fav_path, &self.hist_path, &self.blacklist_path)
        {
            eprintln!("save history failed: {e}");
        }
        self.state.begin_play(pick);
    }

    fn shuffle(&mut self) {
        if let Some(pick) = self.state.pick_shuffle() {
            self.play_pick(pick);
        }
    }

    fn play_last(&mut self) {
        match catalog_src::last_played(&self.catalog) {
            Ok(Some(pick)) => self.play_pick(pick),
            Ok(None) => self.shuffle(),
            Err(e) => {
                eprintln!("load last station failed: {e}");
                self.shuffle();
            }
        }
    }

    fn stop(&mut self) {
        self.state.stop();
        if let Some(engine) = &self.engine {
            engine.stop();
        }
    }

    fn resume(&mut self) {
        match self.state.now.clone() {
            Some(pick) => self.play_pick(pick),
            None => self.shuffle(),
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

    fn handle_tray_clicks(&mut self, ctx: &egui::Context) {
        use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            let toggle = matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Down,
                    ..
                }
            );
            if toggle {
                self.visible = !self.visible;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(self.visible));
                if self.visible {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
            }
        }
    }

    fn set_volume(&mut self, v: f32) {
        self.state.set_volume(v);
        if let Some(engine) = &self.engine {
            engine.set_volume(self.state.volume);
        }
    }

    fn now_is_favorite(&self) -> bool {
        match &self.state.now {
            Some(pick) => self.catalog.is_favorite(&pick.uuid),
            None => false,
        }
    }

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
        if !self.hidden_once {
            self.hidden_once = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        self.ensure_tray();
        self.handle_tray_clicks(ctx);
        let focused = ctx.input(|i| i.focused);
        if self.visible && !focused {
            self.visible = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

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
        let phase = self.state.phase;
        let (dot_label, primary_label) = crate::state::state_labels(phase);

        let dot_color = match phase {
            Phase::Idle => t.dim,
            Phase::Buffering => t.warn,
            Phase::Playing => t.ok,
            Phase::Error => t.err,
        };

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("▌r4dio").color(t.hi).strong());
            ui.label(egui::RichText::new(dot_label).color(dot_color).small());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let meta = self.state.now.as_ref().map(|_| "live").unwrap_or("—");
                ui.label(egui::RichText::new(meta).color(t.dim).small());
            });
        });

        let station = self
            .state
            .now
            .as_ref()
            .map(|n| n.name.clone())
            .unwrap_or_else(|| "Nothing playing".into());
        let now_text = match phase {
            Phase::Idle => "press Shuffle to start listening",
            Phase::Buffering => "connecting to stream…",
            Phase::Playing => "now playing",
            Phase::Error => "stream offline — couldn't connect",
        };
        let now_color = match phase {
            Phase::Error => t.err,
            Phase::Buffering => t.warn,
            Phase::Idle => t.dim,
            Phase::Playing => t.accent,
        };
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                let name_color = match phase {
                    Phase::Idle => t.dim,
                    _ => t.bright,
                };
                ui.label(egui::RichText::new(station).color(name_color).strong());
                ui.label(egui::RichText::new(now_text).color(now_color).small());
            });
            if self.state.now.is_some() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    let star = match self.now_is_favorite() {
                        true => "★",
                        false => "☆",
                    };
                    if ui.button(egui::RichText::new(star).color(t.hi)).clicked() {
                        self.toggle_favorite();
                    }
                });
            }
        });

        ui.horizontal(|ui| {
            let bars = crate::state::spectrum_bars(16);
            let active = phase == Phase::Playing;
            let bar_color = match active {
                true => t.hi,
                false => t.dim,
            };
            let (rect, _) = ui.allocate_exact_size(egui::vec2(120.0, 16.0), egui::Sense::hover());
            let painter = ui.painter_at(rect);
            let bw = rect.width() / bars.len() as f32;
            for (i, &h) in bars.iter().enumerate() {
                let x = rect.left() + i as f32 * bw;
                let bar_h = (h * rect.height()).max(2.0);
                painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(x + 1.0, rect.bottom() - bar_h),
                        egui::pos2(x + bw - 1.0, rect.bottom()),
                    ),
                    0.0,
                    bar_color,
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new("VOL").color(t.dim).small());
                let mut v = self.state.volume;
                if ui
                    .add(egui::Slider::new(&mut v, 0.0..=1.0).show_value(false))
                    .changed()
                {
                    self.set_volume(v);
                }
            });
        });

        ui.horizontal(|ui| {
            if ui
                .button(
                    egui::RichText::new(format!("⇄ {primary_label}"))
                        .color(t.bg)
                        .strong(),
                )
                .clicked()
            {
                self.shuffle();
            }
            let playing = phase == Phase::Playing || phase == Phase::Buffering;
            let glyph = match playing {
                true => "⏸",
                false => "▶",
            };
            if ui.button(egui::RichText::new(glyph).color(t.fg)).clicked() {
                match playing {
                    true => self.stop(),
                    false => self.resume(),
                }
            }
        });

        ui.horizontal(|ui| {
            let scope_all = self.state.scope == Scope::All;
            if ui.selectable_label(scope_all, "ALL").clicked() {
                self.state.set_scope(Scope::All);
            }
            if ui.selectable_label(!scope_all, "★ FAVS").clicked() {
                self.state.set_scope(Scope::Favorites);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(egui::RichText::new("✕ quit").color(t.dim).small())
                    .clicked()
                {
                    std::process::exit(0);
                }
            });
        });
    }
}
