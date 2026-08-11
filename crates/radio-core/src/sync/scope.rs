/// the browse scope as it travels on the wire. every device that shares a key
/// must land on the same scope, so all five states round-trip 1:1 — collapsing
/// recent/blocked/dead into "all" would silently change what the other device
/// is looking at.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Scope {
    #[default]
    All,
    Favorites,
    Recent,
    Blocked,
    Dead,
}

impl Scope {
    /// the wire string. stable: android and the server-side history of stored
    /// values depend on these exact five lowercase words.
    pub fn as_wire(self) -> &'static str {
        match self {
            Scope::All => "all",
            Scope::Favorites => "favorites",
            Scope::Recent => "recent",
            Scope::Blocked => "blocked",
            Scope::Dead => "dead",
        }
    }

    /// `None` for anything unrecognised, so the caller can leave the local scope
    /// alone rather than resetting the user to `All` on a value from a newer
    /// client. "ALL"/"FAVS" are the legacy strings written by clients already in
    /// the wild and must keep parsing.
    pub fn from_wire(value: &str) -> Option<Scope> {
        match value {
            "all" | "ALL" => Some(Scope::All),
            "favorites" | "FAVS" => Some(Scope::Favorites),
            "recent" => Some(Scope::Recent),
            "blocked" => Some(Scope::Blocked),
            "dead" => Some(Scope::Dead),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Scope; 5] = [
        Scope::All,
        Scope::Favorites,
        Scope::Recent,
        Scope::Blocked,
        Scope::Dead,
    ];

    #[test]
    fn every_variant_round_trips_through_the_wire() {
        for s in ALL {
            assert_eq!(Scope::from_wire(s.as_wire()), Some(s), "{s:?}");
        }
    }

    #[test]
    fn the_wire_strings_are_the_published_contract() {
        // android mirrors these exact words; changing one silently desyncs it.
        assert_eq!(Scope::All.as_wire(), "all");
        assert_eq!(Scope::Favorites.as_wire(), "favorites");
        assert_eq!(Scope::Recent.as_wire(), "recent");
        assert_eq!(Scope::Blocked.as_wire(), "blocked");
        assert_eq!(Scope::Dead.as_wire(), "dead");
    }

    #[test]
    fn no_two_variants_share_a_wire_string() {
        let mut seen: Vec<&str> = ALL.iter().map(|s| s.as_wire()).collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), ALL.len(), "wire strings must be 1:1");
    }

    #[test]
    fn legacy_uppercase_values_still_parse() {
        // an older client already in the wild writes these.
        assert_eq!(Scope::from_wire("ALL"), Some(Scope::All));
        assert_eq!(Scope::from_wire("FAVS"), Some(Scope::Favorites));
    }

    #[test]
    fn an_unknown_value_is_none_not_a_default() {
        // falling back to All here would reset the browse scope of a device
        // that received a value from a newer client.
        assert_eq!(Scope::from_wire("nonsense"), None);
        assert_eq!(Scope::from_wire(""), None);
        assert_eq!(Scope::from_wire("All"), None);
        assert_eq!(Scope::from_wire("Favorites"), None);
    }
}
