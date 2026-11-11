//! Owner-controlled automatic panel upgrades from GitHub Releases.
//!
//! Config: `/var/lib/cpn/auto-upgrade.json` (mode 600).
//! Default: **enabled** on fresh installs (and when the config file is
//! missing), with strategy **All newer releases**. The Owner can turn this
//! off under Version Management at any time; an explicit saved `enabled:
//! false` is never overwritten by seeding.
//! When enabled, a background worker periodically checks for a newer
//! installable release and runs the same package upgrade path as Version
//! Management (`spawn_maintenance` / `cpn-installer --upgrade`).
//!
//! Security-only mode prefers machine-readable release metadata
//! (`CPN-SECURITY-RELEASE:` body marker, `cpn-security-release.json` asset,
//! or `CPN-SECURITY-RELEASE` marker asset) over keyword matching on tag and
//! title. Heuristics remain a fallback for older releases. Prefer **All newer
//! releases** (the default) when you need certainty; turn automatic updates
//! off if you do not want scheduled upgrades.

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
    /// Master switch. Default true (enabled on install; Owner may opt out).
    #[serde(default = "default_enabled_on")]
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
    /// Unix time for the next background check (enabled only).
    #[serde(default)]
    pub next_check_at_unix: u64,
    /// Extra delay baked into the current schedule (per-install jitter).
    #[serde(default)]
    pub schedule_jitter_secs: u64,
    #[serde(default)]
    pub updated_at_unix: u64,
}

fn default_enabled_on() -> bool {
    true
}

fn default_interval() -> u64 {
    DEFAULT_INTERVAL_SECS
}

impl Default for AutoUpgradeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: AutoUpgradeMode::All,
            include_prerelease: false,
            check_interval_secs: DEFAULT_INTERVAL_SECS,
            last_check_at: 0,
            last_check_ok: true,
            last_result: String::new(),
            last_applied_version: String::new(),
            next_check_at_unix: 0,
            schedule_jitter_secs: 0,
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
    pub next_check_at_unix: u64,
    pub seconds_until_next_check: u64,
    pub schedule_jitter_secs: u64,
    pub check_interval_base_secs: u64,
    pub show_countdown: bool,
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
    if clean.enabled {
        crate::auto_upgrade_schedule::ensure_next_check_scheduled(&mut clean);
    } else {
        clean.next_check_at_unix = 0;
        clean.schedule_jitter_secs = 0;
    }
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

pub fn public_status(cfg: &AutoUpgradeConfig, show_countdown: bool) -> AutoUpgradePublic {
    let base = interval_secs(cfg);
    let until = crate::auto_upgrade_schedule::seconds_until_next_check(cfg);
    AutoUpgradePublic {
        enabled: cfg.enabled,
        mode: cfg.mode,
        include_prerelease: cfg.include_prerelease,
        check_interval_secs: cfg.check_interval_secs.max(MIN_INTERVAL_SECS),
        last_check_at: cfg.last_check_at,
        last_check_ok: cfg.last_check_ok,
        last_result: cfg.last_result.clone(),
        last_applied_version: cfg.last_applied_version.clone(),
        next_check_at_unix: cfg.next_check_at_unix,
        seconds_until_next_check: until,
        schedule_jitter_secs: cfg.schedule_jitter_secs,
        check_interval_base_secs: base,
        show_countdown: show_countdown && cfg.enabled,
        updated_at_unix: cfg.updated_at_unix,
        default_enabled: true,
        security_detection_note: "Security-only matching prefers release metadata (CPN-SECURITY-RELEASE body marker, cpn-security-release.json, or CPN-SECURITY-RELEASE asset) over tag/title keywords (CVE, GHSA, security, secfix, advisory). Older releases without metadata still use heuristics and can miss or mis-classify updates. Prefer All newer releases (the default). Turn automatic updates off under Version Management if you do not want scheduled upgrades.".into(),
    }
}

/// Parse `CPN-SECURITY-RELEASE: true|false` from a GitHub release body.
pub fn security_release_flag_from_body(body: &str) -> Option<bool> {
    for line in body.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed
            .strip_prefix("CPN-SECURITY-RELEASE:")
            .or_else(|| trimmed.strip_prefix("cpn-security-release:"))
        else {
            continue;
        };
        let value = rest.trim().to_ascii_lowercase();
        return Some(matches!(value.as_str(), "true" | "1" | "yes" | "on"));
    }
    None
}

/// True when the release assets include the explicit security marker file.
pub fn security_release_marker_asset_present(release: &CpnRelease) -> bool {
    release
        .assets
        .iter()
        .any(|asset| asset.name.eq_ignore_ascii_case("CPN-SECURITY-RELEASE"))
}

/// URL of the machine-readable security metadata asset, when present.
pub fn security_release_meta_asset_url(release: &CpnRelease) -> Option<&str> {
    release.assets.iter().find_map(|asset| {
        if asset.name == "cpn-security-release.json" {
            Some(asset.browser_download_url.as_str())
        } else {
            None
        }
    })
}

