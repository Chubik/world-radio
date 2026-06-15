use std::net::ToSocketAddrs;

use crate::catalog::{filter::SearchQuery, station::Station};

pub struct RadioBrowser {
    base_url: String,
    client: reqwest::blocking::Client,
}

impl RadioBrowser {
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        let client = reqwest::blocking::Client::builder()
            .user_agent("world-radio/0.1")
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
}

pub fn resolve_mirror() -> anyhow::Result<String> {
    let mut addrs = ("all.api.radio-browser.info", 443)
        .to_socket_addrs()
        .map_err(|e| anyhow::anyhow!("dns lookup failed: {e}"))?;
    addrs
        .next()
        .ok_or_else(|| anyhow::anyhow!("no radio-browser mirror found"))?;
    Ok("https://all.api.radio-browser.info".to_string())
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
}
