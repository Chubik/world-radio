use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Play,
    Stop,
    ToggleFavorite,
    Blacklist,
    Shuffle,
    ToggleHideUnplayable,
    EnterSearch,
    OpenSettings,
    OpenHelp,
    VolumeUp,
    VolumeDown,
    FocusFilters,
    Quit,
}

impl Action {
    pub const ALL: [Action; 13] = [
        Action::Play,
        Action::Stop,
        Action::ToggleFavorite,
        Action::Blacklist,
        Action::Shuffle,
        Action::ToggleHideUnplayable,
        Action::EnterSearch,
        Action::OpenSettings,
        Action::OpenHelp,
        Action::VolumeUp,
        Action::VolumeDown,
        Action::FocusFilters,
        Action::Quit,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Action::Play => "play station",
            Action::Stop => "stop",
            Action::ToggleFavorite => "toggle favorite",
            Action::Blacklist => "blacklist / block",
            Action::Shuffle => "shuffle",
            Action::ToggleHideUnplayable => "hide dead + unstable",
            Action::EnterSearch => "search",
            Action::OpenSettings => "settings",
            Action::OpenHelp => "help",
            Action::VolumeUp => "volume up",
            Action::VolumeDown => "volume down",
            Action::FocusFilters => "filter focus",
            Action::Quit => "quit",
        }
    }
}

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyName {
    Char(char),
    Enter,
    Esc,
    Tab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyChord {
    pub key: KeyName,
    pub ctrl: bool,
    pub shift: bool,
}

impl KeyChord {
    pub fn from_event(ev: KeyEvent) -> Option<KeyChord> {
        let ctrl = ev.modifiers.contains(KeyModifiers::CONTROL);
        let shift = ev.modifiers.contains(KeyModifiers::SHIFT);
        let key = match ev.code {
            KeyCode::Char(c) => KeyName::Char(c),
            KeyCode::Enter => KeyName::Enter,
            KeyCode::Esc => KeyName::Esc,
            KeyCode::Tab => KeyName::Tab,
            _ => return None,
        };
        Some(KeyChord { key, ctrl, shift })
    }

    pub fn to_string_compact(self) -> String {
        let mut s = String::new();
        if self.ctrl {
            s.push('^');
        }
        match self.key {
            KeyName::Char(c) => s.push(c),
            KeyName::Enter => s.push_str("enter"),
            KeyName::Esc => s.push_str("esc"),
            KeyName::Tab => s.push_str("tab"),
        }
        s
    }

    pub fn from_compact(raw: &str) -> Option<KeyChord> {
        let (ctrl, rest) = match raw.strip_prefix('^') {
            Some(r) => (true, r),
            None => (false, raw),
        };
        let key = match rest {
            "enter" => KeyName::Enter,
            "esc" => KeyName::Esc,
            "tab" => KeyName::Tab,
            s if s.chars().count() == 1 => KeyName::Char(s.chars().next().unwrap()),
            _ => return None,
        };
        let shift = matches!(key, KeyName::Char(c) if c.is_ascii_uppercase());
        Some(KeyChord { key, ctrl, shift })
    }
}

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Keymap {
    map: HashMap<Action, KeyChord>,
}

fn ch(c: char) -> KeyChord {
    KeyChord {
        key: KeyName::Char(c),
        ctrl: false,
        shift: c.is_ascii_uppercase(),
    }
}

impl Default for Keymap {
    fn default() -> Self {
        let mut map = HashMap::new();
        map.insert(
            Action::Play,
            KeyChord {
                key: KeyName::Enter,
                ctrl: false,
                shift: false,
            },
        );
        map.insert(Action::Stop, ch('s'));
        map.insert(Action::ToggleFavorite, ch('f'));
        map.insert(Action::Blacklist, ch('B'));
        map.insert(Action::Shuffle, ch('r'));
        map.insert(Action::ToggleHideUnplayable, ch('h'));
        map.insert(Action::EnterSearch, ch('/'));
        map.insert(Action::OpenSettings, ch(','));
        map.insert(Action::OpenHelp, ch('?'));
        map.insert(Action::VolumeUp, ch(']'));
        map.insert(Action::VolumeDown, ch('['));
        map.insert(
            Action::FocusFilters,
            KeyChord {
                key: KeyName::Tab,
                ctrl: false,
                shift: false,
            },
        );
        map.insert(Action::Quit, ch('q'));
        Keymap { map }
    }
}

impl Keymap {
    pub fn chord_for(&self, action: Action) -> KeyChord {
        self.map
            .get(&action)
            .copied()
            .unwrap_or_else(|| *Keymap::default().map.get(&action).unwrap())
    }

    pub fn action_for(&self, chord: KeyChord) -> Option<Action> {
        self.map.iter().find(|(_, c)| **c == chord).map(|(a, _)| *a)
    }

    pub fn conflict(&self, chord: KeyChord, ignoring: Action) -> Option<Action> {
        self.map
            .iter()
            .find(|(a, c)| **a != ignoring && **c == chord)
            .map(|(a, _)| *a)
    }

    pub fn set(&mut self, action: Action, chord: KeyChord) {
        self.map.insert(action, chord);
    }

    pub fn reset(&mut self) {
        *self = Keymap::default();
    }
}

impl serde::Serialize for Keymap {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(Some(Action::ALL.len()))?;
        for a in Action::ALL {
            let slug = serde_json::to_value(a).ok();
            let key = slug
                .as_ref()
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            m.serialize_entry(&key, &self.chord_for(a).to_string_compact())?;
        }
        m.end()
    }
}

