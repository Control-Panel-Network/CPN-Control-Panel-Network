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
    "sogo",
    "memcached",
];

/// Units that may back the logical `docker` Services row (Docker Engine or Podman).
const CONTAINER_BACKEND_UNITS: &[&str] = &["docker", "podman.socket", "podman"];

/// Units that may back the logical `sogo` Services row (`sogod` on RPM hosts, `sogo` on deb).
const SOGO_BACKEND_UNITS: &[&str] = crate::apps_sogo::SOGO_UNITS;

#[derive(Debug, Clone)]
pub struct ServiceRow {
    pub unit: String,
    /// UI label for runtime state (Active, Inactive, Failed, Not installed, ...).
    pub active: String,
    /// UI label for boot enablement (Enabled, Deactivated, Static, Not installed, ...).
    pub enabled: String,
    pub present: bool,
    /// When set, the row is backed by a different systemd unit (for example `podman.socket`).
    pub via: Option<String>,
    /// Plugin Store (Host) deep link when the unit is missing and can be installed.
    pub install_href: Option<String>,
}

pub fn known_units() -> &'static [&'static str] {
    KNOWN_UNITS
}

/// Host Plugin Store URL for installing a missing allowlisted unit.
pub fn store_install_href(unit: &str) -> String {
    let q = match unit {
        "docker" => "docker",
        "mariadb" | "mysqld" => "mariadb",
        "postfix" | "dovecot" => "email",
        "pure-ftpd" | "vsftpd" => "ftp",
        "pdns" | "named" => "dns",
        "php-fpm" => "php",
        "nginx" | "httpd" => "nginx",
        "lsws" | "lshttpd" | "openlitespeed" => "litespeed",
        "firewalld" => "firewall",
        "sogo" | "sogod" | "memcached" => "sogo",
        other => other,
    };
    format!("/plugins?view=store&target=host&q={q}")
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

/// Prefer Docker Engine; otherwise Podman socket/service; otherwise a docker/podman CLI.
fn resolve_container_backend() -> Option<&'static str> {
    for unit in CONTAINER_BACKEND_UNITS {
        if unit_is_present(unit) {
            return Some(*unit);
        }
    }
    if crate::panel_ops_docker::docker_bin().is_some() {
        return Some("cli");
    }
    None
}

fn container_engine_status() -> ServiceRow {
    let install_href = store_install_href("docker");
    let Some(backend) = resolve_container_backend() else {
        return ServiceRow {
            unit: "docker".into(),
            active: "Not installed".into(),
            enabled: "Not installed".into(),
            present: false,
            via: None,
            install_href: Some(install_href),
        };
    };

    if backend == "cli" {
        let running = crate::panel_ops_docker::docker_bin()
            .map(crate::panel_ops_docker::docker_daemon_ok)
            .unwrap_or(false);
        return ServiceRow {
            unit: "docker".into(),
            active: if running {
                "Active".into()
            } else {
                "Inactive".into()
            },
            enabled: "Enabled".into(),
            present: true,
            via: Some("cli".into()),
            install_href: None,
        };
    }

    let active_raw = systemctl_token(&["is-active", backend]).unwrap_or_else(|| "inactive".into());
    // Socket units are often `enabled` while the oneshot `podman.service` stays inactive;
    // prefer enabled state from the resolved backend, with a fallback to sibling units.
    let enabled_raw = systemctl_token(&["is-enabled", backend])
        .or_else(|| {
            CONTAINER_BACKEND_UNITS
                .iter()
                .filter(|u| **u != backend && unit_is_present(u))
                .find_map(|u| systemctl_token(&["is-enabled", u]))
        })
        .unwrap_or_else(|| "disabled".into());

    let via = if backend == "docker" {
        None
    } else {
        Some(backend.to_string())
    };

    ServiceRow {
        unit: "docker".into(),
        active: map_active_label(&active_raw, true),
        enabled: map_enabled_label(&enabled_raw, true),
        present: true,
        via,
        install_href: None,
    }
}

/// Logical `sogo` row: resolves to whichever SOGo unit name the package shipped.
fn resolve_sogo_backend() -> Option<&'static str> {
    SOGO_BACKEND_UNITS
        .iter()
        .copied()
        .find(|unit| unit_is_present(unit))
}

