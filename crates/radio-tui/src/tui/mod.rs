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
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

const TAP_SAMPLES: usize = 2048;

pub fn run(no_emoji_flag: bool) -> anyhow::Result<()> {
    install_panic_hook();
    let data = paths::ensure_data_dir()?;
    logger::init(&data.join("world-radio.log"));
    let config = Config::load(&data.join("config.toml"));
    let theme = Theme::from_slug(&config.theme);
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
    };
    let worker_handle = worker::spawn(catalog, worker_paths, req_rx, msg_tx.clone());

    let mirror_tx = msg_tx.clone();
    std::thread::spawn(move || loop {
        let Some(key) = radio_core::sync::load_key() else {
            std::thread::sleep(std::time::Duration::from_secs(10));
            continue;
        };
        let client = radio_core::mirror::MirrorClient::new("https://r4dio.net");
        let tx = mirror_tx.clone();
        let stream_key = key.clone();
        let _ = client.events(&key, |evt| {
            if radio_core::sync::load_key().as_deref() != Some(stream_key.as_str()) {
                return;
            }
            let _ = tx.send(Msg::MirrorPlay(evt));
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
    model.browse.filters = config.filters.clone();
    model.browse.excluded_countries = excluded_countries;
    model.fft_divisor = config.fft_divisor;
    model.crossfade = config.crossfade;
    model.spectrum_style = config.spectrum_style;
    model.keymap = config.keybindings.clone();
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
    let out_cfg = Config {
        theme: model.theme.slug().to_string(),
        no_emoji,
        last_station: model.now.uuid.clone(),
        query: model.browse.query.clone(),
        fft_divisor: model.fft_divisor,
        crossfade: model.crossfade,
        spectrum_style: model.spectrum_style,
        keybindings: model.keymap.clone(),
        filters: model.browse.filters.clone(),
    };
    out_cfg.save(&data.join("config.toml"));
    let restore_result = restore_terminal(&mut terminal);
    let _ = req_tx.send(WorkerReq::SaveState);
    let _ = req_tx.send(WorkerReq::Shutdown);
    if let Err(e) = worker_handle.join() {
        eprintln!("worker thread panicked: {e:?}");
    }
    loop_result.and(restore_result)
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
            Effect::MirrorAnnounce { uuid, name, url } => {
                let _ = req_tx.send(WorkerReq::MirrorAnnounce { uuid, name, url });
            }
            Effect::SaveState => {
                let _ = req_tx.send(WorkerReq::SaveState);
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
