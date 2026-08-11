use clap::Subcommand;
use radio_core::catalog::{Favorites, History};
use radio_core::paths;
use radio_core::sync::{self, session, Pending, Profile, SyncClient, SyncData};

const SERVER: &str = "https://r4dio.net";

#[derive(Subcommand)]
pub enum SyncCmd {
    Login,
    Status,
    Logout,
    Delete,
    Use { key: String },
    Run,
}

pub fn run(cmd: &SyncCmd) -> anyhow::Result<()> {
    match cmd {
        SyncCmd::Login => login(),
        SyncCmd::Status => status(),
        SyncCmd::Logout => logout(),
        SyncCmd::Delete => delete(),
        SyncCmd::Use { key } => use_key(key),
        SyncCmd::Run => run_sync(),
    }
}

fn client() -> SyncClient {
    SyncClient::new(SERVER)
}

fn fav_path() -> std::path::PathBuf {
    paths::data_dir().join("favorites.json")
}

fn blacklist_path() -> std::path::PathBuf {
    paths::data_dir().join("blacklist.json")
}

fn excluded_path() -> std::path::PathBuf {
    paths::data_dir().join("excluded_countries.json")
}

fn pending_path() -> std::path::PathBuf {
    paths::data_dir().join("sync_pending.json")
}

fn profile_path() -> std::path::PathBuf {
    paths::data_dir().join("profile.json")
}

fn history_path() -> std::path::PathBuf {
    paths::data_dir().join("history.json")
}

fn favorites_from(ids: Vec<String>) -> Favorites {
    let mut f = Favorites::new();
    for id in ids {
        match f.contains(&id) {
            true => {}
            false => {
                f.toggle(&id);
            }
        }
    }
    f
}

fn print_key_qr(key: &str) {
    let code = match qrcode::QrCode::with_error_correction_level(key, qrcode::EcLevel::M) {
        Err(_) => {
            println!("key: {key}");
            return;
        }
        Ok(c) => c,
    };
    let width = code.width();
    let quiet = 4;
    let colors = code.to_colors();
    let dark = |x: i64, y: i64| -> bool {
        if x < 0 || y < 0 || x >= width as i64 || y >= width as i64 {
            return false;
        }
        colors[y as usize * width + x as usize] == qrcode::Color::Dark
    };
    let white = "\x1b[107m  \x1b[0m";
    let black = "\x1b[40m  \x1b[0m";
    for y in -quiet..width as i64 + quiet {
        let mut line = String::new();
        for x in -quiet..width as i64 + quiet {
            line.push_str(match dark(x, y) {
                true => black,
                false => white,
            });
        }
        line.push_str("\x1b[0m");
        println!("{line}");
    }
    println!("key: {key}");
}

fn login() -> anyhow::Result<()> {
    match sync::load_key() {
        Some(key) => {
            println!("already linked");
            print_key_qr(&key);
        }
        None => {
            let key = client().create_account()?;
            sync::store_key(&key)?;
            println!("account created and linked");
            print_key_qr(&key);
        }
    }
    Ok(())
}

fn status() -> anyhow::Result<()> {
    match sync::load_key() {
        None => println!("not linked (run: world-radio sync login)"),
        Some(key) => {
            print_key_qr(&key);
            let data = client().pull(&key)?;
            println!(
                "server: {} favourites, {} blocked",
                data.favs.len(),
                data.blocked.len()
            );
        }
    }
    Ok(())
}

fn logout() -> anyhow::Result<()> {
    sync::clear_key()?;
    println!("logged out (favourites kept locally)");
    Ok(())
}

fn delete() -> anyhow::Result<()> {
    match sync::load_key() {
        None => println!("not linked"),
        Some(key) => {
            client().delete(&key)?;
            sync::clear_key()?;
            println!("account deleted and unlinked");
        }
    }
    Ok(())
}

