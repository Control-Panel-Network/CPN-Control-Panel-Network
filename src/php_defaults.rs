//! Install-time PHP branch selection and host default persistence.
//!
//! Fresh CPN installs prefer PHP **8.5** on AlmaLinux/RHEL-family 9+ (Remi).
//! Operators may choose 8.4 / 8.3 / 8.2. When the preferred packages are missing,
//! CPN falls back to the next supported branch and records a clear message.
//!
//! Persisted at `/var/lib/cpn/php-default.json` (mode 600) for new sites.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::os_support::GuestOs;
use crate::paths::default_data_dir;
use crate::php_lifecycle::{
    CPN_MIN_PHP_BRANCH, assert_php_branch_not_eol, branch_from_module_stream,
};

/// Preferred default when the operator does not pick another version.
pub const CPN_PREFERRED_PHP_BRANCH: &str = "8.5";

/// Fallback order when packages for the preferred (or selected) branch are missing.
pub const CPN_PHP_FALLBACK_ORDER: &[&str] = &["8.5", "8.4", "8.3", "8.2"];

const CONFIG_FILE: &str = "php-default.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PhpDefaultRecord {
    /// Major.minor branch actually enabled (for example `8.5`).
    pub branch: String,
    /// DNF module stream tag (for example `php:remi-8.5` or `php:8.2`).
    pub stream: String,
    /// Operator request (`8.5`, `auto`, empty) that produced this selection.
    #[serde(default)]
    pub requested: String,
    /// Human-readable note (including fallback explanation when applicable).
    #[serde(default)]
    pub message: String,
}

fn config_path() -> PathBuf {
    default_data_dir().join(CONFIG_FILE)
}

/// Preferred branch for this guest when the operator leaves the choice empty / auto.
pub fn preferred_branch_for_guest(guest: &GuestOs) -> &'static str {
    if guest.uses_dnf() && guest.major == 8 {
        // AlmaLinux/RHEL 8: keep Remi 8.2 as the install default (8.5 is EL9+).
        return "8.2";
    }
    CPN_PREFERRED_PHP_BRANCH
}

/// Normalize installer input: empty / `auto` / `default` → guest preferred branch.
pub fn normalize_php_request(raw: Option<&str>, guest: Option<&GuestOs>) -> String {
    let trimmed = raw.map(str::trim).unwrap_or("").to_ascii_lowercase();
    if trimmed.is_empty() || trimmed == "auto" || trimmed == "default" {
        return guest
            .map(preferred_branch_for_guest)
            .unwrap_or(CPN_PREFERRED_PHP_BRANCH)
            .to_string();
    }
    if let Some((major, minor)) = parse_branch(&trimmed) {
        return format!("{major}.{minor}");
    }
    trimmed
}

fn parse_branch(value: &str) -> Option<(u32, u32)> {
    let cleaned = value
        .trim()
        .trim_start_matches('v')
        .trim_start_matches("php");
    let cleaned = cleaned.trim_start_matches(':').trim_start_matches("remi-");
    let mut parts = cleaned.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

/// Branches to try: requested first, then remaining fallbacks in order, never below CPN min.
pub fn candidate_branches(requested: &str) -> Vec<&'static str> {
    let req = requested.trim();
    let mut out = Vec::new();
    if let Some(exact) = CPN_PHP_FALLBACK_ORDER.iter().copied().find(|b| *b == req) {
        out.push(exact);
    }
    for branch in CPN_PHP_FALLBACK_ORDER {
        if !out.contains(branch) {
            out.push(*branch);
        }
    }
    out.retain(|branch| {
        branch_major_minor(branch)
            .map(|(maj, min)| {
                let (min_maj, min_min) = branch_major_minor(CPN_MIN_PHP_BRANCH).unwrap_or((8, 2));
                (maj, min) >= (min_maj, min_min)
            })
            .unwrap_or(false)
    });
    out
}

