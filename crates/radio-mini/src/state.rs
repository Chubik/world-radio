#[derive(Debug, Clone, PartialEq)]
pub struct StationPick {
    pub uuid: String,
    pub name: String,
    pub url: String,
}

pub fn pick_random(stations: &[StationPick]) -> Option<StationPick> {
    let playable: Vec<&StationPick> = stations.iter().filter(|s| !s.url.is_empty()).collect();
    if playable.is_empty() {
        return None;
    }
    let idx = fastrand::usize(..playable.len());
    Some(playable[idx].clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(uuid: &str, url: &str) -> StationPick {
        StationPick { uuid: uuid.into(), name: uuid.into(), url: url.into() }
    }

    #[test]
    fn pick_returns_none_for_empty() {
        assert!(pick_random(&[]).is_none());
    }

    #[test]
    fn pick_skips_stations_without_url() {
        let list = vec![st("a", ""), st("b", "http://x")];
        let p = pick_random(&list).unwrap();
        assert_eq!(p.uuid, "b");
    }

    #[test]
    fn pick_returns_a_playable_one() {
        let list = vec![st("a", "http://a"), st("b", "http://b")];
        let p = pick_random(&list).unwrap();
        assert!(p.uuid == "a" || p.uuid == "b");
        assert!(!p.url.is_empty());
    }
}
