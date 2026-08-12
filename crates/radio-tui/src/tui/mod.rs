pub mod config;
pub mod keybind;
pub mod keymap;
pub mod logger;
pub mod message;
pub mod model;
pub mod spectrum;
pub mod theme;
pub mod update;
pub mod view;
pub mod worker;

use crate::tui::config::Config;
use crate::tui::keymap::key_to_msg;
use crate::tui::message::{Effect, Msg};
use crate::tui::model::Model;
use crate::tui::spectrum::Spectrum;
use crate::tui::theme::{detect_tier, ColorTier, Glyphs, Theme};
use crate::tui::update::update;
use crate::tui::worker::{WorkerPaths, WorkerReq};
use crossterm::event::{self, Event};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use radio_audio::AudioEngine;
use radio_core::catalog::{Cache, Catalog, Health};
use radio_core::paths;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

/// what the account event stream does to this app. a play mirrors the other
/// device; the doorbell queues a re-sync, at most one at a time — the worker
/// clears `resync_queued` once that sync has run, so a burst of events costs
/// one sync rather than one each.
fn dispatch_stream_event(
    evt: radio_core::mirror::StreamEvent,
    msg_tx: &Sender<Msg>,
    req_tx: &Sender<WorkerReq>,
    resync_queued: &AtomicBool,
) {
    match evt {
        radio_core::mirror::StreamEvent::Play(e) => {
            let _ = msg_tx.send(Msg::MirrorPlay(e));
        }
        // our own push echoes back here too; the re-sync it causes is a no-op
        // that the server answers without ringing again, so it cannot loop.
        radio_core::mirror::StreamEvent::ProfileChanged => {
            let already = resync_queued.swap(true, Ordering::SeqCst);
            if !already {
                let _ = req_tx.send(WorkerReq::SyncQuiet);
            }
        }
    }
}

const TAP_SAMPLES: usize = 2048;

