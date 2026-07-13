use crate::tui::message::{Effect, Msg};
use crate::tui::model::{
    BrowseFocus, Model, NowPlaying, Overlay, RowState, StationRow, StatusFilter,
};
use radio_audio::Status;
use std::time::{Duration, Instant};

const VOLUME_STEP: f32 = 0.05;

pub fn update(model: &mut Model, msg: Msg) -> Vec<Effect> {
    match msg {
        Msg::Quit => {
            model.should_quit = true;
            vec![]
        }
        Msg::OpenSettings => {
            model.overlay = Overlay::Settings;
            vec![]
        }
        Msg::OpenHelp => {
            model.overlay = Overlay::Help;
            vec![]
        }
        Msg::OpenSyncOverlay => {
            model.overlay = Overlay::Sync;
            vec![]
        }
        Msg::CloseOverlay => {
            model.overlay = Overlay::None;
            vec![]
        }
        Msg::SettingsNav(down) => {
            settings_nav(model, down);
            vec![]
        }
        Msg::SettingsToggle => settings_toggle(model),
        Msg::KeybindNav(down) => {
            keybind_nav(model, down);
            vec![]
        }
        Msg::KeybindStartCapture => {
            model.keybind_capturing = true;
            model.keybind_warning = None;
            vec![]
        }
        Msg::CaptureKey(chord) => {
            capture_key(model, chord);
            vec![]
        }
        Msg::KeybindReset => {
            model.keymap.reset();
            model.keybind_warning = None;
            vec![]
        }
        Msg::SettingsAdjust(up) => {
            settings_adjust(model, up);
            vec![]
        }
        Msg::SelectNext => {
            select(model, true);
            vec![]
        }
        Msg::SelectPrev => {
            select(model, false);
            vec![]
        }
        Msg::SelectPageDown => {
            page(model, true);
            vec![]
        }
        Msg::SelectPageUp => {
            page(model, false);
            vec![]
        }
        Msg::EnterSearch => {
            model.browse.searching_input = true;
            vec![]
        }
        Msg::SearchChar(c) => {
            if model.browse.searching_input {
                model.browse.query.push(c);
                model.browse.pending_online_search = Some(Instant::now());
            }
            vec![]
        }
        Msg::SearchBackspace => {
            if model.browse.searching_input {
                model.browse.query.pop();
                model.browse.pending_online_search = Some(Instant::now());
            }
            vec![]
        }
        Msg::SearchClear => {
            if model.browse.searching_input {
                model.browse.query.clear();
                model.browse.pending_online_search = Some(Instant::now());
            }
            vec![]
        }
        Msg::SubmitSearch => {
            model.browse.searching_input = false;
            model.browse.loading = true;
            model.browse.last_error = None;
            let q = model.browse.filters.to_query(&model.browse.query);
            vec![Effect::Search(q, model.browse.filters.clone())]
        }
        Msg::SearchResults(rows) => {
            let playing = model.now.uuid.clone();
            let keep = model.browse.selected_row().map(|r| r.uuid.clone());
            model.browse.set_rows(rows);
            model.browse.selected =
                reselect(&model.browse.rows, playing.as_deref().or(keep.as_deref()));
            model.browse.loading = false;
            vec![]
        }
        Msg::AutoplayStation(row) => play_row(model, row),
        Msg::CatalogSynced { count } => {
            model.catalog_count = Some(count);
            model.catalog_loading = false;
            model.browse.pending_online_search = Some(Instant::now());
            autoplay_random_if_pending(model)
        }
        Msg::CatalogSyncFailed => {
            model.catalog_loading = false;
            vec![]
        }
        Msg::QuickTopReady { count } => {
            if model.catalog_count.is_none() {
                model.catalog_count = Some(count);
            }
            model.catalog_loading = false;
            autoplay_random_if_pending(model)
        }
        Msg::SearchFailed(e) => {
            model.browse.loading = false;
            model.browse.last_error = Some(e);
            vec![]
        }
        Msg::SetOffline(offline) => {
            model.browse.offline = offline;
            vec![]
        }
        Msg::PlaySelected => play_selected(model),
        Msg::Shuffle => shuffle_play(model),
        Msg::CycleSort => {
            model.browse.cycle_sort();
            vec![]
        }
        Msg::ToggleHideUnplayable => toggle_hide_unplayable(model),
        Msg::Stop => {
            vec![Effect::StopAudio]
        }
        Msg::SyncNow => {
            model.notice = Some("syncing…".to_string());
            vec![Effect::Sync]
        }
        Msg::SyncCreate => vec![Effect::SyncCreate],
        Msg::SyncLogout => vec![Effect::SyncLogout],
        Msg::SyncDelete => vec![Effect::SyncDelete],
        Msg::SyncCopy => {
            match &model.sync_key {
                None => model.notice = Some("no key to copy".to_string()),
                Some(key) => {
                    copy_osc52(key);
                    model.notice = Some("copied".to_string());
                }
            }
            vec![]
        }
        Msg::SyncKeyChanged(opt) => {
            model.sync_key = opt;
            model.mirror_seq = 0;
            vec![]
        }
        Msg::Notice(text) => {
            model.notice = Some(text);
            vec![]
        }
        Msg::ToggleFavoriteSelected => toggle_favorite_selected(model),
        Msg::BlacklistSelected => blacklist_selected(model),
        Msg::RecheckSelected => recheck_selected(model),
        Msg::VolumeUp => {
            model.volume = (model.volume + VOLUME_STEP).clamp(0.0, 1.0);
            vec![Effect::SetVolume(model.volume)]
        }
        Msg::VolumeDown => {
            model.volume = (model.volume - VOLUME_STEP).clamp(0.0, 1.0);
            vec![Effect::SetVolume(model.volume)]
        }
        Msg::AudioStatus(s) => audio_status(model, s),
        Msg::FocusToggle => focus_toggle(model),
        Msg::FilterNavNext => filter_nav(model, true),
        Msg::FilterNavPrev => filter_nav(model, false),
        Msg::FilterOptionNext => filter_option_nav(model, true),
        Msg::FilterOptionPrev => filter_option_nav(model, false),
        Msg::FilterApply => filter_apply(model),
        Msg::FilterClear => filter_clear(model, false),
        Msg::FilterClearAll => filter_clear(model, true),
        Msg::FacetsLoaded(f) => {
            model.browse.facets = f;
            model.browse.facets_loading = false;
            vec![]
        }
        Msg::Tick(now) => tick(model, now),
        Msg::MirrorPlay(evt) => mirror_play(model, evt),
        Msg::UpdateAvailable(rel) => {
            model.pending_update = Some(rel);
            vec![]
        }
        Msg::UpdateNow => match model.pending_update.clone() {
            None => vec![],
            Some(rel) => {
                model.notice = Some(format!("updating to v{}…", rel.version));
                vec![Effect::Update(rel)]
            }
        },
    }
}

