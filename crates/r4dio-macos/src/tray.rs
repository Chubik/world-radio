// the panel cannot be shown over another app's fullscreen space — that is an
// appkit constraint, not a bug we can work around. the tray menu and the global
// shortcut are therefore the only controls that reach a user who works
// fullscreen, so both are built to work with no window on screen at all.

// tauri parses this into the ⌥⇧R glyphs macos draws beside the menu item, so the
// menu label and the registered shortcut cannot drift apart.
pub const SHUFFLE_ACCELERATOR: &str = "Alt+Shift+R";

pub const SHUFFLE_ALL: &str = "Shuffle — all stations";
pub const SHUFFLE_FAVOURITES: &str = "Shuffle — favorites";
pub const OPEN: &str = "Open r4dio";
pub const QUIT: &str = "Quit r4dio";

// the account line doubles as the way into the account section, so it has to say
// something even when no key is stored yet.
pub const SIGNED_OUT: &str = "Set up sync";

// play and add-to-favorites read the current state, because a menu that always
// says "Play" leaves the user guessing which way it will go.
pub fn playstop_label(playing: bool) -> &'static str {
    match playing {
        true => "Stop",
        false => "Play",
    }
}

pub fn favorite_label(is_favorite: bool) -> &'static str {
    match is_favorite {
        true => "Remove from favorites",
        false => "Add to favorites",
    }
}

// the menu must never carry a full key: it is drawn over whatever the user is
// screen-sharing or presenting. an empty mask means no account, not a blank row.
pub fn account_label(masked: &str) -> String {
    let trimmed = masked.trim();
    match trimmed.is_empty() {
        true => SIGNED_OUT.to_string(),
        false => trimmed.to_string(),
    }
}

pub fn update_label(version: &str) -> String {
    format!("Update to v{version}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playstop_reads_as_the_action_it_will_take() {
        assert_eq!(playstop_label(true), "Stop");
        assert_eq!(playstop_label(false), "Play");
    }

    #[test]
    fn favorite_label_flips_with_state() {
        assert_eq!(favorite_label(false), "Add to favorites");
        assert_eq!(favorite_label(true), "Remove from favorites");
    }

    #[test]
    fn account_label_shows_the_masked_id() {
        assert_eq!(account_label("r4-7K2P-····-4DF1"), "r4-7K2P-····-4DF1");
    }

    #[test]
    fn account_label_without_a_key_offers_setup() {
        assert_eq!(account_label(""), SIGNED_OUT);
        assert_eq!(account_label("   "), SIGNED_OUT);
    }

    // the row is drawn over shared screens, so a mask that arrived unmasked must
    // not be widened into a full key by this layer.
    #[test]
    fn account_label_does_not_lengthen_what_it_is_given() {
        let masked = "r4-tutg····sjza";
        assert_eq!(account_label(masked).len(), masked.len());
    }

    #[test]
    fn accelerator_is_the_combination_the_menu_advertises() {
        assert_eq!(SHUFFLE_ACCELERATOR, "Alt+Shift+R");
    }

    #[test]
    fn update_label_names_the_version_it_installs() {
        assert_eq!(update_label("1.17.0"), "Update to v1.17.0");
    }
}
