#[derive(Debug, Clone, PartialEq)]
pub struct StationPick {
    pub uuid: String,
    pub name: String,
    pub url: String,
    pub country: String,
    pub codec: String,
    pub bitrate: u32,
    /// radio-browser's comma-run of tags. the window shows the first one as the
    /// station's genre — the rest are usually duplicates and spelling variants.
    pub tags: String,
}

/// radio-browser stores tags as one comma-run, often a dozen near-duplicates
/// ("jazz,Jazz,smooth jazz"). the first is the one the station chose to lead
/// with, and one word is all a row has space for.
pub fn first_tag(tags: &str) -> String {
    tags.split(',')
        .map(str::trim)
        .find(|t| is_genre(t))
        .unwrap_or_default()
        .to_lowercase()
}

/// stations tag themselves with all sorts of things — "107.9 fm", "2024", a
/// callsign. a genre column showing a frequency is worse than an empty one, so
/// anything that leads with a digit is skipped in favour of the next tag.
fn is_genre(tag: &str) -> bool {
    !tag.is_empty() && !tag.starts_with(|c: char| c.is_ascii_digit())
}

/// the panel's top-right line. parts are dropped rather than shown empty, so a
/// station with no codec reads "MX" instead of "MX ·  0k".
pub fn meta_label(country: &str, codec: &str, bitrate: u32) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !country.is_empty() {
        parts.push(country.to_string());
    }
    let rate = match bitrate {
        0 => String::new(),
        n => format!(" {n}k"),
    };
    if !codec.is_empty() {
        parts.push(format!("{codec}{rate}"));
    }
    parts.join(" · ")
}

/// a station with no url cannot be played, so it is never picked — a pool of
/// only those must report nothing rather than hand back an unplayable row.
pub fn pick_from(stations: &[&StationPick]) -> Option<StationPick> {
    let playable: Vec<&&StationPick> = stations.iter().filter(|s| !s.url.is_empty()).collect();
    if playable.is_empty() {
        return None;
    }
    let idx = fastrand::usize(..playable.len());
    Some((*playable[idx]).clone())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Buffering,
    Playing,
    Error,
}