const AUTO_SKIP_MAX: u32 = 5;

fn audio_status(model: &mut Model, s: Status) -> Vec<Effect> {
    match &s {
        Status::Playing { title, .. } => {
            model.now.title = title.clone();
            model.auto_skip_count = 0;
            model.status = s;
            vec![]
        }
        Status::Error(_) => {
            model.now.title = None;
            model.status = s;
            auto_skip(model)
        }
        _ => {
            model.now.title = None;
            model.status = s;
            vec![]
        }
    }
}

fn auto_skip(model: &mut Model) -> Vec<Effect> {
    let mut effects = Vec::new();
    if let Some(uuid) = model.now.uuid.clone() {
        model
            .browse
            .update_row(&uuid, |r| r.state = RowState::Disabled);
        effects.push(Effect::MarkFailed(uuid));
    }
    if model.auto_skip_count >= AUTO_SKIP_MAX {
        return effects;
    }
    let Some(next) = model.browse.next_playable_below() else {
        return effects;
    };
    model.browse.selected = next;
    model.auto_skip_count += 1;
    let row = model.browse.rows[next].clone();
    effects.extend(play_row(model, row));
    effects
}

fn reselect(rows: &[StationRow], keep: Option<&str>) -> usize {
    match keep {
        Some(uuid) => rows.iter().position(|r| r.uuid == uuid).unwrap_or(0),
        None => 0,
    }
}

fn select(model: &mut Model, next: bool) {
    match next {
        true => model.browse.select_next(),
        false => model.browse.select_prev(),
    }
}

const PAGE_SIZE: usize = 10;

fn page(model: &mut Model, down: bool) {
    match down {
        true => model.browse.page_down(PAGE_SIZE),
        false => model.browse.page_up(PAGE_SIZE),
    }
}

fn current_row(model: &Model) -> Option<StationRow> {
    model.browse.selected_row().cloned()
}

fn play_selected(model: &mut Model) -> Vec<Effect> {
    model.auto_skip_count = 0;
    match current_row(model) {
        None => vec![],
        Some(row) => play_row(model, row),
    }
}

fn playable_indices(model: &Model) -> Vec<usize> {
    model
        .browse
        .rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.state != RowState::Disabled)
        .map(|(i, _)| i)
        .collect()
}

fn toggle_hide_unplayable(model: &mut Model) -> Vec<Effect> {
    model.browse.filters.hide_unplayable = !model.browse.filters.hide_unplayable;
    emit_search(model)
}

fn shuffle_play(model: &mut Model) -> Vec<Effect> {
    let candidates = playable_indices(model);
    if candidates.is_empty() {
        return vec![];
    }
    let pick = candidates[fastrand::usize(..candidates.len())];
    model.browse.selected = pick;
    play_selected(model)
}

fn autoplay_random_if_pending(model: &mut Model) -> Vec<Effect> {
    if !model.autoplay_first_pending || model.is_playing() {
        return vec![];
    }
    if model.browse.rows.is_empty() {
        return vec![];
    }
    let idx = fastrand::usize(..model.browse.rows.len());
    let row = model.browse.rows[idx].clone();
    model.autoplay_first_pending = false;
    play_row(model, row)
}

fn play_row(model: &mut Model, row: StationRow) -> Vec<Effect> {
    let effects = vec![
        Effect::Play(row.url.clone()),
        Effect::RecordHistory(row.uuid.clone()),
        Effect::MirrorAnnounce {
            uuid: row.uuid.clone(),
            name: row.name.clone(),
            url: row.url.clone(),
        },
        Effect::SaveState,
    ];
    model.now = NowPlaying {
        station_name: Some(row.name),
        country: row.country,
        bitrate: row.bitrate,
        codec: row.codec,
        url: Some(row.url),
        uuid: Some(row.uuid),
        title: None,
    };
    effects
}

fn mirror_play(model: &mut Model, evt: radio_core::mirror::MirrorEvent) -> Vec<Effect> {
    if evt.origin == radio_core::mirror::device_id() {
        return vec![];
    }
    if evt.seq <= model.mirror_seq {
        return vec![];
    }
    model.mirror_seq = evt.seq;
    if radio_core::catalog::text_is_excluded(&format!("{} {}", evt.name, evt.url)) {
        return vec![];
    }
    let playing = matches!(model.status, Status::Playing { .. });
    match playing {
        true => {
            model.now = NowPlaying {
                station_name: Some(evt.name),
                country: String::new(),
                bitrate: 0,
                codec: String::new(),
                url: Some(evt.url.clone()),
                uuid: Some(evt.uuid),
                title: None,
            };
            vec![Effect::Play(evt.url)]
        }
        false => {
            model.notice = Some(format!("mirror: {}", evt.name));
            vec![]
        }
    }
}

fn toggle_favorite_selected(model: &mut Model) -> Vec<Effect> {
    let uuid = match model.browse.selected_row().map(|r| r.uuid.clone()) {
        None => return vec![],
        Some(uuid) => uuid,
    };
    model.browse.update_row(&uuid, |r| r.favorite = !r.favorite);
    vec![Effect::ToggleFavorite(uuid), Effect::SaveState]
}

