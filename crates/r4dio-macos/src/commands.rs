use crate::backend::Backend;
use crate::state::{Phase, Scope};
use serde::Serialize;
use std::sync::Mutex;

#[derive(Serialize)]
pub struct NowState {
    pub station: Option<String>,
    /// the uuid of what is playing. the window matches rows on this rather than
    /// on the name: two stations can share a name, and a row matched by name
    /// puts the cursor and the ▸ marker on the wrong one.
    pub uuid: Option<String>,
    pub track: String,
    pub phase: String,
    pub volume: f32,
    pub scope: String,
    pub is_favorite: bool,
    pub meta: String,
    /// the country filter this device listens under, already worded. it rides
    /// the state poll rather than a command of its own so it cannot lag a scope
    /// switch — the label is hidden in favourites, which the same poll carries.
    pub filter: String,
    /// seconds the current station has been up, for the "UP 14m" line.
    pub uptime: Option<i64>,
    /// decode buffer occupancy, 0.0-1.0, or None when nothing is playing.
    pub buffer: Option<f32>,
    /// the station shuffle will play next, already drawn.
    pub next: Option<String>,
    pub muted: bool,
    /// how many times the engine has retried without giving up — the design
    /// shows retrying apart from buffering.
    pub retries: u32,
    /// the station's first tag, which reads as its genre.
    pub genre: String,
    /// the meter's style, carried on the state poll so a change made on another
    /// device reaches the window without it asking a second question.
    pub eq_style: String,
    /// the stream url, for the info line. it is already public in the catalogue,
    /// so showing it reveals nothing the user could not look up.
    pub url: String,
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
        uuid: now.as_ref().map(|n| n.uuid.clone()),
        track: b.state.track.clone().unwrap_or_default(),
        phase: phase_str(b.phase()).to_string(),
        volume: b.state.volume,
        scope: scope_str(b.state.scope).to_string(),
        is_favorite: b.now_is_favorite(),
        meta: now
            .as_ref()
            .map(|n| crate::state::meta_label(&n.country, &n.codec, n.bitrate))
            .unwrap_or_default(),
        filter: crate::tray::filter_label(b.state.filter(), b.state.scope),
        uptime: b.uptime(),
        buffer: b.buffer_level(),
        next: b.queued_next().map(|p| p.name.clone()),
        muted: b.is_muted(),
        retries: b.state.retries,
        eq_style: b.eq_settings().0,
        genre: now
            .as_ref()
            .map(|n| crate::state::first_tag(&n.tags))
            .unwrap_or_default(),
        url: now.as_ref().map(|n| n.url.clone()).unwrap_or_default(),
    }
}

#[tauri::command]
/// 34 bands, which is what the window's meter draws — asking for fewer and
/// repeating them across the bars is what makes a meter look like wallpaper.
pub fn spectrum(state: tauri::State<Shared>) -> Vec<f32> {
    state.lock().unwrap().read_spectrum(34)
}

/// how the meter is drawn and how hard it is driven. per-machine, like volume.
#[derive(Serialize)]
pub struct EqSettings {
    pub style: String,
    pub gain: f32,
}

#[tauri::command]
pub fn toggle_mute(state: tauri::State<Shared>) {
    state.lock().unwrap().toggle_mute();
}

#[tauri::command]
pub fn retry(state: tauri::State<Shared>) {
    state.lock().unwrap().retry();
}

#[tauri::command]
pub fn eq_settings(state: tauri::State<Shared>) -> EqSettings {
    let (style, gain) = state.lock().unwrap().eq_settings();
    EqSettings { style, gain }
}

#[tauri::command]
pub fn set_eq(state: tauri::State<Shared>, style: String, gain: f32) {
    state.lock().unwrap().set_eq(style, gain);
}

#[derive(Serialize)]
pub struct StationRow {
    pub uuid: String,
    pub name: String,
    pub country: String,
    pub codec: String,
    pub bitrate: u32,
    pub is_playing: bool,
    /// the station's first tag, shown beside its name as a genre.
    pub genre: String,
    /// the user has played this and it failed enough times to be hidden from
    /// shuffle. the row still shows, marked, rather than vanishing.
    pub dead: bool,
}

#[tauri::command]
pub fn favourites(state: tauri::State<Shared>) -> Vec<StationRow> {
    state.lock().unwrap().favourite_rows()
}

/// a played station plus when it was played. the stamp is what the list shows
/// ("18 min ago"), and it is worded in the window rather than here so the
/// wording can change without a round trip.
#[derive(Serialize)]
pub struct HistoryRow {
    pub uuid: String,
    pub name: String,
    pub genre: String,
    pub dead: bool,
    pub country: String,
    pub codec: String,
    pub bitrate: u32,
    pub is_playing: bool,
    pub is_favorite: bool,
    pub played_at: i64,
}

#[tauri::command]
pub fn history(state: tauri::State<Shared>) -> Vec<HistoryRow> {
    state.lock().unwrap().history_rows()
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

// the window's browse query, one argument per thing the user can set. they are
// listed rather than grouped in a struct because tauri maps them straight from
// the invoke call, and a struct would put every one of them behind a nested
// object in the javascript.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn search(
    state: tauri::State<Shared>,
    name: String,
    genre: Option<String>,
    country: Option<String>,
    codec: Option<String>,
    // tauri maps the window's camelCase argument onto this snake_case name.
    bitrate_min: Option<u32>,
    sort: Option<String>,
    // how many rows to skip: the window asks for the next page as the user
    // reaches the bottom rather than drawing 50,000 rows at once.
    offset: Option<usize>,
) -> StationPage {
    state.lock().unwrap().search_filtered(
        &name,
        genre,
        country,
        codec,
        bitrate_min,
        sort.as_deref().unwrap_or("name"),
        offset.unwrap_or(0),
    )
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
    /// the countries shuffle is limited *to*, as opposed to `excluded`, which
    /// counts the ones it is kept *out of*. the two read alike and mean the
    /// opposite, so the window names this one rather than leaving the user to
    /// infer a narrowed pool from a count of the setting it is not.
    pub filter: String,
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

    // both windows read the label off a serialised field; a rename would leave
    // the row permanently blank rather than fail, so the wire name is pinned.
    #[test]
    fn both_windows_receive_the_filter_under_the_name_they_read() {
        let now = NowState {
            station: None,
            uuid: None,
            track: String::new(),
            phase: "idle".into(),
            volume: 0.8,
            scope: "all".into(),
            is_favorite: false,
            meta: String::new(),
            filter: crate::tray::filter_label(&["UA".to_string()], Scope::All),
            uptime: None,
            buffer: None,
            next: None,
            muted: false,
            retries: 0,
            genre: String::new(),
            eq_style: "bars".into(),
            url: String::new(),
        };
        let json = serde_json::to_value(&now).unwrap();
        assert_eq!(json["filter"], "FILTER: UA");

        let counts = FilterCounts {
            excluded: 0,
            blocked: 0,
            filter: crate::tray::filter_label(&["UA".to_string(), "PL".to_string()], Scope::All),
        };
        let json = serde_json::to_value(&counts).unwrap();
        assert_eq!(json["filter"], "FILTER: UA·PL");
    }
}
