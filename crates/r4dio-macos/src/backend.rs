use crate::catalog_src;
use crate::state::{MiniState, Phase, Scope, StationPick};
use radio_audio::AudioEngine;
use radio_core::catalog::{Cache, Catalog, Health};
use radio_core::mirror::{MirrorEvent, StreamEvent};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

/// the one place the sync host is named — the listener in `main.rs` reads it
/// from here rather than repeating the literal.
pub const SERVER: &str = "https://r4dio.net";

pub struct Backend {
    pub state: MiniState,
    engine: Option<AudioEngine>,
    // the analyser smooths each frame against the last, so it has to live as
    // long as the meter does rather than be rebuilt per read.
    spectrum: radio_core::spectrum::Spectrum,
    catalog: Catalog,
    fav_path: PathBuf,
    hist_path: PathBuf,
    blacklist_path: PathBuf,
    excluded_path: PathBuf,
    pending_path: PathBuf,
    profile_path: PathBuf,
    settings_path: PathBuf,
    settings: Settings,
    /// when the station now playing started, for the "UP 14m" line. it is a
    /// wall-clock stamp rather than a counter so it survives a poll gap.
    started_at: Option<i64>,
    /// the level to come back to when unmuting; None means not muted.
    premute: Option<f32>,
    /// the station shuffle will play next, chosen ahead of time so the panel can
    /// name it. it is drawn from the same pool at the same odds — the only
    /// difference is when the die is rolled, and a "NEXT" line that then played
    /// something else would be a lie.
    queued: Option<StationPick>,
    mirror_seq: u64,
    // set while a play arriving from another device is being started, so the
    // announce below does not push it straight back and start a ping-pong.
    applying_mirror: bool,
    // set while the station in `now` is one another device chose, whether or not
    // it was ever started here — `resume` plays that station and must not
    // announce it back as this Mac's own.
    mirrored_now: bool,
}

/// what a play arriving from the account stream is allowed to do here. our own
/// echo and anything already seen are dropped, and the ru/by ban is re-applied
/// at this boundary because the other device may be on a build without it.
pub fn accepts_mirror(evt: &MirrorEvent, seen_seq: &mut u64) -> bool {
    if evt.origin == radio_core::mirror::device_id() {
        return false;
    }
    if evt.seq <= *seen_seq {
        return false;
    }
    *seen_seq = evt.seq;
    !radio_core::catalog::text_is_excluded(&format!("{} {}", evt.name, evt.url))
}

#[derive(Debug, PartialEq)]
pub enum StreamAction {
    Mirror(MirrorEvent),
    Resync,
    Nothing,
}

/// what the account event stream does to this app. a play mirrors the other
/// device; the doorbell queues a re-sync, at most one at a time — the caller
/// clears `resync_queued` once that sync has run, so a burst of events costs
/// one sync rather than one each.
///
/// our own push echoes back here too, and the re-sync it causes is a no-op that
/// the server answers without ringing again, so it cannot loop.
pub fn dispatch_stream_event(evt: StreamEvent, resync_queued: &AtomicBool) -> StreamAction {
    match evt {
        StreamEvent::Play(e) => StreamAction::Mirror(e),
        StreamEvent::ProfileChanged => match resync_queued.swap(true, Ordering::SeqCst) {
            true => StreamAction::Nothing,
            false => StreamAction::Resync,
        },
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// this app's shuffle scope is only all-or-favourites. recent/blocked/dead are
/// real synced scopes it has no equivalent for, so they leave the panel where
/// it is instead of being collapsed into `All` — the profile on disk still
/// carries them for the surfaces that do.
pub fn scope_from_wire(wire: &str) -> Option<Scope> {
    match radio_core::sync::Scope::from_wire(wire)? {
        radio_core::sync::Scope::All => Some(Scope::All),
        radio_core::sync::Scope::Favorites => Some(Scope::Favorites),
        _ => None,
    }
}

/// pushes what the account decides into the live pick state. it runs at startup
/// and again after every sync, because a filter, a scope or a block set on
/// another device only ever arrives on disk — without this it would sit there
/// unapplied, which is exactly how the filter came to govern every surface but
/// this one.
///
/// the scope is seeded here and nowhere else. it used to sit inside `sync`'s
/// fallible body, after `save_state`, so a failing disk left the merged scope
/// on disk and the old one in `MiniState`: the panel says ALL while the account
/// says FAVS and shuffle draws from the wrong pool. re-seeding it from disk is
/// always right because `set_scope` writes the user's own choice through to
/// `profile.json` before anything reads it back — the disk is the only owner.
fn seed_from_profile(
    state: &mut MiniState,
    profile: &radio_core::sync::Profile,
    blocked: &[String],
) {
    state.set_filter(profile.countries.clone());
    state.set_blocked(blocked.to_vec());
    // an unknown or unset scope leaves the panel alone rather than resetting the
    // user to ALL from a value a newer client wrote.
    if let Some(scope) = scope_from_wire(&profile.scope) {
        state.set_scope(scope);
    }
}

/// the profile this app starts from, adoption included.
///
/// this app can be the first thing a machine ever syncs with — the tui may
/// never have been opened here. adoption used to live only in the tui, so a mac
/// that synced first published an empty profile, took whatever another device
/// had chosen, and the filter and theme that only `config.toml` held were then
/// gone for good. it belongs on every path that can reach the server, so it
/// sits at startup, before anything reads the profile.
fn startup_profile(
    profile_path: &std::path::Path,
    config_path: &std::path::Path,
) -> radio_core::sync::Profile {
    let mut profile = radio_core::sync::Profile::load(profile_path);
    let legacy = radio_core::sync::legacy_settings(config_path);
    if radio_core::sync::adopt_legacy(&mut profile, &legacy, profile_path) {
        eprintln!("startup: failed to save the adopted profile");
    }
    profile
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Settings {
    #[serde(default = "default_volume")]
    volume: f32,
    /// how the meter is drawn. it is a name rather than an enum here because the
    /// window owns the drawing; the backend only remembers the choice.
    #[serde(default = "default_eq_style")]
    eq_style: String,
    /// the analyser's divisor: lower reads quieter music, higher tames a loud
    /// station. mirrors the tui's `fft_divisor`.
    #[serde(default = "default_eq_gain")]
    eq_gain: f32,
}

fn default_volume() -> f32 {
    0.8
}

fn default_eq_style() -> String {
    "bars".to_string()
}

fn default_eq_gain() -> f32 {
    12.0
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            volume: default_volume(),
            eq_style: default_eq_style(),
            eq_gain: default_eq_gain(),
        }
    }
}

/// these are per-machine — unlike everything in `profile.json`, they must never
/// travel to another device, so they get their own file. a screen the user sits
/// two feet from and one across the room want different gain.
fn load_settings(path: &std::path::Path) -> Settings {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Settings::default();
    };
    serde_json::from_str::<Settings>(&raw).unwrap_or_default()
}

