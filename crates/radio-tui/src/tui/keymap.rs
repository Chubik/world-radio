use crate::tui::message::Msg;
use crate::tui::model::{BrowseFocus, Model, Overlay};
use crossterm::event::{KeyCode, KeyEvent};

pub fn key_to_msg(model: &Model, ev: KeyEvent) -> Option<Msg> {
    if model.overlay != Overlay::None {
        return overlay_key(model, ev);
    }
    if model.browse.searching_input {
        return search_input_key(ev);
    }
    if let BrowseFocus::Filters { .. } = model.browse.focus {
        if let Some(msg) = filters_key(ev) {
            return Some(msg);
        }
    }
    global_key(model, ev)
}

fn overlay_key(model: &Model, ev: KeyEvent) -> Option<Msg> {
    if model.overlay == Overlay::Keybindings && model.keybind_capturing {
        return crate::tui::keybind::KeyChord::from_event(ev).map(Msg::CaptureKey);
    }
    if matches!(ev.code, KeyCode::Esc | KeyCode::Char('q')) {
        return Some(Msg::CloseOverlay);
    }
    if model.overlay == Overlay::Settings && matches!(ev.code, KeyCode::Char(',')) {
        return Some(Msg::CloseOverlay);
    }
    if model.overlay == Overlay::Help && matches!(ev.code, KeyCode::Char('?')) {
        return Some(Msg::CloseOverlay);
    }
    if model.overlay == Overlay::Sync && matches!(ev.code, KeyCode::Char('y')) {
        return Some(Msg::CloseOverlay);
    }
    if model.overlay == Overlay::Settings {
        return match ev.code {
            KeyCode::Char('j') | KeyCode::Down => Some(Msg::SettingsNav(true)),
            KeyCode::Char('k') | KeyCode::Up => Some(Msg::SettingsNav(false)),
            KeyCode::Enter => Some(Msg::SettingsToggle),
            KeyCode::Char('l') | KeyCode::Right => Some(Msg::SettingsAdjust(true)),
            KeyCode::Char('h') | KeyCode::Left => Some(Msg::SettingsAdjust(false)),
            _ => None,
        };
    }
    if model.overlay == Overlay::Keybindings {
        return match ev.code {
            KeyCode::Char('j') | KeyCode::Down => Some(Msg::KeybindNav(true)),
            KeyCode::Char('k') | KeyCode::Up => Some(Msg::KeybindNav(false)),
            KeyCode::Enter => Some(Msg::KeybindStartCapture),
            KeyCode::Char('r') => Some(Msg::KeybindReset),
            _ => None,
        };
    }
    if model.overlay == Overlay::Sync {
        return match ev.code {
            KeyCode::Char('n') => Some(Msg::SyncCreate),
            KeyCode::Char('c') => Some(Msg::SyncCopy),
            KeyCode::Char('r') => Some(Msg::SyncNow),
            KeyCode::Char('l') => Some(Msg::SyncLogout),
            KeyCode::Char('d') => Some(Msg::SyncDelete),
            _ => None,
        };
    }
    None
}

fn filters_key(ev: KeyEvent) -> Option<Msg> {
    match ev.code {
        KeyCode::Tab => Some(Msg::FocusToggle),
        KeyCode::Esc => Some(Msg::FocusToggle),
        KeyCode::Char('j') | KeyCode::Down => Some(Msg::FilterOptionNext),
        KeyCode::Char('k') | KeyCode::Up => Some(Msg::FilterOptionPrev),
        KeyCode::Char('l') | KeyCode::Right => Some(Msg::FilterNavNext),
        KeyCode::Char('h') | KeyCode::Left => Some(Msg::FilterNavPrev),
        KeyCode::Enter => Some(Msg::FilterApply),
        KeyCode::Char('c') => Some(Msg::FilterClear),
        KeyCode::Char('C') => Some(Msg::FilterClearAll),
        KeyCode::Char('/') => Some(Msg::EnterSearch),
        KeyCode::Char('q') => Some(Msg::Quit),
        _ => None,
    }
}

fn search_input_key(ev: KeyEvent) -> Option<Msg> {
    let ctrl = ev
        .modifiers
        .contains(crossterm::event::KeyModifiers::CONTROL);
    if ctrl && matches!(ev.code, KeyCode::Char('u') | KeyCode::Char('w')) {
        return Some(Msg::SearchClear);
    }
    match ev.code {
        KeyCode::Enter => Some(Msg::SubmitSearch),
        KeyCode::Esc => Some(Msg::SubmitSearch),
        KeyCode::Backspace => Some(Msg::SearchBackspace),
        KeyCode::Char(c) => Some(Msg::SearchChar(c)),
        _ => None,
    }
}

