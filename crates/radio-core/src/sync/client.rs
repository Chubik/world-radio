use crate::sync::Pending;
use serde::{Deserialize, Serialize};

/// a last-write-wins field: `value` is opaque to the transport (the server
/// never inspects it), `at` is the client-side unix time of the change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lww {
    pub value: serde_json::Value,
    pub at: i64,
}

/// one play-history entry: `id` is the station uuid, `at` is when it was
/// played, `gone` marks a removal the same way favourites/blocked do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryRecord {
    pub id: String,
    pub at: i64,
    pub gone: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SyncData {
    pub favs: Vec<String>,
    pub blocked: Vec<String>,
    #[serde(default)]
    pub excluded_countries: Vec<String>,
    // omitted entirely when there is nothing pending, so the request stays
    // byte-identical to the old format for an unchanged device.
    #[serde(default, skip_serializing_if = "Pending::is_empty")]
    pub changed: Pending,
    // the listening profile. optional/empty and skipped on serialize so a
    // pre-upgrade server still accepts this payload unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shuffle_filter: Option<Lww>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Lww>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<Lww>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<HistoryRecord>,
}

pub struct SyncClient {
    base_url: String,
    client: reqwest::blocking::Client,
}

impl SyncClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            // lets a local dev server stand in for r4dio.net without touching call sites
            base_url: std::env::var("R4DIO_SYNC_URL").unwrap_or_else(|_| base_url.into()),
            client: reqwest::blocking::Client::builder()
                .user_agent("world-radio-sync/1")
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("http client"),
        }
    }

    pub fn create_account(&self) -> anyhow::Result<String> {
        #[derive(Deserialize)]
        struct KeyResp {
            key: String,
        }
        let resp: KeyResp = self
            .client
            .post(format!("{}/account", self.base_url))
            .send()?
            .error_for_status()?
            .json()?;
        Ok(resp.key)
    }

    pub fn pull(&self, key: &str) -> anyhow::Result<SyncData> {
        let data = self
            .client
            .get(format!("{}/sync", self.base_url))
            .bearer_auth(key)
            .send()?
            .error_for_status()?
            .json()?;
        Ok(data)
    }

    pub fn push(&self, key: &str, data: &SyncData) -> anyhow::Result<SyncData> {
        let merged = self
            .client
            .put(format!("{}/sync", self.base_url))
            .bearer_auth(key)
            .json(data)
            .send()?
            .error_for_status()?
            .json()?;
        Ok(merged)
    }

    pub fn delete(&self, key: &str) -> anyhow::Result<bool> {
        let resp = self
            .client
            .delete(format!("{}/account", self.base_url))
            .bearer_auth(key)
            .send()?
            .error_for_status()?;
        Ok(resp.status().is_success())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_account_returns_key() {
        let mut server = mockito::Server::new();
        let m = server
            .mock("POST", "/account")
            .with_body(r#"{"key":"r4-abc"}"#)
            .create();
        let c = SyncClient::new(server.url());
        assert_eq!(c.create_account().unwrap(), "r4-abc");
        m.assert();
    }

    #[test]
    fn create_account_500_is_err() {
        let mut server = mockito::Server::new();
        server.mock("POST", "/account").with_status(500).create();
        let c = SyncClient::new(server.url());
        assert!(c.create_account().is_err());
    }

    #[test]
    fn pull_parses_data() {
        let mut server = mockito::Server::new();
        server
            .mock("GET", "/sync")
            .with_body(r#"{"favs":["a","b"],"blocked":["x"]}"#)
            .create();
        let c = SyncClient::new(server.url());
        let d = c.pull("r4-k").unwrap();
        assert_eq!(
            d,
            SyncData {
                favs: vec!["a".into(), "b".into()],
                blocked: vec!["x".into()],
                excluded_countries: vec![],
                ..Default::default()
            }
        );
    }

    #[test]
    fn pull_401_is_err() {
        let mut server = mockito::Server::new();
        server.mock("GET", "/sync").with_status(401).create();
        let c = SyncClient::new(server.url());
        assert!(c.pull("r4-bad").is_err());
    }

    #[test]
    fn push_returns_server_state_verbatim() {
        let mut server = mockito::Server::new();
        server
            .mock("PUT", "/sync")
            .with_body(r#"{"favs":["c"],"blocked":[]}"#)
            .create();
        let c = SyncClient::new(server.url());
        let d = c
            .push(
                "r4-k",
                &SyncData {
                    favs: vec!["c".into()],
                    blocked: vec![],
                    excluded_countries: vec![],
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(d.favs, vec!["c".to_string()]);
    }

    #[test]
    fn delete_204_true() {
        let mut server = mockito::Server::new();
        server.mock("DELETE", "/account").with_status(204).create();
        let c = SyncClient::new(server.url());
        assert!(c.delete("r4-k").unwrap());
    }

    #[test]
    fn delete_401_is_err() {
        let mut server = mockito::Server::new();
        server.mock("DELETE", "/account").with_status(401).create();
        let c = SyncClient::new(server.url());
        assert!(c.delete("r4-bad").is_err());
    }

    #[test]
    fn syncdata_serializes_excluded_countries_key() {
        let d = SyncData {
            favs: vec![],
            blocked: vec![],
            excluded_countries: vec!["US".into()],
            ..Default::default()
        };
        let j = serde_json::to_string(&d).unwrap();
        assert!(
            j.contains("\"excluded_countries\":[\"US\"]"),
            "wire key must be excluded_countries: {j}"
        );
    }

    #[test]
    fn syncdata_deserializes_without_excluded_field() {
        // older server response with no excluded_countries must still parse
        let d: SyncData = serde_json::from_str("{\"favs\":[],\"blocked\":[]}").unwrap();
        assert!(d.excluded_countries.is_empty());
    }

    #[test]
    fn syncdata_omits_profile_fields_when_unset() {
        // a pre-upgrade server must see a byte-identical payload to before
        let d = SyncData::default();
        let j = serde_json::to_string(&d).unwrap();
        assert!(!j.contains("shuffle_filter"), "{j}");
        assert!(!j.contains("\"scope\""), "{j}");
        assert!(!j.contains("\"theme\""), "{j}");
        assert!(!j.contains("history"), "{j}");
    }

    #[test]
    fn syncdata_serializes_profile_fields_when_set() {
        let d = SyncData {
            shuffle_filter: Some(Lww {
                value: serde_json::json!({"countries": ["UA"]}),
                at: 10,
            }),
            scope: Some(Lww {
                value: serde_json::json!("FAVS"),
                at: 20,
            }),
            theme: Some(Lww {
                value: serde_json::json!("nord"),
                at: 30,
            }),
            history: vec![HistoryRecord {
                id: "s1".into(),
                at: 40,
                gone: false,
            }],
            ..Default::default()
        };
        let j = serde_json::to_string(&d).unwrap();
        assert!(j.contains("\"shuffle_filter\":{\"value\":{\"countries\":[\"UA\"]},\"at\":10}"));
        assert!(j.contains("\"scope\":{\"value\":\"FAVS\",\"at\":20}"));
        assert!(j.contains("\"theme\":{\"value\":\"nord\",\"at\":30}"));
        assert!(j.contains("\"history\":[{\"id\":\"s1\",\"at\":40,\"gone\":false}]"));
    }

    #[test]
    fn syncdata_deserializes_without_profile_fields() {
        // an old server response has none of the new keys at all
        let d: SyncData = serde_json::from_str("{\"favs\":[],\"blocked\":[]}").unwrap();
        assert!(d.shuffle_filter.is_none());
        assert!(d.scope.is_none());
        assert!(d.theme.is_none());
        assert!(d.history.is_empty());
    }
}
