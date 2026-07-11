use serde::Deserialize;
use std::io::Read;
use std::path::Path;

pub const BIN_NAME: &str = "r4dio";

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
    let releases: Vec<ApiRelease> = client
        .get(api_url)
        .header("Accept", "application/vnd.github+json")
        .send()?
        .error_for_status()?
        .json()?;
    let version = match releases
        .iter()
        .map(|r| r.tag_name.trim_start_matches('v').to_string())
        .max_by(|a, b| match is_newer(a, b) {
            true => std::cmp::Ordering::Greater,
            false => std::cmp::Ordering::Less,
        }) {
        None => return Ok(None),
        Some(v) => v,
    };
    if !is_newer(&version, current_version()) {
        return Ok(None);
    }
    let asset = format!("{}-{}-{}.tar.gz", BIN_NAME, version, target_triple());
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
    Ok(Some(Release {
        version,
        tarball_url,
        sha256,
    }))
}

pub fn fetch_latest() -> anyhow::Result<Option<Release>> {
    latest_from(
        "https://api.github.com/repos/Chubik/world-radio/releases?per_page=10",
        "https://r4dio.net/releases",
    )
}

pub fn apply(release: &Release) -> anyhow::Result<()> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("world-radio-update/1")
        .build()?;
    let bytes = client
        .get(&release.tarball_url)
        .send()?
        .error_for_status()?
        .bytes()?;
    let exe = std::env::current_exe()?;
    verify_and_extract(&bytes, &release.sha256, &exe)
}

pub fn verify_and_extract(
    tarball: &[u8],
    expected_sha: &str,
    dest_exe: &Path,
) -> anyhow::Result<()> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(tarball);
    let got: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if got != expected_sha {
        anyhow::bail!("checksum mismatch");
    }
    let gz = flate2::read::GzDecoder::new(tarball);
    let mut archive = tar::Archive::new(gz);
    let mut binary: Vec<u8> = Vec::new();
    let mut found = false;
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();
        if path.file_name().and_then(|n| n.to_str()) == Some(BIN_NAME) {
            entry.read_to_end(&mut binary)?;
            found = true;
            break;
        }
    }
    if !found {
        anyhow::bail!("{} binary not found in archive", BIN_NAME);
    }
    let dir = dest_exe.parent().unwrap_or_else(|| Path::new("."));
    let tmp = dir.join(format!(".{BIN_NAME}.update"));
    std::fs::write(&tmp, &binary)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::rename(&tmp, dest_exe)?;
    Ok(())
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
            .with_body(format!(r#"[{{"tag_name":"{tag}"}}]"#))
            .create();
        let asset = format!("{}-99.0.0-{}.tar.gz", BIN_NAME, target_triple());
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
            .with_body(r#"[{"tag_name":"v0.0.1"}]"#)
            .create();
        let out = latest_from(&format!("{}/releases/latest", server.url()), &server.url()).unwrap();
        assert!(out.is_none());
    }

    fn make_tarball(bin_contents: &[u8]) -> Vec<u8> {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        {
            let mut ar = tar::Builder::new(&mut enc);
            let mut header = tar::Header::new_gnu();
            header.set_size(bin_contents.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            ar.append_data(&mut header, BIN_NAME, bin_contents).unwrap();
            ar.finish().unwrap();
        }
        enc.finish().unwrap()
    }

    fn sha_hex(data: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(data);
        h.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn verify_and_extract_replaces_on_good_checksum() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join(BIN_NAME);
        std::fs::write(&dest, b"OLD").unwrap();
        let tarball = make_tarball(b"NEWBINARY");
        let sha = sha_hex(&tarball);
        verify_and_extract(&tarball, &sha, &dest).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"NEWBINARY");
    }

    #[test]
    fn verify_and_extract_rejects_bad_checksum() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join(BIN_NAME);
        std::fs::write(&dest, b"OLD").unwrap();
        let tarball = make_tarball(b"NEWBINARY");
        let err = verify_and_extract(&tarball, "deadbeef", &dest);
        assert!(err.is_err());
        assert_eq!(std::fs::read(&dest).unwrap(), b"OLD");
    }
}
