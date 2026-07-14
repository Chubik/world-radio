mod sync_cmd;
mod tui;

use clap::{Parser, Subcommand};
use radio_audio::AudioEngine;
use radio_core::catalog::{api, Cache, Catalog, Health, SearchQuery};
use radio_core::paths;

#[derive(Parser)]
#[command(name = "r4dio")]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    country: Option<String>,
    #[arg(long)]
    tag: Option<String>,
    #[arg(long)]
    bitrate_min: Option<u32>,
    #[arg(long, help = "search local cache only")]
    offline: bool,
    #[arg(long, help = "use [FR] codes instead of emoji flags")]
    no_emoji: bool,
}

#[derive(Subcommand)]
enum Cmd {
    Play {
        url: String,
    },
    Sync {
        #[command(subcommand)]
        action: sync_cmd::SyncCmd,
    },
    Update,
    SyncCatalog,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if let Some(Cmd::Sync { action }) = &cli.command {
        return sync_cmd::run(action);
    }

    if let Some(Cmd::Update) = &cli.command {
        return run_update();
    }

    if let Some(Cmd::SyncCatalog) = &cli.command {
        return run_sync_catalog();
    }

    if let Some(Cmd::Play { url }) = &cli.command {
        let engine = AudioEngine::spawn()?;
        engine.play(url);
        println!("playing {url} (ctrl-c to stop)");
        loop {
            if let Some(status) = engine.poll_status() {
                println!("status: {status:?}");
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }

    if cli.offline
        || cli.name.is_some()
        || cli.country.is_some()
        || cli.tag.is_some()
        || cli.bitrate_min.is_some()
    {
        return search_cli(&cli);
    }

    radio_core::single_instance::take_over();
    tui::run(cli.no_emoji)
}

fn search_cli(cli: &Cli) -> anyhow::Result<()> {
    let data = paths::ensure_data_dir()?;
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

    let query = SearchQuery {
        name: cli.name.clone(),
        countrycode: cli.country.clone(),
        tag: cli.tag.clone(),
        bitrate_min: cli.bitrate_min,
        ..Default::default()
    };

    if cli.offline {
        let term = cli.name.clone().unwrap_or_default();
        for s in catalog.search_offline(&term)? {
            println!(
                "{:<40} {:>3} {:>4}kbps {}",
                s.name, s.countrycode, s.bitrate, s.codec
            );
        }
        return Ok(());
    }

    let rb = api::resolve();
    let stations = rb.search(&query)?;
    catalog.ingest(&stations)?;
    for s in &stations {
        println!(
            "{:<40} {:>3} {:>4}kbps {}",
            s.name, s.countrycode, s.bitrate, s.codec
        );
    }
    println!(
        "\n{} stations (cached to {})",
        stations.len(),
        data.display()
    );
    Ok(())
}

fn run_sync_catalog() -> anyhow::Result<()> {
    let data = paths::ensure_data_dir()?;
    let cache = Cache::open(&data.join("stations.db"))?;
    let rb = api::resolve();
    let stations = rb.fetch_all()?;
    let n = cache.replace_all(&stations)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    cache.set_last_sync(now)?;
    println!("synced {n} stations");
    Ok(())
}

fn run_update() -> anyhow::Result<()> {
    match radio_core::update::fetch_latest()? {
        None => println!(
            "already up to date (v{})",
            radio_core::update::current_version()
        ),
        Some(rel) => {
            println!("updating to v{}…", rel.version);
            radio_core::update::apply(&rel)?;
            println!("updated to v{} — restart to apply", rel.version);
        }
    }
    Ok(())
}
