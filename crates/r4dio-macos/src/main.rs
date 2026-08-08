mod backend;
mod catalog_src;
mod commands;
mod state;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;
use tauri_plugin_positioner::{Position, WindowExt};

// clicking the tray icon while the popover holds key-window status makes AppKit
// fire our focus-loss hide *before* Tauri's tray click callback runs, so a plain
// visibility check sees "hidden" and re-opens the panel the click meant to close.
// treating a hide that just happened as "still visible" closes the race.
const REOPEN_GUARD: Duration = Duration::from_millis(200);

type LastHide = Arc<Mutex<Option<Instant>>>;

// the activation policy is a macos-only api, but ci runs clippy over the whole
// workspace on linux, so the calls are wrapped rather than sprinkled with cfgs.
#[cfg(target_os = "macos")]
fn set_accessory(app: &tauri::AppHandle) {
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
}

#[cfg(not(target_os = "macos"))]
fn set_accessory(_app: &tauri::AppHandle) {}

// an accessory app cannot become active, so a window shown while in that policy
// would open behind whatever the user is looking at.
#[cfg(target_os = "macos")]
fn set_regular(app: &tauri::AppHandle) {
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
}

#[cfg(not(target_os = "macos"))]
fn set_regular(_app: &tauri::AppHandle) {}

fn main() {
    radio_core::single_instance::take_over_named(radio_core::single_instance::MACOS_LOCK);

    let mut backend = backend::Backend::new().expect("failed to init backend");
    if radio_core::sync::load_key().is_some() {
        let _ = backend.sync();
    }
    backend.play_last();
    run(backend);
}

// the menubar is at the top of the screen, so the panel belongs below the icon.
// TrayCenter puts it *above* — `tray_y - window_height`, which is off-screen here.
// TrayBottomCenter lands on `tray_y`, i.e. covering the menubar and the icon
// itself, so the user cannot click the icon again to close it. drop it clear.
fn show_popover(app: &tauri::AppHandle) {
    let Some(win) = app.get_webview_window("popover") else {
        return;
    };
    let _ = win.move_window(Position::TrayBottomCenter);
    let _ = win.show();
    let _ = win.set_focus();
}

// hang the panel off the bottom edge of the icon's own rect. the positioner
// plugin only knows the icon's origin, so it parks the window over the menubar
// and the icon with it — leaving no icon left to click to close the panel.
fn drop_below_tray(win: &tauri::WebviewWindow, tray: &tauri::Rect) {
    let Ok(size) = win.outer_size() else {
        return;
    };
    let scale = win.scale_factor().unwrap_or(1.0);
    let pos = tray.position.to_physical::<i32>(scale);
    let tray_size = tray.size.to_physical::<i32>(scale);
    let x = pos.x + (tray_size.width - size.width as i32) / 2;
    let y = pos.y + tray_size.height;
    let _ = win.set_position(tauri::PhysicalPosition { x, y });
}

// an accessory app cannot become active, so a plain show() would leave this
// window behind whatever the user was looking at. going regular for as long as
// it is open buys it focus; closing it returns us to a dock-less menubar app.
fn show_sync(app: &tauri::AppHandle) {
    let Some(win) = app.get_webview_window("sync") else {
        return;
    };
    set_regular(app);
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
            set_accessory(app.handle());

            let shuffle = MenuItem::with_id(app, "shuffle", "Shuffle", true, None::<&str>)?;
            let playstop = MenuItem::with_id(app, "playstop", "Play / Stop", true, None::<&str>)?;
            let open = MenuItem::with_id(app, "open", "Open r4dio", true, None::<&str>)?;
            let sync = MenuItem::with_id(app, "sync", "Sync…", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit r4dio", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&shuffle, &playstop, &open, &sync, &quit])?;

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
                        "sync" => show_sync(app),
                        "quit" => app.exit(0),
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        rect,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        let last_hide = app.state::<LastHide>();
                        toggle_popover(app, &last_hide);
                        if let Some(win) = app.get_webview_window("popover") {
                            if win.is_visible().unwrap_or(false) {
                                drop_below_tray(&win, &rect);
                            }
                        }
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
            // closing the settings window must not quit the app and must not
            // leave a dock icon behind for a window that is gone.
            if let Some(win) = app.get_webview_window("sync") {
                let handle = win.clone();
                let app_handle = app.handle().clone();
                win.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = handle.hide();
                        set_accessory(&app_handle);
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
            commands::has_sync_key,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