impl<'de> serde::Deserialize<'de> for Keymap {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw: HashMap<String, String> = HashMap::deserialize(d)?;
        let mut km = Keymap::default();
        for a in Action::ALL {
            let slug = serde_json::to_value(a)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string));
            if let Some(slug) = slug {
                if let Some(s) = raw.get(&slug) {
                    if let Some(chord) = KeyChord::from_compact(s) {
                        km.set(a, chord);
                    }
                }
            }
        }
        Ok(km)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_actions_have_unique_labels() {
        let mut labels: Vec<&str> = Action::ALL.iter().map(|a| a.label()).collect();
        labels.sort();
        labels.dedup();
        assert_eq!(labels.len(), Action::ALL.len());
    }

    #[test]
    fn chord_roundtrips_plain_char() {
        let c = KeyChord::from_compact("f").unwrap();
        assert_eq!(c.key, KeyName::Char('f'));
        assert!(!c.ctrl && !c.shift);
        assert_eq!(c.to_string_compact(), "f");
    }

    #[test]
    fn chord_roundtrips_ctrl_and_named() {
        assert_eq!(
            KeyChord::from_compact("^u").unwrap().to_string_compact(),
            "^u"
        );
        assert_eq!(KeyChord::from_compact("tab").unwrap().key, KeyName::Tab);
        assert_eq!(KeyChord::from_compact("esc").unwrap().key, KeyName::Esc);
    }

    #[test]
    fn chord_uppercase_implies_shift() {
        let c = KeyChord::from_compact("B").unwrap();
        assert!(c.shift);
    }

    #[test]
    fn chord_from_event_reads_ctrl() {
        let ev = KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL);
        let c = KeyChord::from_event(ev).unwrap();
        assert!(c.ctrl);
        assert_eq!(c.key, KeyName::Char('u'));
    }

    #[test]
    fn chord_rejects_garbage() {
        assert!(KeyChord::from_compact("nonsense").is_none());
    }

    #[test]
    fn defaults_cover_every_action() {
        let km = Keymap::default();
        for a in Action::ALL {
            let _ = km.chord_for(a);
        }
    }

    #[test]
    fn defaults_have_no_internal_conflict() {
        let km = Keymap::default();
        for a in Action::ALL {
            let chord = km.chord_for(a);
            assert_eq!(km.conflict(chord, a), None, "{a:?} conflicts");
        }
    }

    #[test]
    fn action_for_reverse_lookup_matches() {
        let km = Keymap::default();
        let chord = km.chord_for(Action::Stop);
        assert_eq!(km.action_for(chord), Some(Action::Stop));
    }

    #[test]
    fn conflict_detects_taken_chord() {
        let km = Keymap::default();
        let stop_chord = km.chord_for(Action::Stop);
        assert_eq!(km.conflict(stop_chord, Action::Play), Some(Action::Stop));
    }

    #[test]
    fn set_then_reset_restores_defaults() {
        let mut km = Keymap::default();
        km.set(Action::Stop, ch('z'));
        assert_eq!(km.chord_for(Action::Stop), ch('z'));
        km.reset();
        assert_eq!(km.chord_for(Action::Stop), ch('s'));
    }

    #[test]
    fn keymap_roundtrips_through_toml() {
        let mut km = Keymap::default();
        km.set(Action::Stop, ch('z'));
        let s = toml::to_string(&km).unwrap();
        let back: Keymap = toml::from_str(&s).unwrap();
        assert_eq!(back.chord_for(Action::Stop), ch('z'));
        assert_eq!(back.chord_for(Action::Play).key, KeyName::Enter);
    }

    #[test]
    fn keymap_unknown_or_invalid_entries_fall_back() {
        let toml_src = "stop = \"nonsense\"\nbogus = \"x\"\n";
        let back: Keymap = toml::from_str(toml_src).unwrap();
        assert_eq!(back.chord_for(Action::Stop), ch('s'));
    }
}