const SETTINGS_THEME_ROW: usize = 0;
const SETTINGS_GLYPH_ROW: usize = 1;
const SETTINGS_CROSSFADE_ROW: usize = 2;
const SETTINGS_SPECTRUM_ROW: usize = 3;
const SETTINGS_KEYBIND_ROW: usize = 4;
const SETTINGS_DIVISOR_ROW: usize = 5;
const SETTINGS_ROW_COUNT: usize = 6;

fn settings_nav(model: &mut Model, down: bool) {
    model.settings_cursor = match down {
        true => (model.settings_cursor + 1) % SETTINGS_ROW_COUNT,
        false => (model.settings_cursor + SETTINGS_ROW_COUNT - 1) % SETTINGS_ROW_COUNT,
    };
}

fn keybind_nav(model: &mut Model, down: bool) {
    let n = crate::tui::keybind::Action::ALL.len();
    model.keybind_cursor = match down {
        true => (model.keybind_cursor + 1) % n,
        false => (model.keybind_cursor + n - 1) % n,
    };
}

fn capture_key(model: &mut Model, chord: crate::tui::keybind::KeyChord) {
    use crate::tui::keybind::{Action, KeyName};
    model.keybind_capturing = false;
    if chord.key == KeyName::Esc {
        return;
    }
    let action = Action::ALL[model.keybind_cursor.min(Action::ALL.len() - 1)];
    match model.keymap.conflict(chord, action) {
        Some(other) => {
            model.keybind_warning = Some(format!("already bound to {}", other.label()));
        }
        None => {
            model.keymap.set(action, chord);
            model.keybind_warning = None;
        }
    }
}

fn settings_toggle(model: &mut Model) -> Vec<Effect> {
    match model.settings_cursor {
        SETTINGS_THEME_ROW => model.theme = model.theme.next(),
        SETTINGS_GLYPH_ROW => model.glyphs.emoji_flags = !model.glyphs.emoji_flags,
        SETTINGS_CROSSFADE_ROW => {
            model.crossfade = !model.crossfade;
            return vec![Effect::SetCrossfade(model.crossfade)];
        }
        SETTINGS_SPECTRUM_ROW => model.spectrum_style = model.spectrum_style.next(),
        SETTINGS_KEYBIND_ROW => {
            model.overlay = crate::tui::model::Overlay::Keybindings;
            model.keybind_cursor = 0;
            model.keybind_capturing = false;
            model.keybind_warning = None;
        }
        _ => {}
    }
    vec![]
}

fn settings_adjust(model: &mut Model, up: bool) {
    if model.settings_cursor == SETTINGS_DIVISOR_ROW {
        let step = if up { 1.0 } else { -1.0 };
        model.fft_divisor = (model.fft_divisor + step).clamp(2.0, 24.0);
    }
}

fn blacklist_selected(model: &mut Model) -> Vec<Effect> {
    let uuid = match model.browse.selected_row().map(|r| r.uuid.clone()) {
        Some(uuid) => uuid,
        None => return vec![],
    };
    model
        .browse
        .update_row(&uuid, |r| r.state = RowState::Disabled);
    vec![Effect::Blacklist(uuid), Effect::SaveState]
}

fn recheck_selected(model: &mut Model) -> Vec<Effect> {
    let effect = match model.browse.selected_row().map(|r| r.uuid.clone()) {
        Some(uuid) => Effect::Recheck(uuid),
        None => Effect::RecheckAll,
    };
    let q = model.browse.filters.to_query(&model.browse.query);
    vec![
        effect,
        Effect::SaveState,
        Effect::Search(q, model.browse.filters.clone()),
    ]
}

const DEBOUNCE_MS: u64 = 500;

fn focus_toggle(model: &mut Model) -> Vec<Effect> {
    model.browse.focus = match model.browse.focus {
        BrowseFocus::Stations => BrowseFocus::Filters {
            group: 0,
            option: 0,
        },
        BrowseFocus::Filters { .. } => BrowseFocus::Stations,
    };
    vec![]
}

fn filter_nav(model: &mut Model, next: bool) -> Vec<Effect> {
    if let BrowseFocus::Filters { group, .. } = model.browse.focus {
        let g = match next {
            true => (group + 1) % 5,
            false => (group + 4) % 5,
        };
        model.browse.focus = BrowseFocus::Filters {
            group: g,
            option: 0,
        };
    }
    vec![]
}

fn filter_option_nav(model: &mut Model, next: bool) -> Vec<Effect> {
    if let BrowseFocus::Filters { group, option } = model.browse.focus {
        let max = group_option_count(model, group);
        let new_option = match next {
            true => (option + 1).min(max.saturating_sub(1)),
            false => option.saturating_sub(1),
        };
        model.browse.focus = BrowseFocus::Filters {
            group,
            option: new_option,
        };
    }
    vec![]
}

fn group_option_count(model: &Model, group: usize) -> usize {
    match group {
        0 => 4,
        1 => 1 + model.browse.facets.countries.len(),
        2 => 1 + model.browse.facets.tags.len(),
        3 => 1 + model.browse.facets.codecs.len(),
        4 => 4,
        _ => 1,
    }
}

fn filter_apply(model: &mut Model) -> Vec<Effect> {
    if let BrowseFocus::Filters { group, option } = model.browse.focus {
        apply_option(model, group, option);
        model.browse.pending_online_search = Some(Instant::now());
        let q = model.browse.filters.to_query(&model.browse.query);
        return vec![Effect::Search(q, model.browse.filters.clone())];
    }
    vec![]
}

