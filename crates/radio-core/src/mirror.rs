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

/// what a `data:` line on the account stream can be. anything else — an
/// unknown event type from a newer server, a keep-alive — parses to `None` and
/// is dropped, which is what keeps this client working against any server.
#[derive(Clone, Debug, PartialEq)]
pub enum StreamEvent {
    Play(MirrorEvent),
    ProfileChanged,
}

pub fn parse_sse_data(line: &str) -> Option<MirrorEvent> {
    match parse_stream_event(line) {
        Some(StreamEvent::Play(e)) => Some(e),
        _ => None,
    }
}

pub fn parse_stream_event(line: &str) -> Option<StreamEvent> {
    let json = line.strip_prefix("data:")?.trim();
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    if v.get("type").and_then(|t| t.as_str()) == Some("profile_changed") {
        return Some(StreamEvent::ProfileChanged);
    }
    serde_json::from_value(v).ok().map(StreamEvent::Play)
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
                .connect_timeout(std::time::Duration::from_secs(10))
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

    pub fn events<F: FnMut(StreamEvent)>(&self, key: &str, mut on_event: F) -> anyhow::Result<()> {
        let resp = self
            .client
            .get(format!("{}/events", self.base_url))
            .bearer_auth(key)
            .send()?
            .error_for_status()?;
        let reader = BufReader::new(resp);
        for line in reader.lines() {
            let line = line?;
            if let Some(evt) = parse_stream_event(&line) {
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
    fn parse_stream_event_reads_the_doorbell() {
        let line = r#"data: {"type":"profile_changed"}"#;
        assert_eq!(parse_stream_event(line), Some(StreamEvent::ProfileChanged));
    }

    #[test]
    fn parse_stream_event_still_reads_a_play() {
        let line = r#"data: {"uuid":"u1","name":"One","url":"http://x/1","origin":"devA","seq":3}"#;
        match parse_stream_event(line).unwrap() {
            StreamEvent::Play(e) => assert_eq!(e.uuid, "u1"),
            other => panic!("expected a play event, got {other:?}"),
        }
    }

    // the doorbell must never be mistaken for a play event: that would set a
    // bogus now-playing station on every profile change.
    #[test]
    fn the_doorbell_is_not_a_play_event() {
        assert!(parse_sse_data(r#"data: {"type":"profile_changed"}"#).is_none());
    }

    // an event type this build does not know must be dropped, not crash or be
    // misread — this is what lets an old client talk to a newer server.
    #[test]
    fn an_unknown_event_type_is_dropped() {
        assert!(parse_stream_event(r#"data: {"type":"something_new"}"#).is_none());
        assert!(parse_sse_data(r#"data: {"type":"something_new"}"#).is_none());
        assert!(parse_stream_event("data: not json at all").is_none());
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
