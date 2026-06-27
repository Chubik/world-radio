mod app;
mod catalog_src;
mod state;
mod theme;
mod tray;

use eframe::egui;

fn main() -> eframe::Result<()> {
    radio_core::single_instance::take_over();

    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([300.0, 200.0])
            .with_decorations(false)
            .with_taskbar(false)
            .with_always_on_top(),
        ..Default::default()
    };

    #[cfg(target_os = "macos")]
    {
        options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
            builder.with_activation_policy(ActivationPolicy::Accessory);
        }));
    }

    eframe::run_native(
        "World Radio Mini",
        options,
        Box::new(|_cc| {
            let app = app::MiniApp::new()
                .map_err(|e| Box::<dyn std::error::Error + Send + Sync>::from(e.to_string()))?;
            Ok(Box::new(app))
        }),
    )
}
