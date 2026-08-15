// the panel cannot be shown over another app's fullscreen space — that is an
// appkit constraint, not a bug we can work around. the tray menu and the global
// shortcut are therefore the only controls that reach a user who works
// fullscreen, so both are built to work with no window on screen at all.

// tauri parses this into the ⌥R glyphs macos draws beside the menu item, so the
// menu label and the registered shortcut cannot drift apart.
pub const SHUFFLE_ACCELERATOR: &str = "Alt+R";

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

// the row is a button into the sync screen and says so. the key never appears
// here: the menu is drawn over whatever the user is screen-sharing, and a key
// nobody can copy from a menu item earns nothing by being shown. an empty mask
// means no account yet, which is a different offer.
pub fn account_label(masked: &str) -> String {
    match masked.trim().is_empty() {
        true => SIGNED_OUT.to_string(),
        false => SIGNED_IN.to_string(),
    }
}

/// the running build, for the panel's brand row. empty in, empty out — a
/// version-less build shows nothing rather than a bare "v".
pub fn version_label(version: &str) -> String {
    match version.trim().is_empty() {
        true => String::new(),
        false => format!("v{version}"),
    }
}

pub fn update_label(version: &str) -> String {
    format!("Update to v{version}")
}

/// how many codes the label spells out before it counts the rest. the row is
/// narrow, so a long list is cut rather than allowed to push the scope buttons
/// off the panel's edge.
const FILTER_CODES: usize = 3;

/// what the window shows the user they are filtered to. empty means there is
/// nothing to show and the row stays hidden — a filter in effect and a filter
/// unapplied look identical without it.
///
/// favourites are deliberately excluded: `MiniState::active_stations` lets a
/// star outrank the country filter, so naming a filter in that scope would tell
/// the user something untrue about what ★ will play. android's `filterPillLabel`
/// makes the same cut with the same wording, so the two surfaces read alike.
pub fn filter_label(countries: &[String], scope: crate::state::Scope) -> String {
    if countries.is_empty() || scope == crate::state::Scope::Favorites {
        return String::new();
    }
    let shown: Vec<&str> = countries
        .iter()
        .take(FILTER_CODES)
        .map(|c| c.as_str())
        .collect();
    let shown = shown.join("·");
    match countries.len().saturating_sub(FILTER_CODES) {
        0 => format!("FILTER: {shown}"),
        rest => format!("FILTER: {shown} +{rest}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Scope;

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

    // the key names nothing the user can act on and is visible to anyone
    // watching a shared screen, so the row carries the action alone. the key
    // itself belongs in the sync screen, where it can be copied.
    #[test]
    fn account_label_names_the_action_and_never_the_key() {
        let label = account_label("r4-7K2P-····-4DF1");
        assert_eq!(label, "Sync");
        assert!(
            !label.contains("r4-"),
            "the key must not reach the menu: {label}"
        );
    }

    #[test]
    fn account_label_without_a_key_offers_setup() {
        assert_eq!(account_label(""), SIGNED_OUT);
        assert_eq!(account_label("   "), SIGNED_OUT);
    }

    // the menu is drawn over whatever the user is screen-sharing, so no part of
    // the key may reach it — not even a masked one.
    #[test]
    fn account_label_leaks_no_part_of_the_key() {
        for key in ["r4-tutg····sjza", "r4-AAAA····BBBB", "r4-7K2P-····-4DF1"] {
            let label = account_label(key);
            assert_eq!(label, SIGNED_IN);
            assert!(!label.contains("····"), "mask leaked: {label}");
            assert!(!label.contains("r4-"), "key leaked: {label}");
        }
    }

    #[test]
    fn accelerator_is_the_combination_the_menu_advertises() {
        assert_eq!(SHUFFLE_ACCELERATOR, "Alt+R");
    }

    #[test]
    fn update_label_names_the_version_it_installs() {
        assert_eq!(update_label("1.17.0"), "Update to v1.17.0");
    }

    // the panel shows which build is running, small, next to the brand mark.
    #[test]
    fn version_label_is_the_running_build() {
        assert_eq!(version_label("1.18.1"), "v1.18.1");
        assert_eq!(version_label(""), "");
    }

    fn codes(list: &[&str]) -> Vec<String> {
        list.iter().map(|c| c.to_string()).collect()
    }

    // no filter is not a filter of nothing: the row must disappear rather than
    // print an empty "FILTER:" that reads like a setting that failed to load.
    #[test]
    fn no_filter_shows_nothing() {
        assert_eq!(filter_label(&[], Scope::All), "");
    }

    #[test]
    fn one_country_is_named_outright() {
        assert_eq!(filter_label(&codes(&["UA"]), Scope::All), "FILTER: UA");
    }

    #[test]
    fn up_to_three_countries_are_all_spelled_out() {
        assert_eq!(
            filter_label(&codes(&["UA", "PL"]), Scope::All),
            "FILTER: UA·PL"
        );
        assert_eq!(
            filter_label(&codes(&["UA", "PL", "DE"]), Scope::All),
            "FILTER: UA·PL·DE"
        );
    }

    // the row is narrow, so a longer list is cut and the rest counted — the same
    // shape android's filterPillLabel produces, so the two surfaces read alike.
    #[test]
    fn a_longer_list_is_cut_and_the_rest_counted() {
        assert_eq!(
            filter_label(&codes(&["UA", "PL", "DE", "FR"]), Scope::All),
            "FILTER: UA·PL·DE +1"
        );
        assert_eq!(
            filter_label(&codes(&["UA", "PL", "DE", "FR", "IT", "ES"]), Scope::All),
            "FILTER: UA·PL·DE +3"
        );
    }

    // favourites bypass the country filter entirely, so naming one there would
    // tell the user something untrue about what the ★ scope will play.
    #[test]
    fn favourites_never_advertise_a_filter_they_ignore() {
        assert_eq!(filter_label(&codes(&["UA"]), Scope::Favorites), "");
        assert_eq!(
            filter_label(&codes(&["UA", "PL", "DE", "FR"]), Scope::Favorites),
            ""
        );
    }
}