fn sogo_status() -> ServiceRow {
    let Some(backend) = resolve_sogo_backend() else {
        return ServiceRow {
            unit: "sogo".into(),
            active: "Not installed".into(),
            enabled: "Not installed".into(),
            present: false,
            via: None,
            install_href: Some(store_install_href("sogo")),
        };
    };
    let active_raw = systemctl_token(&["is-active", backend]).unwrap_or_else(|| "inactive".into());
    let enabled_raw =
        systemctl_token(&["is-enabled", backend]).unwrap_or_else(|| "disabled".into());
    ServiceRow {
        unit: "sogo".into(),
        active: map_active_label(&active_raw, true),
        enabled: map_enabled_label(&enabled_raw, true),
        present: true,
        via: (backend != "sogo").then(|| backend.to_string()),
        install_href: None,
    }
}

fn unit_status(unit: &str) -> ServiceRow {
    if unit == "docker" {
        return container_engine_status();
    }
    if unit == "sogo" {
        return sogo_status();
    }

    if !systemctl_available() {
        return ServiceRow {
            unit: unit.to_string(),
            active: "Unavailable".into(),
            enabled: "Unavailable".into(),
            present: false,
            via: None,
            install_href: Some(store_install_href(unit)),
        };
    }

    let present = unit_is_present(unit);
    if !present {
        return ServiceRow {
            unit: unit.to_string(),
            active: "Not installed".into(),
            enabled: "Not installed".into(),
            present: false,
            via: None,
            install_href: Some(store_install_href(unit)),
        };
    }

    let active_raw = systemctl_token(&["is-active", unit]).unwrap_or_else(|| "inactive".into());
    let enabled_raw = systemctl_token(&["is-enabled", unit]).unwrap_or_else(|| "disabled".into());

    ServiceRow {
        unit: unit.to_string(),
        active: map_active_label(&active_raw, true),
        enabled: map_enabled_label(&enabled_raw, true),
        present: true,
        via: None,
        install_href: None,
    }
}

pub fn list_known_services() -> Vec<ServiceRow> {
    KNOWN_UNITS.iter().map(|u| unit_status(u)).collect()
}

fn resolve_control_unit(unit: &str) -> Result<String, String> {
    if unit == "docker" {
        match resolve_container_backend() {
            Some("cli") => Err(
                "Container CLI is present but no docker/podman systemd unit was found to control"
                    .into(),
            ),
            Some(backend) => Ok(backend.to_string()),
            None => Err("Unit `docker` is not installed on this host".into()),
        }
    } else if unit == "sogo" {
        resolve_sogo_backend()
            .map(str::to_string)
            .ok_or_else(|| "Unit `sogo` is not installed on this host".to_string())
    } else if unit_is_present(unit) {
        Ok(unit.to_string())
    } else {
        Err(format!("Unit `{unit}` is not installed on this host"))
    }
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
    let target = resolve_control_unit(unit)?;
    let out = Command::new("systemctl")
        .args([action, &target])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("Failed to run systemctl: {e}"))?;
    if out.status.success() {
        // Host package / sidebar gates read a short-lived docker CLI cache; clear it after
        // start/stop so Services and Docker tiles stay consistent.
        if unit == "docker" {
            crate::panel_feature_gate::invalidate_feature_cache();
        }
        if target == unit {
            Ok(format!("{action} issued for {unit}"))
        } else {
            Ok(format!("{action} issued for {unit} via {target}"))
        }
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(format!(
            "systemctl {action} {target} failed: {}",
            err.trim()
        ))
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
    fn store_install_href_targets_host_plugin_store() {
        let docker = store_install_href("docker");
        assert!(docker.contains("/plugins?view=store"));
        assert!(docker.contains("target=host"));
        assert!(docker.contains("q=docker"));
        assert!(store_install_href("pure-ftpd").contains("q=ftp"));
        assert!(store_install_href("pdns").contains("q=dns"));
        assert!(store_install_href("mariadb").contains("q=mariadb"));
        assert!(store_install_href("postfix").contains("q=email"));
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
            if !row.present {
                assert!(
                    row.install_href.is_some(),
                    "missing unit {} should link to Plugin Store",
                    row.unit
                );
            }
            eprintln!(
                "svc {} active={} enabled={} present={} via={:?}",
                row.unit, row.active, row.enabled, row.present, row.via
            );
        }
    }
}
