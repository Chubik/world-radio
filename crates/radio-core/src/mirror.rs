use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MirrorEvent {
    pub uuid: String,
    pub name: String,
    pub url: String,
    pub origin: String,
    pub seq: u64,
}

pub fn device_id() -> String {
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| {
        let n: u32 = seed_from_time_pid();
        format!("dev-{n:08x}")
    })
    .clone()
}

fn seed_from_time_pid() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    nanos
        .wrapping_mul(2654435761)
        .wrapping_add(std::process::id())
}

pub fn parse_sse_data(line: &str) -> Option<MirrorEvent> {
    let json = line.strip_prefix("data:")?.trim();
    serde_json::from_str(json).ok()
}

pub struct MirrorClient {
    base_url: String,
    client: reqwest::blocking::Client,
}

impl MirrorClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: reqwest::blocking::Client::builder()
                .user_agent("world-radio-mirror/1")
                .build()
                .expect("http client"),
        }
    }

    pub fn play(
        &self,
        key: &str,
        uuid: &str,
        name: &str,
        url: &str,
        origin: &str,
    ) -> anyhow::Result<u64> {
        #[derive(Serialize)]
        struct Req<'a> {
            uuid: &'a str,
            name: &'a str,
            url: &'a str,
            origin: &'a str,
        }
        #[derive(Deserialize)]
        struct Resp {
            seq: u64,
        }
        let resp: Resp = self
            .client
            .post(format!("{}/play", self.base_url))
            .bearer_auth(key)
            .json(&Req {
                uuid,
                name,
                url,
                origin,
            })
            .send()?
            .error_for_status()?
            .json()?;
        Ok(resp.seq)
    }

    pub fn events<F: FnMut(MirrorEvent)>(&self, key: &str, mut on_event: F) -> anyhow::Result<()> {
        let resp = self
            .client
            .get(format!("{}/events", self.base_url))
            .bearer_auth(key)
            .send()?
            .error_for_status()?;
        let reader = BufReader::new(resp);
        for line in reader.lines() {
            let line = line?;
            if let Some(evt) = parse_sse_data(&line) {
                on_event(evt);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sse_data_reads_event() {
        let line = r#"data: {"uuid":"u1","name":"One","url":"http://x/1","origin":"devA","seq":3}"#;
        let e = parse_sse_data(line).unwrap();
        assert_eq!(e.uuid, "u1");
        assert_eq!(e.seq, 3);
        assert_eq!(e.origin, "devA");
    }

    #[test]
    fn parse_sse_data_ignores_non_data_lines() {
        assert!(parse_sse_data("event: play").is_none());
        assert!(parse_sse_data(": keep-alive").is_none());
        assert!(parse_sse_data("").is_none());
    }

    #[test]
    fn device_id_is_stable_and_prefixed() {
        let a = device_id();
        let b = device_id();
        assert_eq!(a, b);
        assert!(a.starts_with("dev-"));
    }

    #[test]
    fn play_posts_and_returns_seq() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/play")
            .with_body(r#"{"seq":7}"#)
            .create();
        let c = MirrorClient::new(server.url());
        let seq = c.play("r4-k", "u1", "One", "http://x/1", "devA").unwrap();
        assert_eq!(seq, 7);
    }

    #[test]
    fn play_error_is_err() {
        let mut server = mockito::Server::new();
        server.mock("POST", "/play").with_status(401).create();
        let c = MirrorClient::new(server.url());
        assert!(c.play("r4-bad", "u", "n", "u", "d").is_err());
    }
}
