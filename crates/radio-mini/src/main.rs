mod backend;
mod catalog_src;
mod commands;
mod state;

use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{ActivationPolicy, Manager};
use tauri_plugin_positioner::{Position, WindowExt};

fn main() {
    radio_core::single_instance::take_over();

    let mut backend = backend::Backend::new().expect("failed to init backend");
    if radio_core::sync::load_key().is_some() {
        let _ = backend.sync();
    }
    backend.play_last();
    run(backend);
}

fn toggle_popover(app: &tauri::AppHandle) {
    let Some(win) = app.get_webview_window("popover") else {
        return;
    };
    match win.is_visible().unwrap_or(false) {
        true => {
            let _ = win.hide();
        }
        false => {
            let _ = win.move_window(Position::TrayCenter);
            let _ = win.show();
            let _ = win.set_focus();
        }
    }
}

fn run(backend: backend::Backend) {
    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .manage(Mutex::new(backend))
        .setup(|app| {
            // accessory: a menubar app has no dock icon and no menu bar of its own.
            app.set_activation_policy(ActivationPolicy::Accessory);

            let shuffle = MenuItem::with_id(app, "shuffle", "Shuffle", true, None::<&str>)?;
            let playstop = MenuItem::with_id(app, "playstop", "Play / Stop", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit r4dio", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&shuffle, &playstop, &quit])?;

            TrayIconBuilder::new()
                .icon(tauri::image::Image::from_bytes(include_bytes!(
                    "../icons/tray.png"
                ))?)
                .icon_as_template(true)
                .menu(&menu)
                // the menu is for right-click only; a left-click must reach our own
                // handler instead of opening it.
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    let state = app.state::<commands::Shared>();
                    match event.id().as_ref() {
                        "shuffle" => state.lock().unwrap().shuffle(),
                        "playstop" => state.lock().unwrap().stop(),
                        "quit" => app.exit(0),
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_popover(tray.app_handle());
                    }
                })
                .build(app)?;

            // clicking anywhere else must dismiss the panel, or an always-on-top
            // window sits over everything until the icon is clicked again.
            if let Some(win) = app.get_webview_window("popover") {
                let handle = win.clone();
                win.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        let _ = handle.hide();
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::shuffle,
            commands::play_last,
            commands::resume,
            commands::stop,
            commands::set_volume,
            commands::set_scope,
            commands::toggle_favorite,
            commands::now_state,
            commands::spectrum,
            commands::sync,
            commands::set_sync_key,
            commands::clear_sync_key,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
