//! Reserved panel usernames: live GitHub list with disk cache and bundled fallback.
//!
//! Live URL (stable):
//! `https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/docs/reserved-usernames.txt`
//!
//! Cache: `$CPN_DATA_DIR/cache/reserved-usernames.txt` (+ `.meta.json` with fetched_at).
//! Offline / tests: set `CPN_RESERVED_USERNAMES_OFFLINE=1` to skip network and use cache/bundle.

use crate::account::{data_dir, now_unix};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, io::Write, path::PathBuf, sync::Mutex, time::Duration};

const BUNDLED_LIST: &str = include_str!("../docs/reserved-usernames.txt");
const DEFAULT_RAW_URL: &str = "https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/docs/reserved-usernames.txt";
const CACHE_TTL_SECS: u64 = 24 * 60 * 60;
const FETCH_TIMEOUT_SECS: u64 = 8;

static CACHE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CacheMeta {
    #[serde(default)]
    fetched_at_unix: u64,
    #[serde(default)]
    source_url: String,
}

fn cache_dir() -> PathBuf {
    data_dir().join("cache")
}

fn cache_list_path() -> PathBuf {
    cache_dir().join("reserved-usernames.txt")
}

fn cache_meta_path() -> PathBuf {
    cache_dir().join("reserved-usernames.meta.json")
}

fn raw_url() -> String {
    std::env::var("CPN_RESERVED_USERNAMES_URL").unwrap_or_else(|_| DEFAULT_RAW_URL.to_string())
}

fn offline_mode() -> bool {
    matches!(
        std::env::var("CPN_RESERVED_USERNAMES_OFFLINE")
            .map(|v| v.trim().to_ascii_lowercase())
            .as_deref(),
        Ok("1") | Ok("true") | Ok("yes") | Ok("on")
    )
}

fn parse_list(raw: &str) -> HashSet<String> {
    raw.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.to_ascii_lowercase())
        .collect()
}

fn write_mode_600(path: impl AsRef<std::path::Path>, bytes: &[u8]) -> Result<(), String> {
    let path = path.as_ref();
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
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn load_cache_meta() -> CacheMeta {
    let Ok(raw) = fs::read_to_string(cache_meta_path()) else {
        return CacheMeta::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn cache_is_fresh(meta: &CacheMeta) -> bool {
    meta.fetched_at_unix > 0 && now_unix().saturating_sub(meta.fetched_at_unix) < CACHE_TTL_SECS
}

fn read_cached_list() -> Option<String> {
    let path = cache_list_path();
    let raw = fs::read_to_string(&path).ok()?;
    if raw.trim().is_empty() {
        return None;
    }
    Some(raw)
}

fn persist_cache(body: &str, url: &str) -> Result<(), String> {
    write_mode_600(cache_list_path(), body.as_bytes())?;
    let meta = CacheMeta {
        fetched_at_unix: now_unix(),
        source_url: url.to_string(),
    };
    let json = serde_json::to_string_pretty(&meta)
        .map_err(|err| format!("Could not serialize reserved-username cache meta: {err}"))?;
    write_mode_600(cache_meta_path(), json.as_bytes())
}

fn fetch_remote(url: &str) -> Result<String, String> {
    let output = std::process::Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--location",
            "--fail",
            "--max-time",
            &FETCH_TIMEOUT_SECS.to_string(),
            "-A",
            "cpn-installer-reserved-usernames",
            url,
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|err| format!("Could not fetch reserved usernames: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Reserved username list fetch failed ({})",
            stderr.trim().chars().take(160).collect::<String>()
        ));
    }
    let body = String::from_utf8(output.stdout)
        .map_err(|_| "Reserved username list was not valid UTF-8".to_string())?;
    if parse_list(&body).is_empty() {
        return Err("Reserved username list from GitHub was empty".into());
    }
    Ok(body)
}

/// Resolve the active reserved-username set (fresh cache, remote refresh, stale cache, or bundle).
pub fn reserved_username_set() -> HashSet<String> {
    let _guard = CACHE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let meta = load_cache_meta();
    if cache_is_fresh(&meta)
        && let Some(cached) = read_cached_list()
    {
        return parse_list(&cached);
    }

    if !offline_mode() {
        let url = raw_url();
        match fetch_remote(&url) {
            Ok(body) => {
                let _ = persist_cache(&body, &url);
                return parse_list(&body);
            }
            Err(_) => {
                // Fall through to stale cache, then bundled seed.
            }
        }
    }

    if let Some(cached) = read_cached_list() {
        return parse_list(&cached);
    }

    parse_list(BUNDLED_LIST)
}

/// True when `username` matches a reserved name (case-insensitive ASCII).
pub fn is_reserved_username(username: &str) -> bool {
    let key = username.trim().to_ascii_lowercase();
    if key.is_empty() {
        return false;
    }
    reserved_username_set().contains(&key)
}

/// Operator-facing rejection message when a reserved name is chosen.
pub fn reserved_username_error(username: &str) -> String {
    format!(
        "Username `{username}` is reserved and cannot be used for panel accounts. Choose a different name."
    )
}

/// Reject reserved usernames during create / rename / first-admin setup.
pub fn reject_if_reserved(username: &str) -> Result<(), String> {
    if is_reserved_username(username) {
        return Err(reserved_username_error(username));
    }
    Ok(())
}

/// Live GitHub raw URL used when not overridden by `CPN_RESERVED_USERNAMES_URL`.
pub fn default_list_url() -> &'static str {
    DEFAULT_RAW_URL
}

/// Cache TTL used for freshness checks.
pub fn cache_ttl() -> Duration {
    Duration::from_secs(CACHE_TTL_SECS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn bundled_seed_blocks_admin_variants() {
        with_test_data_dir(|| {
            // SAFETY: exclusive test data dir lock held by with_test_data_dir.
            unsafe {
                std::env::set_var("CPN_RESERVED_USERNAMES_OFFLINE", "1");
            }
            assert!(is_reserved_username("admin"));
            assert!(is_reserved_username("Admin"));
            assert!(is_reserved_username("ADMINISTRATOR"));
            assert!(is_reserved_username("root"));
            assert!(is_reserved_username("domainadmin"));
            assert!(!is_reserved_username("panelowner"));
            assert!(!is_reserved_username("kim"));
            unsafe {
                std::env::remove_var("CPN_RESERVED_USERNAMES_OFFLINE");
            }
        });
    }

    #[test]
    fn reject_if_reserved_message() {
        with_test_data_dir(|| {
            unsafe {
                std::env::set_var("CPN_RESERVED_USERNAMES_OFFLINE", "1");
            }
            let err = reject_if_reserved("root").unwrap_err();
            assert!(err.contains("reserved"));
            assert!(reject_if_reserved("cpnadmin").is_ok());
            unsafe {
                std::env::remove_var("CPN_RESERVED_USERNAMES_OFFLINE");
            }
        });
    }
}
