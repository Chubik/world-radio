use crate::commands::Shared;
use serde::Serialize;

const SERVER: &str = "https://r4dio.net";

const MASK: &str = "····";

#[derive(Serialize)]
pub struct AccountState {
    pub signed_in: bool,
    // never the full key: the ui may render only this form once the save-this
    // screen is gone.
    pub masked: String,
    pub favourites: u32,
}

// keeps the first and last segments so the account stays recognisable and drops
// everything between. a key with no segments is masked whole rather than shown,
// because failing towards hiding is the only safe direction for a secret.
pub fn mask_key(key: &str) -> String {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let parts: Vec<&str> = trimmed.split('-').collect();
    if parts.len() >= 4 {
        return format!(
            "{}-{}-{MASK}-{}",
            parts[0],
            parts[1],
            parts[parts.len() - 1]
        );
    }
    // real keys are one long run, not the segmented shape the mockup drew. showing
    // both ends tells two accounts apart; the body stays far too short to rebuild.
    // this mirrors maskKey in ui/labels.js — the two must agree or the tray and
    // the window would disagree about the same account.
    let body: String = parts[1..].join("-");
    if body.chars().count() <= EDGE * 2 {
        return format!("{}-{MASK}", parts[0]);
    }
    let head: String = body.chars().take(EDGE).collect();
    let tail: String = body
        .chars()
        .skip(body.chars().count() - EDGE)
        .collect::<String>();
    format!("{}-{head}{MASK}{tail}", parts[0])
}

const EDGE: usize = 4;

fn client() -> radio_core::sync::SyncClient {
    radio_core::sync::SyncClient::new(SERVER)
}

// the ui shows a real error rather than a silent no-op, so every fallible step
// is mapped to a string the user can act on.
#[tauri::command]
pub fn create_account() -> Result<String, String> {
    if radio_core::sync::load_key().is_some() {
        return Err("this Mac is already signed in".into());
    }
    let key = client()
        .create_account()
        .map_err(|_| "could not reach the sync server".to_string())?;
    radio_core::sync::store_key(&key)
        .map_err(|_| "could not save the key on this Mac".to_string())?;
    Ok(key)
}

#[tauri::command]
pub fn account_state(state: tauri::State<Shared>) -> AccountState {
    let Some(key) = radio_core::sync::load_key() else {
        return AccountState {
            signed_in: false,
            masked: String::new(),
            favourites: 0,
        };
    };
    AccountState {
        signed_in: true,
        masked: mask_key(&key),
        favourites: state.lock().unwrap().favourite_count(),
    }
}

#[tauri::command]
pub fn sign_in(state: tauri::State<Shared>, key: String) -> Result<(), String> {
    let key = key.trim().to_lowercase();
    if !radio_core::sync::is_valid_format(&key) {
        return Err("that does not look like an r4dio ID".into());
    }
    radio_core::sync::store_key(&key)
        .map_err(|_| "could not save the key on this Mac".to_string())?;
    // a bad key stores fine but fails here, so the stored key is rolled back
    // rather than leaving the app signed in to an account that does not exist.
    if let Err(e) = state.lock().unwrap().sync() {
        let _ = radio_core::sync::clear_key();
        eprintln!("sign in failed: {e}");
        return Err("could not sign in with that ID".into());
    }
    Ok(())
}

#[tauri::command]
pub fn sign_out() -> Result<(), String> {
    // local favourites stay: signing out unlinks this Mac, it does not erase it.
    radio_core::sync::clear_key().map_err(|_| "could not sign out".to_string())
}

#[tauri::command]
pub fn delete_account() -> Result<(), String> {
    let Some(key) = radio_core::sync::load_key() else {
        return Err("not signed in".into());
    };
    client()
        .delete(&key)
        .map_err(|_| "could not delete the account on the server".to_string())?;
    radio_core::sync::clear_key()
        .map_err(|_| "account deleted, but this Mac is still linked".to_string())
}

// the qr is built here so the key itself never has to reach the webview once the
// save-this screen is gone; the ui receives only a grid of dark/light cells.
#[tauri::command]
pub fn account_qr() -> Result<Vec<Vec<bool>>, String> {
    let key = radio_core::sync::load_key().ok_or_else(|| "not signed in".to_string())?;
    let code = qrcode::QrCode::with_error_correction_level(&key, qrcode::EcLevel::M)
        .map_err(|_| "could not build the QR code".to_string())?;
    let width = code.width();
    let colors = code.to_colors();
    Ok((0..width)
        .map(|y| {
            (0..width)
                .map(|x| colors[y * width + x] == qrcode::Color::Dark)
                .collect()
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_hides_middle_segments() {
        assert_eq!(mask_key("r4-7K2P-9QXM-4DF1"), "r4-7K2P-····-4DF1");
    }

    #[test]
    fn mask_never_contains_the_middle() {
        let masked = mask_key("r4-AAAA-SECRET-BBBB");
        assert!(!masked.contains("SECRET"), "leaked: {masked}");
    }

    #[test]
    fn unsegmented_key_shows_both_ends() {
        // the server issues one lowercase run, which is the shape that matters most
        assert_eq!(mask_key("r4-abc123def"), "r4-abc1····3def");
        // and it must agree with maskKey in ui/labels.js, or the tray menu and the
        // account section would print two different things for one account.
        assert_eq!(
            mask_key("r4-tutgsmisaqwertyuiopasdfghjklzxcvbnmqwertyuiopasdfg"),
            "r4-tutg····sdfg"
        );
    }

    #[test]
    fn an_unsegmented_mask_never_carries_the_middle() {
        assert!(!mask_key("r4-abcSECRETMIDdef").contains("SECRETMID"));
    }

    #[test]
    fn short_and_empty_keys_do_not_leak() {
        assert_eq!(mask_key("r4-ab"), "r4-····");
        assert_eq!(mask_key("r4-"), "r4-····");
        assert_eq!(mask_key(""), "");
        assert_eq!(mask_key("   "), "");
    }

    #[test]
    fn mask_of_a_real_server_key_keeps_only_its_ends() {
        // the ends are deliberate — they are what tells two accounts apart. the
        // middle is the part worth guessing from, so no chunk of it may survive.
        let key = "r4-7k2p9qxm4df1secret";
        let masked = mask_key(key);
        let body = key.strip_prefix("r4-").unwrap();
        assert!(!masked.contains(body), "leaked whole: {masked}");
        let middle = &body[EDGE..body.len() - EDGE];
        for len in 2..=middle.len() {
            for start in 0..=middle.len() - len {
                let chunk = &middle[start..start + len];
                assert!(!masked.contains(chunk), "leaked chunk {chunk} in {masked}");
            }
        }
        // and what is kept is exactly the two ends, nothing more.
        assert_eq!(masked, "r4-7k2p····cret");
    }

    #[test]
    fn mask_drops_every_middle_segment() {
        let masked = mask_key("r4-AAAA-BBBB-CCCC-DDDD");
        assert_eq!(masked, "r4-AAAA-····-DDDD");
        assert!(!masked.contains("BBBB") && !masked.contains("CCCC"));
    }
}