/// the files one round-trip reads and writes, injected so the round-trip is
/// testable against a mock server rather than the real data dir.
struct CliPaths {
    fav: std::path::PathBuf,
    blacklist: std::path::PathBuf,
    excluded: std::path::PathBuf,
    pending: std::path::PathBuf,
    profile: std::path::PathBuf,
    history: std::path::PathBuf,
}

fn cli_paths() -> CliPaths {
    CliPaths {
        fav: fav_path(),
        blacklist: blacklist_path(),
        excluded: excluded_path(),
        pending: pending_path(),
        profile: profile_path(),
        history: history_path(),
    }
}

/// one round-trip against the server from the files on disk. `sync run` and
/// `sync use` differ only in their message, so they share this: a first link
/// that skipped the profile would leave the new device without it entirely.
fn push_and_apply(client: &SyncClient, key: &str, paths: &CliPaths) -> anyhow::Result<SyncData> {
    let profile = Profile::load(&paths.profile);
    let history = History::load(&paths.history);
    let local = session::outgoing(session::LocalState {
        favs: Favorites::load(&paths.fav).ids().to_vec(),
        blocked: Favorites::load(&paths.blacklist).ids().to_vec(),
        excluded_countries: Favorites::load(&paths.excluded).ids().to_vec(),
        changed: Pending::load(&paths.pending),
        profile: &profile,
        plays: history.plays(),
    });
    let merged = client.push(key, &local)?;
    // remove exactly what we just sent; keep anything another surface wrote
    // to the log during the round-trip (a plain clear would destroy it).
    Pending::clear_pushed(&local.changed, &paths.pending)?;
    favorites_from(merged.favs.clone()).save(&paths.fav)?;
    favorites_from(merged.blocked.clone()).save(&paths.blacklist)?;
    favorites_from(merged.excluded_countries.clone()).save(&paths.excluded)?;

    let mut profile = profile;
    let changed = session::apply_remote_profile(
        &mut profile,
        &merged.shuffle_filter,
        &merged.scope,
        &merged.theme,
    );
    if changed.any() {
        profile.save(&paths.profile)?;
    }
    if let Some(plays) = session::merge_history(history.plays(), &merged.history) {
        let mut history = History::new();
        history.set_from(plays);
        history.save(&paths.history)?;
    }
    Ok(merged)
}

fn run_sync() -> anyhow::Result<()> {
    let Some(key) = sync::load_key() else {
        println!("not linked (run: world-radio sync login)");
        return Ok(());
    };
    let merged = push_and_apply(&client(), &key, &cli_paths())?;
    println!(
        "synced: {} favourites, {} blocked, {} excluded countries",
        merged.favs.len(),
        merged.blocked.len(),
        merged.excluded_countries.len()
    );
    Ok(())
}