pub fn run(no_emoji_flag: bool) -> anyhow::Result<()> {
    install_panic_hook();
    let data = paths::ensure_data_dir()?;
    logger::init(&data.join("world-radio.log"));
    let config = Config::load(&data.join("config.toml"));
    // the profile owns the theme now; this is only the fallback the seeding
    // below keeps when the profile has not chosen one.
    let theme = Theme::from_slug("");
    let tier = detect_tier();
    let glyphs = pick_glyphs(&config, no_emoji_flag, tier);

    let cache = Cache::open(&data.join("stations.db"))?;
    let health = Health::load(&data.join("station_health.json"));
    let catalog = Catalog::load(
        cache,
        health,
        &data.join("favorites.json"),
        &data.join("history.json"),
        &data.join("blacklist.json"),
        &data.join("excluded_countries.json"),
    );

    let fav_ids: Vec<String> = catalog.favorite_ids().to_vec();
    let excluded_countries: Vec<String> = catalog.excluded_country_ids().to_vec();
    let seed_rows: Vec<crate::tui::model::StationRow> =
        match catalog.list_by_popularity(&fav_ids, 200) {
            Ok(stations) => stations
                .iter()
                .map(|s| {
                    let uuid = &s.stationuuid;
                    worker::station_to_row(s, catalog.is_favorite(uuid), catalog.is_hidden(uuid))
                })
                .collect(),
            Err(e) => {
                crate::log_warn!("startup: list_by_popularity failed: {e}");
                Vec::new()
            }
        };
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let should_sync =
        radio_core::catalog::should_sync(catalog.last_sync().ok().flatten(), now_secs, 86_400);
    let catalog_count = catalog.catalog_count().ok().filter(|c| *c > 0);

    let (req_tx, req_rx) = channel::<WorkerReq>();
    let (msg_tx, msg_rx) = channel::<Msg>();
    let worker_paths = WorkerPaths {
        fav: data.join("favorites.json"),
        hist: data.join("history.json"),
        health: data.join("station_health.json"),
        blacklist: data.join("blacklist.json"),
        excluded: data.join("excluded_countries.json"),
        pending: data.join("sync_pending.json"),
        profile: data.join("profile.json"),
    };
    // the debounce: set while a doorbell-triggered sync is queued, cleared by
    // the worker when that sync finishes. a burst of events therefore costs one
    // re-sync, not one per event.
    let resync_queued = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let worker_handle = worker::spawn(
        catalog,
        worker_paths,
        req_rx,
        msg_tx.clone(),
        resync_queued.clone(),
    );

    let mirror_tx = msg_tx.clone();
    let mirror_req_tx = req_tx.clone();
    let listener_queued = resync_queued.clone();
    std::thread::spawn(move || loop {
        let Some(key) = radio_core::sync::load_key() else {
            std::thread::sleep(std::time::Duration::from_secs(10));
            continue;
        };
        let client = radio_core::mirror::MirrorClient::new("https://r4dio.net");
        let tx = mirror_tx.clone();
        let req = mirror_req_tx.clone();
        let queued = listener_queued.clone();
        let stream_key = key.clone();
        let _ = client.events(&key, |evt| {
            if radio_core::sync::load_key().as_deref() != Some(stream_key.as_str()) {
                return;
            }
            dispatch_stream_event(evt, &tx, &req, &queued);
        });
        std::thread::sleep(std::time::Duration::from_secs(3));
    });

    let update_tx = msg_tx.clone();
    std::thread::spawn(move || {
        if let Ok(Some(rel)) = radio_core::update::fetch_latest() {
            let _ = update_tx.send(Msg::UpdateAvailable(rel));
        }
    });

    let engine = AudioEngine::spawn()?;
    engine.set_volume(1.0);

    let mut model = Model::new(theme, tier, glyphs);
    model.browse.facets_loading = true;
    model.browse.loading = true;
    model.browse.query = config.query.clone();
    model.browse.excluded_countries = excluded_countries;
    model.fft_divisor = config.fft_divisor;
    model.crossfade = config.crossfade;
    model.spectrum_style = config.spectrum_style;
    model.keymap = config.keybindings.clone();
    let profile_path = data.join("profile.json");
    model.profile = radio_core::sync::Profile::load(&profile_path);
    // settings chosen before this build existed were never stamped, so without
    // this they would never be published and every other device would see an
    // account with no filter at all. a field the profile already stamped is
    // never touched, so this cannot undo what another device chose.
    let legacy = config.legacy_settings();
    let migration_pending = adopt_legacy(&mut model.profile, &legacy, &profile_path, now_secs);
    seed_from_profile(&mut model);
    if let Some(c) = catalog_count {
        model.catalog_count = Some(c);
    }
    let seed_empty = seed_rows.is_empty();
    if !seed_empty {
        model.browse.rows_api = seed_rows.clone();
        model.browse.rows = seed_rows;
        model.browse.loading = false;
    }
    engine.set_crossfade(config.crossfade);
    let mut spectrum = Spectrum::new();
    let mut tap_buf = vec![0.0_f32; TAP_SAMPLES];

    let mut terminal = setup_terminal()?;
    let _ = req_tx.send(WorkerReq::LoadFacets);
    let restored_query = model.browse.filters.to_query(&model.browse.query);
    let _ = req_tx.send(WorkerReq::Search(
        restored_query,
        model.browse.filters.clone(),
    ));
    match config.last_station.clone() {
        Some(uuid) => {
            let _ = req_tx.send(WorkerReq::ResolveAndPlay(uuid));
        }
        None if !model.browse.rows.is_empty() => {
            // pick a random available station
            let idx = fastrand::usize(..model.browse.rows.len());
            let row = model.browse.rows[idx].clone();
            run_effects(
                update(&mut model, Msg::AutoplayStation(row)),
                &mut model,
                &engine,
                &req_tx,
            );
        }
        None => model.autoplay_first_pending = true,
    }
    if should_sync {
        model.catalog_loading = model.browse.rows.is_empty();
        if seed_empty {
            let _ = req_tx.send(WorkerReq::QuickTop);
        }
        let _ = req_tx.send(WorkerReq::SyncCatalog);
    }
    let _ = req_tx.send(WorkerReq::Sync);

    let loop_result = event_loop(
        &mut terminal,
        &mut model,
        &mut spectrum,
        &mut tap_buf,
        &engine,
        &req_tx,
        &msg_rx,
    );

    let no_emoji = match tier {
        ColorTier::Truecolor => !model.glyphs.emoji_flags,
        ColorTier::Ansi16 => config.no_emoji,
    };
    // rewriting the config drops the legacy keys. when the migration could not
    // be persisted, they are the user's only remaining copy — leave them.
    match migration_pending {
        true => {
            crate::log_warn!("startup: keeping the legacy config keys, the profile is unwritten")
        }
        false => exit_config(&model, no_emoji).save(&data.join("config.toml")),
    }
    let restore_result = restore_terminal(&mut terminal);
    // state is saved on every mutation, so exit does not wait for the worker;
    // signal shutdown best-effort and return immediately.
    let _ = req_tx.send(WorkerReq::Shutdown);
    drop(worker_handle);
    loop_result.and(restore_result)
}

