mod app;
mod catalog_src;
mod state;
mod theme;
mod tray;

use eframe::egui;

const DETACH_MARKER: &str = "WR_MINI_DETACHED";

fn detach_to_background() {
    if std::env::var_os(DETACH_MARKER).is_some() {
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let spawned = std::process::Command::new(exe)
        .env(DETACH_MARKER, "1")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    match spawned {
        Ok(_) => std::process::exit(0),
        Err(e) => eprintln!("detach failed, running in foreground: {e}"),
    }
}

fn main() -> eframe::Result<()> {
    detach_to_background();

    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([300.0, 200.0])
            .with_decorations(false)
            .with_taskbar(false)
            .with_always_on_top()
            .with_visible(false),
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