fn save_settings(path: &std::path::Path, settings: &Settings) {
    let body = match serde_json::to_string(settings) {
        Ok(body) => body,
        Err(e) => {
            eprintln!("encode settings failed: {e}");
            return;
        }
    };
    if let Err(e) = std::fs::write(path, body) {
        eprintln!("save settings failed: {e}");
    }
}

impl Backend {
    pub fn new() -> anyhow::Result<Backend> {
        let data = radio_core::paths::ensure_data_dir()?;
        let cache = Cache::open(&data.join("stations.db"))?;
        let health = Health::load(&data.join("station_health.json"));
        let fav_path = data.join("favorites.json");
        let hist_path = data.join("history.json");
        let blacklist_path = data.join("blacklist.json");
        let excluded_path = data.join("excluded_countries.json");
        let pending_path = data.join("sync_pending.json");
        let profile_path = data.join("profile.json");
        let settings_path = data.join("settings.json");
        let mut catalog = Catalog::load(
            cache,
            health,
            &fav_path,
            &hist_path,
            &blacklist_path,
            &excluded_path,
        );
        // a prior session may have quit or gone offline before its sync landed;
        // pick that delta back up rather than starting the log over.
        catalog.pending = radio_core::sync::Pending::load(&pending_path);

        let all = catalog_src::all_stations(&catalog)?;
        let favorites = catalog_src::favorite_stations(&catalog)?;

        let mut state = MiniState::new();
        state.load_stations(all, favorites);
        // a scope, a filter and a blocklist synced from another device are all on
        // disk before this app starts; reading them here is what makes the panel
        // open on the same scope and shuffle inside the same filter.
        let profile = startup_profile(&profile_path, &data.join("config.toml"));
        seed_from_profile(&mut state, &profile, catalog.blacklist_ids());
        let settings = load_settings(&settings_path);
        state.set_volume(settings.volume);

        let engine = AudioEngine::spawn().ok();
        if let Some(engine) = &engine {
            engine.set_volume(state.volume);
        }

        Ok(Backend {
            state,
            engine,
            spectrum: radio_core::spectrum::Spectrum::new(),
            settings,
            started_at: None,
            premute: None,
            queued: None,
            catalog,
            fav_path,
            hist_path,
            blacklist_path,
            excluded_path,
            pending_path,
            profile_path,
            settings_path,
            mirror_seq: 0,
            applying_mirror: false,
            mirrored_now: false,
        })
    }

    // the guard `announce` applies, exposed so a test reads the real condition
    // rather than a restatement of it that could drift from it.
    #[cfg(test)]
    fn would_announce(&self) -> bool {
        !(self.applying_mirror || self.mirrored_now)
    }

    // the mirror is a two-way street: without this a station played on the Mac
    // reaches no other device, though every other device's play reaches here.
    //
    // two guards, not one. `applying_mirror` covers the moment a mirrored play is
    // started; `mirrored_now` covers the station sitting in `now` afterwards,
    // which `resume` would otherwise announce back as this Mac's own.
    fn announce(&self, pick: &StationPick) {
        if self.applying_mirror || self.mirrored_now {
            return;
        }
        let Some(key) = radio_core::sync::load_key() else {
            return;
        };
        let (uuid, name, url) = (pick.uuid.clone(), pick.name.clone(), pick.url.clone());
        // the announce is a blocking http call and this runs under the backend
        // mutex, so it goes to a thread rather than freezing the panel.
        std::thread::spawn(move || {
            let client = radio_core::mirror::MirrorClient::new(SERVER);
            let origin = radio_core::mirror::device_id();
            if let Err(e) = client.play(&key, &uuid, &name, &url, &origin) {
                eprintln!("mirror announce failed: {e}");
            }
        });
    }

    /// a play from another device. it only takes over the speakers when this Mac
    /// is already playing — mirroring onto a silent Mac would start audio the
    /// user never asked for.
    pub fn apply_mirror(&mut self, evt: MirrorEvent) {
        if !accepts_mirror(&evt, &mut self.mirror_seq) {
            return;
        }
        let pick = StationPick {
            uuid: evt.uuid,
            name: evt.name,
            url: evt.url,
            country: String::new(),
            codec: String::new(),
            bitrate: 0,
            tags: String::new(),
        };
        // a station blocked on this account must not arrive by the back door
        // either. the country filter is deliberately not applied: a deliberate
        // play on another device is an explicit choice, like a star.
        if !radio_core::catalog::allowed_row(
            &pick.uuid,
            &pick.country,
            &[],
            self.catalog.blacklist_ids(),
            &[],
        ) {
            return;
        }
        // the flag covers the silent branch too. that branch starts no audio, but
        // it does park the other device's station in `now` — and `resume` later
        // plays exactly that, which would announce the phone's station back to
        // the account as this Mac's own play. the station, not the branch, is
        // what must not be re-announced.
        self.mirrored_now = true;
        match self.state.phase == Phase::Playing {
            true => {
                self.applying_mirror = true;
                self.play_pick(pick);
                self.applying_mirror = false;
            }
            false => self.state.now = Some(pick),
        }
    }

    /// the doorbell's sync. the gate is released here rather than by the caller
    /// so that events arriving while it is in flight collapse into this one.
    pub fn resync(&mut self, queued: &AtomicBool) {
        if let Err(e) = self.sync() {
            eprintln!("resync failed: {e}");
        }
        queued.store(false, Ordering::SeqCst);
    }

    fn play_pick(&mut self, pick: StationPick) {
        if let Some(engine) = &self.engine {
            engine.play(&pick.url);
        }
        self.started_at = Some(now_secs());
        self.announce(&pick);
        // whatever plays now is what `now` holds, so the mirror mark belongs to
        // this play and no longer to the one it replaced. `resume` re-plays the
        // same station and must keep the mark; every other caller brings a pick
        // the user chose here and clears it.
        self.mirrored_now = self.applying_mirror;
        self.catalog.record_history(&pick.uuid);
        if let Err(e) = self.catalog.save_state(
            &self.fav_path,
            &self.hist_path,
            &self.blacklist_path,
            &self.excluded_path,
        ) {
            eprintln!("save history failed: {e}");
        }
        self.state.begin_play(pick);
        // queued after `now` moves, never before: the draw skips whatever is
        // playing, and reading the previous station here would let the panel
        // promise the one that just started.
        self.queue_next();
    }

    pub fn shuffle(&mut self) {
        // whatever was queued is what the panel promised, so it is what plays.
        let pick = self.queued.take().or_else(|| self.state.pick_shuffle());
        if let Some(pick) = pick {
            self.play_pick(pick);
        }
    }

    /// picks the station after this one. a scope or filter change invalidates it,
    /// so it is re-rolled on every play rather than held across one.
    fn queue_next(&mut self) {
        self.queued = self.state.pick_shuffle().filter(|p| {
            // queueing the station already playing would show "NEXT" naming what
            // is on air, which reads as a bug even when the roll is honest.
            self.state.now.as_ref().map(|n| &n.uuid) != Some(&p.uuid)
        });
    }

    pub fn queued_next(&self) -> Option<&StationPick> {
        self.queued.as_ref()
    }

