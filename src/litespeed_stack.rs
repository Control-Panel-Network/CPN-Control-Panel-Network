//! OpenLiteSpeed / LiteSpeed Enterprise detection, WebAdmin URL, license, and package ops.

use crate::paths::join_data;
use crate::service_detect::{port_open, systemd_unit_active};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

pub const STORE_OWNED_LSWS: &str =
    "https://store.litespeedtech.com/store/index.php?rp=/store/owned-litespeed-web-server";
pub const STORE_SUPPORT: &str =
    "https://store.litespeedtech.com/store/index.php?rp=/store/supportservice";
pub const DEFAULT_WEBADMIN_URL: &str = "https://127.0.0.1:7080";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiteSpeedKind {
    OpenLiteSpeed,
    Enterprise,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiteSpeedPlan {
    pub id: &'static str,
    pub label: &'static str,
    pub owned: bool,
}

/// Owned LSWS tiers from the LiteSpeed store catalog (purchase happens on the store).
pub const OWNED_PLANS: &[LiteSpeedPlan] = &[
    LiteSpeedPlan {
        id: "web_host_lite",
        label: "Web Host Lite",
        owned: true,
    },
    LiteSpeedPlan {
        id: "web_host_essential",
        label: "Web Host Essential",
        owned: true,
    },
    LiteSpeedPlan {
        id: "web_host_professional",
        label: "Web Host Professional",
        owned: true,
    },
    LiteSpeedPlan {
        id: "web_host_enterprise",
        label: "Web Host Enterprise",
        owned: true,
    },
    LiteSpeedPlan {
        id: "web_host_elite",
        label: "Web Host Elite",
        owned: true,
    },
];

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LiteSpeedPanelConfig {
    #[serde(default)]
    pub webadmin_url: Option<String>,
    #[serde(default)]
    pub selected_tier: Option<String>,
    /// License serial (never log or echo in full).
    #[serde(default)]
    pub serial: Option<String>,
}

fn config_path() -> PathBuf {
    join_data("litespeed.json")
}

fn serial_path() -> &'static Path {
    Path::new("/usr/local/lsws/conf/serial.no")
}

fn admin_config_path() -> &'static Path {
    Path::new("/usr/local/lsws/admin/conf/admin_config.conf")
}

fn version_file_candidates() -> [&'static Path; 2] {
    [
        Path::new("/usr/local/lsws/VERSION"),
        Path::new("/usr/local/lsws/lshttpd/VERSION"),
    ]
}

fn write_mode_600(path: &Path, contents: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create dir: {e}"))?;
    }
    #[cfg(unix)]
    {
        let mut options = fs::OpenOptions::new();
        options.create(true).write(true).truncate(true).mode(0o600);
        let mut file = options
            .open(path)
            .map_err(|e| format!("open {}: {e}", path.display()))?;
        file.write_all(contents)
            .map_err(|e| format!("write {}: {e}", path.display()))?;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        fs::write(path, contents).map_err(|e| format!("write {}: {e}", path.display()))?;
    }
    Ok(())
}

pub fn load_config() -> LiteSpeedPanelConfig {
    let path = config_path();
    let Ok(raw) = fs::read_to_string(&path) else {
        return LiteSpeedPanelConfig::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save_config(cfg: &LiteSpeedPanelConfig) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(cfg).map_err(|e| format!("serialize: {e}"))?;
    write_mode_600(&config_path(), &json)
}

pub fn openlitespeed_binary_present() -> bool {
    [
        "/usr/local/lsws/bin/openlitespeed",
        "/usr/bin/openlitespeed",
        "/usr/sbin/openlitespeed",
    ]
    .iter()
    .any(|p| Path::new(p).is_file())
}

pub fn lshttpd_binary_present() -> bool {
    Path::new("/usr/local/lsws/bin/lshttpd").is_file()
}

fn package_installed(names: &[&str]) -> bool {
    for name in names {
        if Command::new("rpm")
            .args(["-q", name])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return true;
        }
        if Command::new("dpkg-query")
            .args(["-W", "-f=${Status}", name])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).contains("install ok installed"))
                } else {
                    None
                }
            })
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

/// OpenLiteSpeed is present when the OLS binary or `openlitespeed` package exists.
pub fn openlitespeed_installed() -> bool {
    openlitespeed_binary_present() || package_installed(&["openlitespeed"])
}

/// LiteSpeed Enterprise: lshttpd without the OpenLiteSpeed binary (owned/leased LSWS).
pub fn litespeed_enterprise_installed() -> bool {
    if openlitespeed_binary_present() {
        return false;
    }
    lshttpd_binary_present() || package_installed(&["lsws", "litespeed"])
}

pub fn any_litespeed_installed() -> bool {
    openlitespeed_installed() || litespeed_enterprise_installed()
}

pub fn detect_kind() -> Option<LiteSpeedKind> {
    if openlitespeed_installed() {
        Some(LiteSpeedKind::OpenLiteSpeed)
    } else if litespeed_enterprise_installed() {
        Some(LiteSpeedKind::Enterprise)
    } else {
        None
    }
}

