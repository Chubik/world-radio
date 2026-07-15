use crate::backend::Backend;
use crate::state::{Phase, Scope};
use serde::Serialize;
use std::sync::Mutex;

fn parse_scope(s: &str) -> Scope {
    match s {
        "favorites" => Scope::Favorites,
        _ => Scope::All,
    }
}

#[derive(Serialize)]
pub struct NowState {
    pub station: Option<String>,
    pub track: String,
    pub phase: String,
    pub volume: f32,
    pub scope: String,
    pub is_favorite: bool,
    pub meta: String,
}

fn phase_str(phase: Phase) -> &'static str {
    match phase {
        Phase::Idle => "idle",
        Phase::Buffering => "buffering",
        Phase::Playing => "playing",
        Phase::Error => "error",
    }
}

fn scope_str(scope: Scope) -> &'static str {
    match scope {
        Scope::All => "all",
        Scope::Favorites => "favorites",
    }
}

pub type Shared = Mutex<Backend>;

#[tauri::command]
pub fn shuffle(state: tauri::State<Shared>) {
    state.lock().unwrap().shuffle();
}

#[tauri::command]
pub fn play_last(state: tauri::State<Shared>) {
    state.lock().unwrap().play_last();
}

#[tauri::command]
pub fn resume(state: tauri::State<Shared>) {
    state.lock().unwrap().resume();
}

#[tauri::command]
pub fn stop(state: tauri::State<Shared>) {
    state.lock().unwrap().stop();
}

#[tauri::command]
pub fn set_volume(state: tauri::State<Shared>, v: f32) {
    state.lock().unwrap().set_volume(v);
}

#[tauri::command]
pub fn set_scope(state: tauri::State<Shared>, scope: String) {
    state.lock().unwrap().set_scope(parse_scope(&scope));
}

#[tauri::command]
pub fn toggle_favorite(state: tauri::State<Shared>) {
    state.lock().unwrap().toggle_favorite();
}

#[tauri::command]
pub fn now_state(state: tauri::State<Shared>) -> NowState {
    let mut b = state.lock().unwrap();
    b.poll_engine();
    let now = b.state.now.clone();
    NowState {
        station: now.as_ref().map(|n| n.name.clone()),
        track: String::new(),
        phase: phase_str(b.phase()).to_string(),
        volume: b.state.volume,
        scope: scope_str(b.state.scope).to_string(),
        is_favorite: b.now_is_favorite(),
        meta: now.as_ref().map(|_| "live".to_string()).unwrap_or_default(),
    }
}

#[tauri::command]
pub fn spectrum(state: tauri::State<Shared>) -> Vec<f32> {
    state.lock().unwrap().read_spectrum(16)
}

#[tauri::command]
pub fn sync(state: tauri::State<Shared>) {
    let mut backend = state.lock().unwrap();
    if let Err(e) = backend.sync() {
        eprintln!("sync failed: {e}");
    }
}

#[tauri::command]
pub fn set_sync_key(key: String) -> bool {
    if !radio_core::sync::is_valid_format(&key) {
        return false;
    }
    radio_core::sync::store_key(&key).is_ok()
}

#[tauri::command]
pub fn clear_sync_key() {
    if let Err(e) = radio_core::sync::clear_key() {
        eprintln!("clear sync key failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_scope_maps_known_and_defaults_to_all() {
        assert_eq!(parse_scope("favorites"), Scope::Favorites);
        assert_eq!(parse_scope("all"), Scope::All);
        assert_eq!(parse_scope("garbage"), Scope::All);
    }
}