fn apply_option(model: &mut Model, group: usize, option: usize) {
    if group == 0 {
        model.browse.filters.status = match option {
            1 => StatusFilter::Favorites,
            2 => StatusFilter::Recent,
            3 => StatusFilter::Blocked,
            4 => StatusFilter::Dead,
            _ => StatusFilter::All,
        };
        return;
    }
    if option == 0 {
        model.browse.filters.clear_group(group);
        return;
    }
    let idx = option - 1;
    match group {
        1 => {
            if let Some((c, _)) = model.browse.facets.countries.get(idx) {
                model.browse.filters.toggle(1, c.clone());
            }
        }
        2 => {
            if let Some((t, _)) = model.browse.facets.tags.get(idx) {
                model.browse.filters.toggle(2, t.clone());
            }
        }
        3 => {
            if let Some((c, _)) = model.browse.facets.codecs.get(idx) {
                model.browse.filters.toggle(3, c.clone());
            }
        }
        4 => {
            let v = match idx {
                0 => Some(128),
                1 => Some(256),
                2 => Some(320),
                _ => None,
            };
            model.browse.filters.bitrate_min = v;
        }
        _ => {}
    }
}

fn filter_clear(model: &mut Model, all: bool) -> Vec<Effect> {
    match all {
        true => model.browse.filters.clear(),
        false => {
            if let BrowseFocus::Filters { group, .. } = model.browse.focus {
                model.browse.filters.clear_group(group);
            }
        }
    }
    model.browse.pending_online_search = Some(Instant::now());
    let q = model.browse.filters.to_query(&model.browse.query);
    vec![Effect::Search(q, model.browse.filters.clone())]
}

fn emit_search(model: &mut Model) -> Vec<Effect> {
    model.browse.loading = true;
    let q = model.browse.filters.to_query(&model.browse.query);
    vec![Effect::Search(q, model.browse.filters.clone())]
}

fn copy_osc52(text: &str) {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
    print!("\x1b]52;c;{b64}\x07");
    use std::io::Write;
    let _ = std::io::stdout().flush();
}

