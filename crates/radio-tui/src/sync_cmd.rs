use clap::Subcommand;
use radio_core::catalog::Favorites;
use radio_core::paths;
use radio_core::sync::{self, SyncClient, SyncData};

const SERVER: &str = "https://r4dio.net";

#[derive(Subcommand)]
pub enum SyncCmd {
    Login,
    Status,
    Logout,
    Delete,
    Run,
}

pub fn run(cmd: &SyncCmd) -> anyhow::Result<()> {
    match cmd {
        SyncCmd::Login => login(),
        SyncCmd::Status => status(),
        SyncCmd::Logout => logout(),
        SyncCmd::Delete => delete(),
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

fn run_sync() -> anyhow::Result<()> {
    let Some(key) = sync::load_key() else {
        println!("not linked (run: world-radio sync login)");
        return Ok(());
    };
    let favs = Favorites::load(&fav_path());
    let blocked = Favorites::load(&blacklist_path());
    let local = SyncData {
        favs: favs.ids().to_vec(),
        blocked: blocked.ids().to_vec(),
    };
    let merged = client().push(&key, &local)?;
    favorites_from(merged.favs.clone()).save(&fav_path())?;
    favorites_from(merged.blocked.clone()).save(&blacklist_path())?;
    println!(
        "synced: {} favourites, {} blocked",
        merged.favs.len(),
        merged.blocked.len()
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
}