#[allow(dead_code)]
pub fn state_labels(phase: Phase) -> (&'static str, &'static str) {
    match phase {
        Phase::Idle => ("IDLE", "SHUFFLE"),
        Phase::Buffering => ("···", "SHUFFLE"),
        Phase::Playing => ("LIVE", "SHUFFLE"),
        Phase::Error => ("OFFLINE", "RETRY"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    All,
    Favorites,
}

#[derive(Debug, Clone)]
pub struct MiniState {
    pub phase: Phase,
    pub now: Option<StationPick>,
    /// what the stream says is playing right now, from its icy metadata. many
    /// stations send nothing, so this is absent far more often than it is set.
    pub track: Option<String>,
    /// how many times the engine has retried this station without giving up.
    /// the design shows retrying apart from buffering, because one is the
    /// stream working and the other is it failing.
    pub retries: u32,
    pub volume: f32,
    pub scope: Scope,
    all: Vec<StationPick>,
    favorites: Vec<StationPick>,
    filter: Vec<String>,
    blocked: Vec<String>,
}

impl MiniState {
    pub fn new() -> Self {
        Self {
            phase: Phase::Idle,
            track: None,
            retries: 0,
            now: None,
            volume: 0.8,
            scope: Scope::All,
            all: Vec::new(),
            favorites: Vec::new(),
            filter: Vec::new(),
            blocked: Vec::new(),
        }
    }

    pub fn load_stations(&mut self, all: Vec<StationPick>, favorites: Vec<StationPick>) {
        self.all = all;
        self.favorites = favorites;
    }

    pub fn set_favorites(&mut self, favorites: Vec<StationPick>) {
        self.favorites = favorites;
    }

    // changing the country filter changes which stations shuffle may reach, so
    // the all-scope list is replaced without disturbing favourites.
    pub fn set_all(&mut self, all: Vec<StationPick>) {
        self.all = all;
    }

    /// the favourites the ★ button may actually land on — it shuffles them
    /// regardless of the panel's scope, so it reads this rather than switching
    /// scope as a side effect. blocked ones are gone: without that, the button
    /// would play a station the user blocked on another device even though the
    /// FAVS scope will not. there is deliberately no unfiltered accessor —
    /// that one was the trap this fix removed.
    pub fn playable_favorites(&self) -> Vec<&StationPick> {
        self.favorites
            .iter()
            .filter(|s| {
                radio_core::catalog::allowed_row(&s.uuid, &s.country, &[], &self.blocked, &[])
            })
            .collect()
    }

    /// the country filter this device is listening under. empty means
    /// unrestricted; the codes come from `profile.json`, never from here.
    pub fn set_filter(&mut self, filter: Vec<String>) {
        self.filter = filter;
    }

    /// what the window shows the user they are filtered to. a filter in effect
    /// and a filter unapplied look identical without it.
    pub fn filter(&self) -> &[String] {
        &self.filter
    }

    pub fn set_blocked(&mut self, blocked: Vec<String>) {
        self.blocked = blocked;
    }

    /// the pool shuffle draws from. the filter is applied here rather than baked
    /// into `all`, so a filter arriving from another device narrows the pool
    /// without reloading the catalogue.
    ///
    /// the two arms are deliberately asymmetric, matching android: a star
    /// outranks a broad taste filter, so favourites take no country filter — but
    /// blocking is a pointed "never this one", so it applies to both.
    ///
    /// `excluded_countries` is empty in both arms because the lists were built by
    /// `catalog_src`, whose query already dropped excluded countries in sql.
    /// passing them again would be redundant, not safer — do not "fix" this by
    /// filling it in without first moving that cut off the query.
    pub fn active_stations(&self) -> Vec<&StationPick> {
        let allowed = |s: &&StationPick, included: &[String]| {
            radio_core::catalog::allowed_row(&s.uuid, &s.country, &[], &self.blocked, included)
        };
        match self.scope {
            Scope::All => self
                .all
                .iter()
                .filter(|s| allowed(s, &self.filter))
                .collect(),
            Scope::Favorites => self.favorites.iter().filter(|s| allowed(s, &[])).collect(),
        }
    }

    pub fn pick_shuffle(&self) -> Option<StationPick> {
        pick_from(&self.active_stations())
    }

    pub fn begin_play(&mut self, pick: StationPick) {
        self.now = Some(pick);
        self.phase = Phase::Buffering;
    }

    pub fn stop(&mut self) {
        self.phase = Phase::Idle;
    }

    pub fn set_volume(&mut self, v: f32) {
        self.volume = v.clamp(0.0, 1.0);
    }

    pub fn set_scope(&mut self, scope: Scope) {
        self.scope = scope;
    }

    pub fn apply_status(&mut self, status: radio_audio::Status) {
        use radio_audio::Status;
        // the title rides the status and is dropped by anything that is not a
        // play: carrying the last station's track under the next one is worse
        // than showing none.
        match &status {
            Status::Playing { title, .. } => {
                self.track = title.clone().filter(|t| !t.trim().is_empty());
                self.retries = 0;
            }
            Status::Retrying(n) => self.retries = *n,
            _ => {
                self.track = None;
                self.retries = 0;
            }
        }
        self.phase = match status {
            Status::Playing { .. } => Phase::Playing,
            Status::Buffering | Status::Retrying(_) => Phase::Buffering,
            Status::Error(_) => Phase::Error,
            Status::StreamError { .. } => Phase::Error,
            Status::Idle => Phase::Idle,
        };
    }
}

impl Default for MiniState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_tag_takes_the_leading_tag() {
        assert_eq!(first_tag("jazz,smooth jazz,lounge"), "jazz");
        assert_eq!(first_tag("Lounge"), "lounge");
    }

    #[test]
    fn first_tag_skips_a_frequency_for_a_real_genre() {
        // "107.9 fm,dance" is a real row: the station led with its dial position.
        assert_eq!(first_tag("107.9 fm,dance"), "dance");
        assert_eq!(first_tag("2024,pop"), "pop");
    }

    #[test]
    fn first_tag_is_empty_rather_than_wrong() {
        assert_eq!(first_tag(""), "");
        assert_eq!(first_tag(" , , "), "");
        // nothing but a frequency leaves the column blank, which is honest.
        assert_eq!(first_tag("107.9 fm"), "");
    }

    fn st(uuid: &str, url: &str) -> StationPick {
        StationPick {
            uuid: uuid.into(),
            name: uuid.into(),
            url: url.into(),
            country: String::new(),
            codec: String::new(),
            bitrate: 0,
                        tags: String::new(),
        }
    }

    fn st_in(uuid: &str, country: &str) -> StationPick {
        StationPick {
            uuid: uuid.into(),
            name: uuid.into(),
            url: format!("http://{uuid}"),
            country: country.into(),
            codec: String::new(),
            bitrate: 0,
                        tags: String::new(),
        }
    }

    #[test]
    fn the_shuffle_pool_honours_the_country_filter() {
        let mut s = MiniState::new();
        s.set_all(vec![st_in("a", "UA"), st_in("b", "PL")]);
        s.set_filter(vec!["UA".into()]);
        for _ in 0..20 {
            assert_eq!(s.pick_shuffle().unwrap().uuid, "a");
        }
    }

    #[test]
    fn an_empty_filter_leaves_the_pool_whole() {
        let mut s = MiniState::new();
        s.set_all(vec![st_in("a", "UA"), st_in("b", "PL")]);
        s.set_filter(vec![]);
        assert_eq!(s.active_stations().len(), 2);
    }

    // a station blocked on another device must not be shuffled into here.
    #[test]
    fn the_shuffle_pool_skips_blocked_stations() {
        let mut s = MiniState::new();
        s.set_all(vec![st_in("a", "UA"), st_in("b", "UA")]);
        s.set_blocked(vec!["a".into()]);
        for _ in 0..20 {
            assert_eq!(s.pick_shuffle().unwrap().uuid, "b");
        }
    }

    // the star outranks the taste filter, exactly as on android.
    #[test]
    fn favourites_ignore_the_country_filter() {
        let mut s = MiniState::new();
        s.set_favorites(vec![st_in("f", "PL")]);
        s.set_filter(vec!["UA".into()]);
        s.set_scope(Scope::Favorites);
        assert_eq!(s.pick_shuffle().unwrap().uuid, "f");
    }

    // blocking is a pointed "never this one", so it beats a star — unlike an
    // excluded country, which a star outranks. android pins the same asymmetry
    // in CatalogFilterTest.blocking_beats_favouriting.
    #[test]
    fn blocking_beats_favouriting() {
        let mut s = MiniState::new();
        s.set_favorites(vec![st_in("f1", "UA"), st_in("f2", "UA")]);
        s.set_blocked(vec!["f1".into()]);
        s.set_scope(Scope::Favorites);
        for _ in 0..20 {
            assert_eq!(s.pick_shuffle().unwrap().uuid, "f2");
        }
    }

    // the ★ button bypasses the scope, so it needs the block check of its own.
    #[test]
    fn the_favourites_button_never_lands_on_a_blocked_star() {
        let mut s = MiniState::new();
        s.set_favorites(vec![st_in("f1", "UA"), st_in("f2", "UA")]);
        s.set_blocked(vec!["f1".into()]);
        // deliberately left on ALL: this button ignores the panel's scope.
        for _ in 0..20 {
            assert_eq!(pick_from(&s.playable_favorites()).unwrap().uuid, "f2");
        }
    }

    // a star outranks a taste filter on this button too.
    #[test]
    fn the_favourites_button_ignores_the_country_filter() {
        let mut s = MiniState::new();
        s.set_favorites(vec![st_in("f", "PL")]);
        s.set_filter(vec!["UA".into()]);
        assert_eq!(pick_from(&s.playable_favorites()).unwrap().uuid, "f");
    }

    // every favourite blocked leaves nothing to play rather than falling back
    // to one of them.
    #[test]
    fn every_favourite_blocked_leaves_nothing_to_pick() {
        let mut s = MiniState::new();
        s.set_favorites(vec![st_in("f1", "UA"), st_in("f2", "UA")]);
        s.set_blocked(vec!["f1".into(), "f2".into()]);
        s.set_scope(Scope::Favorites);
        assert!(s.pick_shuffle().is_none());
    }

    #[test]
    fn the_country_filter_ignores_case() {
        let mut s = MiniState::new();
        s.set_all(vec![st_in("a", "ua"), st_in("b", "PL")]);
        s.set_filter(vec!["Ua".into()]);
        assert_eq!(s.active_stations().len(), 1);
        assert_eq!(s.active_stations()[0].uuid, "a");
    }

    #[test]
    fn pick_returns_none_for_empty() {
        assert!(pick_from(&[]).is_none());
    }

    #[test]
    fn pick_skips_stations_without_url() {
        let list = [st("a", ""), st("b", "http://x")];
        let p = pick_from(&list.iter().collect::<Vec<&StationPick>>()).unwrap();
        assert_eq!(p.uuid, "b");
    }

    #[test]
    fn pick_returns_a_playable_one() {
        let list = [st("a", "http://a"), st("b", "http://b")];
        let p = pick_from(&list.iter().collect::<Vec<&StationPick>>()).unwrap();
        assert!(p.uuid == "a" || p.uuid == "b");
        assert!(!p.url.is_empty());
    }

    #[test]
    fn starts_idle() {
        let m = MiniState::new();
        assert_eq!(m.phase, Phase::Idle);
        assert!(m.now.is_none());
    }

    #[test]
    fn shuffle_sets_buffering_and_now() {
        let mut m = MiniState::new();
        m.begin_play(st("a", "http://a"));
        assert_eq!(m.phase, Phase::Buffering);
        assert_eq!(m.now.as_ref().unwrap().uuid, "a");
    }

    #[test]
    fn stop_goes_idle_but_keeps_station() {
        let mut m = MiniState::new();
        m.begin_play(st("a", "http://a"));
        m.stop();
        assert_eq!(m.phase, Phase::Idle);
        assert_eq!(m.now.as_ref().unwrap().uuid, "a");
    }

    #[test]
    fn volume_clamps() {
        let mut m = MiniState::new();
        m.set_volume(1.5);
        assert_eq!(m.volume, 1.0);
        m.set_volume(-0.2);
        assert_eq!(m.volume, 0.0);
    }

    #[test]
    fn scope_toggles() {
        let mut m = MiniState::new();
        assert_eq!(m.scope, Scope::All);
        m.set_scope(Scope::Favorites);
        assert_eq!(m.scope, Scope::Favorites);
    }

    #[test]
    fn pick_shuffle_picks_from_active_scope() {
        let mut m = MiniState::new();
        m.load_stations(vec![st("a", "http://a")], vec![st("f", "http://f")]);
        assert_eq!(m.pick_shuffle().unwrap().uuid, "a");

        m.set_scope(Scope::Favorites);
        assert_eq!(m.pick_shuffle().unwrap().uuid, "f");
    }

    #[test]
    fn pick_shuffle_returns_none_when_active_scope_empty() {
        let mut m = MiniState::new();
        m.load_stations(vec![st("a", "http://a")], vec![]);
        m.set_scope(Scope::Favorites);
        assert!(m.pick_shuffle().is_none());
    }

    #[test]
    fn favorites_are_readable_without_switching_scope() {
        let mut m = MiniState::new();
        m.load_stations(vec![st("a", "http://a")], vec![st("f", "http://f")]);
        assert_eq!(m.playable_favorites().len(), 1);
        assert_eq!(m.playable_favorites()[0].uuid, "f");
        // reading the list must not move the panel off the ALL scope.
        assert_eq!(m.scope, Scope::All);
    }

    #[test]
    fn active_stations_follows_scope() {
        let mut m = MiniState::new();
        m.load_stations(vec![st("a", "http://a")], vec![st("f", "http://f")]);
        assert_eq!(m.active_stations().len(), 1);
        assert_eq!(m.active_stations()[0].uuid, "a");
        m.set_scope(Scope::Favorites);
        assert_eq!(m.active_stations()[0].uuid, "f");
    }

    #[test]
    fn status_playing_maps_to_playing() {
        let mut m = MiniState::new();
        m.begin_play(st("a", "http://a"));
        m.apply_status(radio_audio::Status::Playing {
            sample_rate: 44100,
            channels: 2,
            title: None,
        });
        assert_eq!(m.phase, Phase::Playing);
    }

    #[test]
    fn status_error_maps_to_error() {
        let mut m = MiniState::new();
        m.begin_play(st("a", "http://a"));
        m.apply_status(radio_audio::Status::Error("x".into()));
        assert_eq!(m.phase, Phase::Error);
    }

    #[test]
    fn state_labels_cover_all_phases() {
        assert_eq!(state_labels(Phase::Idle), ("IDLE", "SHUFFLE"));
        assert_eq!(state_labels(Phase::Buffering), ("···", "SHUFFLE"));
        assert_eq!(state_labels(Phase::Playing), ("LIVE", "SHUFFLE"));
        assert_eq!(state_labels(Phase::Error), ("OFFLINE", "RETRY"));
    }

    #[test]
    fn meta_label_joins_what_is_present() {
        assert_eq!(meta_label("MX", "AAC", 48), "MX · AAC 48k");
    }

    #[test]
    fn meta_label_drops_a_zero_bitrate() {
        // radio-browser reports 0 for plenty of stations; "AAC 0k" is noise.
        assert_eq!(meta_label("MX", "AAC", 0), "MX · AAC");
    }

    #[test]
    fn meta_label_drops_empty_parts() {
        assert_eq!(meta_label("", "MP3", 128), "MP3 128k");
        assert_eq!(meta_label("DE", "", 0), "DE");
        assert_eq!(meta_label("", "", 0), "");
    }
}
