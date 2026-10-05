//! Persisted WebAuthn origins and RP ID under `/var/lib/cpn/mfa/`.
//!
//! Credentials are bound to RP ID at enroll time. Loopback always uses RP ID
//! `localhost`. Allowed origins for `localhost` vs `127.0.0.1` (and NAT
//! `panel_public_url` ports) are remembered so restart, repair, and
//! upgrade/downgrade do not drop origins that already-issued credentials need.
//! This file is never deleted by installer upgrade/repair.

use crate::account::data_dir;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use url::Url;

const MAX_ORIGINS: usize = 24;
const LOOPBACK_RP_ID: &str = "localhost";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct OriginStore {
    #[serde(default)]
    origins: Vec<String>,
}

fn mfa_dir() -> PathBuf {
    data_dir().join("mfa")
}

fn origins_path() -> PathBuf {
    mfa_dir().join("webauthn-origins.json")
}

fn rpid_path() -> PathBuf {
    mfa_dir().join("webauthn-rpid")
}

fn write_secret_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Could not create {}: {err}", parent.display()))?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|err| format!("Could not write {}: {err}", path.display()))?;
    file.write_all(bytes)
        .map_err(|err| format!("Could not save {}: {err}", path.display()))?;
    Ok(())
}

fn normalize_origin(raw: &str) -> Option<String> {
    let parsed = Url::parse(raw.trim()).ok()?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return None;
    }
    let mut out = parsed.origin().ascii_serialization();
    if !out.ends_with('/') {
        out.push('/');
    }
    Some(out)
}

/// Loopback RP ID is always `localhost` (never an IP). Persist once; never rotate
/// on upgrade, downgrade, repair, or restart.
pub fn persist_loopback_rpid() {
    let path = rpid_path();
    if path.is_file() {
        return;
    }
    let _ = write_secret_file(&path, LOOPBACK_RP_ID.as_bytes());
}

/// RP ID written at first loopback use, or `localhost` when unset.
pub fn persisted_loopback_rpid() -> String {
    persist_loopback_rpid();
    fs::read_to_string(rpid_path())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| s.eq_ignore_ascii_case("localhost"))
        .unwrap_or_else(|| LOOPBACK_RP_ID.to_string())
}

pub fn load_persisted_origins() -> Vec<Url> {
    let Ok(raw) = fs::read_to_string(origins_path()) else {
        return Vec::new();
    };
    let store: OriginStore = serde_json::from_str(&raw).unwrap_or_default();
    store
        .origins
        .into_iter()
        .filter_map(|s| Url::parse(&s).ok())
        .take(MAX_ORIGINS)
        .collect()
}

/// Remember origins used during a successful ceremony. Never shrinks the list.
pub fn remember_origins(urls: &[Url]) {
    persist_loopback_rpid();
    let mut store = if let Ok(raw) = fs::read_to_string(origins_path()) {
        serde_json::from_str(&raw).unwrap_or_default()
    } else {
        OriginStore::default()
    };
    let mut seen: std::collections::BTreeSet<String> = store.origins.iter().cloned().collect();
    for url in urls {
        if let Some(norm) = normalize_origin(url.as_str()) {
            seen.insert(norm);
        }
    }
    store.origins = seen.into_iter().take(MAX_ORIGINS).collect();
    if let Ok(json) = serde_json::to_string_pretty(&store) {
        let _ = write_secret_file(&origins_path(), json.as_bytes());
    }
}

pub fn remember_origin_strs(raw: &[String]) {
    let urls: Vec<Url> = raw.iter().filter_map(|s| Url::parse(s).ok()).collect();
    if !urls.is_empty() {
        remember_origins(&urls);
    }
}

pub fn parse_browser_origin(raw: Option<&str>) -> Option<Url> {
    let value = raw.map(str::trim).filter(|s| !s.is_empty())?;
    if value.len() > 200 {
        return None;
    }
    let url = Url::parse(value).ok()?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return None;
    }
    Some(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn loopback_rpid_persists_and_is_not_rotated() {
        with_test_data_dir(|| {
            assert_eq!(persisted_loopback_rpid(), "localhost");
            assert_eq!(fs::read_to_string(rpid_path()).unwrap().trim(), "localhost");
            persist_loopback_rpid();
            assert_eq!(persisted_loopback_rpid(), "localhost");
        });
    }

    #[test]
    fn origins_accumulate_across_remember_calls() {
        with_test_data_dir(|| {
            remember_origins(&[Url::parse("http://localhost:2087/").unwrap()]);
            remember_origins(&[Url::parse("http://127.0.0.1:2087/").unwrap()]);
            remember_origins(&[Url::parse("http://localhost:2091/").unwrap()]);
            let loaded = load_persisted_origins();
            let as_str: Vec<String> = loaded.iter().map(|u| u.as_str().to_string()).collect();
            assert!(
                as_str.iter().any(|s| s.contains("localhost:2087")),
                "{as_str:?}"
            );
            assert!(
                as_str.iter().any(|s| s.contains("127.0.0.1:2087")),
                "{as_str:?}"
            );
            assert!(
                as_str.iter().any(|s| s.contains("localhost:2091")),
                "{as_str:?}"
            );
        });
    }

    #[test]
    fn rpid_file_lives_under_mfa() {
        with_test_data_dir(|| {
            persist_loopback_rpid();
            let path = rpid_path();
            let text = path.to_string_lossy();
            assert!(
                text.contains("mfa"),
                "RP ID file must live under mfa/: {text}"
            );
        });
    }
}
