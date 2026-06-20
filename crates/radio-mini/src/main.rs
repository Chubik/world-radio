mod app;
mod catalog_src;
mod state;
mod theme;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([300.0, 180.0]),
        ..Default::default()
    };
    eframe::run_native(
        "World Radio Mini",
        options,
        Box::new(|_cc| Ok(Box::new(app::MiniApp::new()))),
    )
}
