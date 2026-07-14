mod backend;
mod catalog_src;
mod commands;
mod state;

use std::sync::Mutex;

fn main() {
    radio_core::single_instance::take_over();

    let mut backend = backend::Backend::new().expect("failed to init backend");
    if radio_core::sync::load_key().is_some() {
        let _ = backend.sync();
    }
    backend.play_last();
    run(backend);
}

fn run(backend: backend::Backend) {
    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .manage(Mutex::new(backend))
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