pub fn detect_version_label() -> String {
    for path in version_file_candidates() {
        if let Ok(raw) = fs::read_to_string(path) {
            let line = raw.lines().next().unwrap_or("").trim();
            if !line.is_empty() {
                return line.to_string();
            }
        }
    }
    if let Ok(out) = Command::new("/usr/local/lsws/bin/openlitespeed")
        .arg("-v")
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        if let Some(first) = text.lines().next() {
            let t = first.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
    }
    "Unknown".into()
}

fn parse_admin_address(contents: &str) -> Option<(String, u16)> {
    for line in contents.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("address") {
            continue;
        }
        let value = trimmed
            .split_whitespace()
            .nth(1)
            .unwrap_or("")
            .trim()
            .trim_matches('"');
        if value.is_empty() {
            continue;
        }
        if let Some((host, port_s)) = value.rsplit_once(':') {
            let port: u16 = port_s.parse().ok()?;
            let host = if host == "*" || host.is_empty() {
                "127.0.0.1".into()
            } else {
                host.trim_matches(['[', ']']).to_string()
            };
            return Some((host, port));
        }
    }
    None
}

/// Prefer panel override, then admin_config.conf, then lab default https://127.0.0.1:7080.
pub fn webadmin_url() -> String {
    let cfg = load_config();
    if let Some(url) = cfg.webadmin_url.as_ref() {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    if let Ok(raw) = fs::read_to_string(admin_config_path())
        && let Some((host, port)) = parse_admin_address(&raw)
    {
        return format!("https://{host}:{port}");
    }
    DEFAULT_WEBADMIN_URL.to_string()
}

pub fn webadmin_reachable() -> bool {
    let url = webadmin_url();
    if let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    {
        let hostport = rest.split('/').next().unwrap_or(rest);
        let addr = if hostport.contains(':') {
            hostport.to_string()
        } else {
            format!("{hostport}:7080")
        };
        return port_open(&addr, 400);
    }
    false
}

pub fn service_unit_hint() -> String {
    for unit in ["openlitespeed", "lsws", "lshttpd"] {
        if systemd_unit_active(unit) {
            return format!("{unit} (active)");
        }
    }
    for unit in ["openlitespeed", "lsws", "lshttpd"] {
        if Path::new(&format!("/usr/lib/systemd/system/{unit}.service")).exists()
            || Path::new(&format!("/etc/systemd/system/{unit}.service")).exists()
        {
            return format!("{unit} (present)");
        }
    }
    "Not detected".into()
}

pub fn mask_serial(serial: &str) -> String {
    let s = serial.trim();
    if s.len() <= 4 {
        return "****".into();
    }
    format!("****{}", &s[s.len().saturating_sub(4)..])
}

pub fn apply_serial(serial: &str) -> Result<String, String> {
    let serial = serial.trim();
    if serial.is_empty() {
        return Err("Serial is empty.".into());
    }
    if serial.len() > 256 || serial.chars().any(|c| c.is_control()) {
        return Err("Serial looks invalid.".into());
    }
    if !Path::new("/usr/local/lsws/conf").is_dir() {
        return Err("LiteSpeed conf directory is missing (/usr/local/lsws/conf).".into());
    }
    let mut body = serial.to_string();
    body.push('\n');
    write_mode_600(serial_path(), body.as_bytes())?;
    let mut cfg = load_config();
    cfg.serial = Some(serial.to_string());
    save_config(&cfg)?;
    let restart = restart_litespeed();
    Ok(format!(
        "License serial applied to serial.no ({masked}). {restart}",
        masked = mask_serial(serial)
    ))
}

pub fn set_selected_tier(tier_id: &str) -> Result<String, String> {
    let tier_id = tier_id.trim();
    if tier_id.is_empty() {
        return Err("Select a plan tier.".into());
    }
    if !OWNED_PLANS.iter().any(|p| p.id == tier_id) && tier_id != "support_service" {
        return Err("Unknown plan tier.".into());
    }
    let mut cfg = load_config();
    cfg.selected_tier = Some(tier_id.to_string());
    save_config(&cfg)?;
    let label = OWNED_PLANS
        .iter()
        .find(|p| p.id == tier_id)
        .map(|p| p.label)
        .unwrap_or(tier_id);
    Ok(format!(
        "Selected plan preference saved: {label}. Purchase or switch the license on the LiteSpeed store, then apply the serial here."
    ))
}

pub fn set_webadmin_url(url: &str) -> Result<String, String> {
    let url = url.trim();
    if url.is_empty() {
        let mut cfg = load_config();
        cfg.webadmin_url = None;
        save_config(&cfg)?;
        return Ok("WebAdmin URL cleared; using admin_config / default.".into());
    }
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("WebAdmin URL must start with https:// or http://.".into());
    }
    if url.len() > 512 {
        return Err("WebAdmin URL is too long.".into());
    }
    let mut cfg = load_config();
    cfg.webadmin_url = Some(url.to_string());
    save_config(&cfg)?;
    Ok("WebAdmin URL saved.".into())
}