/// takes up settings an old config still carries, and reports whether the
/// migration is still unwritten. adoption is once per device and the legacy
/// values live nowhere else, so a failed save must keep the config as it is
/// rather than let exit drop the keys.
fn adopt_legacy(
    profile: &mut radio_core::sync::Profile,
    legacy: &crate::tui::config::LegacySettings,
    profile_path: &std::path::Path,
    now: i64,
) -> bool {
    let adopted = profile.adopt_existing(
        &legacy.countries,
        legacy.scope.as_deref().unwrap_or(""),
        legacy.theme.as_deref().unwrap_or(""),
        now,
    );
    if !adopted {
        return false;
    }
    match profile.save(profile_path) {
        Ok(()) => false,
        Err(e) => {
            crate::log_warn!("startup: failed to save the adopted profile: {e}");
            true
        }
    }
}

/// pushes the profile into the ui at startup. this is the only seed for the
/// filter, the scope and the theme — the defect this replaces read them from
/// `config.toml`, which a sync could never update. a value the profile has not
/// chosen, or one this build does not recognise, leaves the default in place
/// rather than resetting the user.
fn seed_from_profile(model: &mut Model) {
    model.browse.filters.countries = model.profile.countries.clone();
    if let Some(status) = update::scope_to_status_filter(&model.profile.scope) {
        model.browse.filters.status = status;
    }
    if let Some(theme) = Theme::try_from_slug(&model.profile.theme) {
        model.theme = theme;
    }
}

/// what exit is still allowed to write. the filter, the scope and the theme are
/// deliberately absent: writing them here is exactly what used to overwrite the
/// values a sync had just brought in.
fn exit_config(model: &Model, no_emoji: bool) -> Config {
    Config {
        legacy_theme: None,
        legacy_filters: None,
        no_emoji,
        last_station: model.now.uuid.clone(),
        query: model.browse.query.clone(),
        fft_divisor: model.fft_divisor,
        crossfade: model.crossfade,
        spectrum_style: model.spectrum_style,
        keybindings: model.keymap.clone(),
    }
}

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = std::io::stdout().execute(LeaveAlternateScreen);
        default_hook(info);
    }));
}

fn pick_glyphs(config: &Config, no_emoji_flag: bool, tier: ColorTier) -> Glyphs {
    match tier {
        ColorTier::Ansi16 => Glyphs::ascii(),
        ColorTier::Truecolor => Glyphs::for_config(config.no_emoji || no_emoji_flag),
    }
}

fn setup_terminal() -> anyhow::Result<Terminal<CrosstermBackend<std::io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) -> anyhow::Result<()> {
    disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    model: &mut Model,
    spectrum: &mut Spectrum,
    tap_buf: &mut [f32],
    engine: &AudioEngine,
    req_tx: &Sender<WorkerReq>,
    msg_rx: &Receiver<Msg>,
) -> anyhow::Result<()> {
    terminal.draw(|f| view::view(model, f))?;
    loop {
        if model.should_quit {
            return Ok(());
        }

        let tick = if model.is_animating() {
            Duration::from_millis(66)
        } else {
            Duration::from_millis(150)
        };
        let start = Instant::now();
        let mut needs_redraw = false;

        if event::poll(tick)? {
            match event::read()? {
                Event::Key(key) => {
                    if let Some(msg) = key_to_msg(model, key) {
                        run_effects(update(model, msg), model, engine, req_tx);
                        needs_redraw = true;
                    }
                }
                Event::Resize(_, _) => needs_redraw = true,
                _ => {}
            }
        }

        if model.should_quit {
            return Ok(());
        }

        while let Some(status) = engine.poll_status() {
            run_effects(
                update(model, Msg::AudioStatus(status)),
                model,
                engine,
                req_tx,
            );
            needs_redraw = true;
        }

        while let Ok(msg) = msg_rx.try_recv() {
            run_effects(update(model, msg), model, engine, req_tx);
            needs_redraw = true;
        }

        if model.is_playing() && !model.spectrum_style.is_off() {
            let n = engine.read_tap(tap_buf);
            let width = terminal.size().map(|s| s.width).unwrap_or(80);
            let bars_width = (width.max(8) as usize).min(256);
            spectrum.set_divisor(model.fft_divisor);
            model.spectrum_bars = spectrum.analyze(&tap_buf[..n], bars_width);
            needs_redraw = true;
        }

        run_effects(
            update(model, Msg::Tick(Instant::now())),
            model,
            engine,
            req_tx,
        );
        if model.is_animating() {
            needs_redraw = true;
        }
        if needs_redraw {
            terminal.draw(|f| view::view(model, f))?;
        }

        let elapsed = start.elapsed();
        if elapsed < tick {
            std::thread::sleep(tick - elapsed);
        }
    }
}

