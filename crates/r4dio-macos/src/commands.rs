use crate::backend::Backend;
use crate::state::{Phase, Scope};
use serde::Serialize;
use std::sync::Mutex;

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

// the panel prints which build is running. it never changes while the app is
// up, so it is fetched once rather than ridden along with the state poll.
#[tauri::command]
pub fn app_version() -> String {
    crate::tray::version_label(env!("CARGO_PKG_VERSION"))
}

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

/// an unrecognised value leaves the panel where it is rather than snapping it to
/// All — the window only ever sends the two it draws, so anything else is a bug
/// or a newer client, and neither is a reason to move the user off favourites.
#[tauri::command]
pub fn set_scope(state: tauri::State<Shared>, scope: String) {
    let Some(scope) = crate::backend::scope_from_wire(&scope) else {
        return;
    };
    state.lock().unwrap().set_scope(scope);
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
        meta: now
            .as_ref()
            .map(|n| crate::state::meta_label(&n.country, &n.codec, n.bitrate))
            .unwrap_or_default(),
    }
}

#[tauri::command]
pub fn spectrum(state: tauri::State<Shared>) -> Vec<f32> {
    state.lock().unwrap().read_spectrum(16)
}

#[derive(Serialize)]
pub struct StationRow {
    pub uuid: String,
    pub name: String,
    pub country: String,
    pub codec: String,
    pub bitrate: u32,
    pub is_playing: bool,
}

#[tauri::command]
pub fn favourites(state: tauri::State<Shared>) -> Vec<StationRow> {
    state.lock().unwrap().favourite_rows()
}

#[tauri::command]
pub fn play_uuid(state: tauri::State<Shared>, uuid: String) {
    state.lock().unwrap().play_uuid(&uuid);
}

#[tauri::command]
pub fn remove_favourite(state: tauri::State<Shared>, uuid: String) -> Vec<StationRow> {
    state.lock().unwrap().remove_favourite(&uuid)
}

#[tauri::command]
pub fn shuffle_favourites(state: tauri::State<Shared>) {
    state.lock().unwrap().shuffle_favourites();
}

#[tauri::command]
pub fn blocked(state: tauri::State<Shared>) -> Vec<StationRow> {
    state.lock().unwrap().blocked_rows()
}

#[tauri::command]
pub fn unblock(state: tauri::State<Shared>, uuid: String) -> Vec<StationRow> {
    state.lock().unwrap().unblock(&uuid)
}

#[derive(Serialize)]
pub struct CountryRow {
    pub code: String,
    pub count: u32,
    pub excluded: bool,
}

#[tauri::command]
pub fn countries(state: tauri::State<Shared>) -> Vec<CountryRow> {
    state.lock().unwrap().country_rows()
}

/// the whole list is sent back rather than one toggle, so a row the window never
/// drew cannot be dropped from the account by a partial update.
#[tauri::command]
pub fn set_excluded(state: tauri::State<Shared>, codes: Vec<String>) -> Vec<CountryRow> {
    state.lock().unwrap().set_excluded(codes)
}

/// `capped` is the whole point of the type: the window has to be able to say
/// "first 200 results" rather than presenting a cut list as the full answer.
#[derive(Serialize)]
pub struct StationPage {
    pub stations: Vec<StationRow>,
    pub capped: bool,
}

#[tauri::command]
pub fn search(state: tauri::State<Shared>, name: String) -> StationPage {
    state.lock().unwrap().search(&name)
}

#[tauri::command]
pub fn stations_in(state: tauri::State<Shared>, country: String) -> StationPage {
    state.lock().unwrap().stations_in(&country)
}

#[tauri::command]
pub fn add_favourite(state: tauri::State<Shared>, uuid: String) -> Vec<String> {
    state.lock().unwrap().add_favourite(&uuid)
}

#[tauri::command]
pub fn favourite_ids(state: tauri::State<Shared>) -> Vec<String> {
    state.lock().unwrap().favourite_ids()
}

#[derive(Serialize)]
pub struct FilterCounts {
    pub excluded: u32,
    pub blocked: u32,
}

#[tauri::command]
pub fn filter_counts(state: tauri::State<Shared>) -> FilterCounts {
    state.lock().unwrap().filter_counts()
}

#[tauri::command]
pub fn sync(state: tauri::State<Shared>) {
    let mut backend = state.lock().unwrap();
    if let Err(e) = backend.sync() {
        eprintln!("sync failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_sends_the_two_scopes_this_app_has() {
        assert_eq!(
            crate::backend::scope_from_wire("favorites"),
            Some(Scope::Favorites)
        );
        assert_eq!(crate::backend::scope_from_wire("all"), Some(Scope::All));
    }

    // a value only a newer client knows must leave the panel where it is; the
    // old parser turned every unknown string into All and moved the user off
    // favourites.
    #[test]
    fn an_unknown_scope_moves_nothing() {
        assert_eq!(crate::backend::scope_from_wire("garbage"), None);
        assert_eq!(crate::backend::scope_from_wire(""), None);
    }

    // recent/blocked/dead are real synced scopes with no panel equivalent, so
    // they must not collapse into All either.
    #[test]
    fn a_scope_the_panel_cannot_show_moves_nothing() {
        assert_eq!(crate::backend::scope_from_wire("recent"), None);
        assert_eq!(crate::backend::scope_from_wire("blocked"), None);
        assert_eq!(crate::backend::scope_from_wire("dead"), None);
    }
}