fn global_key(model: &Model, ev: KeyEvent) -> Option<Msg> {
    let shift = ev.modifiers.contains(crossterm::event::KeyModifiers::SHIFT);
    match ev.code {
        KeyCode::Down if shift => return Some(Msg::SelectPageDown),
        KeyCode::Up if shift => return Some(Msg::SelectPageUp),
        KeyCode::PageDown => return Some(Msg::SelectPageDown),
        KeyCode::PageUp => return Some(Msg::SelectPageUp),
        KeyCode::Char('J') => return Some(Msg::SelectPageDown),
        KeyCode::Char('K') => return Some(Msg::SelectPageUp),
        KeyCode::Char('j') | KeyCode::Down => return Some(Msg::SelectNext),
        KeyCode::Char('k') | KeyCode::Up => return Some(Msg::SelectPrev),
        KeyCode::Char('o') => return Some(Msg::CycleSort),
        _ => {}
    }
    if let Some(chord) = crate::tui::keybind::KeyChord::from_event(ev) {
        if let Some(action) = model.keymap.action_for(chord) {
            return Some(action_to_msg(action));
        }
    }
    None
}

fn action_to_msg(action: crate::tui::keybind::Action) -> Msg {
    use crate::tui::keybind::Action;
    match action {
        Action::Play => Msg::PlaySelected,
        Action::Stop => Msg::Stop,
        Action::ToggleFavorite => Msg::ToggleFavoriteSelected,
        Action::Blacklist => Msg::BlacklistSelected,
        Action::ExcludeCountry => Msg::ExcludeCountrySelected,
        Action::Recheck => Msg::RecheckSelected,
        Action::Shuffle => Msg::Shuffle,
        Action::Sync => Msg::OpenSyncOverlay,
        Action::ToggleHideUnplayable => Msg::ToggleHideUnplayable,
        Action::EnterSearch => Msg::EnterSearch,
        Action::OpenSettings => Msg::OpenSettings,
        Action::OpenHelp => Msg::OpenHelp,
        Action::FocusFilters => Msg::FocusToggle,
        Action::Quit => Msg::Quit,
        Action::Update => Msg::UpdateNow,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::model::{BrowseFocus, Model, Overlay};
    use crate::tui::theme::{ColorTier, Glyphs, Theme};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn model() -> Model {
        Model::new(Theme::AmberCrt, ColorTier::Truecolor, Glyphs::unicode())
    }

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    fn ch(c: char) -> KeyEvent {
        KeyEvent::from(KeyCode::Char(c))
    }

    #[test]
    fn q_quits_when_not_searching() {
        let m = model();
        assert!(matches!(key_to_msg(&m, ch('q')), Some(Msg::Quit)));
    }

    #[test]
    fn j_and_down_select_next() {
        let m = model();
        assert!(matches!(key_to_msg(&m, ch('j')), Some(Msg::SelectNext)));
        assert!(matches!(
            key_to_msg(&m, key(KeyCode::Down)),
            Some(Msg::SelectNext)
        ));
    }

    #[test]
    fn slash_enters_search() {
        let m = model();
        assert!(matches!(key_to_msg(&m, ch('/')), Some(Msg::EnterSearch)));
    }

    #[test]
    fn enter_plays_when_not_searching() {
        let m = model();
        assert!(matches!(
            key_to_msg(&m, key(KeyCode::Enter)),
            Some(Msg::PlaySelected)
        ));
    }

    #[test]
    fn comma_opens_settings() {
        let m = model();
        assert!(matches!(key_to_msg(&m, ch(',')), Some(Msg::OpenSettings)));
    }

    #[test]
    fn question_opens_help() {
        let m = model();
        assert!(matches!(key_to_msg(&m, ch('?')), Some(Msg::OpenHelp)));
    }

    #[test]
    fn comma_toggles_settings_closed() {
        let mut m = model();
        m.overlay = Overlay::Settings;
        assert!(matches!(key_to_msg(&m, ch(',')), Some(Msg::CloseOverlay)));
    }

    #[test]
    fn question_toggles_help_closed() {
        let mut m = model();
        m.overlay = Overlay::Help;
        assert!(matches!(key_to_msg(&m, ch('?')), Some(Msg::CloseOverlay)));
    }

    #[test]
    fn ctrl_u_clears_search() {
        let mut m = model();
        m.browse.searching_input = true;
        let ev = KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL);
        assert!(matches!(key_to_msg(&m, ev), Some(Msg::SearchClear)));
    }

    #[test]
    fn esc_closes_overlay_and_main_keys_suppressed() {
        let mut m = model();
        m.overlay = Overlay::Settings;
        assert!(matches!(
            key_to_msg(&m, key(KeyCode::Esc)),
            Some(Msg::CloseOverlay)
        ));
        assert!(!matches!(
            key_to_msg(&m, ch('f')),
            Some(Msg::ToggleFavoriteSelected)
        ));
    }

    #[test]
    fn in_search_input_chars_feed_query_and_enter_submits() {
        let mut m = model();
        m.browse.searching_input = true;
        assert!(matches!(
            key_to_msg(&m, ch('j')),
            Some(Msg::SearchChar('j'))
        ));
        assert!(matches!(
            key_to_msg(&m, key(KeyCode::Enter)),
            Some(Msg::SubmitSearch)
        ));
        assert!(matches!(
            key_to_msg(&m, key(KeyCode::Backspace)),
            Some(Msg::SearchBackspace)
        ));
    }

    #[test]
    fn tab_in_stations_toggles_focus_to_filters() {
        let m = model();
        let ev = KeyEvent::from(KeyCode::Tab);
        assert!(matches!(key_to_msg(&m, ev), Some(Msg::FocusToggle)));
    }

    #[test]
    fn j_in_filters_focus_navigates_options() {
        let mut m = model();
        m.browse.focus = BrowseFocus::Filters {
            group: 0,
            option: 0,
        };
        assert!(matches!(
            key_to_msg(&m, ch('j')),
            Some(Msg::FilterOptionNext)
        ));
    }

    #[test]
    fn l_in_filters_focus_navigates_groups() {
        let mut m = model();
        m.browse.focus = BrowseFocus::Filters {
            group: 0,
            option: 0,
        };
        assert!(matches!(key_to_msg(&m, ch('l')), Some(Msg::FilterNavNext)));
    }

    #[test]
    fn enter_in_filters_focus_applies() {
        let mut m = model();
        m.browse.focus = BrowseFocus::Filters {
            group: 0,
            option: 1,
        };
        assert!(matches!(
            key_to_msg(&m, key(KeyCode::Enter)),
            Some(Msg::FilterApply)
        ));
    }

    #[test]
    fn lowercase_c_in_filters_clears_current_group() {
        let mut m = model();
        m.browse.focus = BrowseFocus::Filters {
            group: 0,
            option: 0,
        };
        assert!(matches!(key_to_msg(&m, ch('c')), Some(Msg::FilterClear)));
    }

    #[test]
    fn shift_c_in_filters_clears_all() {
        let mut m = model();
        m.browse.focus = BrowseFocus::Filters {
            group: 0,
            option: 0,
        };
        let ev = KeyEvent::new(KeyCode::Char('C'), KeyModifiers::SHIFT);
        assert!(matches!(key_to_msg(&m, ev), Some(Msg::FilterClearAll)));
    }

    #[test]
    fn remapped_action_triggers_its_msg() {
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
        assert!(matches!(key_to_msg(&m, ch('z')), Some(Msg::Stop)));
    }

    #[test]
    fn default_stop_key_still_works() {
        let m = model();
        assert!(matches!(key_to_msg(&m, ch('s')), Some(Msg::Stop)));
    }

    #[test]
    fn navigation_keys_unaffected_by_remap() {
        let m = model();
        assert!(matches!(key_to_msg(&m, ch('j')), Some(Msg::SelectNext)));
    }

    #[test]
    fn o_cycles_sort() {
        let m = model();
        assert!(matches!(key_to_msg(&m, ch('o')), Some(Msg::CycleSort)));
    }

    #[test]
    fn capture_mode_routes_key_to_capturekey() {
        let mut m = model();
        m.overlay = Overlay::Keybindings;
        m.keybind_capturing = true;
        assert!(matches!(key_to_msg(&m, ch('z')), Some(Msg::CaptureKey(_))));
    }

    #[test]
    fn esc_in_keybindings_overlay_closes_when_not_capturing() {
        let mut m = model();
        m.overlay = Overlay::Keybindings;
        assert!(matches!(
            key_to_msg(&m, key(KeyCode::Esc)),
            Some(Msg::CloseOverlay)
        ));
    }
}