fn run_effects(
    effects: Vec<Effect>,
    _model: &mut Model,
    engine: &AudioEngine,
    req_tx: &Sender<WorkerReq>,
) {
    for fx in effects {
        match fx {
            Effect::Search(term, filters) => {
                let _ = req_tx.send(WorkerReq::Search(term, filters));
            }
            Effect::LoadFacets => {
                let _ = req_tx.send(WorkerReq::LoadFacets);
            }
            Effect::PopularSeed => {
                let _ = req_tx.send(WorkerReq::PopularSeed);
            }
            Effect::Play(url) => engine.play(&url),
            Effect::StopAudio => engine.stop(),
            Effect::SetCrossfade(on) => engine.set_crossfade(on),
            Effect::ToggleFavorite(uuid) => {
                let _ = req_tx.send(WorkerReq::ToggleFavorite(uuid));
            }
            Effect::Blacklist(uuid) => {
                let _ = req_tx.send(WorkerReq::Blacklist(uuid));
            }
            Effect::ToggleExcludedCountry(code) => {
                let _ = req_tx.send(WorkerReq::ToggleExcludedCountry(code));
            }
            Effect::Recheck(uuid) => {
                let _ = req_tx.send(WorkerReq::Recheck(uuid));
            }
            Effect::RecheckAll => {
                let _ = req_tx.send(WorkerReq::RecheckAll);
            }
            Effect::Restart => {
                // fully restore the terminal first, then replace this process with
                // the freshly-written binary. the earlier i/o error came from
                // exec-ing while still in raw mode / the alternate screen with an
                // unflushed stdout — restoring and flushing before exec fixes it.
                use std::io::Write;
                let _ = disable_raw_mode();
                let mut out = std::io::stdout();
                let _ = out.execute(LeaveAlternateScreen);
                let _ = out.execute(crossterm::cursor::Show);
                let _ = out.flush();
                if let Ok(exe) = std::env::current_exe() {
                    use std::os::unix::process::CommandExt;
                    // exec replaces the image in place; it only returns on failure.
                    let err = std::process::Command::new(exe)
                        .args(std::env::args_os().skip(1))
                        .exec();
                    let _ = writeln!(
                        std::io::stderr(),
                        "could not relaunch ({err}) — run r4dio again to use the new version"
                    );
                }
                std::process::exit(0);
            }
            Effect::RecordHistory(uuid) => {
                let _ = req_tx.send(WorkerReq::RecordHistory(uuid));
            }
            Effect::MarkFailed(uuid) => {
                let _ = req_tx.send(WorkerReq::MarkFailed(uuid));
            }
            Effect::MarkSuccess(uuid) => {
                let _ = req_tx.send(WorkerReq::MarkSuccess(uuid));
            }
            Effect::MirrorAnnounce { uuid, name, url } => {
                let _ = req_tx.send(WorkerReq::MirrorAnnounce { uuid, name, url });
            }
            Effect::SaveState => {
                let _ = req_tx.send(WorkerReq::SaveState);
            }
            Effect::SaveProfile(profile) => {
                let _ = req_tx.send(WorkerReq::SaveProfile(profile));
            }
            Effect::Sync => {
                let _ = req_tx.send(WorkerReq::Sync);
            }
            Effect::SyncCreate => {
                let _ = req_tx.send(WorkerReq::SyncCreate);
            }
            Effect::SyncLogout => {
                let _ = req_tx.send(WorkerReq::SyncLogout);
            }
            Effect::SyncDelete => {
                let _ = req_tx.send(WorkerReq::SyncDelete);
            }
            Effect::CheckUpdate => {
                let _ = req_tx.send(WorkerReq::CheckUpdate);
            }
            Effect::Update(rel) => {
                let _ = req_tx.send(WorkerReq::Update(rel));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radio_core::mirror::{MirrorEvent, StreamEvent};

    fn play_event() -> StreamEvent {
        StreamEvent::Play(MirrorEvent {
            uuid: "u1".into(),
            name: "One".into(),
            url: "http://x/1".into(),
            origin: "devA".into(),
            seq: 1,
        })
    }

    #[test]
    fn a_doorbell_queues_one_resync() {
        let (msg_tx, _msg_rx) = channel::<Msg>();
        let (req_tx, req_rx) = channel::<WorkerReq>();
        let queued = AtomicBool::new(false);

        dispatch_stream_event(StreamEvent::ProfileChanged, &msg_tx, &req_tx, &queued);

        assert!(matches!(req_rx.try_recv(), Ok(WorkerReq::SyncQuiet)));
        assert!(req_rx.try_recv().is_err());
    }

    // the debounce: a burst while a sync is still queued must not pile up one
    // request per event.
    #[test]
    fn two_rapid_doorbells_still_queue_only_one_resync() {
        let (msg_tx, _msg_rx) = channel::<Msg>();
        let (req_tx, req_rx) = channel::<WorkerReq>();
        let queued = AtomicBool::new(false);

        dispatch_stream_event(StreamEvent::ProfileChanged, &msg_tx, &req_tx, &queued);
        dispatch_stream_event(StreamEvent::ProfileChanged, &msg_tx, &req_tx, &queued);
        dispatch_stream_event(StreamEvent::ProfileChanged, &msg_tx, &req_tx, &queued);

        assert!(matches!(req_rx.try_recv(), Ok(WorkerReq::SyncQuiet)));
        assert!(
            req_rx.try_recv().is_err(),
            "the burst must collapse into one"
        );
    }

    // once the worker has run that sync it clears the flag, and the next event
    // must be able to queue again — otherwise live updates stop after the first.
    #[test]
    fn a_doorbell_after_the_sync_ran_queues_again() {
        let (msg_tx, _msg_rx) = channel::<Msg>();
        let (req_tx, req_rx) = channel::<WorkerReq>();
        let queued = AtomicBool::new(false);

        dispatch_stream_event(StreamEvent::ProfileChanged, &msg_tx, &req_tx, &queued);
        assert!(matches!(req_rx.try_recv(), Ok(WorkerReq::SyncQuiet)));

        // what WorkerReq::SyncQuiet does when the worker finishes it.
        queued.store(false, Ordering::SeqCst);

        dispatch_stream_event(StreamEvent::ProfileChanged, &msg_tx, &req_tx, &queued);
        assert!(matches!(req_rx.try_recv(), Ok(WorkerReq::SyncQuiet)));
    }

    fn profile_with(countries: &[&str], scope: &str, theme: &str) -> radio_core::sync::Profile {
        let mut p = radio_core::sync::Profile::default();
        p.set_countries(countries.iter().map(|c| c.to_string()).collect(), 100);
        p.set_scope(scope, 100);
        p.set_theme(theme, 100);
        p
    }

    fn legacy_of(raw: &str) -> crate::tui::config::LegacySettings {
        Config::from_toml_str(raw).unwrap().legacy_settings()
    }

    const OLD_CONFIG: &str =
        "theme = \"monokai\"\n[filters]\nstatus = \"all\"\ncountries = [\"UA\"]\n";

    // a successful adoption is written and the config is then free to drop the
    // legacy keys.
    #[test]
    fn a_written_adoption_leaves_no_migration_pending() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profile.json");
        let mut profile = radio_core::sync::Profile::default();

        let pending = adopt_legacy(&mut profile, &legacy_of(OLD_CONFIG), &path, 500);

        assert!(!pending);
        assert_eq!(
            radio_core::sync::Profile::load(&path).countries,
            vec!["UA".to_string()],
            "the adoption never reached the disk"
        );
    }

    // the unrecoverable path: the profile could not be written, so the legacy
    // values exist nowhere but config.toml and the config must keep them.
    #[test]
    fn an_unwritable_profile_leaves_the_migration_pending() {
        let dir = tempfile::tempdir().unwrap();
        // a file where the parent directory must be: create_dir_all fails, so
        // the save fails for a real filesystem reason.
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, "not a directory").unwrap();
        let path = blocker.join("profile.json");
        let mut profile = radio_core::sync::Profile::default();

        let pending = adopt_legacy(&mut profile, &legacy_of(OLD_CONFIG), &path, 500);

        assert!(pending, "a failed migration save must be reported");
    }

    // nothing to adopt is not a failure: the config may still be rewritten.
    #[test]
    fn adopting_nothing_leaves_no_migration_pending() {
        let dir = tempfile::tempdir().unwrap();
        let mut profile = radio_core::sync::Profile::default();
        let legacy = Config::default().legacy_settings();
        assert!(!adopt_legacy(
            &mut profile,
            &legacy,
            &dir.path().join("profile.json"),
            500
        ));
    }

    // an already-stamped profile has nothing to adopt, so a save that would
    // fail is never attempted and the config is still free to be rewritten.
    #[test]
    fn an_already_migrated_device_leaves_no_migration_pending() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, "not a directory").unwrap();
        let mut profile = profile_with(&["PL"], "favorites", "nord");

        let pending = adopt_legacy(
            &mut profile,
            &legacy_of(OLD_CONFIG),
            &blocker.join("profile.json"),
            500,
        );

        assert!(!pending);
        assert_eq!(
            profile.countries,
            vec!["PL".to_string()],
            "stamped value lost"
        );
    }

    // the whole defect: device b changed the filter while this tui was closed,
    // the sync wrote it into profile.json, and launching must show that value —
    // not whatever config.toml happened to hold.
    #[test]
    fn startup_seeds_the_filter_and_scope_from_the_profile() {
        let mut model = Model::new(Theme::AmberCrt, ColorTier::Truecolor, Glyphs::ascii());
        model.profile = profile_with(&["PL", "DE"], "favorites", "nord");
        seed_from_profile(&mut model);
        assert_eq!(
            model.browse.filters.countries,
            vec!["PL".to_string(), "DE".to_string()]
        );
        assert_eq!(
            model.browse.filters.status,
            crate::tui::model::StatusFilter::Favorites
        );
    }

    #[test]
    fn startup_takes_the_theme_from_the_profile() {
        let mut model = Model::new(Theme::AmberCrt, ColorTier::Truecolor, Glyphs::ascii());
        model.profile = profile_with(&[], "all", "monokai");
        seed_from_profile(&mut model);
        assert_eq!(model.theme, Theme::Monokai);
    }

    // an unset or unknown profile theme must leave the built-in default in
    // place rather than blanking the ui.
    #[test]
    fn an_empty_profile_theme_keeps_the_default() {
        let mut model = Model::new(Theme::Nord, ColorTier::Truecolor, Glyphs::ascii());
        model.profile = radio_core::sync::Profile::default();
        seed_from_profile(&mut model);
        assert_eq!(model.theme, Theme::Nord);
        assert!(model.browse.filters.countries.is_empty());
        assert_eq!(
            model.browse.filters.status,
            crate::tui::model::StatusFilter::All
        );
    }

    #[test]
    fn an_unknown_profile_theme_keeps_the_default() {
        let mut model = Model::new(Theme::Nord, ColorTier::Truecolor, Glyphs::ascii());
        model.profile = profile_with(&[], "all", "midnight");
        seed_from_profile(&mut model);
        assert_eq!(model.theme, Theme::Nord);
    }

    // an unrecognised scope must not reset the user to "all".
    #[test]
    fn an_unknown_profile_scope_keeps_the_default() {
        let mut model = Model::new(Theme::AmberCrt, ColorTier::Truecolor, Glyphs::ascii());
        model.browse.filters.status = crate::tui::model::StatusFilter::Dead;
        model.profile = profile_with(&[], "nonsense", "");
        seed_from_profile(&mut model);
        assert_eq!(
            model.browse.filters.status,
            crate::tui::model::StatusFilter::Dead
        );
    }

    // a fresh install has nothing to rescue: adoption must stamp nothing, or the
    // new device would publish empty settings over another device's real ones.
    #[test]
    fn a_fresh_install_adopts_nothing() {
        let mut profile = radio_core::sync::Profile::default();
        let legacy = Config::default().legacy_settings();
        let adopted = profile.adopt_existing(
            &legacy.countries,
            legacy.scope.as_deref().unwrap_or(""),
            legacy.theme.as_deref().unwrap_or(""),
            999,
        );
        assert!(!adopted);
        assert_eq!(profile, radio_core::sync::Profile::default());
    }

    // the end-to-end shape of the defect: an old config still says UA/monokai,
    // a sync has already put device b's choice in the profile, and a whole
    // launch-then-exit cycle must show device b's values and write back neither.
    #[test]
    fn a_synced_filter_survives_a_launch_that_still_has_an_old_config() {
        let old = Config::from_toml_str(
            "theme = \"monokai\"\n[filters]\nstatus = \"all\"\ncountries = [\"UA\"]\n",
        )
        .unwrap();
        let mut model = Model::new(Theme::from_slug(""), ColorTier::Truecolor, Glyphs::ascii());
        model.profile = profile_with(&["PL", "DE"], "favorites", "nord");

        let legacy = old.legacy_settings();
        let adopted = model.profile.adopt_existing(
            &legacy.countries,
            legacy.scope.as_deref().unwrap_or(""),
            legacy.theme.as_deref().unwrap_or(""),
            999,
        );
        assert!(!adopted, "already-stamped fields must not be adopted");
        seed_from_profile(&mut model);

        assert_eq!(
            model.browse.filters.countries,
            vec!["PL".to_string(), "DE".to_string()],
            "the ui took the stale config filter"
        );
        assert_eq!(
            model.browse.filters.status,
            crate::tui::model::StatusFilter::Favorites
        );
        assert_eq!(model.theme, Theme::Nord);
        assert_eq!(model.profile.countries_at, 100, "the stamp moved");

        let out = exit_config(&model, false).to_toml_string();
        assert!(
            !out.contains("UA"),
            "exit resurrected the old filter: {out}"
        );
        assert!(!out.contains("PL"), "exit wrote the filter back: {out}");
    }

    // the clobber this task removes: exiting used to write the filter and the
    // theme back into config.toml, so the next launch read the stale copy.
    #[test]
    fn the_exit_write_carries_neither_the_filter_nor_the_theme() {
        let mut model = Model::new(Theme::Monokai, ColorTier::Truecolor, Glyphs::ascii());
        model.browse.filters.countries = vec!["UA".into()];
        model.browse.filters.status = crate::tui::model::StatusFilter::Favorites;
        let out = exit_config(&model, false).to_toml_string();
        assert!(!out.contains("[filters]"), "filters were written: {out}");
        assert!(!out.contains("UA"), "countries were written: {out}");
        assert!(!out.contains("monokai"), "theme was written: {out}");
    }

    // everything else the exit write owns must survive the split.
    #[test]
    fn the_exit_write_still_carries_the_view_settings() {
        let mut model = Model::new(Theme::AmberCrt, ColorTier::Truecolor, Glyphs::ascii());
        model.browse.query = "jazz".into();
        model.fft_divisor = 7.0;
        model.crossfade = false;
        model.spectrum_style = crate::tui::model::SpectrumStyle::Wave;
        model.now.uuid = Some("uuid-1".into());
        let cfg = exit_config(&model, true);
        assert_eq!(cfg.query, "jazz");
        assert_eq!(cfg.fft_divisor, 7.0);
        assert!(!cfg.crossfade);
        assert_eq!(cfg.spectrum_style, crate::tui::model::SpectrumStyle::Wave);
        assert_eq!(cfg.last_station.as_deref(), Some("uuid-1"));
        assert!(cfg.no_emoji);
    }

    #[test]
    fn a_play_event_mirrors_and_never_queues_a_resync() {
        let (msg_tx, msg_rx) = channel::<Msg>();
        let (req_tx, req_rx) = channel::<WorkerReq>();
        let queued = AtomicBool::new(false);

        dispatch_stream_event(play_event(), &msg_tx, &req_tx, &queued);

        assert!(matches!(msg_rx.try_recv(), Ok(Msg::MirrorPlay(_))));
        assert!(req_rx.try_recv().is_err(), "a play must not trigger a sync");
        assert!(!queued.load(Ordering::SeqCst));
    }
}
