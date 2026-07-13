pub mod api;
pub mod cache;
#[allow(clippy::module_inception)]
pub mod catalog;
pub mod facets;
pub mod favorites;
pub mod filter;
pub mod health;
pub mod station;

pub use api::{resolve, RadioBrowser};
pub use cache::{text_is_excluded, Cache};
pub use catalog::Catalog;
pub use facets::Facets;
pub use favorites::{Favorites, History};
pub use filter::SearchQuery;
pub use health::Health;
pub use station::{codec_is_unstable, Station};

pub fn should_sync(last: Option<i64>, now: i64, ttl_secs: i64) -> bool {
    match last {
        None => true,
        Some(t) => now - t >= ttl_secs,
    }
}

#[cfg(test)]
mod tests {
    use super::should_sync;
    const DAY: i64 = 86_400;
    #[test]
    fn syncs_when_empty() {
        assert!(should_sync(None, 1_000_000, DAY));
    }
    #[test]
    fn skips_when_fresh() {
        assert!(!should_sync(Some(1_000_000 - 3600), 1_000_000, DAY));
    }
    #[test]
    fn syncs_when_stale() {
        assert!(should_sync(Some(1_000_000 - 25 * 3600), 1_000_000, DAY));
    }
}
