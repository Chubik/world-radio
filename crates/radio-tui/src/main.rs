mod audio;
mod tui;

use audio::AudioEngine;
use clap::{Parser, Subcommand};
use radio_core::catalog::{api, Cache, Catalog, Health, SearchQuery};
use radio_core::paths;

#[derive(Parser)]
#[command(name = "world-radio")]
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
    Play { url: String },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

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

    tui::run(cli.no_emoji)
}

fn search_cli(cli: &Cli) -> anyhow::Result<()> {
    let data = paths::ensure_data_dir()?;
    let cache = Cache::open(&data.join("stations.db"))?;
    let health = Health::load(&data.join("station_health.json"));
    let catalog = Catalog::new(cache, health);

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