fn branch_major_minor(branch: &str) -> Option<(u32, u32)> {
    parse_branch(branch)
}

/// Map a branch to the preferred DNF module stream for this guest.
pub fn stream_for_branch(guest: &GuestOs, branch: &str) -> Option<String> {
    if !guest.uses_dnf() {
        return None;
    }
    // Remi module streams on EL8/EL9/EL10; AppStream php:8.2 is a separate last-resort path.
    Some(format!("php:remi-{branch}"))
}

/// AppStream fallback stream for EL9 when Remi 8.2 is unavailable.
fn appstream_fallback(guest: &GuestOs, branch: &str) -> Option<String> {
    if guest.uses_dnf() && guest.major == 9 && branch == "8.2" {
        Some("php:8.2".into())
    } else {
        None
    }
}

fn remi_release_url(guest: &GuestOs) -> Option<&'static str> {
    if !guest.uses_dnf() {
        return None;
    }
    match guest.major {
        8 => Some("https://rpms.remirepo.net/enterprise/remi-release-8.rpm"),
        9 => Some("https://rpms.remirepo.net/enterprise/remi-release-9.rpm"),
        10 => Some("https://rpms.remirepo.net/enterprise/remi-release-10.rpm"),
        _ => None,
    }
}

fn run_bash(script: &str) -> Result<(), String> {
    let status = Command::new("bash")
        .args(["-c", script])
        .env("LC_ALL", "C")
        .status()
        .map_err(|e| format!("Failed to run PHP prepare script: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "PHP prepare script exited with {}",
            status.code().unwrap_or(-1)
        ))
    }
}

fn ensure_remi_repo(guest: &GuestOs) -> Result<(), String> {
    let Some(url) = remi_release_url(guest) else {
        return Ok(());
    };
    // Idempotent: already-installed Remi is fine.
    let script = format!(
        "rpm -q remi-release >/dev/null 2>&1 || dnf --setopt=lock_timeout=60 install -y '{url}'"
    );
    run_bash(&script).map_err(|e| format!("Could not install Remi release package: {e}"))
}

/// Switch the Remi/AppStream `php` module stream and sync installed packages.
///
/// `dnf module enable` alone leaves prior stream RPMs in place (e.g. host default
/// JSON says 8.4 while `php-fpm` remains 8.5). Prefer `module switch-to`, which
/// enables the stream and upgrades/downgrades modular packages. Fall back to
/// reset+enable for first-boot hosts with no PHP packages yet.
fn try_switch_stream(stream: &str) -> bool {
    let switch = format!(
        "dnf --setopt=lock_timeout=120 -y module switch-to '{stream}'"
    );
    if run_bash(&switch).is_ok() {
        return true;
    }
    let enable = format!(
        "dnf -y module reset php >/dev/null 2>&1 || true; dnf --setopt=lock_timeout=120 -y module enable '{stream}'"
    );
    run_bash(&enable).is_ok()
}