fn tick(model: &mut Model, now: Instant) -> Vec<Effect> {
    model.spinner = model.spinner.wrapping_add(1);
    let deadline = match model.browse.pending_online_search {
        None => return vec![],
        Some(t) => t,
    };
    if now.duration_since(deadline) < Duration::from_millis(DEBOUNCE_MS) {
        return vec![];
    }
    model.browse.pending_online_search = None;
    emit_search(model)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::model::{Model, Overlay, RowState, StationRow, StatusFilter};
    use crate::tui::theme::{ColorTier, Glyphs, Theme};

    fn model() -> Model {
        Model::new(Theme::AmberCrt, ColorTier::Truecolor, Glyphs::unicode())
    }

    fn row(uuid: &str) -> StationRow {
        StationRow {
            uuid: uuid.into(),
            name: uuid.into(),
            url: format!("http://{uuid}"),
            country: "GB".into(),
            tags: String::new(),
            bitrate: 128,
            codec: "MP3".into(),
            favorite: false,
            state: RowState::Normal,
        }
    }

    #[test]
    fn quit_sets_should_quit() {
        let mut m = model();
        let fx = update(&mut m, Msg::Quit);
        assert!(m.should_quit);
        assert!(fx.is_empty());
    }

    #[test]
    fn submit_search_emits_search_effect_with_query() {
        let mut m = model();
        m.browse.query = "jazz".into();
        let fx = update(&mut m, Msg::SubmitSearch);
        assert!(m.browse.loading);
        assert!(
            matches!(fx.as_slice(), [Effect::Search(q, _)] if q.name.as_deref() == Some("jazz"))
        );
    }

    #[test]
    fn submit_search_carries_filters() {
        let mut m = model();
        m.browse.query = "jazz".into();
        m.browse.filters.status = StatusFilter::Favorites;
        let fx = update(&mut m, Msg::SubmitSearch);
        let ok = fx.iter().any(|e| {
            matches!(
                e,
                Effect::Search(q, f) if q.name.as_deref() == Some("jazz") && f.status == StatusFilter::Favorites
            )
        });
        assert!(ok);
    }

    #[test]
    fn search_results_populate_rows_and_clear_loading() {
        let mut m = model();
        m.browse.loading = true;
        update(&mut m, Msg::SearchResults(vec![row("u1"), row("u2")]));
        assert_eq!(m.browse.rows.len(), 2);
        assert!(!m.browse.loading);
        assert_eq!(m.browse.selected, 0);
    }

    #[test]
    fn typing_in_search_schedules_debounced_search() {
        let mut m = model();
        m.browse.searching_input = true;
        update(&mut m, Msg::SearchChar('8'));
        assert_eq!(m.browse.query, "8");
        assert!(m.browse.pending_online_search.is_some());
    }

    #[test]
    fn backspace_in_search_schedules_debounced_search() {
        let mut m = model();
        m.browse.searching_input = true;
        m.browse.query = "80".into();
        update(&mut m, Msg::SearchBackspace);
        assert_eq!(m.browse.query, "8");
        assert!(m.browse.pending_online_search.is_some());
    }

    #[test]
    fn search_clear_empties_query_and_schedules_search() {
        let mut m = model();
        m.browse.searching_input = true;
        m.browse.query = "80s dance".into();
        update(&mut m, Msg::SearchClear);
        assert!(m.browse.query.is_empty());
        assert!(m.browse.pending_online_search.is_some());
    }

    #[test]
    fn search_results_move_cursor_to_playing_station() {
        let mut m = model();
        m.now.uuid = Some("u3".into());
        let _ = update(
            &mut m,
            Msg::SearchResults(vec![row("u1"), row("u2"), row("u3")]),
        );
        assert_eq!(m.browse.selected, 2);
    }

    #[test]
    fn set_offline_toggles_flag() {
        let mut m = model();
        assert!(!m.browse.offline);
        update(&mut m, Msg::SetOffline(true));
        assert!(m.browse.offline);
        update(&mut m, Msg::SetOffline(false));
        assert!(!m.browse.offline);
    }

    #[test]
    fn search_results_keep_selected_station_when_still_present() {
        let mut m = model();
        m.browse.rows = vec![row("u1"), row("u2"), row("u3")];
        m.browse.selected = 2;
        update(
            &mut m,
            Msg::SearchResults(vec![row("u0"), row("u3"), row("u9")]),
        );
        assert_eq!(m.browse.selected, 1);
    }

    #[test]
    fn search_results_reset_to_top_when_selected_gone() {
        let mut m = model();
        m.browse.rows = vec![row("u1"), row("u2")];
        m.browse.selected = 1;
        update(&mut m, Msg::SearchResults(vec![row("u8"), row("u9")]));
        assert_eq!(m.browse.selected, 0);
    }

    #[test]
    fn play_selected_emits_play_and_record_history_and_updates_now() {
        let mut m = model();
        m.browse.rows = vec![row("u1")];
        let fx = update(&mut m, Msg::PlaySelected);
        assert_eq!(m.now.uuid.as_deref(), Some("u1"));
        let kinds: Vec<_> = fx.iter().map(eff_kind).collect();
        assert!(kinds.contains(&"play"));
        assert!(kinds.contains(&"history"));
        assert_eq!(m.now.station_name.as_deref(), Some("u1"));
        assert!(kinds.contains(&"savestate"));
    }

    #[test]
    fn toggle_favorite_from_browse_emits_toggle_and_savestate_no_loadfav() {
        let mut m = model();
        m.browse.rows = vec![row("u1")];
        let fx = update(&mut m, Msg::ToggleFavoriteSelected);
        let kinds: Vec<_> = fx.iter().map(eff_kind).collect();
        assert!(kinds.contains(&"toggle"));
        assert!(kinds.contains(&"savestate"));
        assert!(!kinds.contains(&"loadfav"));
        assert!(m.browse.rows[0].favorite);
    }

    #[test]
    fn volume_up_clamps_at_one_and_emits_set_volume() {
        let mut m = model();
        m.volume = 0.98;
        let fx = update(&mut m, Msg::VolumeUp);
        assert!((m.volume - 1.0).abs() < 1e-6);
        assert!(matches!(fx.as_slice(), [Effect::SetVolume(v)] if (*v - 1.0).abs() < 1e-6));
    }

    #[test]
    fn audio_status_playing_sets_status() {
        let mut m = model();
        update(
            &mut m,
            Msg::AudioStatus(Status::Playing {
                sample_rate: 44100,
                channels: 2,
                title: None,
            }),
        );
        assert!(m.is_playing());
    }

    #[test]
    fn audio_status_playing_with_title_updates_now_title() {
        let mut m = model();
        update(
            &mut m,
            Msg::AudioStatus(Status::Playing {
                sample_rate: 44100,
                channels: 2,
                title: Some("Smooth Jazz \u{2013} Sax".into()),
            }),
        );
        assert_eq!(m.now.title.as_deref(), Some("Smooth Jazz \u{2013} Sax"));
    }

    #[test]
    fn audio_status_non_playing_clears_now_title() {
        let mut m = model();
        m.now.title = Some("Old Track".into());
        update(&mut m, Msg::AudioStatus(Status::Retrying(1)));
        assert!(m.now.title.is_none());
    }

    #[test]
    fn search_char_only_appends_when_in_search_input() {
        let mut m = model();
        update(&mut m, Msg::EnterSearch);
        update(&mut m, Msg::SearchChar('j'));
        update(&mut m, Msg::SearchChar('z'));
        assert_eq!(m.browse.query, "jz");
    }

    use crate::tui::model::BrowseFocus;
    use radio_core::catalog::Facets;
    use std::time::{Duration, Instant};

    #[test]
    fn focus_toggle_round_trip() {
        let mut m = model();
        update(&mut m, Msg::FocusToggle);
        assert!(matches!(
            m.browse.focus,
            BrowseFocus::Filters {
                group: 0,
                option: 0
            }
        ));
        update(&mut m, Msg::FocusToggle);
        assert_eq!(m.browse.focus, BrowseFocus::Stations);
    }

    #[test]
    fn filter_apply_emits_search_with_country_set() {
        let mut m = model();
        m.browse.focus = BrowseFocus::Filters {
            group: 1,
            option: 1,
        };
        m.browse.facets.countries = vec![("GB".into(), 47)];
        let fx = update(&mut m, Msg::FilterApply);
        assert_eq!(m.browse.filters.countries, vec!["GB".to_string()]);
        assert!(
            matches!(fx.as_slice(), [Effect::Search(q, _)] if q.countrycode.as_deref() == Some("GB"))
        );
        assert!(m.browse.pending_online_search.is_some());
    }

    #[test]
    fn filter_apply_option_zero_clears_group() {
        let mut m = model();
        m.browse.filters.countries = vec!["GB".into()];
        m.browse.focus = BrowseFocus::Filters {
            group: 1,
            option: 0,
        };
        m.browse.facets.countries = vec![("GB".into(), 47)];
        update(&mut m, Msg::FilterApply);
        assert!(m.browse.filters.countries.is_empty());
    }

    #[test]
    fn filter_apply_toggles_second_country() {
        let mut m = model();
        m.browse.filters.countries = vec!["GB".into()];
        m.browse.facets.countries = vec![("GB".into(), 47), ("DE".into(), 30)];
        m.browse.focus = BrowseFocus::Filters {
            group: 1,
            option: 2,
        };
        update(&mut m, Msg::FilterApply);
        assert_eq!(
            m.browse.filters.countries,
            vec!["GB".to_string(), "DE".to_string()]
        );
    }

    #[test]
    fn filter_clear_resets_current_group_and_emits_search() {
        let mut m = model();
        m.browse.filters.countries = vec!["GB".into()];
        m.browse.focus = BrowseFocus::Filters {
            group: 1,
            option: 3,
        };
        let fx = update(&mut m, Msg::FilterClear);
        assert!(m.browse.filters.countries.is_empty());
        let kinds: Vec<_> = fx.iter().map(eff_kind).collect();
        assert!(kinds.contains(&"search"));
    }

    #[test]
    fn filter_clear_all_resets_all_filters() {
        let mut m = model();
        m.browse.filters.countries = vec!["GB".into()];
        m.browse.filters.tags = vec!["jazz".into()];
        m.browse.filters.bitrate_min = Some(128);
        update(&mut m, Msg::FilterClearAll);
        assert!(m.browse.filters.is_empty());
    }

    #[test]
    fn tick_emits_debounced_search_when_deadline_past() {
        let mut m = model();
        let past = Instant::now() - Duration::from_millis(600);
        m.browse.pending_online_search = Some(past);
        let fx = update(&mut m, Msg::Tick(Instant::now()));
        assert!(m.browse.pending_online_search.is_none());
        assert!(m.browse.loading, "debounced search must show loading");
        let kinds: Vec<_> = fx.iter().map(eff_kind).collect();
        assert!(kinds.contains(&"search"));
    }

    #[test]
    fn tick_skips_search_when_deadline_not_reached() {
        let mut m = model();
        let recent = Instant::now() - Duration::from_millis(100);
        m.browse.pending_online_search = Some(recent);
        let fx = update(&mut m, Msg::Tick(Instant::now()));
        assert!(m.browse.pending_online_search.is_some());
        assert!(fx.is_empty());
    }

    #[test]
    fn facets_loaded_stores_and_clears_loading() {
        let mut m = model();
        m.browse.facets_loading = true;
        let f = Facets {
            countries: vec![("GB".into(), 5)],
            tags: vec![],
            codecs: vec![],
        };
        update(&mut m, Msg::FacetsLoaded(f));
        assert_eq!(m.browse.facets.countries, vec![("GB".to_string(), 5)]);
        assert!(!m.browse.facets_loading);
    }

    #[test]
    fn filter_nav_wraps_across_five_groups() {
        let mut m = model();
        m.browse.focus = BrowseFocus::Filters {
            group: 4,
            option: 0,
        };
        filter_nav(&mut m, true);
        assert!(matches!(
            m.browse.focus,
            BrowseFocus::Filters { group: 0, .. }
        ));
    }

    #[test]
    fn status_group_has_four_options() {
        let m = model();
        assert_eq!(group_option_count(&m, 0), 4);
    }

    #[test]
    fn apply_status_favorites_sets_filter() {
        let mut m = model();
        apply_option(&mut m, 0, 1);
        assert_eq!(m.browse.filters.status, StatusFilter::Favorites);
    }

    #[test]
    fn apply_status_option_zero_resets_to_all() {
        let mut m = model();
        m.browse.filters.status = StatusFilter::Blocked;
        apply_option(&mut m, 0, 0);
        assert_eq!(m.browse.filters.status, StatusFilter::All);
    }

    #[test]
    fn error_marks_current_row_disabled_and_plays_next() {
        let mut m = model();
        m.browse.rows = vec![row("u1"), row("u2")];
        m.browse.selected = 0;
        m.now.uuid = Some("u1".into());
        let fx = update(&mut m, Msg::AudioStatus(Status::Error("boom".into())));
        assert_eq!(m.browse.rows[0].state, RowState::Disabled);
        assert_eq!(m.browse.selected, 1);
        assert!(fx.iter().map(eff_kind).any(|k| k == "play"));
        assert_eq!(m.auto_skip_count, 1);
        let failed = fx
            .iter()
            .any(|e| matches!(e, Effect::MarkFailed(u) if u == "u1"));
        assert!(failed, "auto-skip should mark the dead station as failed");
    }

    #[test]
    fn error_chain_stops_at_auto_skip_max() {
        let mut m = model();
        m.browse.rows = (0..10).map(|i| row(&format!("u{i}"))).collect();
        m.browse.selected = 0;
        for _ in 0..AUTO_SKIP_MAX {
            let cur = m.browse.selected_row().unwrap().uuid.clone();
            m.now.uuid = Some(cur);
            update(&mut m, Msg::AudioStatus(Status::Error("boom".into())));
        }
        assert_eq!(m.auto_skip_count, AUTO_SKIP_MAX);
        let cur = m.browse.selected_row().unwrap().uuid.clone();
        m.now.uuid = Some(cur);
        let fx = update(&mut m, Msg::AudioStatus(Status::Error("boom".into())));
        assert!(!fx.iter().map(eff_kind).any(|k| k == "play"));
        assert!(matches!(m.status, Status::Error(_)));
    }

    #[test]
    fn playing_resets_auto_skip_count() {
        let mut m = model();
        m.auto_skip_count = 3;
        update(
            &mut m,
            Msg::AudioStatus(Status::Playing {
                sample_rate: 44100,
                channels: 2,
                title: None,
            }),
        );
        assert_eq!(m.auto_skip_count, 0);
    }

    #[test]
    fn manual_play_resets_auto_skip_count() {
        let mut m = model();
        m.browse.rows = vec![row("u1")];
        m.browse.selected = 0;
        m.auto_skip_count = 4;
        update(&mut m, Msg::PlaySelected);
        assert_eq!(m.auto_skip_count, 0);
    }

    #[test]
    fn open_settings_sets_overlay() {
        let mut m = model();
        update(&mut m, Msg::OpenSettings);
        assert_eq!(m.overlay, Overlay::Settings);
    }

    #[test]
    fn close_overlay_resets_to_none() {
        let mut m = model();
        m.overlay = Overlay::Help;
        update(&mut m, Msg::CloseOverlay);
        assert_eq!(m.overlay, Overlay::None);
    }

    #[test]
    fn settings_adjust_changes_divisor_within_bounds() {
        let mut m = model();
        m.overlay = Overlay::Settings;
        m.settings_cursor = SETTINGS_DIVISOR_ROW;
        let before = m.fft_divisor;
        update(&mut m, Msg::SettingsAdjust(true));
        assert!(m.fft_divisor > before);
        m.fft_divisor = 24.0;
        update(&mut m, Msg::SettingsAdjust(true));
        assert!(m.fft_divisor <= 24.0);
        m.fft_divisor = 2.0;
        update(&mut m, Msg::SettingsAdjust(false));
        assert!(m.fft_divisor >= 2.0);
    }

    fn dead_row(uuid: &str) -> StationRow {
        let mut r = row(uuid);
        r.state = RowState::Disabled;
        r
    }

    #[test]
    fn autoplay_station_plays_regardless_of_list() {
        let mut m = model();
        m.browse.rows = vec![row("other")];
        let fx = update(&mut m, Msg::AutoplayStation(row("u2")));
        assert_eq!(m.now.uuid.as_deref(), Some("u2"));
        assert!(fx.iter().map(eff_kind).any(|k| k == "play"));
    }

    #[test]
    fn shuffle_plays_a_playable_row() {
        let mut m = model();
        m.browse.rows = vec![dead_row("d1"), row("u2"), dead_row("d3")];
        let fx = update(&mut m, Msg::Shuffle);
        assert_eq!(m.now.uuid.as_deref(), Some("u2"));
        assert!(fx.iter().map(eff_kind).any(|k| k == "play"));
    }

    #[test]
    fn shuffle_does_nothing_when_all_dead() {
        let mut m = model();
        m.browse.rows = vec![dead_row("d1"), dead_row("d2")];
        let fx = update(&mut m, Msg::Shuffle);
        assert!(m.now.uuid.is_none());
        assert!(!fx.iter().map(eff_kind).any(|k| k == "play"));
    }

    #[test]
    fn recheck_selected_rechecks_one_and_researches() {
        let mut m = model();
        m.browse.rows = vec![dead_row("d1"), dead_row("d2")];
        m.browse.selected = 1;
        let fx = update(&mut m, Msg::RecheckSelected);
        let kinds: Vec<_> = fx.iter().map(eff_kind).collect();
        assert!(kinds.contains(&"recheck"));
        assert!(!kinds.contains(&"recheckall"));
        assert!(kinds.contains(&"search"));
    }

    #[test]
    fn recheck_with_empty_list_rechecks_all() {
        let mut m = model();
        m.browse.rows = vec![];
        let fx = update(&mut m, Msg::RecheckSelected);
        let kinds: Vec<_> = fx.iter().map(eff_kind).collect();
        assert!(kinds.contains(&"recheckall"));
        assert!(!kinds.contains(&"recheck"));
        assert!(kinds.contains(&"search"));
    }

    #[test]
    fn toggle_hide_unplayable_flips_flag_and_searches() {
        let mut m = model();
        assert!(!m.browse.filters.hide_unplayable);
        let fx = update(&mut m, Msg::ToggleHideUnplayable);
        assert!(m.browse.filters.hide_unplayable);
        let kinds: Vec<_> = fx.iter().map(eff_kind).collect();
        assert!(kinds.contains(&"search"));
        update(&mut m, Msg::ToggleHideUnplayable);
        assert!(!m.browse.filters.hide_unplayable);
    }

    #[test]
    fn settings_spectrum_row_cycles_style() {
        use crate::tui::model::SpectrumStyle;
        let mut m = model();
        m.overlay = Overlay::Settings;
        m.settings_cursor = SETTINGS_SPECTRUM_ROW;
        assert_eq!(m.spectrum_style, SpectrumStyle::Bars);
        update(&mut m, Msg::SettingsToggle);
        assert_eq!(m.spectrum_style, SpectrumStyle::Mirror);
    }

    #[test]
    fn settings_crossfade_row_toggles_and_emits_effect() {
        let mut m = model();
        m.overlay = Overlay::Settings;
        m.settings_cursor = SETTINGS_CROSSFADE_ROW;
        assert!(m.crossfade);
        let fx = update(&mut m, Msg::SettingsToggle);
        assert!(!m.crossfade);
        assert!(fx.iter().any(|e| matches!(e, Effect::SetCrossfade(false))));
    }

    #[test]
    fn keybind_start_capture_sets_flag() {
        let mut m = model();
        m.overlay = Overlay::Keybindings;
        update(&mut m, Msg::KeybindStartCapture);
        assert!(m.keybind_capturing);
    }

    #[test]
    fn capture_key_rebinds_action_when_free() {
        use crate::tui::keybind::{Action, KeyChord, KeyName};
        let mut m = model();
        m.overlay = Overlay::Keybindings;
        m.keybind_cursor = 1; // Action::ALL[1] == Stop
        m.keybind_capturing = true;
        let chord = KeyChord {
            key: KeyName::Char('z'),
            ctrl: false,
            shift: false,
        };
        update(&mut m, Msg::CaptureKey(chord));
        assert_eq!(m.keymap.chord_for(Action::Stop), chord);
        assert!(!m.keybind_capturing);
        assert!(m.keybind_warning.is_none());
    }

    #[test]
    fn capture_key_conflict_keeps_old_and_warns() {
        use crate::tui::keybind::{Action, KeyChord, KeyName};
        let mut m = model();
        m.overlay = Overlay::Keybindings;
        m.keybind_cursor = 1; // Stop
        m.keybind_capturing = true;
        let chord = KeyChord {
            key: KeyName::Char('f'),
            ctrl: false,
            shift: false,
        };
        update(&mut m, Msg::CaptureKey(chord));
        assert_eq!(m.keymap.chord_for(Action::Stop).key, KeyName::Char('s'));
        assert!(m.keybind_warning.is_some());
        assert!(!m.keybind_capturing);
    }

    #[test]
    fn capture_esc_cancels_without_rebinding() {
        use crate::tui::keybind::{Action, KeyChord, KeyName};
        let mut m = model();
        m.overlay = Overlay::Keybindings;
        m.keybind_cursor = 1;
        m.keybind_capturing = true;
        let esc = KeyChord {
            key: KeyName::Esc,
            ctrl: false,
            shift: false,
        };
        update(&mut m, Msg::CaptureKey(esc));
        assert_eq!(m.keymap.chord_for(Action::Stop).key, KeyName::Char('s'));
        assert!(!m.keybind_capturing);
    }

    #[test]
    fn keybind_reset_restores_defaults() {
        use crate::tui::keybind::{Action, KeyChord, KeyName};
        let mut m = model();
        m.keymap.set(
            Action::Stop,
            KeyChord {
                key: KeyName::Char('z'),
                ctrl: false,
                shift: false,
            },
        );
        update(&mut m, Msg::KeybindReset);
        assert_eq!(m.keymap.chord_for(Action::Stop).key, KeyName::Char('s'));
    }

    #[test]
    fn mirror_play_ignores_own_and_stale() {
        use radio_core::mirror::{device_id, MirrorEvent};
        let mut m = Model::new(Theme::AmberCrt, ColorTier::Truecolor, Glyphs::unicode());
        let own = MirrorEvent {
            uuid: "u".into(),
            name: "n".into(),
            url: "http://x".into(),
            origin: device_id(),
            seq: 5,
        };
        assert!(update(&mut m, Msg::MirrorPlay(own)).is_empty());
        assert_eq!(m.mirror_seq, 0);
        let stale = MirrorEvent {
            uuid: "u".into(),
            name: "n".into(),
            url: "http://x".into(),
            origin: "other".into(),
            seq: 0,
        };
        m.mirror_seq = 3;
        assert!(update(&mut m, Msg::MirrorPlay(stale)).is_empty());
        assert_eq!(m.mirror_seq, 3);
    }

    #[test]
    fn mirror_play_idle_updates_hint_no_audio() {
        use radio_core::mirror::MirrorEvent;
        let mut m = Model::new(Theme::AmberCrt, ColorTier::Truecolor, Glyphs::unicode());
        let evt = MirrorEvent {
            uuid: "u2".into(),
            name: "Remote".into(),
            url: "http://x/2".into(),
            origin: "other".into(),
            seq: 9,
        };
        let fx = update(&mut m, Msg::MirrorPlay(evt));
        assert!(fx.is_empty());
        assert_eq!(m.mirror_seq, 9);
        assert!(m.notice.as_deref().unwrap().contains("Remote"));
    }

    #[test]
    fn mirror_play_drops_excluded_station() {
        use radio_core::mirror::MirrorEvent;
        let mut m = Model::new(Theme::AmberCrt, ColorTier::Truecolor, Glyphs::unicode());
        let evt = MirrorEvent {
            uuid: "u".into(),
            name: "Radio Moscow".into(),
            url: "http://x/ru".into(),
            origin: "other".into(),
            seq: 7,
        };
        let fx = update(&mut m, Msg::MirrorPlay(evt));
        assert!(fx.is_empty());
        assert!(m.now.station_name.is_none());
        assert!(m.notice.is_none());
    }

    #[test]
    fn catalog_synced_sets_count_and_clears_loading() {
        let mut m = model();
        m.catalog_loading = true;
        let _ = update(&mut m, Msg::CatalogSynced { count: 30241 });
        assert_eq!(m.catalog_count, Some(30241));
        assert!(!m.catalog_loading);
    }

    #[test]
    fn catalog_sync_failed_clears_loading_keeps_count() {
        let mut m = model();
        m.catalog_loading = true;
        let _ = update(&mut m, Msg::CatalogSyncFailed);
        assert!(!m.catalog_loading);
        assert_eq!(m.catalog_count, None);
    }

    #[test]
    fn catalog_synced_autoplays_random_when_pending_and_idle() {
        let mut m = model();
        m.autoplay_first_pending = true;
        m.browse.rows = vec![row("u1")];
        let effects = update(&mut m, Msg::CatalogSynced { count: 10 });
        assert!(
            effects.iter().any(|e| matches!(e, Effect::Play(_))),
            "plays a station"
        );
        assert!(!m.autoplay_first_pending, "flag cleared after autoplay");
    }

    #[test]
    fn catalog_synced_does_not_autoplay_when_already_playing() {
        let mut m = model();
        m.autoplay_first_pending = true;
        m.status = Status::Playing {
            sample_rate: 44100,
            channels: 2,
            title: None,
        };
        m.browse.rows = vec![row("u1")];
        let effects = update(&mut m, Msg::CatalogSynced { count: 10 });
        assert!(!effects.iter().any(|e| matches!(e, Effect::Play(_))));
    }

    #[test]
    fn quick_top_ready_autoplays_first_when_pending_and_idle() {
        let mut m = model();
        m.autoplay_first_pending = true;
        m.browse.rows = vec![row("u1")];
        let effects = update(&mut m, Msg::QuickTopReady { count: 5 });
        assert!(effects.iter().any(|e| matches!(e, Effect::Play(_))));
        assert!(!m.autoplay_first_pending);
    }

    fn eff_kind(e: &Effect) -> &'static str {
        match e {
            Effect::Search(_, _) => "search",
            Effect::LoadFacets => "loadfacets",
            Effect::Play(_) => "play",
            Effect::StopAudio => "stop",
            Effect::SetVolume(_) => "setvol",
            Effect::SetCrossfade(_) => "setcrossfade",
            Effect::ToggleFavorite(_) => "toggle",
            Effect::Blacklist(_) => "blacklist",
            Effect::Recheck(_) => "recheck",
            Effect::RecheckAll => "recheckall",
            Effect::RecordHistory(_) => "history",
            Effect::MarkFailed(_) => "markfailed",
            Effect::MirrorAnnounce { .. } => "mirrorannounce",
            Effect::SaveState => "savestate",
            Effect::Sync => "sync",
            Effect::SyncCreate => "synccreate",
            Effect::SyncLogout => "synclogout",
            Effect::SyncDelete => "syncdelete",
            Effect::Update(_) => "update",
        }
    }
}
