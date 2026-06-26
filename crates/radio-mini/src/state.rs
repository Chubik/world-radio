#[derive(Debug, Clone, PartialEq)]
pub struct StationPick {
    pub uuid: String,
    pub name: String,
    pub url: String,
}

pub fn pick_random(stations: &[StationPick]) -> Option<StationPick> {
    let playable: Vec<&StationPick> = stations.iter().filter(|s| !s.url.is_empty()).collect();
    if playable.is_empty() {
        return None;
    }
    let idx = fastrand::usize(..playable.len());
    Some(playable[idx].clone())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Buffering,
    Playing,
    Error,
}

pub fn state_labels(phase: Phase) -> (&'static str, &'static str) {
    match phase {
        Phase::Idle => ("IDLE", "SHUFFLE"),
        Phase::Buffering => ("···", "SHUFFLE"),
        Phase::Playing => ("LIVE", "SHUFFLE"),
        Phase::Error => ("OFFLINE", "RETRY"),
    }
}

pub fn spectrum_bars(n: usize) -> Vec<f32> {
    const SEED: [f32; 14] = [
        5.0, 7.0, 4.0, 8.0, 6.0, 3.0, 7.0, 5.0, 8.0, 4.0, 6.0, 7.0, 3.0, 5.0,
    ];
    (0..n).map(|i| SEED[i % SEED.len()] / 8.0).collect()
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
    pub volume: f32,
    pub scope: Scope,
    all: Vec<StationPick>,
    favorites: Vec<StationPick>,
}

impl MiniState {
    pub fn new() -> Self {
        Self {
            phase: Phase::Idle,
            now: None,
            volume: 0.8,
            scope: Scope::All,
            all: Vec::new(),
            favorites: Vec::new(),
        }
    }

    pub fn load_stations(&mut self, all: Vec<StationPick>, favorites: Vec<StationPick>) {
        self.all = all;
        self.favorites = favorites;
    }

    pub fn set_favorites(&mut self, favorites: Vec<StationPick>) {
        self.favorites = favorites;
    }

    pub fn active_stations(&self) -> &[StationPick] {
        match self.scope {
            Scope::All => &self.all,
            Scope::Favorites => &self.favorites,
        }
    }

    pub fn pick_shuffle(&self) -> Option<StationPick> {
        pick_random(self.active_stations())
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
        self.phase = match status {
            Status::Playing { .. } => Phase::Playing,
            Status::Buffering | Status::Retrying(_) => Phase::Buffering,
            Status::Error(_) => Phase::Error,
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

    fn st(uuid: &str, url: &str) -> StationPick {
        StationPick {
            uuid: uuid.into(),
            name: uuid.into(),
            url: url.into(),
        }
    }

    #[test]
    fn pick_returns_none_for_empty() {
        assert!(pick_random(&[]).is_none());
    }

    #[test]
    fn pick_skips_stations_without_url() {
        let list = vec![st("a", ""), st("b", "http://x")];
        let p = pick_random(&list).unwrap();
        assert_eq!(p.uuid, "b");
    }

    #[test]
    fn pick_returns_a_playable_one() {
        let list = vec![st("a", "http://a"), st("b", "http://b")];
        let p = pick_random(&list).unwrap();
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
    fn spectrum_bars_returns_n_values_in_range() {
        let b = spectrum_bars(16);
        assert_eq!(b.len(), 16);
        assert!(b.iter().all(|&v| (0.0..=1.0).contains(&v)));
        assert_eq!(spectrum_bars(0).len(), 0);
    }
}
