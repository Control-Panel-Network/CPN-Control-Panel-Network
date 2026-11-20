//! LiteSpeed Enterprise license helpers (`serial.no`, `license.key`, `trial.key`).
//!
//! OpenLiteSpeed does not use these files. Enterprise LSWS expects a serial written
//! to `/usr/local/lsws/conf/serial.no`, then `lshttpd -r` to fetch `license.key`, or
//! an operator-supplied `license.key` / `trial.key`. Paths match CyberPanel 3.0.x.

use crate::litespeed_stack::{
    LiteSpeedKind, detect_kind, load_config, litespeed_enterprise_installed, mask_serial,
    restart_litespeed, save_config, write_mode_600,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const CONF_DIR: &str = "/usr/local/lsws/conf";
const LSHTTPD: &str = "/usr/local/lsws/bin/lshttpd";
const TRIAL_URL: &str = "http://license.litespeedtech.com/reseller/trial.key";
const MAX_LICENSE_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LicenseStatus {
    pub edition: String,
    pub serial_present: bool,
    pub serial_masked: String,
    pub license_key_present: bool,
    pub trial_key_present: bool,
    pub probe_ok: bool,
    pub probe_summary: String,
    pub raw_probe: String,
    pub notes: Vec<String>,
}

fn serial_path() -> PathBuf {
    Path::new(CONF_DIR).join("serial.no")
}

fn license_key_path() -> PathBuf {
    Path::new(CONF_DIR).join("license.key")
}

fn trial_key_path() -> PathBuf {
    Path::new(CONF_DIR).join("trial.key")
}

fn conf_dir_ok() -> Result<(), String> {
    if Path::new(CONF_DIR).is_dir() {
        Ok(())
    } else {
        Err(format!("LiteSpeed conf directory is missing ({CONF_DIR})."))
    }
}

fn read_trimmed_file(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn backup_if_exists(path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Ok(());
    }
    let stamp = chrono_like_stamp();
    let backup = PathBuf::from(format!("{}.cpn-bak-{stamp}", path.display()));
    fs::copy(path, &backup).map_err(|e| format!("backup {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&backup, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn chrono_like_stamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

fn ensure_lsadm_owner(path: &Path) -> String {
    #[cfg(unix)]
    {
        let status = Command::new("chown")
            .args(["lsadm:lsadm"])
            .arg(path)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if status {
            return "Ownership set to lsadm:lsadm.".into();
        }
        let _ = path;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    "Left file ownership unchanged (chown lsadm skipped or failed).".into()
}

fn run_lshttpd(args: &[&str]) -> (bool, String) {
    if !Path::new(LSHTTPD).is_file() {
        return (false, format!("{LSHTTPD} is missing."));
    }
    let output = Command::new(LSHTTPD)
        .args(args)
        .output()
        .map_err(|e| format!("spawn lshttpd: {e}"));
    match output {
        Ok(out) => {
            let mut text = String::new();
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            if !stdout.trim().is_empty() {
                text.push_str(stdout.trim());
            }
            if !stderr.trim().is_empty() {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(stderr.trim());
            }
            if text.is_empty() {
                text = if out.status.success() {
                    "lshttpd completed with no output.".into()
                } else {
                    format!("lshttpd exited {}", out.status)
                };
            }
            (out.status.success(), text)
        }
        Err(err) => (false, err),
    }
}

fn summarize_probe(raw: &str) -> String {
    let interesting: Vec<&str> = raw
        .lines()
        .map(str::trim)
        .filter(|line| {
            if line.is_empty() {
                return false;
            }
            let lower = line.to_ascii_lowercase();
            lower.contains("license")
                || lower.contains("expir")
                || lower.contains("serial")
                || lower.contains("error")
                || lower.contains("trial")
                || lower.contains("valid")
                || lower.contains("domain")
                || lower.contains("running")
        })
        .take(12)
        .collect();
    if interesting.is_empty() {
        raw.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .take(6)
            .collect::<Vec<_>>()
            .join(" | ")
    } else {
        interesting.join(" | ")
    }
}

/// Read serial / license.key / trial.key presence and probe `lshttpd -V` when possible.
pub fn license_status() -> LicenseStatus {
    let edition = match detect_kind() {
        Some(LiteSpeedKind::OpenLiteSpeed) => "OpenLiteSpeed (no Enterprise license files required)",
        Some(LiteSpeedKind::Enterprise) => "LiteSpeed Enterprise",
        None => "LiteSpeed not detected",
    }
    .to_string();

    let serial = read_trimmed_file(&serial_path());
    let serial_present = serial.is_some();
    let serial_masked = serial
        .as_deref()
        .map(mask_serial)
        .unwrap_or_else(|| "-".into());
    let license_key_present = license_key_path().is_file();
    let trial_key_present = trial_key_path().is_file();

    let mut notes = Vec::new();
    if matches!(detect_kind(), Some(LiteSpeedKind::OpenLiteSpeed)) {
        notes.push(
            "This host runs OpenLiteSpeed. serial.no / license.key apply only after switching to LiteSpeed Enterprise."
                .into(),
        );
    }
    if !Path::new(CONF_DIR).is_dir() {
        notes.push(format!("Missing conf directory: {CONF_DIR}"));
    }

    let (probe_ok, raw_probe) = if Path::new(LSHTTPD).is_file() {
        run_lshttpd(&["-V"])
    } else {
        (false, format!("{LSHTTPD} is missing."))
    };
    if raw_probe.to_ascii_lowercase().contains("is running") {
        notes.push(
            "lshttpd -V refused while the server is running. Status below is file presence plus probe stderr; stop briefly or use Refresh after applying a serial if you need a full license dump."
                .into(),
        );
    }

    LicenseStatus {
        edition,
        serial_present,
        serial_masked,
        license_key_present,
        trial_key_present,
        probe_ok,
        probe_summary: summarize_probe(&raw_probe),
        raw_probe,
        notes,
    }
}

/// Write serial.no, register with `lshttpd -r` (Enterprise), store masked copy in panel config, restart.
pub fn apply_serial_and_register(serial: &str) -> Result<String, String> {
    let serial = serial.trim();
    if serial.is_empty() {
        return Err("Serial is empty.".into());
    }
    if serial.len() > 256 || serial.chars().any(|c| c.is_control()) {
        return Err("Serial looks invalid.".into());
    }
    conf_dir_ok()?;
    backup_if_exists(&serial_path())?;
    let mut body = serial.to_string();
    body.push('\n');
    write_mode_600(&serial_path(), body.as_bytes())?;
    let owner_note = ensure_lsadm_owner(&serial_path());

    let mut cfg = load_config();
    cfg.serial = Some(serial.to_string());
    save_config(&cfg)?;

    let mut parts = vec![format!(
        "License serial applied to serial.no ({}).",
        mask_serial(serial)
    )];
    parts.push(owner_note);

    if litespeed_enterprise_installed() || Path::new(LSHTTPD).is_file() {
        let (ok, out) = run_lshttpd(&["-r"]);
        if ok {
            parts.push("Registered serial with lshttpd -r (license.key refreshed when the license server answered).".into());
        } else {
            parts.push(format!(
                "lshttpd -r did not succeed (common on OpenLiteSpeed or offline hosts): {}",
                summarize_probe(&out)
            ));
        }
        if license_key_path().is_file() {
            let _ = ensure_lsadm_owner(&license_key_path());
            parts.push("license.key is present under conf/.".into());
        }
    }

    let restart = restart_litespeed();
    parts.push(restart);
    Ok(parts.join(" "))
}

/// Upload or paste a vendor `license.key` into `/usr/local/lsws/conf/license.key`.
pub fn apply_license_key_bytes(raw: &[u8]) -> Result<String, String> {
    if raw.is_empty() {
        return Err("license.key content is empty.".into());
    }
    if raw.len() > MAX_LICENSE_BYTES {
        return Err(format!(
            "license.key is too large (max {MAX_LICENSE_BYTES} bytes)."
        ));
    }
    if raw.iter().any(|b| *b == 0) {
        return Err("license.key must not contain NUL bytes.".into());
    }
    // Reject obvious HTML/error pages mistaken for keys (validate before host paths for unit tests).
    let as_text = String::from_utf8_lossy(raw);
    let lower = as_text.to_ascii_lowercase();
    if lower.contains("<html") || lower.contains("<!doctype") {
        return Err("Content looks like HTML, not a LiteSpeed license.key.".into());
    }
    conf_dir_ok()?;

    backup_if_exists(&license_key_path())?;
    write_mode_600(&license_key_path(), raw)?;
    let owner_note = ensure_lsadm_owner(&license_key_path());
    let restart = restart_litespeed();
    Ok(format!(
        "license.key written to {} ({} bytes, mode 600). {owner_note} {restart}",
        license_key_path().display(),
        raw.len()
    ))
}

pub fn apply_license_key_text(text: &str) -> Result<String, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("Paste the license.key contents first.".into());
    }
    apply_license_key_bytes(trimmed.as_bytes())
}

/// Fetch trial.key from LiteSpeed (Enterprise evaluation) and restart.
pub fn apply_trial_key() -> Result<String, String> {
    conf_dir_ok()?;
    if matches!(detect_kind(), Some(LiteSpeedKind::OpenLiteSpeed)) {
        return Err(
            "OpenLiteSpeed does not use trial.key. Install LiteSpeed Enterprise first, then request a trial."
                .into(),
        );
    }
    let body = http_get_bytes(TRIAL_URL)?;
    if body.len() < 16 || body.len() > MAX_LICENSE_BYTES {
        return Err("Downloaded trial.key looks invalid (size).".into());
    }
    let as_text = String::from_utf8_lossy(&body);
    if as_text.to_ascii_lowercase().contains("<html") {
        return Err("Trial download returned HTML instead of a key file.".into());
    }
    backup_if_exists(&trial_key_path())?;
    // Trial installs typically drop serial.no so the trial file is authoritative.
    if serial_path().is_file() {
        backup_if_exists(&serial_path())?;
        let _ = fs::remove_file(serial_path());
    }
    write_mode_600(&trial_key_path(), &body)?;
    let owner_note = ensure_lsadm_owner(&trial_key_path());
    let (ok, out) = run_lshttpd(&["-V"]);
    let probe = summarize_probe(&out);
    let restart = restart_litespeed();
    Ok(format!(
        "trial.key installed ({} bytes). {owner_note} Probe {}: {}. {restart}",
        body.len(),
        if ok { "ok" } else { "warned" },
        probe
    ))
}

/// Re-run `lshttpd -r` against the current serial.no and restart LSWS safely.
pub fn refresh_license() -> Result<String, String> {
    conf_dir_ok()?;
    if !serial_path().is_file() && !license_key_path().is_file() && !trial_key_path().is_file() {
        return Err(
            "No serial.no, license.key, or trial.key found under /usr/local/lsws/conf.".into(),
        );
    }
    let (ok, out) = run_lshttpd(&["-r"]);
    let summary = summarize_probe(&out);
    let restart = restart_litespeed();
    if ok {
        Ok(format!(
            "License refresh (lshttpd -r) completed. {summary} {restart}"
        ))
    } else {
        Err(format!(
            "License refresh failed: {summary}. {restart}"
        ))
    }
}

fn http_get_bytes(url: &str) -> Result<Vec<u8>, String> {
    // Prefer curl (present on AlmaLinux CyberPanel hosts); fall back to wget.
    for (bin, args) in [
        (
            "curl",
            vec![
                "-fsSL",
                "--connect-timeout",
                "15",
                "--max-time",
                "60",
                "-o",
                "-",
                url,
            ],
        ),
        (
            "wget",
            vec![
                "-q",
                "--timeout=60",
                "-O",
                "-",
                url,
            ],
        ),
    ] {
        if let Ok(out) = Command::new(bin).args(&args).output() {
            if out.status.success() && !out.stdout.is_empty() {
                return Ok(out.stdout);
            }
        }
    }
    // Last resort: ureq-less blocking via `std::net` is not wired; fail clearly.
    let _ = Duration::from_secs(1);
    Err(format!(
        "Could not download {url} (curl/wget missing or failed)."
    ))
}

/// High-signal migration checklist lines for CyberPanel → CPN → AlmaLinux 10.
pub fn nt_migration_checklist() -> Vec<&'static str> {
    vec![
        "Backup /home/backup (sites, DBs, mail, SSL) before any panel swap.",
        "Install CPN beside CyberPanel (different listen port if 8090/lscpd stays up); confirm OLS/LSWS license path under /usr/local/lsws/conf.",
        "Run: cpn panel-migrate detect  (CyberPanel maps as id=classic).",
        "Run: cpn panel-migrate plan --id classic && cpn panel-migrate apply --id classic --yes",
        "Verify sites, PHP per-site, SSL, mail, DNS, databases in CPN; re-apply LSWS serial/license.key if Enterprise.",
        "Wipe CyberPanel only after verified apply: cpn panel-migrate wipe --id classic --yes",
        "On EL9 with CPN owning the box: Remi php:remi-8.6, matching el9 RPM, SOGo EL9 channel.",
        "Upgrade host EL9 → AlmaLinux 10 only after CPN owns services; install matching el10 RPM; SOGo uses EL9-compat libsodium path (docs/SOGO-HOST-PACKAGE.md).",
        "Retest NewsTargeted sites, ACME renewals, mail, and LiteSpeed WebAdmin after the OS upgrade.",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_keeps_license_lines() {
        let raw = "LiteSpeed Web Server\nLicense: valid\nExpires: 2027-01-01\nnoise\n";
        let s = summarize_probe(raw);
        assert!(s.to_ascii_lowercase().contains("license"));
        assert!(s.to_ascii_lowercase().contains("expir"));
    }

    #[test]
    fn reject_html_license() {
        let err = apply_license_key_text("<html>nope</html>").unwrap_err();
        assert!(err.to_ascii_lowercase().contains("html"));
    }

    #[test]
    fn reject_empty_license() {
        assert!(apply_license_key_text("   ").is_err());
    }

    #[test]
    fn checklist_mentions_classic_and_el10() {
        let lines = nt_migration_checklist();
        assert!(lines.iter().any(|l| l.contains("classic")));
        assert!(lines.iter().any(|l| l.contains("AlmaLinux 10")));
        assert!(lines.iter().any(|l| l.contains("license")));
        assert!(lines.len() >= 7 && lines.len() <= 12);
    }

    #[test]
    fn mask_serial_still_works_via_stack() {
        assert_eq!(mask_serial("AAAA-BBBB-CCCC-DDDD"), "****DDDD");
    }
}
