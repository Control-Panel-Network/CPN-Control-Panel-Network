//! Service status helpers (systemctl) for the Server hub.

use crate::service_detect::systemd_unit_file_exists;
use std::process::{Command, Stdio};

const KNOWN_UNITS: &[&str] = &[
    "nginx",
    "httpd",
    "lsws",
    "lshttpd",
    "openlitespeed",
    "mariadb",
    "mysqld",
    "postfix",
    "dovecot",
    "pure-ftpd",
    "vsftpd",
    "docker",
    "pdns",
    "named",
    "php-fpm",
    "firewalld",
];

#[derive(Debug, Clone)]
pub struct ServiceRow {
    pub unit: String,
    /// UI label for runtime state (Active, Inactive, Failed, Not installed, ...).
    pub active: String,
    /// UI label for boot enablement (Enabled, Deactivated, Static, Not installed, ...).
    pub enabled: String,
    pub present: bool,
}

pub fn known_units() -> &'static [&'static str] {
    KNOWN_UNITS
}

fn systemctl_available() -> bool {
    Command::new("systemctl")
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Capture `systemctl` stdout even when the exit status is non-zero.
///
/// `is-active` / `is-enabled` return non-zero for inactive/disabled units while still
/// printing a usable state token on stdout.
fn systemctl_token(args: &[&str]) -> Option<String> {
    let out = Command::new("systemctl")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let token = String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if token.is_empty() { None } else { Some(token) }
}

/// Map systemd `ActiveState` / `is-active` tokens to panel UI copy.
pub fn map_active_label(raw: &str, present: bool) -> String {
    if !present {
        return "Not installed".into();
    }
    match raw.trim().to_ascii_lowercase().as_str() {
        "active" => "Active".into(),
        "inactive" => "Inactive".into(),
        "failed" => "Failed".into(),
        "activating" => "Starting".into(),
        "deactivating" => "Stopping".into(),
        "reloading" => "Reloading".into(),
        "maintenance" => "Maintenance".into(),
        "" => "Inactive".into(),
        other => {
            // Prefer the raw systemd token over inventing "unknown".
            let mut label = other.to_string();
            if let Some(first) = label.get_mut(0..1) {
                first.make_ascii_uppercase();
            }
            label
        }
    }
}

/// Map systemd `UnitFileState` / `is-enabled` tokens to panel UI copy.
///
/// Aligns with Plugins language: Enabled vs Deactivated (systemd `disabled`).
pub fn map_enabled_label(raw: &str, present: bool) -> String {
    if !present {
        return "Not installed".into();
    }
    match raw.trim().to_ascii_lowercase().as_str() {
        "enabled" | "enabled-runtime" | "alias" | "indirect" | "generated" => "Enabled".into(),
        "disabled" => "Deactivated".into(),
        "static" => "Static".into(),
        "masked" | "masked-runtime" => "Masked".into(),
        "linked" | "linked-runtime" => "Linked".into(),
        "bad" | "invalid" => "Invalid".into(),
        "not-found" => "Not installed".into(),
        "" => "Deactivated".into(),
        other => {
            let mut label = other.to_string();
            if let Some(first) = label.get_mut(0..1) {
                first.make_ascii_uppercase();
            }
            label
        }
    }
}

fn unit_is_present(unit: &str) -> bool {
    if systemd_unit_file_exists(unit) {
        return true;
    }
    // Fallback for unusual layouts: ask systemd LoadState without treating missing as present.
    match systemctl_token(&["show", unit, "-p", "LoadState", "--value"]) {
        Some(state) => state.eq_ignore_ascii_case("loaded"),
        None => false,
    }
}

fn unit_status(unit: &str) -> ServiceRow {
    if !systemctl_available() {
        return ServiceRow {
            unit: unit.to_string(),
            active: "Unavailable".into(),
            enabled: "Unavailable".into(),
            present: false,
        };
    }

    let present = unit_is_present(unit);
    if !present {
        return ServiceRow {
            unit: unit.to_string(),
            active: "Not installed".into(),
            enabled: "Not installed".into(),
            present: false,
        };
    }

    let active_raw = systemctl_token(&["is-active", unit]).unwrap_or_else(|| "inactive".into());
    let enabled_raw = systemctl_token(&["is-enabled", unit]).unwrap_or_else(|| "disabled".into());

    ServiceRow {
        unit: unit.to_string(),
        active: map_active_label(&active_raw, true),
        enabled: map_enabled_label(&enabled_raw, true),
        present: true,
    }
}

pub fn list_known_services() -> Vec<ServiceRow> {
    KNOWN_UNITS.iter().map(|u| unit_status(u)).collect()
}

pub fn control_service(unit: &str, action: &str) -> Result<String, String> {
    if !KNOWN_UNITS.contains(&unit) {
        return Err(format!("Unit `{unit}` is not in the CPN allowlist"));
    }
    let action = match action {
        "start" | "stop" | "restart" | "reload" => action,
        _ => return Err("Action must be start, stop, restart, or reload".into()),
    };
    if !systemctl_available() {
        return Err("systemctl is not available on this host".into());
    }
    if !unit_is_present(unit) {
        return Err(format!("Unit `{unit}` is not installed on this host"));
    }
    let out = Command::new("systemctl")
        .args([action, unit])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("Failed to run systemctl: {e}"))?;
    if out.status.success() {
        Ok(format!("{action} issued for {unit}"))
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(format!("systemctl {action} {unit} failed: {}", err.trim()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_unit() {
        assert!(control_service("totally-fake-unit", "start").is_err());
    }

    #[test]
    fn rejects_bad_action() {
        assert!(control_service("nginx", "explode").is_err());
    }

    #[test]
    fn maps_active_labels_without_unknown() {
        assert_eq!(map_active_label("active", true), "Active");
        assert_eq!(map_active_label("inactive", true), "Inactive");
        assert_eq!(map_active_label("failed", true), "Failed");
        assert_eq!(map_active_label("", true), "Inactive");
        assert_eq!(map_active_label("active", false), "Not installed");
        assert!(!map_active_label("inactive", true).eq_ignore_ascii_case("unknown"));
    }

    #[test]
    fn maps_enabled_labels_to_plugin_language() {
        assert_eq!(map_enabled_label("enabled", true), "Enabled");
        assert_eq!(map_enabled_label("alias", true), "Enabled");
        assert_eq!(map_enabled_label("disabled", true), "Deactivated");
        assert_eq!(map_enabled_label("", true), "Deactivated");
        assert_eq!(map_enabled_label("not-found", true), "Not installed");
        assert_eq!(map_enabled_label("enabled", false), "Not installed");
        assert!(!map_enabled_label("disabled", true).eq_ignore_ascii_case("unknown"));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn live_known_units_never_report_unknown() {
        for row in list_known_services() {
            assert!(
                !row.active.eq_ignore_ascii_case("unknown"),
                "unit {} active={}",
                row.unit,
                row.active
            );
            assert!(
                !row.enabled.eq_ignore_ascii_case("unknown"),
                "unit {} enabled={}",
                row.unit,
                row.enabled
            );
            eprintln!(
                "svc {} active={} enabled={} present={}",
                row.unit, row.active, row.enabled, row.present
            );
        }
    }
}