fn use_key(key: &str) -> anyhow::Result<()> {
    if !sync::is_valid_format(key) {
        println!("invalid key");
        return Ok(());
    }
    sync::store_key(key)?;
    // union-merging local state with the server's here would resurrect anything
    // this device deleted before linking; send our state plus our delta and let
    // the server's authoritative per-item merge decide, same as run_sync.
    let stored = push_and_apply(&client(), key, &cli_paths())?;
    println!(
        "linked and merged: {} favourites, {} blocked, {} excluded countries",
        stored.favs.len(),
        stored.blocked.len(),
        stored.excluded_countries.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn favorites_from_builds_ordered_set() {
        let f = favorites_from(vec!["a".to_string(), "b".into(), "c".into()]);
        assert_eq!(f.ids(), &["a".to_string(), "b".into(), "c".into()]);
    }

    #[test]
    fn favorites_from_dedups_without_dropping() {
        let f = favorites_from(vec!["a".to_string(), "b".into(), "a".into()]);
        assert_eq!(f.ids(), &["a".to_string(), "b".into()]);
    }

    fn paths_in(dir: &std::path::Path) -> CliPaths {
        CliPaths {
            fav: dir.join("favorites.json"),
            blacklist: dir.join("blacklist.json"),
            excluded: dir.join("excluded_countries.json"),
            pending: dir.join("sync_pending.json"),
            profile: dir.join("profile.json"),
            history: dir.join("history.json"),
        }
    }

    #[test]
    fn the_cli_publishes_the_local_profile_and_history() {
        // the defect: `sync run` sent ..Default::default(), so a cli-only user
        // never pushed their filters, scope, theme or history at all.
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let mut p = Profile::default();
        p.set_countries(vec!["UA".into()], 10);
        p.set_scope("dead", 20);
        p.set_theme("nord", 30);
        p.save(&paths.profile).unwrap();
        let mut h = History::new();
        h.record("s1", 40);
        h.save(&paths.history).unwrap();

        let mut server = mockito::Server::new();
        let m = server
            .mock("PUT", "/sync")
            .match_body(mockito::Matcher::AllOf(vec![
                mockito::Matcher::Regex(
                    r#""shuffle_filter":\{"value":\{"countries":\["UA"\]\},"at":10\}"#.into(),
                ),
                mockito::Matcher::Regex(r#""scope":\{"value":"dead","at":20\}"#.into()),
                mockito::Matcher::Regex(r#""theme":\{"value":"nord","at":30\}"#.into()),
                mockito::Matcher::Regex(
                    r#""history":\[\{"id":"s1","at":40,"gone":false\}\]"#.into(),
                ),
            ]))
            .with_body(r#"{"favs":[],"blocked":[]}"#)
            .create();
        push_and_apply(&SyncClient::new(server.url()), "r4-k", &paths).unwrap();
        m.assert();
    }

    #[test]
    fn the_cli_lands_a_newer_remote_profile_and_history_on_disk() {
        // a first link must bring the profile across, not just favourites.
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let mut server = mockito::Server::new();
        server
            .mock("PUT", "/sync")
            .with_body(
                r#"{"favs":[],"blocked":[],
                    "shuffle_filter":{"value":{"countries":["PL"]},"at":900},
                    "scope":{"value":"blocked","at":900},
                    "theme":{"value":"cyber-neon","at":900},
                    "history":[{"id":"remote","at":800,"gone":false}]}"#,
            )
            .create();
        push_and_apply(&SyncClient::new(server.url()), "r4-k", &paths).unwrap();

        let landed = Profile::load(&paths.profile);
        assert_eq!(landed.countries, vec!["PL".to_string()]);
        assert_eq!(landed.scope, "blocked");
        assert_eq!(landed.theme, "cyber-neon");
        assert_eq!(landed.scope_at, 900);
        assert_eq!(
            History::load(&paths.history).ids(),
            vec!["remote".to_string()]
        );
    }

    #[test]
    fn an_older_remote_profile_does_not_overwrite_the_local_one() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let mut p = Profile::default();
        p.set_scope("favorites", 1_000);
        p.save(&paths.profile).unwrap();

        let mut server = mockito::Server::new();
        server
            .mock("PUT", "/sync")
            .with_body(r#"{"favs":[],"blocked":[],"scope":{"value":"all","at":5}}"#)
            .create();
        push_and_apply(&SyncClient::new(server.url()), "r4-k", &paths).unwrap();
        assert_eq!(Profile::load(&paths.profile).scope, "favorites");
    }

    #[test]
    fn an_untouched_cli_device_sends_no_profile_keys_at_all() {
        // a pre-upgrade server must keep seeing the old payload verbatim.
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let mut server = mockito::Server::new();
        let m = server
            .mock("PUT", "/sync")
            .match_body(r#"{"favs":[],"blocked":[],"excluded_countries":[]}"#)
            .with_body(r#"{"favs":[],"blocked":[]}"#)
            .create();
        push_and_apply(&SyncClient::new(server.url()), "r4-k", &paths).unwrap();
        m.assert();
    }
}
