//! Service status helpers (systemctl) for the Server hub.

use std::process::Command;

const KNOWN_UNITS: &[&str] = &[
    "nginx",
    "httpd",
    "lsws",
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
];

#[derive(Debug, Clone)]
pub struct ServiceRow {
    pub unit: String,
    pub active: String,
    pub enabled: String,
    pub present: bool,
}

pub fn known_units() -> &'static [&'static str] {
    KNOWN_UNITS
}

fn systemctl_available() -> bool {
    Command::new("systemctl")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn unit_status(unit: &str) -> ServiceRow {
    if !systemctl_available() {
        return ServiceRow {
            unit: unit.to_string(),
            active: "unknown".into(),
            enabled: "unknown".into(),
            present: false,
        };
    }
    let show = Command::new("systemctl")
        .args([
            "show",
            unit,
            "-p",
            "ActiveState",
            "-p",
            "UnitFileState",
            "--value",
        ])
        .output();
    let Ok(out) = show else {
        return ServiceRow {
            unit: unit.to_string(),
            active: "error".into(),
            enabled: "error".into(),
            present: false,
        };
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut lines = text.lines();
    let active = lines.next().unwrap_or("unknown").trim().to_string();
    let enabled = lines.next().unwrap_or("unknown").trim().to_string();
    let present = active != "unknown" && !active.is_empty() && active != "not-found";
    ServiceRow {
        unit: unit.to_string(),
        active: if active.is_empty() {
            "unknown".into()
        } else {
            active
        },
        enabled: if enabled.is_empty() {
            "unknown".into()
        } else {
            enabled
        },
        present,
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
    let out = Command::new("systemctl")
        .args([action, unit])
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
}
