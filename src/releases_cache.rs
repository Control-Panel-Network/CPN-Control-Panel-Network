//! Disk cache and rate limit for GitHub Releases API fetches.
//!
//! Cache path (default): `/var/lib/cpn/github-releases-cache.json`
//! TTL default: 30 minutes (`CPN_RELEASES_CACHE_TTL_SECS`)
//! Manual check min interval: 60 seconds (`CPN_RELEASES_CHECK_MIN_INTERVAL_SECS`)
//! Optional token: `GITHUB_TOKEN` / `CPN_GITHUB_TOKEN` env, or `{data_dir}/secrets/github-token`

use crate::paths;
use crate::releases::CpnRelease;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_CACHE_TTL_SECS: u64 = 30 * 60;
pub const DEFAULT_CHECK_MIN_INTERVAL_SECS: u64 = 60;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReleasesCacheFile {
    pub schema_version: u32,
    pub repo: String,
    pub fetched_at_unix: u64,
    pub last_attempt_unix: u64,
    pub etag: Option<String>,
    pub releases: Vec<CpnRelease>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ReleasesFetchResult {
    pub releases: Vec<CpnRelease>,
    pub from_cache: bool,
    pub cache_age_secs: Option<u64>,
    pub rate_limited: bool,
    pub retry_after_secs: Option<u64>,
    pub note: Option<String>,
    pub soft_error: Option<String>,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn cache_ttl_secs() -> u64 {
    std::env::var("CPN_RELEASES_CACHE_TTL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&v| v >= 60)
        .unwrap_or(DEFAULT_CACHE_TTL_SECS)
}

pub fn check_min_interval_secs() -> u64 {
    std::env::var("CPN_RELEASES_CHECK_MIN_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&v| v >= 5)
        .unwrap_or(DEFAULT_CHECK_MIN_INTERVAL_SECS)
}

pub fn cache_path() -> PathBuf {
    paths::default_data_dir().join("github-releases-cache.json")
}

pub fn github_token() -> Option<String> {
    for key in ["CPN_GITHUB_TOKEN", "GITHUB_TOKEN"] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }
    let path = paths::default_data_dir()
        .join("secrets")
        .join("github-token");
    let raw = fs::read_to_string(path).ok()?;
    let trimmed = raw.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

pub fn load_cache() -> Option<ReleasesCacheFile> {
    let raw = fs::read_to_string(cache_path()).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn save_cache(cache: &ReleasesCacheFile) -> Result<(), String> {
    let dir = paths::default_data_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    let path = cache_path();
    let body = serde_json::to_string_pretty(cache)
        .map_err(|e| format!("Could not serialize releases cache: {e}"))?;
    fs::write(&path, format!("{body}\n"))
        .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o644));
    }
    Ok(())
}

pub fn cache_is_fresh(cache: &ReleasesCacheFile, repo: &str) -> bool {
    if cache.repo != repo || cache.releases.is_empty() {
        return false;
    }
    let age = now_unix().saturating_sub(cache.fetched_at_unix);
    age <= cache_ttl_secs()
}

pub fn seconds_until_next_check(cache: &ReleasesCacheFile) -> Option<u64> {
    let elapsed = now_unix().saturating_sub(cache.last_attempt_unix);
    let min = check_min_interval_secs();
    if elapsed >= min {
        None
    } else {
        Some(min - elapsed)
    }
}

pub fn friendly_rate_limit_message(status: u16) -> String {
    match status {
        403 => "GitHub Releases returned HTTP 403 (rate limit or access denied). Showing cached results when available. Try again later, or set a token in CPN_GITHUB_TOKEN / secrets/github-token.".into(),
        429 => "GitHub Releases returned HTTP 429 (rate limited). Showing cached results when available. Try again later.".into(),
        other => format!(
            "GitHub Releases request failed (HTTP {other}). Showing cached results when available."
        ),
    }
}

pub fn note_for_cached(age_secs: u64, rate_limited: bool, retry_after: Option<u64>) -> String {
    if rate_limited {
        if let Some(secs) = retry_after {
            return format!(
                "Checked recently; showing cached results. Try again in {secs} seconds."
            );
        }
        return "Checked recently; showing cached results.".into();
    }
    if age_secs == 0 {
        "Showing cached release list.".into()
    } else if age_secs < 120 {
        format!("Showing cached release list (about {age_secs} seconds old).")
    } else {
        let mins = age_secs / 60;
        format!("Showing cached release list (about {mins} minutes old).")
    }
}

pub fn mark_attempt(cache: &mut ReleasesCacheFile) {
    cache.last_attempt_unix = now_unix();
}

pub fn store_success(
    repo: &str,
    releases: Vec<CpnRelease>,
    etag: Option<String>,
    previous: Option<ReleasesCacheFile>,
) -> ReleasesCacheFile {
    let now = now_unix();
    ReleasesCacheFile {
        schema_version: 1,
        repo: repo.to_string(),
        fetched_at_unix: now,
        last_attempt_unix: now,
        etag: etag.or_else(|| previous.and_then(|p| p.etag)),
        releases,
        last_error: None,
    }
}

pub fn age_secs(cache: &ReleasesCacheFile) -> u64 {
    now_unix().saturating_sub(cache.fetched_at_unix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sensible() {
        assert_eq!(DEFAULT_CACHE_TTL_SECS, 1800);
        assert_eq!(DEFAULT_CHECK_MIN_INTERVAL_SECS, 60);
        assert!(!friendly_rate_limit_message(403).contains('\u{2014}'));
        assert!(!friendly_rate_limit_message(429).contains('\u{2013}'));
        assert!(
            !note_for_cached(90, true, Some(30))
                .to_lowercase()
                .contains("cyberpanel")
        );
    }

    #[test]
    fn cache_freshness_requires_matching_repo_and_items() {
        let mut cache = ReleasesCacheFile {
            schema_version: 1,
            repo: "Control-Panel-Network/CPN-Control-Panel-Network".into(),
            fetched_at_unix: now_unix(),
            last_attempt_unix: now_unix(),
            etag: None,
            releases: Vec::new(),
            last_error: None,
        };
        assert!(!cache_is_fresh(&cache, &cache.repo));
        cache.releases.push(CpnRelease {
            tag_name: "v0.2.6-alpha.27".into(),
            version: "0.2.6-alpha.27".into(),
            name: "x".into(),
            published_at: "2026-09-12".into(),
            prerelease: true,
            draft: false,
            html_url: "https://example.invalid".into(),
            assets: Vec::new(),
            rpm_asset: None,
            binary_asset: None,
            checksums_asset: None,
            checksums_asc_asset: None,
        });
        assert!(cache_is_fresh(&cache, &cache.repo));
        assert!(!cache_is_fresh(&cache, "other/repo"));
    }
}
