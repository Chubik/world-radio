mod backend;
mod catalog_src;
mod commands;
mod state;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{ActivationPolicy, Manager};
use tauri_plugin_positioner::{Position, WindowExt};

// clicking the tray icon while the popover holds key-window status makes AppKit
// fire our focus-loss hide *before* Tauri's tray click callback runs, so a plain
// visibility check sees "hidden" and re-opens the panel the click meant to close.
// treating a hide that just happened as "still visible" closes the race.
const REOPEN_GUARD: Duration = Duration::from_millis(200);

type LastHide = Arc<Mutex<Option<Instant>>>;

fn main() {
    radio_core::single_instance::take_over();

    let mut backend = backend::Backend::new().expect("failed to init backend");
    if radio_core::sync::load_key().is_some() {
        let _ = backend.sync();
    }
    backend.play_last();
    run(backend);
}

fn show_popover(app: &tauri::AppHandle) {
    let Some(win) = app.get_webview_window("popover") else {
        return;
    };
    let _ = win.move_window(Position::TrayCenter);
    let _ = win.show();
    let _ = win.set_focus();
}

fn toggle_popover(app: &tauri::AppHandle, last_hide: &LastHide) {
    let Some(win) = app.get_webview_window("popover") else {
        return;
    };
    let recently_hidden = last_hide
        .lock()
        .unwrap()
        .is_some_and(|at| at.elapsed() < REOPEN_GUARD);
    match win.is_visible().unwrap_or(false) || recently_hidden {
        true => {
            let _ = win.hide();
        }
        false => show_popover(app),
    }
}

fn run(backend: backend::Backend) {
    let last_hide: LastHide = Arc::new(Mutex::new(None));

    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .manage(Mutex::new(backend))
        .manage(last_hide.clone())
        .setup(move |app| {
            // accessory: a menubar app has no dock icon and no menu bar of its own.
            app.set_activation_policy(ActivationPolicy::Accessory);

            let shuffle = MenuItem::with_id(app, "shuffle", "Shuffle", true, None::<&str>)?;
            let playstop = MenuItem::with_id(app, "playstop", "Play / Stop", true, None::<&str>)?;
            let open = MenuItem::with_id(app, "open", "Open r4dio", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit r4dio", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&shuffle, &playstop, &open, &quit])?;

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
                        "playstop" => {
                            let mut backend = state.lock().unwrap();
                            match backend.phase() {
                                state::Phase::Playing => backend.stop(),
                                _ => backend.resume(),
                            }
                        }
                        "open" => show_popover(app),
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
                        let last_hide = tray.app_handle().state::<LastHide>();
                        toggle_popover(tray.app_handle(), &last_hide);
                    }
                })
                .build(app)?;

            // clicking anywhere else must dismiss the panel, or an always-on-top
            // window sits over everything until the icon is clicked again.
            if let Some(win) = app.get_webview_window("popover") {
                let handle = win.clone();
                let last_hide = last_hide.clone();
                win.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        let _ = handle.hide();
                        *last_hide.lock().unwrap() = Some(Instant::now());
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
