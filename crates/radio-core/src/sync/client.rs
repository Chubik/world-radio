use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncData {
    pub favs: Vec<String>,
    pub blocked: Vec<String>,
}

pub struct SyncClient {
    base_url: String,
    client: reqwest::blocking::Client,
}

impl SyncClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
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
                blocked: vec!["x".into()]
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
    fn push_returns_merged() {
        let mut server = mockito::Server::new();
        server
            .mock("PUT", "/sync")
            .with_body(r#"{"favs":["a","b","c"],"blocked":[]}"#)
            .create();
        let c = SyncClient::new(server.url());
        let d = c
            .push(
                "r4-k",
                &SyncData {
                    favs: vec!["c".into()],
                    blocked: vec![],
                },
            )
            .unwrap();
        assert_eq!(d.favs, vec!["a".to_string(), "b".into(), "c".into()]);
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
}
