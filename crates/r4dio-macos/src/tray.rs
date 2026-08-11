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
pub const SIGNED_IN: &str = "Sync";

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
// the row is a button into the sync screen, so it leads with the action — a bare
// key names nothing the user can act on.
pub fn account_label(masked: &str) -> String {
    let trimmed = masked.trim();
    match trimmed.is_empty() {
        true => SIGNED_OUT.to_string(),
        false => format!("{SIGNED_IN} · {trimmed}"),
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

    // a bare key reads as a stray string in the menu: it names no action and the
    // user cannot tell that clicking it opens anything.
    #[test]
    fn account_label_names_the_action_before_the_key() {
        assert_eq!(
            account_label("r4-7K2P-····-4DF1"),
            "Sync · r4-7K2P-····-4DF1"
        );
    }

    #[test]
    fn account_label_without_a_key_offers_setup() {
        assert_eq!(account_label(""), SIGNED_OUT);
        assert_eq!(account_label("   "), SIGNED_OUT);
    }

    // the row is drawn over shared screens, so this layer may prefix the action
    // but must never widen the key itself back into something readable.
    #[test]
    fn account_label_carries_the_key_exactly_as_masked() {
        let masked = "r4-tutg····sjza";
        let label = account_label(masked);
        assert!(label.ends_with(masked), "key was altered: {label}");
        assert_eq!(label, format!("{SIGNED_IN} · {masked}"));
    }

    #[test]
    fn account_label_never_reveals_a_hidden_middle() {
        let label = account_label("r4-AAAA····BBBB");
        assert!(!label.contains("SECRET"));
        assert_eq!(label.matches("····").count(), 1);
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
