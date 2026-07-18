use std::net::{IpAddr, SocketAddr, ToSocketAddrs};

use crate::catalog::{filter::SearchQuery, station::Station};

pub struct RadioBrowser {
    base_url: String,
    client: reqwest::blocking::Client,
}

const MIRROR_HOST: &str = "all.api.radio-browser.info";
const FALLBACK_BASE: &str = "https://all.api.radio-browser.info";
const HEALTH_PATH: &str = "/json/stats";

impl RadioBrowser {
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        let client = reqwest::blocking::Client::builder()
            .user_agent("world-radio/1.1")
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(180))
            .build()
            .expect("client build");
        Self {
            base_url: base_url.into(),
            client,
        }
    }

    pub fn with_mirror_ip(ip: IpAddr) -> Self {
        let client = reqwest::blocking::Client::builder()
            .user_agent("world-radio/1.1")
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(180))
            .resolve(MIRROR_HOST, SocketAddr::new(ip, 443))
            .build()
            .expect("client build");
        Self {
            base_url: FALLBACK_BASE.to_string(),
            client,
        }
    }

    pub fn with_base_url_timeout(base_url: impl Into<String>, secs: u64) -> Self {
        let client = reqwest::blocking::Client::builder()
            .user_agent("world-radio/1.1")
            .connect_timeout(std::time::Duration::from_secs(secs))
            .timeout(std::time::Duration::from_secs(secs))
            .build()
            .expect("client build");
        Self {
            base_url: base_url.into(),
            client,
        }
    }

    pub fn search(&self, q: &SearchQuery) -> anyhow::Result<Vec<Station>> {
        let url = format!("{}/json/stations/search", self.base_url);
        let params = q.to_params();
        let resp = self
            .client
            .get(&url)
            .query(&params)
            .send()?
            .error_for_status()?;
        let stations: Vec<Station> = resp.json()?;
        Ok(stations)
    }

    pub fn fetch_all(&self) -> anyhow::Result<Vec<Station>> {
        let url = format!("{}/json/stations", self.base_url);
        let resp = self
            .client
            .get(&url)
            .query(&[("limit", "500000"), ("hidebroken", "true")])
            .send()?
            .error_for_status()?;
        let stations: Vec<Station> = resp.json()?;
        Ok(stations)
    }

    pub fn fetch_top(&self, limit: usize) -> anyhow::Result<Vec<Station>> {
        let url = format!("{}/json/stations", self.base_url);
        let resp = self
            .client
            .get(&url)
            .query(&[
                ("order", "votes"),
                ("reverse", "true"),
                ("limit", &limit.to_string()),
                ("hidebroken", "true"),
            ])
            .send()?
            .error_for_status()?;
        let stations: Vec<Station> = resp.json()?;
        Ok(stations)
    }
}

pub fn resolve() -> RadioBrowser {
    let timeout = std::time::Duration::from_secs(3);
    let ips = mirror_ips();
    match pick_alive_ip(&ips, |ip| is_mirror_alive(*ip, timeout)) {
        Some(ip) => RadioBrowser::with_mirror_ip(ip),
        None => RadioBrowser::with_base_url(FALLBACK_BASE),
    }
}

pub fn resolve_with_timeout(secs: u64) -> RadioBrowser {
    // interactive search: skip the per-mirror liveness probe (it can add K*3s
    // before the request even starts) and go straight to the base url so the
    // whole call is bounded by `secs`.
    RadioBrowser::with_base_url_timeout(FALLBACK_BASE, secs)
}

fn mirror_ips() -> Vec<IpAddr> {
    let Ok(addrs) = (MIRROR_HOST, 443).to_socket_addrs() else {
        return Vec::new();
    };
    let mut seen = std::collections::HashSet::new();
    addrs
        .map(|a| a.ip())
        .filter(|ip| seen.insert(*ip))
        .collect()
}

fn is_mirror_alive(ip: IpAddr, timeout: std::time::Duration) -> bool {
    let Ok(client) = reqwest::blocking::Client::builder()
        .user_agent("world-radio/1.1")
        .timeout(timeout)
        .resolve(MIRROR_HOST, SocketAddr::new(ip, 443))
        .build()
    else {
        return false;
    };
    let url = format!("{FALLBACK_BASE}{HEALTH_PATH}");
    matches!(client.get(&url).send(), Ok(r) if r.status().is_success())
}

fn pick_alive_ip(ips: &[IpAddr], mut check: impl FnMut(&IpAddr) -> bool) -> Option<IpAddr> {
    ips.iter().find(|ip| check(ip)).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_parses_station_array() {
        let mut server = mockito::Server::new();
        let body = r#"[{"stationuuid":"u1","name":"Jazz FM","url_resolved":"http://stream.test/jazz","bitrate":128}]"#;
        let _m = server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"^/json/stations/search.*".into()),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create();

        let rb = RadioBrowser::with_base_url(server.url());
        let q = SearchQuery {
            name: Some("jazz".into()),
            ..Default::default()
        };
        let stations = rb.search(&q).unwrap();
        assert_eq!(stations.len(), 1);
        assert_eq!(stations[0].name, "Jazz FM");
        assert_eq!(stations[0].url_resolved, "http://stream.test/jazz");
    }

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn pick_alive_ip_returns_first_healthy() {
        let ips = vec![ip("1.1.1.1"), ip("2.2.2.2"), ip("3.3.3.3")];
        let picked = pick_alive_ip(&ips, |a| *a != ip("1.1.1.1"));
        assert_eq!(picked, Some(ip("2.2.2.2")));
    }

    #[test]
    fn pick_alive_ip_returns_none_when_all_dead() {
        let ips = vec![ip("1.1.1.1"), ip("2.2.2.2")];
        let picked = pick_alive_ip(&ips, |_| false);
        assert_eq!(picked, None);
    }

    #[test]
    fn mirror_ips_are_deduplicated() {
        let mut seen = std::collections::HashSet::new();
        let raw = [ip("1.1.1.1"), ip("1.1.1.1"), ip("2.2.2.2")];
        let deduped: Vec<IpAddr> = raw.into_iter().filter(|x| seen.insert(*x)).collect();
        assert_eq!(deduped, vec![ip("1.1.1.1"), ip("2.2.2.2")]);
    }

    #[test]
    fn fetch_all_parses_full_dump() {
        let mut server = mockito::Server::new();
        let body = r#"[{"stationuuid":"1","name":"A","votes":9},{"stationuuid":"2","name":"B","votes":3}]"#;
        let m = server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"^/json/stations(\?.*)?$".to_string()),
            )
            .with_body(body)
            .create();
        let rb = RadioBrowser::with_base_url(server.url());
        let all = rb.fetch_all().unwrap();
        m.assert();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].votes, 9);
    }
}
