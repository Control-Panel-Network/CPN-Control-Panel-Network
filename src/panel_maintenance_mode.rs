//! Shared panel upgrade maintenance flag (`/var/lib/cpn/maintenance.json`).
//!
//! Written before package replace / restart so every visitor sees a friendly
//! maintenance page instead of blank connection errors. Cleared after a healthy
//! listen, on failure, on timeout, or via `cpn doctor --heal`.

use crate::account::{data_dir, now_unix};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Default auto-heal window so a crashed upgrade cannot leave the panel stuck.
pub const DEFAULT_TTL_SECS: u64 = 45 * 60;

const FLAG_NAME: &str = "maintenance.json";
const STATIC_HTML_NAME: &str = "maintenance-page.html";
const BYPASS_COOKIE: &str = "cpn_maint_bypass";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MaintenanceFlag {
    pub active: bool,
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub progress: u8,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default)]
    pub source: String,
    pub started_at_unix: u64,
    pub updated_at_unix: u64,
    pub expires_at_unix: u64,
    #[serde(default)]
    pub bypass_token: String,
}

impl Default for MaintenanceFlag {
    fn default() -> Self {
        let now = now_unix();
        Self {
            active: false,
            phase: String::new(),
            progress: 0,
            message: String::new(),
            title: String::new(),
            target: None,
            source: String::new(),
            started_at_unix: now,
            updated_at_unix: now,
            expires_at_unix: now,
            bypass_token: String::new(),
        }
    }
}

pub fn flag_path() -> PathBuf {
    data_dir().join(FLAG_NAME)
}

pub fn static_html_path() -> PathBuf {
    data_dir().join(STATIC_HTML_NAME)
}

pub fn bypass_cookie_name() -> &'static str {
    BYPASS_COOKIE
}