/// Best-effort sync classification: prefer body/marker metadata, else tag/title heuristics.
pub fn looks_like_security_release(release: &CpnRelease) -> bool {
    if let Some(flag) = security_release_flag_from_body(&release.body) {
        return flag;
    }
    if security_release_marker_asset_present(release) {
        return true;
    }
    security_release_title_heuristics(release)
}

fn security_release_title_heuristics(release: &CpnRelease) -> bool {
    let hay = format!("{} {}", release.tag_name, release.name).to_ascii_lowercase();
    const KEYS: &[&str] = &[
        "security", "cve-", "cve ", "ghsa-", "ghsa ", "secfix", "sec-fix", "advisory", "vulnerab",
        "hotfix",
    ];
    KEYS.iter().any(|k| hay.contains(k))
}

/// Async classification: prefer body marker, then download `cpn-security-release.json`, then marker asset, then heuristics.
pub async fn resolve_security_release(release: &CpnRelease) -> bool {
    if let Some(flag) = security_release_flag_from_body(&release.body) {
        return flag;
    }
    if let Some(url) = security_release_meta_asset_url(release)
        && let Ok(flag) = fetch_security_release_json_flag(url).await
    {
        return flag;
    }
    if security_release_marker_asset_present(release) {
        return true;
    }
    security_release_title_heuristics(release)
}

async fn fetch_security_release_json_flag(url: &str) -> Result<bool, String> {
    use std::process::Stdio;
    use tokio::process::Command;

    let tmp = std::env::temp_dir().join(format!(
        "cpn-sec-meta-{}-{}.json",
        std::process::id(),
        now_unix()
    ));
    let _ = std::fs::remove_file(&tmp);
    let user_agent = format!("User-Agent: CPN-Installer/{}", env!("CARGO_PKG_VERSION"));
    let output = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--location",
            "--max-time",
            "15",
            "-o",
            tmp.to_string_lossy().as_ref(),
            "-w",
            "%{http_code}",
            "-H",
            user_agent.as_str(),
            url,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("Could not download security metadata: {e}"))?;
    let status = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u16>()
        .unwrap_or(0);
    let body = std::fs::read_to_string(&tmp).unwrap_or_default();
    let _ = std::fs::remove_file(&tmp);
    if !(200..300).contains(&status) {
        return Err(format!("security metadata HTTP {status}"));
    }
    let value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| format!("Invalid cpn-security-release.json: {e}"))?;
    value
        .get("security_release")
        .and_then(|v| v.as_bool())
        .ok_or_else(|| "cpn-security-release.json missing boolean security_release".into())
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
        sample_with_body(tag, name, "")
    }

    fn sample_with_body(tag: &str, name: &str, body: &str) -> CpnRelease {
        CpnRelease {
            tag_name: tag.into(),
            version: releases::normalize_version(tag),
            name: name.into(),
            published_at: "2026-11-01".into(),
            prerelease: false,
            draft: false,
            html_url: String::new(),
            body: body.into(),
            assets: Vec::new(),
            rpm_asset: None,
            binary_asset: None,
            checksums_asset: None,
            checksums_asc_asset: None,
        }
    }

    #[test]
    fn default_is_on_all_releases() {
        let cfg = AutoUpgradeConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.mode, AutoUpgradeMode::All);
        assert!(!cfg.include_prerelease);
        assert_eq!(cfg.check_interval_secs, DEFAULT_INTERVAL_SECS);
    }

    #[test]
    fn missing_enabled_field_deserializes_on() {
        let cfg: AutoUpgradeConfig =
            serde_json::from_str(r#"{"mode":"all","include_prerelease":false}"#).unwrap();
        assert!(cfg.enabled);
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
    fn security_metadata_preferred_over_title() {
        assert!(!looks_like_security_release(&sample_with_body(
            "v1.4.8",
            "Security polish wording only",
            "CPN-SECURITY-RELEASE: false\n\nNotes"
        )));
        assert!(looks_like_security_release(&sample_with_body(
            "v1.4.9",
            "Routine package bump",
            "CPN-SECURITY-RELEASE: true\n"
        )));
        let mut with_marker = sample("v1.5.0", "Routine");
        with_marker.assets.push(crate::releases::ReleaseAsset {
            name: "CPN-SECURITY-RELEASE".into(),
            browser_download_url: "https://example.invalid/CPN-SECURITY-RELEASE".into(),
            content_type: "text/plain".into(),
            size: 5,
        });
        assert!(looks_like_security_release(&with_marker));
    }

    #[test]
    fn public_status_documents_default_on() {
        let pub_status = public_status(&AutoUpgradeConfig::default(), true);
        assert!(pub_status.default_enabled);
        assert!(pub_status.security_detection_note.contains("Security-only"));
        assert!(
            pub_status
                .security_detection_note
                .contains("All newer releases")
        );
        assert!(!pub_status.security_detection_note.contains('\u{2014}'));
        assert!(!pub_status.security_detection_note.contains('\u{2013}'));
    }
}