/// Major.minor reported by host `php` CLI (empty when php is missing).
pub fn host_php_cli_branch() -> Option<String> {
    let out = Command::new("php")
        .args(["-r", "echo PHP_MAJOR_VERSION, '.', PHP_MINOR_VERSION;"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() || !s.contains('.') {
        return None;
    }
    Some(s)
}

/// Major.minor reported by `/usr/sbin/php-fpm -v` (phpMyAdmin pool binary).
pub fn host_php_fpm_branch() -> Option<String> {
    let bin = if Path::new("/usr/sbin/php-fpm").is_file() {
        "/usr/sbin/php-fpm"
    } else {
        "php-fpm"
    };
    let out = Command::new(bin).arg("-v").output().ok()?;
    if !out.status.success() && out.stdout.is_empty() {
        return None;
    }
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    // "PHP 8.5.10 (fpm-fcgi) ..."
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("PHP ") {
            let ver = rest.split_whitespace().next().unwrap_or("");
            let mut parts = ver.split('.');
            let maj = parts.next()?.to_string();
            let min = parts.next()?.to_string();
            if !maj.is_empty() && !min.is_empty() {
                return Some(format!("{maj}.{min}"));
            }
        }
    }
    None
}

/// Enable the best available PHP module stream for this guest and persist the choice.
///
/// `requested`: operator choice (`8.5`, `8.4`, …) or `None`/`auto` for preferred 8.5.
/// When the preferred packages are missing, falls back and sets `message` accordingly.
pub fn prepare_and_persist_php(
    guest: &GuestOs,
    requested: Option<&str>,
    today_ymd: &str,
) -> Result<PhpDefaultRecord, String> {
    if guest.is_windows() {
        return Err("PHP module streams are not available on Windows Server Phase A".into());
    }
    let preferred = preferred_branch_for_guest(guest);
    if guest.uses_apt() {
        let record = PhpDefaultRecord {
            branch: preferred.to_string(),
            stream: "apt".into(),
            requested: normalize_php_request(requested, Some(guest)),
            message: format!(
                "Using distro PHP packages on {} (no DNF module stream). Preferred CPN default remains {}.",
                guest.label, preferred
            ),
        };
        let _ = save_php_default(&record);
        return Ok(record);
    }
    if !guest.uses_dnf() {
        return Err(format!("No PHP module path for guest {}", guest.label));
    }

    let explicit = requested
        .map(str::trim)
        .filter(|v| !v.is_empty() && *v != "auto" && *v != "default");
    let requested_norm = normalize_php_request(requested, Some(guest));
    if let Some(raw) = explicit {
        // Explicit operator pick must pass lifecycle; auto/default may fall back.
        assert_php_branch_not_eol(&requested_norm, today_ymd)
            .map_err(|e| format!("PHP {raw} is not allowed: {e}"))?;
    }

    ensure_remi_repo(guest)?;

    let candidates = candidate_branches(&requested_norm);
    let mut attempts: Vec<String> = Vec::new();
    for branch in &candidates {
        if assert_php_branch_not_eol(branch, today_ymd).is_err() {
            continue;
        }
        let mut streams = Vec::new();
        if let Some(s) = stream_for_branch(guest, branch) {
            streams.push(s);
        }
        if let Some(s) = appstream_fallback(guest, branch)
            && !streams.contains(&s)
        {
            streams.push(s);
        }
        for stream in streams {
            if try_switch_stream(&stream) {
                let fallback = *branch != requested_norm.as_str();
                let message = if fallback {
                    format!(
                        "PHP {requested_norm} packages were not available; switched to {stream} (PHP {branch}) instead."
                    )
                } else {
                    format!("Switched host PHP to {stream} (PHP {branch}).")
                };
                let record = PhpDefaultRecord {
                    branch: (*branch).to_string(),
                    stream: stream.clone(),
                    requested: requested_norm,
                    message,
                };
                save_php_default(&record)?;
                return Ok(record);
            }
            attempts.push(stream);
        }
    }

    Err(format!(
        "Could not switch host PHP to {}. Tried streams: {}. Install Remi PHP packages or choose another version.",
        requested_norm,
        attempts.join(", ")
    ))
}

pub fn save_php_default(record: &PhpDefaultRecord) -> Result<(), String> {
    let dir = default_data_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create {}: {e}", dir.display()))?;
    let path = config_path();
    let body = serde_json::to_vec_pretty(record)
        .map_err(|e| format!("Cannot encode php-default.json: {e}"))?;
    fs::write(&path, body).map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Approximate UTC `YYYY-MM-DD` from unix seconds (no chrono dependency).
pub fn utc_ymd_approx(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let mut y = 1970i32;
    let mut rem = days;
    loop {
        let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
        let diy = if leap { 366 } else { 365 };
        if rem < diy {
            break;
        }
        rem -= diy;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let mdays = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u32;
    for (idx, dim) in mdays.iter().enumerate() {
        if rem < *dim {
            month = (idx + 1) as u32;
            break;
        }
        rem -= *dim;
    }
    let day = (rem + 1) as u32;
    format!("{y:04}-{month:02}-{day:02}")
}

pub fn today_ymd() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_secs())
        .unwrap_or(0);
    utc_ymd_approx(secs)
}

pub fn load_php_default() -> Option<PhpDefaultRecord> {
    let path = config_path();
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

/// Branch used for new sites when no per-site override is set.
pub fn default_php_branch_for_sites() -> String {
    load_php_default()
        .map(|r| r.branch)
        .unwrap_or_else(|| CPN_PREFERRED_PHP_BRANCH.to_string())
}

/// Default module stream hint for recipes that still call GuestOs::php_module_stream.
pub fn preferred_stream_for_guest(guest: &GuestOs) -> Option<&'static str> {
    if !guest.uses_dnf() {
        return None;
    }
    match guest.major {
        8 => Some("remi-8.2"), // EL8: stay on Remi 8.2 unless operator overrides at install
        9 | 10 => Some("php:remi-8.5"),
        _ => None,
    }
}

/// Validate a stream string against lifecycle rules (used by install_php_runtime).
pub fn assert_stream_ok(stream: &str, today_ymd: &str) -> Result<(), String> {
    let branch = branch_from_module_stream(stream)
        .ok_or_else(|| format!("Cannot derive PHP branch from module stream '{stream}'"))?;
    assert_php_branch_not_eol(branch, today_ymd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os_support::detect_from_os_release;

    #[test]
    fn normalize_auto_is_85_on_el9() {
        let nine = detect_from_os_release(
            "ID=almalinux\nVERSION_ID=\"9.8\"\nPRETTY_NAME=\"AlmaLinux 9.8\"\n",
        )
        .unwrap();
        assert_eq!(normalize_php_request(None, Some(&nine)), "8.5");
        assert_eq!(normalize_php_request(Some("auto"), Some(&nine)), "8.5");
        assert_eq!(normalize_php_request(Some("8.4"), Some(&nine)), "8.4");
    }

    #[test]
    fn candidates_prefer_requested_then_fallback() {
        assert_eq!(candidate_branches("8.5"), vec!["8.5", "8.4", "8.3", "8.2"]);
        assert_eq!(candidate_branches("8.3"), vec!["8.3", "8.5", "8.4", "8.2"]);
    }

    #[test]
    fn parses_fpm_version_line() {
        let sample = "PHP 8.4.25 (fpm-fcgi) (built: Aug 25 2026)\nCopyright";
        let mut found = None;
        for line in sample.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("PHP ") {
                let ver = rest.split_whitespace().next().unwrap_or("");
                let mut parts = ver.split('.');
                if let (Some(maj), Some(min)) = (parts.next(), parts.next()) {
                    found = Some(format!("{maj}.{min}"));
                }
            }
        }
        assert_eq!(found.as_deref(), Some("8.4"));
    }

    #[test]
    fn el9_preferred_stream_is_remi_85() {
        let nine = detect_from_os_release(
            "ID=almalinux\nVERSION_ID=\"9.8\"\nPRETTY_NAME=\"AlmaLinux 9.8\"\n",
        )
        .unwrap();
        assert_eq!(preferred_stream_for_guest(&nine), Some("php:remi-8.5"));
        assert_eq!(
            stream_for_branch(&nine, "8.5").as_deref(),
            Some("php:remi-8.5")
        );
    }

    #[test]
    fn el8_preferred_stream_stays_remi_82() {
        let eight = detect_from_os_release("ID=almalinux\nVERSION_ID=\"8.10\"\n").unwrap();
        assert_eq!(preferred_stream_for_guest(&eight), Some("remi-8.2"));
    }
}
