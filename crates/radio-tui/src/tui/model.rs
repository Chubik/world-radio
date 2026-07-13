use crate::tui::keybind::Keymap;
use crate::tui::theme::{ColorTier, Glyphs, Palette, Theme};
use radio_core::audio::Status;
use radio_core::catalog::{Facets, SearchQuery};
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum RowState {
    Normal,
    Disabled,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatusFilter {
    #[default]
    All,
    Favorites,
    Recent,
    Blocked,
    Dead,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SpectrumStyle {
    #[default]
    Bars,
    Mirror,
    Dots,
    Wave,
    Off,
}

impl SpectrumStyle {
    pub fn next(self) -> SpectrumStyle {
        match self {
            SpectrumStyle::Bars => SpectrumStyle::Mirror,
            SpectrumStyle::Mirror => SpectrumStyle::Dots,
            SpectrumStyle::Dots => SpectrumStyle::Wave,
            SpectrumStyle::Wave => SpectrumStyle::Off,
            SpectrumStyle::Off => SpectrumStyle::Bars,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SpectrumStyle::Bars => "bars",
            SpectrumStyle::Mirror => "mirror",
            SpectrumStyle::Dots => "dots",
            SpectrumStyle::Wave => "wave",
            SpectrumStyle::Off => "off",
        }
    }

    pub fn is_off(self) -> bool {
        matches!(self, SpectrumStyle::Off)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SortOrder {
    #[default]
    ApiOrder,
    Name,
    Country,
    Bitrate,
}

impl SortOrder {
    pub fn next(self) -> SortOrder {
        match self {
            SortOrder::ApiOrder => SortOrder::Name,
            SortOrder::Name => SortOrder::Country,
            SortOrder::Country => SortOrder::Bitrate,
            SortOrder::Bitrate => SortOrder::ApiOrder,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SortOrder::ApiOrder => "default",
            SortOrder::Name => "name",
            SortOrder::Country => "country",
            SortOrder::Bitrate => "bitrate",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Overlay {
    #[default]
    None,
    Settings,
    Help,
    Keybindings,
    Sync,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowseFilters {
    #[serde(default)]
    pub status: StatusFilter,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub countries: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub codecs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bitrate_min: Option<u32>,
    #[serde(default)]
    pub hide_unplayable: bool,
}

#[allow(dead_code)]
impl BrowseFilters {
    pub fn to_query(&self, name: &str) -> SearchQuery {
        let name = match name.trim().is_empty() {
            true => None,
            false => Some(name.to_string()),
        };
        SearchQuery {
            name,
            countrycode: self.countries.first().cloned(),
            language: None,
            tag: self.tags.first().cloned(),
            codec: self.codecs.first().cloned(),
            bitrate_min: self.bitrate_min,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.countries.is_empty()
            && self.tags.is_empty()
            && self.codecs.is_empty()
            && self.bitrate_min.is_none()
    }

    pub fn clear(&mut self) {
        self.status = StatusFilter::All;
        self.countries.clear();
        self.tags.clear();
        self.codecs.clear();
        self.bitrate_min = None;
    }

    pub fn clear_group(&mut self, group: usize) {
        match group {
            0 => self.status = StatusFilter::All,
            1 => self.countries.clear(),
            2 => self.tags.clear(),
            3 => self.codecs.clear(),
            4 => self.bitrate_min = None,
            _ => {}
        }
    }

    pub fn toggle(&mut self, group: usize, value: String) {
        let vec = match group {
            1 => &mut self.countries,
            2 => &mut self.tags,
            3 => &mut self.codecs,
            _ => return,
        };
        match vec.iter().position(|v| *v == value) {
            Some(i) => {
                vec.remove(i);
            }
            None => vec.push(value),
        }
    }

    pub fn group_selected(&self, group: usize, value: &str) -> bool {
        match group {
            1 => self.countries.iter().any(|v| v == value),
            2 => self.tags.iter().any(|v| v == value),
            3 => self.codecs.iter().any(|v| v == value),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub enum BrowseFocus {
    #[default]
    Stations,
    Filters {
        group: usize,
        option: usize,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct StationRow {
    pub uuid: String,
    pub name: String,
    pub url: String,
    pub country: String,
    pub tags: String,
    pub bitrate: u32,
    pub codec: String,
    pub favorite: bool,
    pub state: RowState,
}

impl StationRow {
    pub fn unstable(&self) -> bool {
        radio_core::catalog::codec_is_unstable(&self.codec)
    }
}

#[derive(Debug, Default)]
pub struct BrowseState {
    pub query: String,
    pub searching_input: bool,
    pub rows_api: Vec<StationRow>,
    pub rows: Vec<StationRow>,
    pub sort: SortOrder,
    pub selected: usize,
    pub loading: bool,
    pub last_error: Option<String>,
    pub offline: bool,
    #[allow(dead_code)]
    pub filters: BrowseFilters,
    #[allow(dead_code)]
    pub focus: BrowseFocus,
    #[allow(dead_code)]
    pub facets: Facets,
    #[allow(dead_code)]
    pub facets_loading: bool,
    #[allow(dead_code)]
    pub pending_online_search: Option<Instant>,
}

impl BrowseState {
    pub fn select_next(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.rows.len();
    }

    pub fn select_prev(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        self.selected = match self.selected {
            0 => self.rows.len() - 1,
            n => n - 1,
        };
    }

    pub fn page_down(&mut self, page: usize) {
        if self.rows.is_empty() {
            return;
        }
        self.selected = (self.selected + page).min(self.rows.len() - 1);
    }

    pub fn page_up(&mut self, page: usize) {
        self.selected = self.selected.saturating_sub(page);
    }

    pub fn selected_row(&self) -> Option<&StationRow> {
        self.rows.get(self.selected)
    }

    pub fn next_playable_below(&self) -> Option<usize> {
        self.rows
            .iter()
            .enumerate()
            .skip(self.selected + 1)
            .find(|(_, r)| r.state == RowState::Normal)
            .map(|(i, _)| i)
    }

    pub fn set_rows(&mut self, rows: Vec<StationRow>) {
        self.rows_api = rows;
        self.derive_rows();
    }

    pub fn derive_rows(&mut self) {
        let mut rows = self.rows_api.clone();
        match self.sort {
            SortOrder::ApiOrder => {}
            SortOrder::Name => {
                rows.sort_by_key(|r| r.name.to_lowercase());
            }
            SortOrder::Country => {
                rows.sort_by(|a, b| {
                    a.country
                        .cmp(&b.country)
                        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                });
            }
            SortOrder::Bitrate => {
                rows.sort_by(|a, b| {
                    b.bitrate
                        .cmp(&a.bitrate)
                        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                });
            }
        }
        self.rows = rows;
    }

    pub fn cycle_sort(&mut self) {
        self.sort = self.sort.next();
        self.derive_rows();
        self.selected = 0;
    }

    pub fn update_row(&mut self, uuid: &str, f: impl Fn(&mut StationRow)) {
        if let Some(r) = self.rows.iter_mut().find(|r| r.uuid == uuid) {
            f(r);
        }
        if let Some(r) = self.rows_api.iter_mut().find(|r| r.uuid == uuid) {
            f(r);
        }
    }
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct NowPlaying {
    pub station_name: Option<String>,
    pub country: String,
    pub bitrate: u32,
    pub codec: String,
    pub url: Option<String>,
    pub uuid: Option<String>,
    pub title: Option<String>,
}

pub struct Model {
    pub theme: Theme,
    pub tier: ColorTier,
    pub glyphs: Glyphs,
    pub status: Status,
    pub volume: f32,
    pub now: NowPlaying,
    pub browse: BrowseState,
    pub spectrum_bars: Vec<f32>,
    pub should_quit: bool,
    pub overlay: Overlay,
    pub auto_skip_count: u32,
    pub fft_divisor: f32,
    pub crossfade: bool,
    pub spectrum_style: SpectrumStyle,
    pub settings_cursor: usize,
    pub keymap: Keymap,
    pub keybind_cursor: usize,
    pub keybind_capturing: bool,
    pub keybind_warning: Option<String>,
    pub spinner: usize,
    pub notice: Option<String>,
    pub sync_key: Option<String>,
    pub mirror_seq: u64,
    pub pending_update: Option<radio_core::update::Release>,
    pub catalog_loading: bool,
    pub catalog_count: Option<usize>,
    pub autoplay_first_pending: bool,
}

impl Model {
    pub fn new(theme: Theme, tier: ColorTier, glyphs: Glyphs) -> Model {
        Model {
            theme,
            tier,
            glyphs,
            status: Status::Idle,
            volume: 0.6,
            now: NowPlaying::default(),
            browse: BrowseState::default(),
            spectrum_bars: Vec::new(),
            should_quit: false,
            overlay: Overlay::None,
            auto_skip_count: 0,
            fft_divisor: 12.0,
            crossfade: true,
            spectrum_style: SpectrumStyle::Bars,
            settings_cursor: 0,
            keymap: Keymap::default(),
            keybind_cursor: 0,
            keybind_capturing: false,
            keybind_warning: None,
            spinner: 0,
            notice: None,
            sync_key: radio_core::sync::load_key(),
            mirror_seq: 0,
            pending_update: None,
            catalog_loading: false,
            catalog_count: None,
            autoplay_first_pending: false,
        }
    }

    pub fn palette(&self) -> Palette {
        self.theme.palette().downgraded(self.tier)
    }

    pub fn is_playing(&self) -> bool {
        matches!(
            self.status,
            Status::Playing { .. } | Status::Buffering | Status::Retrying(_)
        )
    }

    pub fn is_animating(&self) -> bool {
        self.is_playing()
            || self.browse.loading
            || self.browse.facets_loading
            || self.browse.pending_online_search.is_some()
    }

    pub fn synced(&self) -> bool {
        self.sync_key.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_update_defaults_none() {
        let m = Model::new(Theme::AmberCrt, ColorTier::Truecolor, Glyphs::unicode());
        assert!(m.pending_update.is_none());
    }

    #[test]
    fn synced_reflects_key_presence() {
        let mut m = Model::new(Theme::AmberCrt, ColorTier::Truecolor, Glyphs::unicode());
        assert!(!m.synced());
        m.sync_key = Some("r4-x".to_string());
        assert!(m.synced());
    }

    #[test]
    fn spectrum_cycle_includes_off_at_end() {
        assert_eq!(SpectrumStyle::Wave.next(), SpectrumStyle::Off);
        assert_eq!(SpectrumStyle::Off.next(), SpectrumStyle::Bars);
        assert_eq!(SpectrumStyle::Off.label(), "off");
        assert!(SpectrumStyle::Off.is_off());
        assert!(!SpectrumStyle::Bars.is_off());
    }

    #[test]
    fn select_next_wraps_at_end_and_select_prev_wraps_at_zero() {
        let mut m = Model::new(Theme::AmberCrt, ColorTier::Truecolor, Glyphs::unicode());
        m.browse.rows = vec![row("a"), row("b"), row("c")];
        m.browse.selected = 2;
        m.browse.select_next();
        assert_eq!(m.browse.selected, 0);
        m.browse.select_prev();
        assert_eq!(m.browse.selected, 2);
    }

    #[test]
    fn page_down_clamps_at_end_page_up_clamps_at_zero() {
        let mut m = Model::new(Theme::AmberCrt, ColorTier::Truecolor, Glyphs::unicode());
        m.browse.rows = (0..20).map(|i| row(&format!("r{i}"))).collect();
        m.browse.selected = 0;
        m.browse.page_down(10);
        assert_eq!(m.browse.selected, 10);
        m.browse.page_down(15);
        assert_eq!(m.browse.selected, 19);
        m.browse.page_up(5);
        assert_eq!(m.browse.selected, 14);
        m.browse.page_up(50);
        assert_eq!(m.browse.selected, 0);
    }

    fn row(name: &str) -> StationRow {
        StationRow {
            uuid: name.into(),
            name: name.into(),
            url: String::new(),
            country: String::new(),
            tags: String::new(),
            bitrate: 0,
            codec: String::new(),
            favorite: false,
            state: RowState::Normal,
        }
    }

    fn srow(name: &str, country: &str, bitrate: u32) -> StationRow {
        let mut r = row(name);
        r.country = country.into();
        r.bitrate = bitrate;
        r
    }

    fn names(b: &BrowseState) -> Vec<String> {
        b.rows.iter().map(|r| r.name.clone()).collect()
    }

    #[test]
    fn sort_default_preserves_api_order() {
        let mut b = BrowseState::default();
        b.set_rows(vec![srow("Zeta", "US", 128), srow("Alpha", "GB", 320)]);
        assert_eq!(names(&b), vec!["Zeta", "Alpha"]);
    }

    #[test]
    fn sort_by_name_is_case_insensitive_az() {
        let mut b = BrowseState::default();
        b.set_rows(vec![srow("zeta", "US", 128), srow("Alpha", "GB", 320)]);
        b.sort = SortOrder::Name;
        b.derive_rows();
        assert_eq!(names(&b), vec!["Alpha", "zeta"]);
    }

    #[test]
    fn sort_by_country_then_name() {
        let mut b = BrowseState::default();
        b.set_rows(vec![
            srow("B", "US", 128),
            srow("A", "US", 128),
            srow("C", "GB", 128),
        ]);
        b.sort = SortOrder::Country;
        b.derive_rows();
        assert_eq!(names(&b), vec!["C", "A", "B"]);
    }

    #[test]
    fn sort_by_bitrate_descending_then_name() {
        let mut b = BrowseState::default();
        b.set_rows(vec![
            srow("Lo", "US", 64),
            srow("HiB", "US", 320),
            srow("HiA", "US", 320),
        ]);
        b.sort = SortOrder::Bitrate;
        b.derive_rows();
        assert_eq!(names(&b), vec!["HiA", "HiB", "Lo"]);
    }

    #[test]
    fn cycle_sort_advances_and_wraps_resetting_selected() {
        let mut b = BrowseState::default();
        b.set_rows(vec![srow("a", "US", 1), srow("b", "US", 2)]);
        b.selected = 1;
        b.cycle_sort();
        assert_eq!(b.sort, SortOrder::Name);
        assert_eq!(b.selected, 0);
        b.cycle_sort();
        assert_eq!(b.sort, SortOrder::Country);
        b.cycle_sort();
        assert_eq!(b.sort, SortOrder::Bitrate);
        b.cycle_sort();
        assert_eq!(b.sort, SortOrder::ApiOrder);
    }

    #[test]
    fn update_row_touches_both_vectors() {
        let mut b = BrowseState::default();
        b.set_rows(vec![srow("a", "US", 1)]);
        b.sort = SortOrder::Name;
        b.derive_rows();
        b.update_row("a", |r| r.favorite = true);
        assert!(b.rows[0].favorite);
        assert!(b.rows_api[0].favorite, "rows_api must survive re-derive");
        b.derive_rows();
        assert!(b.rows[0].favorite, "favorite persists after re-sort");
    }

    #[test]
    fn browse_filters_to_query_uses_first_of_each_group() {
        let f = BrowseFilters {
            status: StatusFilter::All,
            countries: vec!["GB".into(), "DE".into()],
            tags: vec!["jazz".into()],
            codecs: vec!["MP3".into()],
            bitrate_min: Some(128),
            ..Default::default()
        };
        let q = f.to_query("rock");
        assert_eq!(q.name.as_deref(), Some("rock"));
        assert_eq!(q.countrycode.as_deref(), Some("GB"));
        assert_eq!(q.tag.as_deref(), Some("jazz"));
        assert_eq!(q.codec.as_deref(), Some("MP3"));
        assert_eq!(q.bitrate_min, Some(128));
    }

    #[test]
    fn browse_filters_to_query_skips_empty_name() {
        let f = BrowseFilters::default();
        let q = f.to_query("");
        assert!(q.name.is_none() || q.name.as_deref() == Some(""));
    }

    #[test]
    fn browse_filters_toggle_adds_and_removes() {
        let mut f = BrowseFilters::default();
        f.toggle(2, "jazz".into());
        f.toggle(2, "rock".into());
        assert_eq!(f.tags, vec!["jazz".to_string(), "rock".to_string()]);
        assert!(f.group_selected(2, "jazz"));
        f.toggle(2, "jazz".into());
        assert_eq!(f.tags, vec!["rock".to_string()]);
        assert!(!f.group_selected(2, "jazz"));
    }

    #[test]
    fn browse_filters_clear_resets_all_fields() {
        let mut f = BrowseFilters {
            status: StatusFilter::Favorites,
            countries: vec!["GB".into()],
            tags: vec!["jazz".into()],
            codecs: vec!["MP3".into()],
            bitrate_min: Some(128),
            ..Default::default()
        };
        f.clear();
        assert!(f.is_empty());
    }

    #[test]
    fn browse_filters_clear_group_resets_one_field() {
        let mut f = BrowseFilters {
            status: StatusFilter::All,
            countries: vec!["GB".into()],
            tags: vec!["jazz".into()],
            ..Default::default()
        };
        f.clear_group(1);
        assert!(f.countries.is_empty());
        assert_eq!(f.tags, vec!["jazz".to_string()]);
    }

    #[test]
    fn browse_focus_default_is_stations() {
        assert_eq!(BrowseFocus::default(), BrowseFocus::Stations);
    }

    #[test]
    fn status_filter_defaults_to_all() {
        assert_eq!(StatusFilter::default(), StatusFilter::All);
    }

    #[test]
    fn next_playable_below_skips_disabled() {
        let b = BrowseState {
            rows: vec![
                sample_row_state("a", RowState::Normal),
                sample_row_state("b", RowState::Disabled),
                sample_row_state("c", RowState::Normal),
            ],
            selected: 0,
            ..Default::default()
        };
        assert_eq!(b.next_playable_below(), Some(2));
    }

    #[test]
    fn next_playable_below_none_when_all_below_disabled() {
        let b = BrowseState {
            rows: vec![
                sample_row_state("a", RowState::Normal),
                sample_row_state("b", RowState::Disabled),
            ],
            selected: 0,
            ..Default::default()
        };
        assert_eq!(b.next_playable_below(), None);
    }

    fn sample_row_state(uuid: &str, state: RowState) -> StationRow {
        StationRow {
            uuid: uuid.into(),
            name: uuid.into(),
            url: format!("http://{uuid}"),
            country: "GB".into(),
            tags: String::new(),
            bitrate: 128,
            codec: "MP3".into(),
            favorite: false,
            state,
        }
    }
}
