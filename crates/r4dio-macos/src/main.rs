mod account;
mod backend;
mod catalog_src;
mod commands;
mod state;
mod tray;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};
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

// ⌥⇧R shuffles from inside any app, including another app's fullscreen space,
// where the panel cannot be drawn at all.
//
// no accessibility permission is involved: a plain key like R resolves to a
// carbon `RegisterEventHotKey` registration, which macos grants without a
// prompt. the crate only reaches for a `CGEventTap` — the api that *does* need
// permission — when the shortcut is a media key, which this one is not. so
// there is nothing to ask the user for, and nothing that can be declined.
//
// a failure here is deliberately not fatal: the shortcut may already be taken by
// another app, and losing one shortcut must not cost the user the tray, the
// panel or the window.
#[cfg(target_os = "macos")]
fn register_shuffle_hotkey(app: &tauri::AppHandle) {
    use tauri_plugin_global_shortcut::{
        Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
    };

    let shortcut = Shortcut::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyR);
    let plugin = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(move |app, pressed, event| {
            // the handler fires on press *and* release; acting on both would
            // change station twice per keystroke.
            if pressed != &shortcut || event.state() != ShortcutState::Pressed {
                return;
            }
            app.state::<commands::Shared>().lock().unwrap().shuffle();
        })
        .build();

    if let Err(e) = app.plugin(plugin) {
        eprintln!("global shortcut plugin failed to load: {e}");
        return;
    }
    if let Err(e) = app.global_shortcut().register(shortcut) {
        eprintln!("could not register the shuffle shortcut: {e}");
    }
}

#[cfg(not(target_os = "macos"))]
fn register_shuffle_hotkey(_app: &tauri::AppHandle) {}

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
fn show_main(app: &tauri::AppHandle, section: &str) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    set_regular(app);
    let _ = win.show();
    let _ = win.set_focus();
    // the window is reused rather than recreated, so the section it should open
    // on has to be pushed in; a fresh load would otherwise land on favourites.
    let _ = win.emit("show-section", section);
}

fn account_masked() -> String {
    radio_core::sync::load_key()
        .map(|key| account::mask_key(&key))
        .unwrap_or_default()
}

// the menu rows whose text depends on what is playing right now. they are held
// so the tray can rewrite them just before the menu is drawn.
struct LiveRows {
    playstop: MenuItem<tauri::Wry>,
    favorite: MenuItem<tauri::Wry>,
    account: MenuItem<tauri::Wry>,
}