    pub fn play_last(&mut self) {
        match catalog_src::last_played(&self.catalog) {
            Ok(Some(pick)) => self.play_pick(pick),
            Ok(None) => self.shuffle(),
            Err(e) => {
                eprintln!("load last station failed: {e}");
                self.shuffle();
            }
        }
    }

    pub fn resume(&mut self) {
        match self.state.now.clone() {
            // resume re-plays the station already in `now`, so a mirrored one
            // stays mirrored however many times the user presses play; only a
            // genuinely new pick may clear the mark and start announcing again.
            Some(pick) => {
                let mirrored = self.mirrored_now;
                self.play_pick(pick);
                self.mirrored_now = mirrored;
            }
            None => self.shuffle(),
        }
    }

    pub fn stop(&mut self) {
        self.state.stop();
        if let Some(engine) = &self.engine {
            engine.stop();
        }
    }

    /// silence without losing the level the user set. a second call restores it,
    /// so mute is a toggle rather than a slide to zero and back by hand.
    pub fn toggle_mute(&mut self) {
        match self.premute.take() {
            Some(level) => self.set_volume(level),
            None => {
                let was = self.state.volume;
                self.set_volume(0.0);
                // taken after the set, which writes the new level to disk: the
                // level to come back to is the one the user chose, not zero.
                self.premute = Some(was);
            }
        }
    }

    pub fn is_muted(&self) -> bool {
        self.premute.is_some()
    }

    /// re-opens the stream that is already selected. a station that dropped mid
    /// play leaves `now` set, and this is the one action that fixes it without
    /// making the user find the row again.
    pub fn retry(&mut self) {
        let Some(pick) = self.state.now.clone() else {
            return;
        };
        if let Some(engine) = &self.engine {
            engine.play(&pick.url);
        }
        self.started_at = Some(now_secs());
    }

    /// how full the decode buffer is, 0.0-1.0. this is the number that predicts
    /// a stutter before the listener hears one.
    pub fn buffer_level(&self) -> Option<f32> {
        if self.state.phase == Phase::Idle {
            return None;
        }
        self.engine.as_ref().map(|e| e.buffer_level())
    }

    /// how long the station now playing has been up, in seconds.
    pub fn uptime(&self) -> Option<i64> {
        let started = self.started_at?;
        if self.state.phase == Phase::Idle {
            return None;
        }
        Some((now_secs() - started).max(0))
    }

    pub fn set_volume(&mut self, v: f32) {
        self.state.set_volume(v);
        if let Some(engine) = &self.engine {
            engine.set_volume(self.state.volume);
        }
        self.settings.volume = self.state.volume;
        save_settings(&self.settings_path, &self.settings);
    }

    // the stamp is taken here, at the moment the user changes the scope, never
    // at sync time — a sync-time stamp would always outrank the other device.
    pub fn set_scope(&mut self, scope: Scope) {
        // the pool changed, so the station queued from the old one is no longer
        // a fair draw.
        self.queued = None;
        self.state.set_scope(scope);
        let wire = match scope {
            Scope::All => radio_core::sync::Scope::All,
            Scope::Favorites => radio_core::sync::Scope::Favorites,
        };
        let mut profile = radio_core::sync::Profile::load(&self.profile_path);
        profile.set_scope(wire.as_wire(), now_secs());
        if let Err(e) = profile.save(&self.profile_path) {
            eprintln!("save profile failed: {e}");
        }
    }

    pub fn now_is_favorite(&self) -> bool {
        match &self.state.now {
            Some(pick) => self.catalog.is_favorite(&pick.uuid),
            None => false,
        }
    }

    pub fn toggle_favorite(&mut self) {
        let Some(pick) = self.state.now.clone() else {
            return;
        };
        match catalog_src::toggle_and_reload(&mut self.catalog, &pick.uuid) {
            Ok(favorites) => self.state.set_favorites(favorites),
            Err(e) => eprintln!("toggle favorite failed: {e}"),
        }
        self.persist();
    }

    pub fn poll_engine(&mut self) {
        if let Some(engine) = &self.engine {
            while let Some(status) = engine.poll_status() {
                self.state.apply_status(status);
            }
        }
    }

    /// the levels the window draws, taken from the audio actually being played.
    ///
    /// the engine keeps a tap of the mixed output, and radio-core's Spectrum
    /// turns it into bands — the same analyser the terminal client has always
    /// used, so both meters move the same way for the same sound.
    pub fn read_spectrum(&mut self, bars: usize) -> Vec<f32> {
        if bars == 0 {
            return Vec::new();
        }
        // silence has to read as silence: nothing playing means an empty tap,
        // and inventing a level for it is what made the old meter a decoration.
        let Some(engine) = &self.engine else {
            return vec![0.0; bars];
        };
        if self.state.phase != Phase::Playing {
            return vec![0.0; bars];
        }
        let mut buf = vec![0.0f32; 2048];
        let got = engine.read_tap(&mut buf);
        self.spectrum.set_divisor(self.settings.eq_gain);
        self.spectrum.analyze(&buf[..got], bars)
    }

    pub fn phase(&self) -> Phase {
        self.state.phase
    }

    pub fn favourite_count(&self) -> u32 {
        self.catalog.favorite_ids().len() as u32
    }

