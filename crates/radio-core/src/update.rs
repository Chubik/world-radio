use serde::Deserialize;

#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    pub version: String,
    pub tarball_url: String,
    pub sha256: String,
}

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn target_triple() -> &'static str {
    env!("BUILD_TARGET")
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.trim_start_matches('v')
            .split('.')
            .map(|p| p.parse::<u64>().unwrap_or(0))
            .collect()
    };
    let a = parse(latest);
    let b = parse(current);
    let n = a.len().max(b.len());
    for i in 0..n {
        let ai = a.get(i).copied().unwrap_or(0);
        let bi = b.get(i).copied().unwrap_or(0);
        match ai.cmp(&bi) {
            std::cmp::Ordering::Greater => return true,
            std::cmp::Ordering::Less => return false,
            std::cmp::Ordering::Equal => {}
        }
    }
    false
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
}

pub fn latest_from(api_url: &str, releases_base: &str) -> anyhow::Result<Option<Release>> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("world-radio-update/1")
        .build()?;
    let rel: ApiRelease = client
        .get(api_url)
        .header("Accept", "application/vnd.github+json")
        .send()?
        .error_for_status()?
        .json()?;
    let version = rel.tag_name.trim_start_matches('v').to_string();
    if !is_newer(&version, current_version()) {
        return Ok(None);
    }
    let asset = format!("world-radio-{}-{}.tar.gz", version, target_triple());
    let tarball_url = format!("{releases_base}/{asset}");
    let sums = client
        .get(format!("{releases_base}/SHA256SUMS"))
        .send()?
        .error_for_status()?
        .text()?;
    let sha256 = match sums
        .lines()
        .find(|l| l.trim_end().ends_with(&asset))
        .and_then(|l| l.split_whitespace().next())
    {
        None => return Ok(None),
        Some(s) => s.to_string(),
    };
    Ok(Some(Release { version, tarball_url, sha256 }))
}

pub fn fetch_latest() -> anyhow::Result<Option<Release>> {
    latest_from(
        "https://api.github.com/repos/Chubik/world-radio/releases/latest",
        "https://r4dio.net/releases",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_newer_compares_semver() {
        assert!(is_newer("1.5.0", "1.4.4"));
        assert!(!is_newer("1.4.4", "1.4.4"));
        assert!(is_newer("1.10.0", "1.9.0"));
        assert!(!is_newer("1.4.0", "1.4.4"));
        assert!(is_newer("v1.5.0", "1.4.4"));
    }

    #[test]
    fn malformed_version_does_not_panic() {
        assert!(!is_newer("garbage", "1.0.0"));
    }

    #[test]
    fn latest_from_returns_release_when_newer() {
        let mut server = mockito::Server::new();
        let tag = "v99.0.0";
        server
            .mock("GET", "/releases/latest")
            .with_body(format!(r#"{{"tag_name":"{tag}"}}"#))
            .create();
        let asset = format!("world-radio-99.0.0-{}.tar.gz", target_triple());
        server
            .mock("GET", "/SHA256SUMS")
            .with_body(format!("abc123  {asset}\n"))
            .create();
        let rel = latest_from(&format!("{}/releases/latest", server.url()), &server.url())
            .unwrap()
            .unwrap();
        assert_eq!(rel.version, "99.0.0");
        assert_eq!(rel.sha256, "abc123");
        assert!(rel.tarball_url.ends_with(&asset));
    }

    #[test]
    fn latest_from_none_when_not_newer() {
        let mut server = mockito::Server::new();
        server
            .mock("GET", "/releases/latest")
            .with_body(r#"{"tag_name":"v0.0.1"}"#)
            .create();
        let out = latest_from(&format!("{}/releases/latest", server.url()), &server.url()).unwrap();
        assert!(out.is_none());
    }
}