impl LiveRows {
    fn refresh(&self, app: &tauri::AppHandle) {
        let state = app.state::<commands::Shared>();
        let (playing, is_favorite) = {
            let backend = state.lock().unwrap();
            (
                backend.phase() == state::Phase::Playing,
                backend.now_is_favorite(),
            )
        };
        let _ = self.playstop.set_text(tray::playstop_label(playing));
        let _ = self.favorite.set_text(tray::favorite_label(is_favorite));
        let _ = self
            .account
            .set_text(tray::account_label(&account_masked()));
    }
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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(Mutex::new(backend))
        .manage(last_hide.clone())
        .setup(move |app| {
            // accessory: a menubar app has no dock icon and no menu bar of its own.
            set_accessory(app.handle());
            register_shuffle_hotkey(app.handle());

            let shuffle = MenuItem::with_id(
                app,
                "shuffle",
                tray::SHUFFLE_ALL,
                true,
                Some(tray::SHUFFLE_ACCELERATOR),
            )?;
            let shuffle_favs = MenuItem::with_id(
                app,
                "shuffle_favs",
                tray::SHUFFLE_FAVOURITES,
                true,
                None::<&str>,
            )?;
            let playstop = MenuItem::with_id(
                app,
                "playstop",
                tray::playstop_label(false),
                true,
                None::<&str>,
            )?;
            let favorite = MenuItem::with_id(
                app,
                "favorite",
                tray::favorite_label(false),
                true,
                None::<&str>,
            )?;
            let open = MenuItem::with_id(app, "open", tray::OPEN, true, None::<&str>)?;
            let account_item = MenuItem::with_id(
                app,
                "account",
                tray::account_label(&account_masked()),
                true,
                None::<&str>,
            )?;
            let quit = MenuItem::with_id(app, "quit", tray::QUIT, true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[
                    &shuffle,
                    &shuffle_favs,
                    &PredefinedMenuItem::separator(app)?,
                    &playstop,
                    &favorite,
                    &PredefinedMenuItem::separator(app)?,
                    &open,
                    &account_item,
                    &PredefinedMenuItem::separator(app)?,
                    &quit,
                ],
            )?;

            // the three stateful rows are read straight before the menu is drawn,
            // so a station changed by the hotkey or the panel cannot leave the menu
            // offering "Play" for something already playing.
            let live_rows = LiveRows {
                playstop: playstop.clone(),
                favorite: favorite.clone(),
                account: account_item.clone(),
            };

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
                        "shuffle_favs" => state.lock().unwrap().shuffle_favourites(),
                        "playstop" => {
                            let mut backend = state.lock().unwrap();
                            match backend.phase() {
                                state::Phase::Playing => backend.stop(),
                                _ => backend.resume(),
                            }
                        }
                        "favorite" => state.lock().unwrap().toggle_favorite(),
                        "open" => show_main(app, "favourites"),
                        "account" => show_main(app, "sync"),
                        "update" => {
                            let app = app.clone();
                            tauri::async_runtime::spawn(async move {
                                let update = {
                                    let pending =
                                        app.state::<Mutex<Option<tauri_plugin_updater::Update>>>();
                                    let guard = pending.lock().unwrap();
                                    guard.clone()
                                };
                                let Some(update) = update else { return };
                                match update.download_and_install(|_, _| {}, || {}).await {
                                    Ok(()) => app.restart(),
                                    Err(e) => eprintln!("update install failed: {e}"),
                                }
                            });
                        }
                        "quit" => app.exit(0),
                        _ => {}
                    }
                })
                .on_tray_icon_event(move |tray, event| {
                    tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
                    // appkit opens the menu on the press, so the labels have to be
                    // rewritten on Down: a refresh on Up would land after the menu
                    // the user is already reading has been drawn.
                    if let TrayIconEvent::Click {
                        button: MouseButton::Right,
                        button_state: MouseButtonState::Down,
                        ..
                    } = event
                    {
                        live_rows.refresh(tray.app_handle());
                    }
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

            let update_item = MenuItem::with_id(app, "update", "", true, None::<&str>)?;
            app.manage(Mutex::new(None::<tauri_plugin_updater::Update>));
            {
                use tauri_plugin_updater::UpdaterExt;
                let handle = app.handle().clone();
                let menu_handle = menu.clone();
                let row = update_item.clone();
                tauri::async_runtime::spawn(async move {
                    let updater = match handle.updater() {
                        Ok(u) => u,
                        Err(e) => {
                            eprintln!("update check unavailable: {e}");
                            return;
                        }
                    };
                    match updater.check().await {
                        Ok(Some(update)) => {
                            let _ = row.set_text(tray::update_label(&update.version));
                            // insert above the final separator, next to the account row
                            let _ = menu_handle.insert(&row, 8);
                            let pending = handle.state::<Mutex<Option<tauri_plugin_updater::Update>>>();
                            *pending.lock().unwrap() = Some(update);
                        }
                        Ok(None) => {}
                        Err(e) => eprintln!("update check failed: {e}"),
                    }
                });
            }

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
            // closing the main window must not quit the app and must not leave a
            // dock icon behind for a window that is gone.
            if let Some(win) = app.get_webview_window("main") {
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
            commands::favourites,
            commands::play_uuid,
            commands::remove_favourite,
            commands::shuffle_favourites,
            commands::filter_counts,
            commands::blocked,
            commands::unblock,
            commands::countries,
            commands::set_excluded,
            commands::search,
            commands::stations_in,
            commands::add_favourite,
            commands::favourite_ids,
            account::create_account,
            account::account_state,
            account::sign_in,
            account::sign_out,
            account::delete_account,
            account::account_qr,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
