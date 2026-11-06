//! Owner-controlled automatic panel upgrades from GitHub Releases.
//!
//! Config: `/var/lib/cpn/auto-upgrade.json` (mode 600).
//! Default: **disabled** so upgrades never surprise a production host.
//! When enabled, a background worker periodically checks for a newer
//! installable release and runs the same package upgrade path as Version
//! Management (`spawn_maintenance` / `cpn-installer --upgrade`).
//!
//! Security-only mode is best-effort: GitHub release tags/names rarely carry
//! reliable security labels, so keyword matching on tag and title is used.
//! Prefer leaving automatic updates off, or use all newer releases, when
//! you need certainty.

use crate::paths;
use crate::releases::CpnRelease;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Default check interval: 6 hours.
pub const DEFAULT_INTERVAL_SECS: u64 = 6 * 60 * 60;
/// Floor so operators cannot hammer GitHub Releases.
pub const MIN_INTERVAL_SECS: u64 = 60 * 60;
/// Startup delay before the first background check.
pub const STARTUP_DELAY_SECS: u64 = 90;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AutoUpgradeMode {
    /// Upgrade when any newer installable release exists (recommended when ON).
    #[default]
    All,
    /// Upgrade only when the newer release tag/title looks security-related.
    Security,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoUpgradeConfig {
    /// Master switch. Default false (Owner must opt in).
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub mode: AutoUpgradeMode,
    /// When false (default), skip GitHub prerelease tags.
    #[serde(default)]
    pub include_prerelease: bool,
    /// Seconds between background checks (clamped to at least [`MIN_INTERVAL_SECS`]).
    #[serde(default = "default_interval")]
    pub check_interval_secs: u64,
    #[serde(default)]
    pub last_check_at: u64,
    #[serde(default)]
    pub last_check_ok: bool,
    #[serde(default)]
    pub last_result: String,
    #[serde(default)]
    pub last_applied_version: String,
    #[serde(default)]
    pub updated_at_unix: u64,
}

fn default_interval() -> u64 {
    DEFAULT_INTERVAL_SECS
}

impl Default for AutoUpgradeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: AutoUpgradeMode::All,
            include_prerelease: false,
            check_interval_secs: DEFAULT_INTERVAL_SECS,
            last_check_at: 0,
            last_check_ok: true,
            last_result: String::new(),
            last_applied_version: String::new(),
            updated_at_unix: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AutoUpgradePublic {
    pub enabled: bool,
    pub mode: AutoUpgradeMode,
    pub include_prerelease: bool,
    pub check_interval_secs: u64,
    pub last_check_at: u64,
    pub last_check_ok: bool,
    pub last_result: String,
    pub last_applied_version: String,
    pub updated_at_unix: u64,
    pub default_enabled: bool,
    pub security_detection_note: String,
}

pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn config_path() -> PathBuf {
    paths::default_data_dir().join("auto-upgrade.json")
}

fn set_private(path: &PathBuf) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

fn ensure_data_dir() -> Result<(), String> {
    let dir = paths::default_data_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create data dir: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

pub fn load_auto_upgrade_config() -> AutoUpgradeConfig {
    let Ok(raw) = fs::read_to_string(config_path()) else {
        return AutoUpgradeConfig::default();
    };
    let mut cfg: AutoUpgradeConfig = serde_json::from_str(&raw).unwrap_or_default();
    cfg.check_interval_secs = cfg.check_interval_secs.max(MIN_INTERVAL_SECS);
    cfg
}

/// Seed product defaults when the config file is missing (fresh install /
/// first panel start). Never overwrites an existing file, so Owner-saved OFF
/// and any prior explicit config stay as written.
pub fn ensure_default_auto_upgrade_config() {
    if config_path().is_file() {
        return;
    }
    let _ = save_auto_upgrade_config(&AutoUpgradeConfig::default());
}

pub fn save_auto_upgrade_config(cfg: &AutoUpgradeConfig) -> Result<(), String> {
    ensure_data_dir()?;
    let mut clean = cfg.clone();
    clean.check_interval_secs = clean.check_interval_secs.max(MIN_INTERVAL_SECS);
    clean.updated_at_unix = now_unix();
    let path = config_path();
    let raw = serde_json::to_string_pretty(&clean)
        .map_err(|e| format!("Could not serialize auto-upgrade config: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp)
            .map_err(|e| format!("Could not write auto-upgrade temp: {e}"))?;
        f.write_all(raw.as_bytes())
            .map_err(|e| format!("Could not write auto-upgrade config: {e}"))?;
        f.write_all(b"\n")
            .map_err(|e| format!("Could not write auto-upgrade newline: {e}"))?;
        f.sync_all()
            .map_err(|e| format!("Could not sync auto-upgrade config: {e}"))?;
    }
    set_private(&tmp);
    fs::rename(&tmp, &path).map_err(|e| format!("Could not replace auto-upgrade config: {e}"))?;
    set_private(&path);
    Ok(())
}

pub fn public_status(cfg: &AutoUpgradeConfig) -> AutoUpgradePublic {
    AutoUpgradePublic {
        enabled: cfg.enabled,
        mode: cfg.mode,
        include_prerelease: cfg.include_prerelease,
        check_interval_secs: cfg.check_interval_secs.max(MIN_INTERVAL_SECS),
        last_check_at: cfg.last_check_at,
        last_check_ok: cfg.last_check_ok,
        last_result: cfg.last_result.clone(),
        last_applied_version: cfg.last_applied_version.clone(),
        updated_at_unix: cfg.updated_at_unix,
        default_enabled: false,
        security_detection_note: "Security-only matching uses the release tag and title (CVE, GHSA, security, secfix, advisory). GitHub does not always label security releases, so this mode can miss or mis-classify updates. Prefer off, or all newer releases, when you need certainty.".into(),
    }
}

/// Best-effort security keyword match on tag + title (no release body required).
pub fn looks_like_security_release(release: &CpnRelease) -> bool {
    let hay = format!("{} {}", release.tag_name, release.name).to_ascii_lowercase();
    const KEYS: &[&str] = &[
        "security", "cve-", "cve ", "ghsa-", "ghsa ", "secfix", "sec-fix", "advisory", "vulnerab",
        "hotfix",
    ];
    KEYS.iter().any(|k| hay.contains(k))
}

pub fn interval_secs(cfg: &AutoUpgradeConfig) -> u64 {
    std::env::var("CPN_AUTO_UPGRADE_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v >= MIN_INTERVAL_SECS)
        .unwrap_or_else(|| cfg.check_interval_secs.max(MIN_INTERVAL_SECS))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::releases;

    fn sample(tag: &str, name: &str) -> CpnRelease {
        CpnRelease {
            tag_name: tag.into(),
            version: releases::normalize_version(tag),
            name: name.into(),
            published_at: "2026-11-01".into(),
            prerelease: false,
            draft: false,
            html_url: String::new(),
            assets: Vec::new(),
            rpm_asset: None,
            binary_asset: None,
            checksums_asset: None,
            checksums_asc_asset: None,
        }
    }

    #[test]
    fn default_is_off() {
        let cfg = AutoUpgradeConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.mode, AutoUpgradeMode::All);
        assert!(!cfg.include_prerelease);
        assert_eq!(cfg.check_interval_secs, DEFAULT_INTERVAL_SECS);
    }

    #[test]
    fn missing_enabled_field_deserializes_off() {
        let cfg: AutoUpgradeConfig =
            serde_json::from_str(r#"{"mode":"all","include_prerelease":false}"#).unwrap();
        assert!(!cfg.enabled);
        assert_eq!(cfg.mode, AutoUpgradeMode::All);
    }

    #[test]
    fn explicit_off_deserializes_off() {
        let cfg: AutoUpgradeConfig =
            serde_json::from_str(r#"{"enabled":false,"mode":"security"}"#).unwrap();
        assert!(!cfg.enabled);
        assert_eq!(cfg.mode, AutoUpgradeMode::Security);
    }

    #[test]
    fn security_keywords_detected() {
        assert!(looks_like_security_release(&sample(
            "v1.4.8",
            "Security fix for auth bypass"
        )));
        assert!(looks_like_security_release(&sample(
            "v1.4.8-cve-2026-1",
            ""
        )));
        assert!(looks_like_security_release(&sample(
            "v1.4.9",
            "GHSA-xxxx advisory"
        )));
        assert!(!looks_like_security_release(&sample(
            "v1.4.8",
            "Routine polish and docs"
        )));
    }

    #[test]
    fn public_status_documents_default_off() {
        let pub_status = public_status(&AutoUpgradeConfig::default());
        assert!(!pub_status.default_enabled);
        assert!(pub_status.security_detection_note.contains("Security-only"));
        assert!(pub_status.security_detection_note.contains("Prefer off"));
        assert!(!pub_status.security_detection_note.contains('\u{2014}'));
        assert!(!pub_status.security_detection_note.contains('\u{2013}'));
    }
}
