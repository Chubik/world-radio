use crate::state::{MiniState, Phase, Scope};
use crate::theme::Theme;
use eframe::egui;

pub struct MiniApp {
    state: MiniState,
    theme: Theme,
}

impl MiniApp {
    pub fn new() -> Self {
        Self {
            state: MiniState::new(),
            theme: Theme::amber(),
        }
    }
}

impl eframe::App for MiniApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
                    self.state.begin_play(crate::state::StationPick {
                        uuid: "demo".into(),
                        name: "Demo Station".into(),
                        url: "http://demo".into(),
                    });
                }
                let playing = self.state.phase == Phase::Playing
                    || self.state.phase == Phase::Buffering;
                if ui.button(if playing { "⏸ Stop" } else { "▶ Play" }).clicked() {
                    self.state.stop();
                }
            });

            ui.horizontal(|ui| {
                ui.label("vol");
                let mut v = self.state.volume;
                if ui.add(egui::Slider::new(&mut v, 0.0..=1.0).show_value(false)).changed() {
                    self.state.set_volume(v);
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