fn write_mode_600(path: &PathBuf, contents: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
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
        .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    file.write_all(contents)
        .map_err(|e| format!("Could not save {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn random_token() -> String {
    rand::rng()
        .sample_iter(&rand::distr::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

fn persist(flag: &MaintenanceFlag) -> Result<(), String> {
    let raw = serde_json::to_vec_pretty(flag)
        .map_err(|e| format!("Could not encode maintenance flag: {e}"))?;
    write_mode_600(&flag_path(), &raw)?;
    // Mirror HTML for reverse-proxy ErrorDocument when the panel process is down.
    let html = crate::panel_maintenance_page::render_html(flag);
    let html_path = static_html_path();
    let _ = write_mode_600(&html_path, html.as_bytes());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Static page may be read by a reverse proxy as a non-root user.
        let _ = fs::set_permissions(&html_path, fs::Permissions::from_mode(0o644));
    }
    Ok(())
}

/// Load the flag file without healing. Prefer [`load_active`].
pub fn load_raw() -> Option<MaintenanceFlag> {
    let raw = fs::read_to_string(flag_path()).ok()?;
    serde_json::from_str(&raw).ok()
}

/// Clear expired / inactive flags. Returns the active flag when maintenance applies.
pub fn load_active() -> Option<MaintenanceFlag> {
    let mut flag = load_raw()?;
    if !flag.active {
        return None;
    }
    let now = now_unix();
    if flag.expires_at_unix > 0 && now >= flag.expires_at_unix {
        let _ = clear_with_reason("expired");
        return None;
    }
    Some(flag)
}

/// True when visitors should see the maintenance page.
pub fn is_active() -> bool {
    load_active().is_some()
}

/// Enter maintenance before package stop/replace. Idempotent refresh if already active.
pub fn begin(
    source: &str,
    title: &str,
    message: &str,
    target: Option<&str>,
) -> Result<MaintenanceFlag, String> {
    let now = now_unix();
    let existing = load_raw();
    let bypass = existing
        .as_ref()
        .map(|f| f.bypass_token.clone())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(random_token);
    let started = existing
        .as_ref()
        .filter(|f| f.active)
        .map(|f| f.started_at_unix)
        .unwrap_or(now);
    let flag = MaintenanceFlag {
        active: true,
        phase: "preparing".into(),
        progress: 1,
        message: message.to_string(),
        title: title.to_string(),
        target: target.map(str::to_string),
        source: source.to_string(),
        started_at_unix: started,
        updated_at_unix: now,
        expires_at_unix: now.saturating_add(DEFAULT_TTL_SECS),
        bypass_token: bypass,
    };
    persist(&flag)?;
    Ok(flag)
}

/// Update phase / progress / message while the flag is active.
pub fn update(phase: &str, progress: u8, message: &str) -> Result<(), String> {
    let Some(mut flag) = load_raw() else {
        return Ok(());
    };
    if !flag.active {
        return Ok(());
    }
    flag.phase = phase.to_string();
    flag.progress = progress.min(100);
    flag.message = message.to_string();
    flag.updated_at_unix = now_unix();
    // Soft-extend TTL while progress is still flowing (capped).
    let extend_to = flag.updated_at_unix.saturating_add(DEFAULT_TTL_SECS);
    if extend_to > flag.expires_at_unix {
        flag.expires_at_unix = extend_to;
    }
    persist(&flag)
}

/// Mark restarting so the new process can clear on healthy boot.
pub fn mark_restarting(message: &str) -> Result<(), String> {
    update("restarting", 99, message)
}

pub fn clear_with_reason(_reason: &str) -> Result<(), String> {
    let path = flag_path();
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|e| format!("Could not clear {}: {e}", path.display()))?;
    }
    let html = static_html_path();
    if html.exists() {
        let _ = fs::remove_file(&html);
    }
    Ok(())
}

pub fn clear() -> Result<(), String> {
    clear_with_reason("cleared")
}

/// Called on panel startup: drop restarting / completed / expired flags.
pub fn heal_on_startup() {
    let Some(flag) = load_raw() else {
        return;
    };
    if !flag.active {
        let _ = clear_with_reason("inactive");
        return;
    }
    let now = now_unix();
    if flag.expires_at_unix > 0 && now >= flag.expires_at_unix {
        let _ = clear_with_reason("expired");
        return;
    }
    if matches!(
        flag.phase.as_str(),
        "restarting" | "completed" | "ready" | "failed"
    ) {
        let _ = clear_with_reason("startup-heal");
    }
}

/// Operator heal: always clear a stuck flag (used by `cpn doctor --heal`).
pub fn heal_stuck() -> Option<String> {
    if load_raw().is_some_and(|f| f.active) {
        let _ = clear_with_reason("doctor-heal");
        Some("cleared stuck panel maintenance flag (maintenance.json)".into())
    } else {
        None
    }
}

pub fn bypass_token_matches(candidate: &str) -> bool {
    let Some(flag) = load_active() else {
        return false;
    };
    let want = flag.bypass_token.trim();
    let got = candidate.trim();
    !want.is_empty() && !got.is_empty() && constant_time_eq(want.as_bytes(), got.as_bytes())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Norwegian/European `dd/mm/yyyy HH:MM` (24h) for UI timestamps (UTC label).
pub fn format_nb_datetime(unix: u64) -> String {
    if unix == 0 {
        return String::new();
    }
    let days = unix / 86400;
    let tod = unix % 86400;
    let hour = tod / 3600;
    let min = (tod % 3600) / 60;
    let (y, m, d) = civil_from_days(days as i64);
    format!("{d:02}/{m:02}/{y} {hour:02}:{min:02} UTC")
}

/// Howard Hinnant civil_from_days (proleptic Gregorian).
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn begin_update_clear_roundtrip() {
        with_test_data_dir(|| {
            let flag = begin(
                "cli",
                "Updating CPN Panel",
                "Preparing package apply",
                Some("1.0.1"),
            )
            .expect("begin");
            assert!(flag.active);
            assert!(!flag.bypass_token.is_empty());
            assert!(is_active());
            update("installing", 42, "Installing package").expect("update");
            let loaded = load_active().expect("active");
            assert_eq!(loaded.progress, 42);
            assert_eq!(loaded.phase, "installing");
            clear().expect("clear");
            assert!(!is_active());
            assert!(!flag_path().exists());
        });
    }

    #[test]
    fn expired_flag_auto_heals() {
        with_test_data_dir(|| {
            let mut flag = begin("ui", "Updating CPN Panel", "test", None).expect("begin");
            flag.expires_at_unix = 1;
            persist(&flag).expect("persist");
            assert!(load_active().is_none());
            assert!(!flag_path().exists());
        });
    }

    #[test]
    fn nb_datetime_format() {
        // 01/01/2026 00:00 UTC
        let s = format_nb_datetime(1_767_225_600);
        assert!(s.starts_with("01/01/2026"), "got {s}");
        assert!(s.contains(':'));
    }

    #[test]
    fn bypass_token_constant_time() {
        with_test_data_dir(|| {
            let flag = begin("ui", "Updating CPN Panel", "test", None).expect("begin");
            assert!(bypass_token_matches(&flag.bypass_token));
            assert!(!bypass_token_matches("wrong-token-value-here-xxxxx"));
        });
    }
}