fn restart_litespeed() -> String {
    for unit in ["openlitespeed", "lsws", "lshttpd"] {
        if systemd_unit_active(unit)
            || Path::new(&format!("/usr/lib/systemd/system/{unit}.service")).exists()
        {
            let status = Command::new("systemctl")
                .args(["restart", unit])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if status {
                return format!("Restarted {unit}.");
            }
        }
    }
    if Path::new("/usr/local/lsws/bin/lswsctrl").is_file() {
        let status = Command::new("/usr/local/lsws/bin/lswsctrl")
            .arg("restart")
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if status {
            return "Restarted via lswsctrl.".into();
        }
    }
    "Could not restart LiteSpeed automatically; restart the service from Server > Services.".into()
}

/// Soft package refresh for OpenLiteSpeed (best-effort).
pub fn upgrade_openlitespeed_packages() -> Result<String, String> {
    if !openlitespeed_installed() {
        return Err("OpenLiteSpeed is not installed on this host.".into());
    }
    if Command::new("dnf").arg("--version").status().is_ok() {
        let status = Command::new("dnf")
            .args(["upgrade", "-y", "openlitespeed"])
            .status()
            .map_err(|e| format!("dnf: {e}"))?;
        if !status.success() {
            return Err("dnf upgrade openlitespeed failed.".into());
        }
        let _ = restart_litespeed();
        return Ok("OpenLiteSpeed packages upgraded via dnf.".into());
    }
    if Command::new("apt-get").arg("--version").status().is_ok() {
        let _ = Command::new("apt-get").args(["update", "-y"]).status();
        let status = Command::new("apt-get")
            .args(["install", "--only-upgrade", "-y", "openlitespeed"])
            .status()
            .map_err(|e| format!("apt-get: {e}"))?;
        if !status.success() {
            return Err("apt-get upgrade openlitespeed failed.".into());
        }
        let _ = restart_litespeed();
        return Ok("OpenLiteSpeed packages upgraded via apt.".into());
    }
    Err("No supported package manager (dnf/apt-get).".into())
}

/// Best-effort downgrade of `openlitespeed` to an explicit version string.
pub fn downgrade_openlitespeed_to(version: &str) -> Result<String, String> {
    let version = version.trim();
    if version.is_empty()
        || version.contains(' ')
        || version.contains(';')
        || version.contains('&')
        || version.contains('|')
        || version.contains('`')
    {
        return Err("Provide a clean package version (example: 1.8.2).".into());
    }
    if !openlitespeed_installed() {
        return Err("OpenLiteSpeed is not installed on this host.".into());
    }
    let pkg = format!("openlitespeed-{version}");
    if Command::new("dnf").arg("--version").status().is_ok() {
        let status = Command::new("dnf")
            .args(["downgrade", "-y", &pkg])
            .status()
            .map_err(|e| format!("dnf: {e}"))?;
        if !status.success() {
            let status2 = Command::new("dnf")
                .args(["install", "-y", "--allowerasing", &pkg])
                .status()
                .map_err(|e| format!("dnf: {e}"))?;
            if !status2.success() {
                return Err(format!(
                    "dnf could not downgrade/install {pkg}. Check repo mirrors for that version."
                ));
            }
        }
        let _ = restart_litespeed();
        return Ok(format!("OpenLiteSpeed moved toward {pkg} via dnf."));
    }
    if Command::new("apt-get").arg("--version").status().is_ok() {
        let status = Command::new("apt-get")
            .args([
                "install",
                "-y",
                "--allow-downgrades",
                &format!("openlitespeed={version}"),
            ])
            .status()
            .map_err(|e| format!("apt-get: {e}"))?;
        if !status.success() {
            return Err(format!(
                "apt-get could not install openlitespeed={version}."
            ));
        }
        let _ = restart_litespeed();
        return Ok(format!(
            "OpenLiteSpeed moved toward openlitespeed={version} via apt."
        ));
    }
    Err("No supported package manager (dnf/apt-get).".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_admin_loopback() {
        let sample = "listener adminListener {\n  address                 127.0.0.1:7080\n}\n";
        assert_eq!(
            parse_admin_address(sample),
            Some(("127.0.0.1".into(), 7080))
        );
    }

    #[test]
    fn parse_admin_wildcard() {
        let sample = "address                 *:7080\n";
        assert_eq!(
            parse_admin_address(sample),
            Some(("127.0.0.1".into(), 7080))
        );
    }

    #[test]
    fn mask_serial_keeps_tail() {
        assert_eq!(mask_serial("ABCD-EFGH-1234"), "****1234");
    }

    #[test]
    fn owned_plans_cover_store_tiers() {
        assert!(OWNED_PLANS.iter().any(|p| p.id == "web_host_lite"));
        assert!(OWNED_PLANS.iter().any(|p| p.id == "web_host_elite"));
        assert_eq!(OWNED_PLANS.len(), 5);
    }
}
