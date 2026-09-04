//! Honest local service detection for Panel Dashboard and Databases pages.

use std::net::TcpStream;
use std::process::Command;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct DatabaseStatus {
    /// Short label for UI (for example `MariaDB (active)` or `Not detected`).
    pub service_label: String,
    pub listening_3306: bool,
    pub detail: String,
}

/// True when `systemctl is-active --quiet <name>` succeeds.
pub fn systemd_unit_active(name: &str) -> bool {
    Command::new("systemctl")
        .args(["is-active", "--quiet", name])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// First matching active unit among `names`, with a human label.
pub fn first_active_service(names: &[(&str, &str)]) -> Option<String> {
    for (unit, label) in names {
        if systemd_unit_active(unit) {
            return Some((*label).to_string());
        }
    }
    None
}

pub fn port_open(addr: &str, timeout_ms: u64) -> bool {
    let Ok(sock) = addr.parse() else {
        return false;
    };
    TcpStream::connect_timeout(&sock, Duration::from_millis(timeout_ms)).is_ok()
}

/// Same MariaDB/MySQL detection used by `/databases` and Dashboard System Health.
pub fn detect_database() -> DatabaseStatus {
    let service = first_active_service(&[
        ("mariadb", "MariaDB (active)"),
        ("mysql", "MySQL (active)"),
        ("mysqld", "mysqld (active)"),
    ]);
    let listening = port_open("127.0.0.1:3306", 250);
    match (service, listening) {
        (Some(label), true) => DatabaseStatus {
            service_label: label,
            listening_3306: true,
            detail: "A database daemon is active and accepting connections on 127.0.0.1:3306."
                .into(),
        },
        (Some(label), false) => DatabaseStatus {
            service_label: label,
            listening_3306: false,
            detail: "The service unit is active, but TCP :3306 did not accept a quick probe."
                .into(),
        },
        (None, true) => DatabaseStatus {
            service_label: "Listener on :3306".into(),
            listening_3306: true,
            detail: "Something accepts connections on 127.0.0.1:3306, but no MariaDB/MySQL systemd unit was active."
                .into(),
        },
        (None, false) => DatabaseStatus {
            service_label: "Not detected".into(),
            listening_3306: false,
            detail: "CPN does not provision database instances yet. Install MariaDB/MySQL on the host, then re-open this page."
                .into(),
        },
    }
}

/// Short status for Dashboard System Health (no fake Running).
pub fn database_health_label(status: &DatabaseStatus) -> String {
    if status.service_label == "Not detected" && !status.listening_3306 {
        "Not detected".into()
    } else if status.listening_3306
        && (status.service_label.starts_with("MariaDB")
            || status.service_label.starts_with("MySQL")
            || status.service_label.starts_with("mysqld"))
    {
        "Running".into()
    } else if status.service_label != "Not detected" {
        status.service_label.clone()
    } else {
        "Not detected".into()
    }
}

pub fn detect_web_server_label() -> String {
    if let Some(label) = first_active_service(&[
        ("nginx", "Running"),
        ("openlitespeed", "Running"),
        ("lshttpd", "Running"),
        ("caddy", "Running"),
        ("httpd", "Running"),
    ]) {
        return label;
    }
    "Not detected".into()
}

pub fn detect_mail_service_label() -> String {
    if let Some(label) = first_active_service(&[
        ("postfix", "Running"),
        ("exim4", "Running"),
        ("exim", "Running"),
        ("dovecot", "Running"),
    ]) {
        return label;
    }
    "Not detected".into()
}

/// Install MariaDB server packages via the host package manager (operator confirmed).
pub fn install_mariadb_server() -> Result<String, String> {
    let existing = detect_database();
    if existing.service_label != "Not detected" || existing.listening_3306 {
        return Err(
            "A database service or :3306 listener is already present. No install was started."
                .into(),
        );
    }
    if Command::new("dnf").arg("--version").status().is_ok() {
        let status = Command::new("dnf")
            .args(["install", "-y", "mariadb-server"])
            .status()
            .map_err(|error| format!("Could not start dnf: {error}"))?;
        if !status.success() {
            return Err("dnf install mariadb-server failed".into());
        }
        let _ = Command::new("systemctl")
            .args(["enable", "--now", "mariadb"])
            .status();
        return Ok("Installed and started MariaDB via dnf (mariadb-server).".into());
    }
    if Command::new("apt-get").arg("--version").status().is_ok() {
        let update = Command::new("apt-get")
            .args(["update", "-y"])
            .status()
            .map_err(|error| format!("Could not start apt-get update: {error}"))?;
        if !update.success() {
            return Err("apt-get update failed".into());
        }
        let status = Command::new("apt-get")
            .args(["install", "-y", "mariadb-server"])
            .status()
            .map_err(|error| format!("Could not start apt-get: {error}"))?;
        if !status.success() {
            return Err("apt-get install mariadb-server failed".into());
        }
        let _ = Command::new("systemctl")
            .args(["enable", "--now", "mariadb"])
            .status();
        return Ok("Installed and started MariaDB via apt (mariadb-server).".into());
    }
    Err("No supported package manager found (need dnf or apt-get).".into())
}