    pub fn favourite_rows(&mut self) -> Vec<crate::commands::StationRow> {
        // the star reflects what is on screen, so the rows are read from the
        // catalog rather than from state.favorites, which only tracks shuffle scope.
        let favorites = match catalog_src::favorite_stations(&self.catalog) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("load favourites failed: {e}");
                return Vec::new();
            }
        };
        let now = self.state.now.as_ref().map(|n| n.uuid.clone());
        let playing = self.state.phase != Phase::Idle;
        favorites
            .into_iter()
            .map(|s| crate::commands::StationRow {
                is_playing: playing && now.as_deref() == Some(s.uuid.as_str()),
                genre: crate::state::first_tag(&s.tags),
                dead: self.catalog.is_dead(&s.uuid),
                uuid: s.uuid,
                name: s.name,
                country: s.country,
                codec: s.codec,
                bitrate: s.bitrate,
            })
            .collect()
    }

    /// the meter's look, taken from the account rather than this machine — the
    /// user picked it once and every device they own should draw it that way.
    /// the local file is the fallback for a device that has never synced.
    pub fn eq_settings(&self) -> (String, f32) {
        let profile = radio_core::sync::Profile::load(&self.profile_path);
        let style = profile
            .setting("eq_style")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| self.settings.eq_style.clone());
        let gain = profile
            .setting("eq_gain")
            .and_then(serde_json::Value::as_f64)
            .map(|g| g as f32)
            .unwrap_or(self.settings.eq_gain);
        (style, gain)
    }

    pub fn set_eq(&mut self, style: String, gain: f32) {
        // the analyser divides by this, so zero would be a division by nothing
        // and a negative one would invert the meter.
        let gain = gain.clamp(2.0, 40.0);
        self.settings.eq_style = style.clone();
        self.settings.eq_gain = gain;
        // written locally as well as to the profile: the meter has to draw
        // correctly on a machine that has never signed in.
        save_settings(&self.settings_path, &self.settings);

        let mut profile = radio_core::sync::Profile::load(&self.profile_path);
        let now = now_secs();
        profile.set_setting("eq_style", serde_json::json!(style), now);
        profile.set_setting("eq_gain", serde_json::json!(gain), now);
        if let Err(e) = profile.save(&self.profile_path) {
            eprintln!("save eq settings failed: {e}");
        }
    }

    pub fn history_rows(&mut self) -> Vec<crate::commands::HistoryRow> {
        let played = match catalog_src::played_before(&self.catalog) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("load history failed: {e}");
                return Vec::new();
            }
        };
        let now = self.state.now.as_ref().map(|n| n.uuid.clone());
        let playing = self.state.phase != Phase::Idle;
        played
            .into_iter()
            .map(|(s, at)| crate::commands::HistoryRow {
                is_playing: playing && now.as_deref() == Some(s.uuid.as_str()),
                is_favorite: self.catalog.is_favorite(&s.uuid),
                genre: crate::state::first_tag(&s.tags),
                dead: self.catalog.is_dead(&s.uuid),
                played_at: at,
                uuid: s.uuid,
                name: s.name,
                country: s.country,
                codec: s.codec,
                bitrate: s.bitrate,
            })
            .collect()
    }

    pub fn play_uuid(&mut self, uuid: &str) {
        // a favourite is usually absent from the top-1000 cache, so the row is
        // resolved through the catalog rather than looked up in the loaded lists.
        match catalog_src::station_pick(&self.catalog, uuid) {
            Ok(Some(pick)) => self.play_pick(pick),
            Ok(None) => eprintln!("station {uuid} is not in the catalog"),
            Err(e) => eprintln!("resolve station failed: {e}"),
        }
    }

    pub fn remove_favourite(&mut self, uuid: &str) -> Vec<crate::commands::StationRow> {
        match catalog_src::unfavorite_and_reload(&mut self.catalog, uuid) {
            Ok(favorites) => self.state.set_favorites(favorites),
            Err(e) => eprintln!("remove favourite failed: {e}"),
        }
        self.persist();
        self.favourite_rows()
    }

    pub fn shuffle_favourites(&mut self) {
        if let Some(pick) = crate::state::pick_from(&self.state.playable_favorites()) {
            self.play_pick(pick);
        }
    }

    pub fn blocked_rows(&mut self) -> Vec<crate::commands::StationRow> {
        let blocked = match catalog_src::blocked_stations(&self.catalog) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("load blocked failed: {e}");
                return Vec::new();
            }
        };
        blocked
            .into_iter()
            .map(|s| crate::commands::StationRow {
                is_playing: false,
                genre: crate::state::first_tag(&s.tags),
                // a blocked station is hidden because the user said so, not
                // because it failed; marking it dead too would blame the stream.
                dead: false,
                uuid: s.uuid,
                name: s.name,
                country: s.country,
                codec: s.codec,
                bitrate: s.bitrate,
            })
            .collect()
    }

    pub fn unblock(&mut self, uuid: &str) -> Vec<crate::commands::StationRow> {
        catalog_src::unblock(&mut self.catalog, uuid);
        self.persist();
        // unblocking widens the pool immediately: the station must be reachable
        // by the next shuffle, not only after the next sync.
        self.state
            .set_blocked(self.catalog.blacklist_ids().to_vec());
        self.blocked_rows()
    }

    pub fn country_rows(&self) -> Vec<crate::commands::CountryRow> {
        match catalog_src::country_facets(&self.catalog) {
            Ok(rows) => rows
                .into_iter()
                .map(|c| crate::commands::CountryRow {
                    code: c.code,
                    count: c.count,
                    excluded: c.excluded,
                })
                .collect(),
            Err(e) => {
                eprintln!("load countries failed: {e}");
                Vec::new()
            }
        }
    }

    pub fn set_excluded(&mut self, codes: Vec<String>) -> Vec<crate::commands::CountryRow> {
        self.catalog
            .set_excluded_countries(catalog_src::merge_hidden_exclusions(&self.catalog, codes));
        self.persist();
        // the excluded set changes which stations shuffle may reach, so the
        // loaded lists are stale the moment it is written.
        match catalog_src::all_stations(&self.catalog) {
            Ok(all) => self.state.set_all(all),
            Err(e) => eprintln!("reload stations failed: {e}"),
        }
        self.country_rows()
    }

    fn to_page(&self, page: catalog_src::StationPage) -> crate::commands::StationPage {
        let now = self.state.now.as_ref().map(|n| n.uuid.clone());
        let playing = self.state.phase != Phase::Idle;
        // a page is 200 rows out of ~58,000, so the station on air is almost
        // never among them by chance. it is put at the top instead: the window
        // marks it and parks its cursor there, and a list that cannot show what
        // you are listening to is the wrong list to be looking at.
        let mut stations = page.stations;
        if let Some(uuid) = now.as_deref() {
            if !stations.iter().any(|s| s.uuid == uuid) {
                if let Some(pick) = self.state.now.clone() {
                    stations.insert(0, pick);
                }
            }
        }
        crate::commands::StationPage {
            stations: stations
                .into_iter()
                .map(|s| crate::commands::StationRow {
                    is_playing: playing && now.as_deref() == Some(s.uuid.as_str()),
                    genre: crate::state::first_tag(&s.tags),
                    dead: self.catalog.is_dead(&s.uuid),
                    uuid: s.uuid,
                    name: s.name,
                    country: s.country,
                    codec: s.codec,
                    bitrate: s.bitrate,
                })
                .collect(),
            capped: page.capped,
        }
    }

    fn empty_page() -> crate::commands::StationPage {
        crate::commands::StationPage {
            stations: Vec::new(),
            capped: false,
        }
    }

    pub fn search_filtered(
        &self,
        name: &str,
        genre: Option<String>,
        country: Option<String>,
        codec: Option<String>,
        bitrate_min: Option<u32>,
    ) -> crate::commands::StationPage {
        match catalog_src::search_filtered(&self.catalog, name, genre, country, codec, bitrate_min)
        {
            Ok(page) => self.to_page(page),
            Err(e) => {
                eprintln!("search failed: {e}");
                Self::empty_page()
            }
        }
    }

    pub fn stations_in(&self, country: &str) -> crate::commands::StationPage {
        match catalog_src::stations_in_country(&self.catalog, country) {
            Ok(page) => self.to_page(page),
            Err(e) => {
                eprintln!("load country stations failed: {e}");
                Self::empty_page()
            }
        }
    }

    /// browse marks its rows from this list rather than re-reading a full page,
    /// so starring a station updates every row that shows it without a refetch.
    pub fn favourite_ids(&self) -> Vec<String> {
        self.catalog.favorite_ids().to_vec()
    }

    pub fn add_favourite(&mut self, uuid: &str) -> Vec<String> {
        match catalog_src::favorite_and_reload(&mut self.catalog, uuid) {
            Ok(favorites) => self.state.set_favorites(favorites),
            Err(e) => eprintln!("add favourite failed: {e}"),
        }
        self.persist();
        self.favourite_ids()
    }

    pub fn filter_counts(&self) -> crate::commands::FilterCounts {
        crate::commands::FilterCounts {
            excluded: self.catalog.excluded_country_ids().len() as u32,
            blocked: self.catalog.blacklist_ids().len() as u32,
            // the library window browses the whole catalogue and has no scope of
            // its own, so the filter is worded as the ALL scope always sees it.
            filter: crate::tray::filter_label(self.state.filter(), crate::state::Scope::All),
        }
    }

    // favourites and the sync log are written together: a removal that reached
    // disk but not the log would be re-added by the next sync.
    fn persist(&mut self) {
        if let Err(e) = self.catalog.save_state(
            &self.fav_path,
            &self.hist_path,
            &self.blacklist_path,
            &self.excluded_path,
        ) {
            eprintln!("save favorites failed: {e}");
        }
        // sync_pending.json is shared with the TUI/CLI, which may run concurrently —
        // merge rather than overwrite so this save can't erase their deletion.
        if let Err(e) = self.catalog.pending.save_merged(&self.pending_path) {
            eprintln!("save pending sync log failed: {e}");
        }
    }

    /// the merge, and everything that must happen once it has landed. every step
    /// after the profile is written is fallible, so this is deliberately wrapped
    /// by `sync` rather than called directly — see there for why.
    fn sync_inner(&mut self, key: &str) -> anyhow::Result<()> {
        use radio_core::sync::session;

        let profile = radio_core::sync::Profile::load(&self.profile_path);
        let local = session::outgoing(session::LocalState {
            favs: self.catalog.favorite_ids().to_vec(),
            blocked: self.catalog.blacklist_ids().to_vec(),
            excluded_countries: self.catalog.excluded_country_ids().to_vec(),
            changed: self.catalog.pending.clone(),
            profile: &profile,
            plays: self.catalog.history_plays(),
        });
        let client = radio_core::sync::SyncClient::new(SERVER);
        let merged = client.push(key, &local)?;
        // only now: the server has the delta, so replaying it would be wrong.
        let pushed = self.catalog.pending.clone();
        self.catalog.pending.clear();
        self.catalog.apply_synced_favorites(merged.favs.clone());
        self.catalog.apply_synced_blacklist(merged.blocked.clone());
        self.catalog
            .apply_synced_excluded_countries(merged.excluded_countries.clone());

        let mut profile = profile;
        let changed = session::apply_remote_profile(
            &mut profile,
            &merged.shuffle_filter,
            &merged.scope,
            &merged.theme,
            &merged.settings,
        );
        if changed.any() {
            profile.save(&self.profile_path)?;
        }
        if let Some(plays) = session::merge_history(self.catalog.history_plays(), &merged.history) {
            self.catalog.apply_synced_history(plays);
        }
        // history is written by save_state, so the merge above must land before it.
        self.catalog.save_state(
            &self.fav_path,
            &self.hist_path,
            &self.blacklist_path,
            &self.excluded_path,
        )?;
        // remove exactly what we just sent; keep anything written to the log
        // during the round-trip (a plain overwrite would destroy it).
        radio_core::sync::Pending::clear_pushed(&pushed, &self.pending_path)?;
        let all = catalog_src::all_stations(&self.catalog)?;
        let favorites = catalog_src::favorite_stations(&self.catalog)?;
        self.state.load_stations(all, favorites);
        Ok(())
    }

    /// the merge, then the seed that pushes it into the live pick state.
    ///
    /// the seed runs on the error path too, and that is the whole point of this
    /// wrapper. every step after `profile.save` is fallible — save_state, the
    /// pending-log rewrite and two sqlite reads — and each one used to sit
    /// between the write and the seed, so a failing disk left the new filter on
    /// disk while `MiniState` still held the old countries: the user shuffles and
    /// hears the countries they just stopped filtering to, until relaunch. the
    /// profile is re-read from disk rather than threaded out of the merge so that
    /// the seed reflects what was actually written, whatever failed after it.
    pub fn sync(&mut self) -> anyhow::Result<()> {
        let Some(key) = radio_core::sync::load_key() else {
            return Ok(());
        };
        self.sync_with_key(&key)
    }

    /// the key is a parameter so a test can drive the whole of `sync` against a
    /// stub server without a key in the real data dir — the invariant above is
    /// only worth anything if something exercises it end to end.
    fn sync_with_key(&mut self, key: &str) -> anyhow::Result<()> {
        let result = self.sync_inner(key);
        let profile = radio_core::sync::Profile::load(&self.profile_path);
        seed_from_profile(&mut self.state, &profile, self.catalog.blacklist_ids());
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_scopes_this_app_has_come_across_the_wire() {
        assert_eq!(scope_from_wire("all"), Some(Scope::All));
        assert_eq!(scope_from_wire("favorites"), Some(Scope::Favorites));
    }

    #[test]
    fn legacy_wire_scopes_still_map() {
        assert_eq!(scope_from_wire("ALL"), Some(Scope::All));
        assert_eq!(scope_from_wire("FAVS"), Some(Scope::Favorites));
    }

    #[test]
    fn a_scope_this_app_cannot_show_leaves_the_panel_alone() {
        // collapsing these into All would move the panel off favourites just
        // because another device opened its blocked list.
        assert_eq!(scope_from_wire("recent"), None);
        assert_eq!(scope_from_wire("blocked"), None);
        assert_eq!(scope_from_wire("dead"), None);
    }

    #[test]
    fn an_unknown_scope_leaves_the_panel_alone() {
        assert_eq!(scope_from_wire("something-new"), None);
        assert_eq!(scope_from_wire(""), None);
    }

    /// a backend with no engine and no account, over a scratch dir: enough to
    /// drive the mirror/announce logic without touching the user's data dir and
    /// without starting any audio. `announce` is a no-op here because
    /// `load_key()` finds nothing, so `would_announce` reads the guards directly.
    fn test_backend() -> Backend {
        let dir = std::env::temp_dir().join(format!(
            "r4dio-backend-{}-{}",
            std::process::id(),
            fastrand::u32(..)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let cache = Cache::open(&dir.join("stations.db")).unwrap();
        let health = Health::load(&dir.join("station_health.json"));
        let catalog = Catalog::load(
            cache,
            health,
            &dir.join("favorites.json"),
            &dir.join("history.json"),
            &dir.join("blacklist.json"),
            &dir.join("excluded_countries.json"),
        );
        Backend {
            state: MiniState::new(),
            engine: None,
            spectrum: radio_core::spectrum::Spectrum::new(),
            settings: Settings::default(),
            started_at: None,
            premute: None,
            queued: None,
            catalog,
            fav_path: dir.join("favorites.json"),
            hist_path: dir.join("history.json"),
            blacklist_path: dir.join("blacklist.json"),
            excluded_path: dir.join("excluded_countries.json"),
            pending_path: dir.join("sync_pending.json"),
            profile_path: dir.join("profile.json"),
            settings_path: dir.join("settings.json"),
            mirror_seq: 0,
            applying_mirror: false,
            mirrored_now: false,
        }
    }

    /// a backend wired to a stub sync server that answers the push with a
    /// `UA` filter and a `favorites` scope, so `Backend::sync` can be driven end
    /// to end. `R4DIO_SYNC_URL` is the seam `SyncClient::new` already honours; it
    /// is process-wide, so these tests hold a lock rather than running
    /// concurrently.
    ///
    /// the scope answer is `favorites`, not `all`: `MiniState` starts on `All`,
    /// so an `all` answer could never tell a working seed from a missing one.
    ///
    /// the returned server and mock must stay alive for the call — dropping the
    /// server closes the port and the push fails for the wrong reason.
    fn syncing_backend() -> (
        Backend,
        PathBuf,
        mockito::ServerGuard,
        std::sync::MutexGuard<'static, ()>,
    ) {
        // `R4DIO_SYNC_URL` is process-wide state, so only one of these may run at
        // a time; the guard is returned so it is held for the whole test.
        static ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let lock = ENV.lock().unwrap_or_else(|e| e.into_inner());

        let mut server = mockito::Server::new();
        let _mock = server
            .mock("PUT", "/sync")
            .with_header("content-type", "application/json")
            .with_body(
                // `value` is an object with `countries`, not a bare array — that
                // is the shape `session::remote_lww_filter` reads.
                r#"{"favs":[],"blocked":[],"excluded_countries":[],
                    "shuffle_filter":{"value":{"countries":["UA"]},"at":9999999999},
                    "scope":{"value":"favorites","at":9999999999},
                    "theme":{"value":"","at":0},"history":[]}"#,
            )
            .create();
        std::env::set_var("R4DIO_SYNC_URL", server.url());

        // leaked deliberately: the mock must answer for the whole call, and it is
        // owned by the server guard that the caller keeps alive.
        std::mem::forget(_mock);
        let b = test_backend();
        let dir = b.profile_path.parent().unwrap().to_path_buf();
        (b, dir, server, lock)
    }

    fn mirror(uuid: &str, origin: &str, seq: u64) -> radio_core::mirror::MirrorEvent {
        radio_core::mirror::MirrorEvent {
            uuid: uuid.into(),
            name: uuid.into(),
            url: format!("http://{uuid}"),
            origin: origin.into(),
            seq,
        }
    }

    fn pick(uuid: &str, country: &str) -> StationPick {
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

    // the panel names the station shuffle will play next, so the promise has to
    // be kept: whatever is queued is what plays, and it is never the station
    // already on air.
    #[test]
    fn the_queued_station_is_the_one_that_plays() {
        let mut b = test_backend();
        b.state
            .load_stations(vec![pick("a", "UA"), pick("b", "PL")], Vec::new());
        b.queue_next();
        let promised = b.queued_next().map(|p| p.uuid.clone());
        assert!(promised.is_some());

        b.shuffle();
        assert_eq!(b.state.now.as_ref().map(|n| n.uuid.clone()), promised);
    }

    #[test]
    fn the_queue_never_names_the_station_already_playing() {
        let mut b = test_backend();
        // one station only: the next roll can only come up with that one, and it
        // is what is already on air.
        b.state.load_stations(vec![pick("a", "UA")], Vec::new());
        b.shuffle();
        assert_eq!(b.state.now.as_ref().unwrap().uuid, "a");
        assert!(b.queued_next().is_none());
    }

    #[test]
    fn changing_scope_drops_a_queue_drawn_from_the_old_pool() {
        let mut b = test_backend();
        b.state
            .load_stations(vec![pick("a", "UA")], vec![pick("f", "PL")]);
        b.queue_next();
        assert!(b.queued_next().is_some());

        b.set_scope(Scope::Favorites);
        assert!(b.queued_next().is_none());
    }

    // the filter lives in profile.json, which another device may have written
    // before this app ever started; without this seed it would sit there unread.
    #[test]
    fn a_filter_on_disk_reaches_the_shuffle_pool_at_startup() {
        let mut state = MiniState::new();
        state.load_stations(vec![pick("a", "UA"), pick("b", "PL")], Vec::new());
        let mut profile = radio_core::sync::Profile::default();
        profile.set_countries(vec!["UA".into()], 100);

        seed_from_profile(&mut state, &profile, &[]);

        assert_eq!(state.active_stations().len(), 1);
        assert_eq!(state.active_stations()[0].uuid, "a");
    }

    #[test]
    fn a_blocklist_from_the_account_reaches_the_shuffle_pool() {
        let mut state = MiniState::new();
        state.load_stations(vec![pick("a", "UA"), pick("b", "UA")], Vec::new());

        seed_from_profile(
            &mut state,
            &radio_core::sync::Profile::default(),
            &["a".to_string()],
        );

        assert_eq!(state.active_stations().len(), 1);
        assert_eq!(state.active_stations()[0].uuid, "b");
    }

    // the scope used to be seeded by hand at two call sites, one of them inside
    // a fallible body that could skip it. it belongs here, where every path that
    // pushes the account into the pick state goes through it.
    #[test]
    fn a_scope_on_disk_reaches_the_pick_state() {
        let mut state = MiniState::new();
        let mut profile = radio_core::sync::Profile::default();
        profile.set_scope("favorites", 100);

        seed_from_profile(&mut state, &profile, &[]);

        assert_eq!(state.scope, Scope::Favorites);
    }

    // a scope this app cannot show must leave the panel where it is rather than
    // collapsing it to ALL.
    #[test]
    fn a_scope_this_app_cannot_show_leaves_the_pick_state_alone() {
        let mut state = MiniState::new();
        state.set_scope(Scope::Favorites);
        let mut profile = radio_core::sync::Profile::default();
        profile.set_scope("dead", 100);

        seed_from_profile(&mut state, &profile, &[]);

        assert_eq!(state.scope, Scope::Favorites);
    }

    // the mac may be the first surface on this machine to reach the server, and
    // adoption only ever ran in the tui. without it the mac publishes nothing,
    // the account adopts another device's filter, and the settings that only
    // config.toml held are lost.
    #[test]
    fn the_mac_adopts_an_old_config_at_startup() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.toml");
        let profile_path = dir.path().join("profile.json");
        std::fs::write(
            &config,
            "theme = \"monokai\"\n[filters]\nstatus = \"favorites\"\ncountries = [\"UA\"]\n",
        )
        .unwrap();

        let profile = startup_profile(&profile_path, &config);

        assert_eq!(profile.countries, vec!["UA".to_string()]);
        assert_eq!(profile.scope, "favorites");
        assert_eq!(profile.theme, "monokai");
        assert!(profile.countries_at > 0, "the adoption was never stamped");
        assert_eq!(
            radio_core::sync::Profile::load(&profile_path).countries,
            vec!["UA".to_string()],
            "the adoption never reached the disk"
        );
    }

    // adoption is a rescue, never a writer that outranks another device.
    #[test]
    fn the_mac_never_adopts_over_a_stamped_field() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.toml");
        let profile_path = dir.path().join("profile.json");
        std::fs::write(&config, "[filters]\ncountries = [\"UA\"]\n").unwrap();
        let mut p = radio_core::sync::Profile::default();
        p.set_countries(vec!["PL".into()], 900);
        p.save(&profile_path).unwrap();

        assert_eq!(
            startup_profile(&profile_path, &config).countries,
            vec!["PL".to_string()]
        );
    }

    // a filter cleared on another device has to widen the pool back out, not
    // leave the previous narrowing in place.
    #[test]
    fn a_filter_cleared_elsewhere_widens_the_pool_again() {
        let mut state = MiniState::new();
        state.load_stations(vec![pick("a", "UA"), pick("b", "PL")], Vec::new());
        let mut profile = radio_core::sync::Profile::default();
        profile.set_countries(vec!["UA".into()], 100);
        seed_from_profile(&mut state, &profile, &[]);
        assert_eq!(state.active_stations().len(), 1);

        profile.set_countries(Vec::new(), 200);
        seed_from_profile(&mut state, &profile, &[]);

        assert_eq!(state.active_stations().len(), 2);
    }

    /// the real pick path over this machine's real account and catalogue: ten
    /// shuffles must all land in the filtered countries and never on a station
    /// blocked elsewhere. it runs against a copy, so the account it is proving
    /// cannot be altered by the proof — and it starts no playback.
    #[test]
    #[ignore]
    fn shuffles_of_the_real_account_stay_inside_the_filter() {
        let Ok(real) = radio_core::paths::ensure_data_dir() else {
            eprintln!("no data dir; skipping");
            return;
        };
        if !real.join("stations.db").exists() {
            eprintln!("no local cache; skipping");
            return;
        }
        let data = std::env::temp_dir().join(format!("r4dio-filter-proof-{}", std::process::id()));
        std::fs::create_dir_all(&data).unwrap();
        for name in [
            "stations.db",
            "station_health.json",
            "favorites.json",
            "history.json",
            "blacklist.json",
            "excluded_countries.json",
            "profile.json",
        ] {
            if real.join(name).exists() {
                std::fs::copy(real.join(name), data.join(name)).unwrap();
            }
        }
        let cache = Cache::open(&data.join("stations.db")).unwrap();
        let health = Health::load(&data.join("station_health.json"));
        let catalog = Catalog::load(
            cache,
            health,
            &data.join("favorites.json"),
            &data.join("history.json"),
            &data.join("blacklist.json"),
            &data.join("excluded_countries.json"),
        );
        let profile = radio_core::sync::Profile::load(&data.join("profile.json"));
        eprintln!(
            "profile filter: {:?}, blocked: {}",
            profile.countries,
            catalog.blacklist_ids().len()
        );

        let mut state = MiniState::new();
        state.load_stations(
            catalog_src::all_stations(&catalog).unwrap(),
            catalog_src::favorite_stations(&catalog).unwrap(),
        );
        eprintln!("pool before the seed: {}", state.active_stations().len());
        seed_from_profile(&mut state, &profile, catalog.blacklist_ids());
        eprintln!("pool after the seed:  {}", state.active_stations().len());

        for i in 0..10 {
            let pick = state.pick_shuffle().unwrap();
            eprintln!("shuffle {i}: {} [{}]", pick.name, pick.country);
            if !profile.countries.is_empty() {
                assert!(
                    profile
                        .countries
                        .iter()
                        .any(|c| c.eq_ignore_ascii_case(&pick.country)),
                    "{} is in {}, outside the filter",
                    pick.name,
                    pick.country
                );
            }
            assert!(
                !catalog.blacklist_ids().contains(&pick.uuid),
                "{} is blocked on this account",
                pick.name
            );
        }
    }

    // two doorbells in flight must still cost exactly one resync.
    #[test]
    fn rapid_doorbells_queue_one_resync() {
        let queued = std::sync::atomic::AtomicBool::new(false);
        let mut syncs = 0;
        for _ in 0..2 {
            if dispatch_stream_event(StreamEvent::ProfileChanged, &queued) == StreamAction::Resync {
                syncs += 1;
            }
        }
        assert_eq!(syncs, 1, "a burst must queue one sync, not one per event");
        queued.store(false, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            dispatch_stream_event(StreamEvent::ProfileChanged, &queued),
            StreamAction::Resync
        );
    }

    // a doorbell must reach the sync path; a play event must not be mistaken
    // for one (that would resync on every station change anywhere).
    #[test]
    fn only_a_doorbell_triggers_a_resync() {
        let queued = std::sync::atomic::AtomicBool::new(false);
        dispatch_stream_event(StreamEvent::ProfileChanged, &queued);
        assert!(queued.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn a_play_event_mirrors_and_never_queues_a_resync() {
        let queued = std::sync::atomic::AtomicBool::new(false);
        let action = dispatch_stream_event(StreamEvent::Play(mirror("u", "other", 1)), &queued);
        assert!(matches!(action, StreamAction::Mirror(_)));
        assert!(
            !queued.load(std::sync::atomic::Ordering::SeqCst),
            "a play must not trigger a sync"
        );
    }

    // the two lines below are the literal bytes the live server sent this
    // machine's stream, so the listener is pinned to the wire it actually reads
    // rather than to a shape invented here.
    #[test]
    fn the_lines_the_live_server_sends_reach_the_right_branch() {
        let queued = std::sync::atomic::AtomicBool::new(false);
        let play = radio_core::mirror::parse_stream_event(
            r#"data: {"uuid":"u-test","name":"Proof FM","url":"http://example.invalid/s","origin":"dev-phone01","seq":234}"#,
        )
        .unwrap();
        assert!(matches!(
            dispatch_stream_event(play, &queued),
            StreamAction::Mirror(_)
        ));
        assert!(!queued.load(std::sync::atomic::Ordering::SeqCst));

        let doorbell =
            radio_core::mirror::parse_stream_event(r#"data: {"type":"profile_changed"}"#).unwrap();
        assert_eq!(
            dispatch_stream_event(doorbell, &queued),
            StreamAction::Resync
        );
    }

    // our own announce echoes straight back down the stream; acting on it would
    // make two devices bounce the same station between them for ever.
    #[test]
    fn our_own_play_echoing_back_is_ignored() {
        let mut seen = 0;
        assert!(!accepts_mirror(
            &mirror("u", &radio_core::mirror::device_id(), 5),
            &mut seen
        ));
        assert_eq!(seen, 0);
    }

    #[test]
    fn a_stale_play_is_ignored_and_a_newer_one_is_taken() {
        let mut seen = 3;
        assert!(!accepts_mirror(&mirror("u", "other", 3), &mut seen));
        assert_eq!(seen, 3);
        assert!(accepts_mirror(&mirror("u", "other", 4), &mut seen));
        assert_eq!(seen, 4);
    }

    // the ru/by ban holds at the mirror boundary too: another device may be on
    // a build without it, and its play must still not reach this one.
    #[test]
    fn a_banned_station_never_arrives_from_another_device() {
        let mut seen = 0;
        let mut evt = mirror("u", "other", 1);
        evt.name = "Radio Moscow".into();
        assert!(!accepts_mirror(&evt, &mut seen));
    }

    /// the hazard this branch exists to remove, at its last hiding place.
    ///
    /// `sync` writes the new filter and scope to `profile.json` and only then
    /// runs four more fallible steps — save_state, the pending-log rewrite and
    /// two sqlite reads. when the seed sat after those, any of them failing left
    /// the merged values on disk and the old ones in `MiniState`: the user
    /// shuffles and hears what they just filtered away, and the panel says ALL
    /// while the account says FAVS. this drives the same order `sync` does — the
    /// profile written, then a post-write failure — and asserts both the filter
    /// and the scope followed anyway.
    #[test]
    fn a_failure_after_the_profile_is_written_still_reaches_the_pool() {
        let (mut b, dir, _server, _lock) = syncing_backend();

        // the pending log is where the post-write failure comes from: a directory
        // cannot be overwritten by a file, so `Pending::clear_pushed` — which runs
        // *after* profile.save has already put the new filter on disk — fails for
        // real, with a real io error, on the real code path. nothing is stubbed
        // out of `sync` itself.
        std::fs::create_dir_all(&b.pending_path).unwrap();

        b.state
            .load_stations(vec![pick("a", "UA"), pick("b", "PL")], Vec::new());
        assert_eq!(
            b.state.active_stations().len(),
            2,
            "unfiltered to begin with"
        );

        let err = b
            .sync_with_key("r4-test")
            .expect_err("the post-write step must really fail");

        assert!(
            b.profile_path.exists(),
            "precondition: the merged profile reached disk before the failure ({err})"
        );
        let on_disk = radio_core::sync::Profile::load(&b.profile_path);
        assert_eq!(
            on_disk.countries,
            vec!["UA".to_string()],
            "precondition: the filter on disk is the merged one"
        );
        assert_eq!(
            on_disk.scope, "favorites",
            "precondition: the scope on disk is the merged one"
        );
        // the invariant: everything that reached disk also reached the live pick
        // state, even though sync returned Err. without it the user shuffles and
        // still hears PL until the app is relaunched.
        assert_eq!(
            b.state.filter(),
            ["UA".to_string()],
            "the filter reached disk but never the pick state — the user would still hear PL"
        );
        // the scope half, which used to sit inside the fallible body and so was
        // skipped by exactly this failure: the panel would show ALL while the
        // account says FAVS, and shuffle would draw from the whole catalogue.
        assert_eq!(
            b.state.scope,
            Scope::Favorites,
            "the scope reached disk but never the pick state — the panel would still say ALL"
        );
        // and the filter really governs the picks, not just a field. the sync
        // legitimately reloads the pool from the (empty) test catalogue, so it is
        // re-populated here; the scope is put back to ALL because the country
        // filter deliberately does not apply to the favourites arm.
        b.state
            .load_stations(vec![pick("a", "UA"), pick("b", "PL")], Vec::new());
        b.state.set_scope(Scope::All);
        assert_eq!(b.state.active_stations().len(), 1);
        assert_eq!(b.state.active_stations()[0].uuid, "a");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // the same invariant on the happy path, so a fix that only ever seeds on
    // failure would not pass either.
    #[test]
    fn a_successful_sync_reaches_the_pool_too() {
        let (mut b, dir, _server, _lock) = syncing_backend();

        b.sync_with_key("r4-test").expect("this sync must succeed");

        assert_eq!(b.state.filter(), ["UA".to_string()]);
        assert_eq!(b.state.scope, Scope::Favorites);
        b.state
            .load_stations(vec![pick("a", "UA"), pick("b", "PL")], Vec::new());
        b.state.set_scope(Scope::All);
        assert_eq!(b.state.active_stations().len(), 1);
        assert_eq!(b.state.active_stations()[0].uuid, "a");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // the echo guard's second half. the silent branch starts no audio but does
    // park the other device's station in `now`; pressing play then re-announced
    // it as this Mac's own, which is the ping-pong the guard exists to stop.
    #[test]
    fn a_mirrored_station_is_not_announced_when_the_user_presses_play() {
        let mut b = test_backend();
        b.state.phase = Phase::Idle;

        b.apply_mirror(mirror("remote", "other", 1));

        assert_eq!(b.state.now.as_ref().unwrap().uuid, "remote");
        assert!(
            b.mirrored_now,
            "the station in `now` came from another device and must be marked as such"
        );
        assert!(
            !b.would_announce(),
            "resume would push the phone's station back as this Mac's own play"
        );
    }

    // however many times the user presses play, a mirrored station stays
    // mirrored — the mark must not wear off after the first resume.
    #[test]
    fn resume_never_starts_announcing_a_mirrored_station() {
        let mut b = test_backend();
        b.state.phase = Phase::Idle;
        b.apply_mirror(mirror("remote", "other", 1));

        for _ in 0..3 {
            b.resume();
            assert!(
                !b.would_announce(),
                "a resume must not clear the mirror mark"
            );
        }
    }

    // and the mark must not stick to a station the user did choose, or the Mac
    // would go silent on the mirror for the rest of the session.
    #[test]
    fn a_station_the_user_picks_is_announced_again() {
        let mut b = test_backend();
        b.state.phase = Phase::Idle;
        b.apply_mirror(mirror("remote", "other", 1));
        assert!(!b.would_announce());

        b.state.set_all(vec![pick("mine", "UA")]);
        b.shuffle();

        assert!(
            b.would_announce(),
            "a shuffle the user asked for must reach the other devices"
        );
    }

    // volume is per-machine, so it lives in its own file rather than in
    // profile.json — a loud desktop must not push its level onto a phone.
    #[test]
    fn saved_settings_survive_a_restart() {
        let dir = std::env::temp_dir().join(format!(
            "r4dio-volume-{}-{}",
            std::process::id(),
            fastrand::u32(..)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let settings_path = dir.join("settings.json");

        save_settings(
            &settings_path,
            &Settings {
                volume: 0.35,
                eq_style: "wave".into(),
                eq_gain: 6.0,
            },
        );
        let loaded = load_settings(&settings_path);

        assert_eq!(loaded.volume, 0.35);
        // the meter's settings ride the same file, so they have to survive with it.
        assert_eq!(loaded.eq_style, "wave");
        assert_eq!(loaded.eq_gain, 6.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // the seed must not reach into favourites: an explicit star outranks a
    // broad taste filter, exactly as on android.
    #[test]
    fn the_seed_leaves_favourites_alone() {
        let mut state = MiniState::new();
        state.load_stations(Vec::new(), vec![pick("f", "PL")]);
        state.set_scope(Scope::Favorites);
        let mut profile = radio_core::sync::Profile::default();
        profile.set_countries(vec!["UA".into()], 100);

        seed_from_profile(&mut state, &profile, &[]);

        assert_eq!(state.pick_shuffle().unwrap().uuid, "f");
    }
}
