//! Host stack checks: MariaDB, web, disk, MFA, firewall, SSL, Docker.

use super::host_apps;
use super::{CheckStatus, HealResult, push};
use crate::litespeed_stack;
use crate::panel_feature_gate;
use crate::panel_ops_firewall_heal;
use crate::panel_ops_ssl_le::ssl_status_all_sites;
use crate::paths;
use crate::service_detect::{self, port_open, systemd_unit_active};
use std::path::Path;
use std::process::Command;

pub const HOST_HEAL_IDS: &[&str] = &["firewall", "phpmyadmin", "docker.engine"];

fn free_disk_mb(path: &str) -> Option<(u64, u64)> {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        let c_path = CString::new(path).ok()?;
        let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
        let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
        if rc != 0 {
            return None;
        }
        let total = (stat.f_blocks as u64).saturating_mul(stat.f_frsize as u64) / (1024 * 1024);
        let free = (stat.f_bavail as u64).saturating_mul(stat.f_frsize as u64) / (1024 * 1024);
        Some((free, total))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Some((64 * 1024, 128 * 1024))
    }
}

fn mfa_storage_present() -> (bool, String) {
    let dir = paths::default_data_dir().join("mfa");
    let key = dir.join("mfa-encryption.key");
    if !dir.is_dir() {
        return (
            true,
            "MFA directory not created yet (created on first enroll); storage not wiped by heal"
                .into(),
        );
    }
    if key.is_file() {
        (
            true,
            format!(
                "MFA encryption key present at {} (heal never removes MFA)",
                key.display()
            ),
        )
    } else {
        (
            true,
            format!(
                "MFA dir present at {}; key file appears after first enroll (heal never removes MFA)",
                dir.display()
            ),
        )
    }
}

pub fn collect(checks: &mut Vec<super::RepairCheck>) {
    let db = service_detect::detect_database();
    let db_ok = db.listening_3306
        || systemd_unit_active("mariadb")
        || systemd_unit_active("mysql")
        || systemd_unit_active("mysqld");
    let db_present = db.listening_3306
        || db.service_label.to_lowercase().contains("mariadb")
        || db.service_label.to_lowercase().contains("mysql");
    if db_present {
        push(
            checks,
            "host.database",
            "database",
            "MariaDB",
            if db_ok {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            db.detail.clone(),
            true,
            None,
        );
    } else {
        push(
            checks,
            "host.database",
            "database",
            "MariaDB",
            CheckStatus::Pass,
            "MariaDB not detected (optional until Host package install)",
            false,
            None,
        );
    }

    let ols = Path::new("/usr/local/lsws/bin/openlitespeed").is_file()
        || Path::new("/usr/local/lsws/bin/lshttpd").is_file();
    let ols_active = systemd_unit_active("lshttpd")
        || systemd_unit_active("lsws")
        || (litespeed_stack::openlitespeed_installed() && port_open("127.0.0.1:80", 250));
    push(
        checks,
        "host.web_server",
        "web",
        "Web server",
        if ols {
            if ols_active || port_open("127.0.0.1:80", 250) || port_open("127.0.0.1:443", 250) {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            }
        } else {
            CheckStatus::Pass
        },
        if ols {
            if ols_active || port_open("127.0.0.1:80", 250) {
                "OpenLiteSpeed / lshttpd present and responding"
            } else {
                "OLS/lshttpd binary present but HTTP listeners quiet"
            }
        } else {
            "no OLS/lshttpd binary detected (optional)"
        },
        false,
        None,
    );

    if let Some((free_mb, total_mb)) = free_disk_mb("/") {
        let pct_free = (free_mb.saturating_mul(100))
            .checked_div(total_mb)
            .unwrap_or(100);
        let status = if pct_free < 5 {
            CheckStatus::Fail
        } else if pct_free < 15 {
            CheckStatus::Warn
        } else {
            CheckStatus::Pass
        };
        push(
            checks,
            "host.disk",
            "host",
            "Disk free (/)",
            status,
            format!("{free_mb} MiB free of {total_mb} MiB ({pct_free}% free)"),
            pct_free < 5,
            None,
        );
    } else {
        push(
            checks,
            "host.disk",
            "host",
            "Disk free (/)",
            CheckStatus::Warn,
            "could not read filesystem stats for /",
            false,
            None,
        );
    }

    let (mfa_ok, mfa_detail) = mfa_storage_present();
    push(
        checks,
        "host.mfa",
        "security",
        "MFA storage",
        if mfa_ok {
            CheckStatus::Pass
        } else {
            CheckStatus::Warn
        },
        mfa_detail,
        false,
        None,
    );

    let fw_cmd = crate::panel_ops_security::which_exists("firewall-cmd");
    let fw_active = systemd_unit_active("firewalld");
    let fw_stopped = panel_ops_firewall_heal::operator_stopped();
    push(
        checks,
        "host.firewall",
        "security",
        "Firewalld",
        if !fw_cmd || fw_active {
            CheckStatus::Pass
        } else {
            CheckStatus::Warn
        },
        if !fw_cmd {
            "firewalld not installed"
        } else if fw_active {
            "firewalld is active"
        } else if fw_stopped {
            "firewalld stopped by operator (heal will not restart it)"
        } else {
            "firewalld inactive; heal can start a safe baseline"
        },
        false,
        if fw_cmd && !fw_active && !fw_stopped {
            Some("firewall")
        } else {
            None
        },
    );

    let ssl_rows = ssl_status_all_sites();
    if ssl_rows.is_empty() {
        push(
            checks,
            "host.ssl",
            "ssl",
            "Site SSL",
            CheckStatus::Pass,
            "no websites registered yet",
            false,
            None,
        );
    } else {
        let valid = ssl_rows.iter().filter(|r| r.has_cert).count();
        let total = ssl_rows.len();
        push(
            checks,
            "host.ssl",
            "ssl",
            "Site SSL",
            if valid == total {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            format!("{valid}/{total} sites have a certificate on disk"),
            false,
            None,
        );
    }

    let docker = panel_feature_gate::docker_installed();
    if docker {
        let running = systemd_unit_active("docker")
            || systemd_unit_active("podman")
            || Path::new("/run/docker.sock").exists()
            || Path::new("/run/podman/podman.sock").exists();
        push(
            checks,
            "host.docker",
            "docker",
            "Container engine",
            if running {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            if running {
                "Docker or Podman socket/unit available"
            } else {
                "Docker/Podman installed but engine does not look running"
            },
            false,
            if running { None } else { Some("docker.engine") },
        );
    } else {
        push(
            checks,
            "host.docker",
            "docker",
            "Container engine",
            CheckStatus::Pass,
            "Docker/Podman Host package not installed (optional)",
            false,
            None,
        );
    }

    host_apps::collect(checks);
}

pub fn heal_firewall() -> HealResult {
    #[cfg(unix)]
    {
        if unsafe { libc::geteuid() } != 0 {
            return HealResult {
                heal_id: "firewall".into(),
                ok: false,
                message: "requires root".into(),
            };
        }
    }
    if panel_ops_firewall_heal::operator_stopped() {
        return HealResult {
            heal_id: "firewall".into(),
            ok: true,
            message: "skipped: operator stopped firewalld from the panel".into(),
        };
    }
    match panel_ops_firewall_heal::heal_firewalld("system repair") {
        Some(msg) => HealResult {
            heal_id: "firewall".into(),
            ok: !msg.contains("failed"),
            message: msg,
        },
        None => HealResult {
            heal_id: "firewall".into(),
            ok: true,
            message: "firewalld heal not needed or not installed".into(),
        },
    }
}

pub fn heal_phpmyadmin() -> HealResult {
    host_apps::heal_phpmyadmin()
}

pub fn heal_docker_engine() -> HealResult {
    #[cfg(unix)]
    {
        if unsafe { libc::geteuid() } != 0 {
            return HealResult {
                heal_id: "docker.engine".into(),
                ok: false,
                message: "requires root".into(),
            };
        }
    }
    if !panel_feature_gate::docker_installed() {
        return HealResult {
            heal_id: "docker.engine".into(),
            ok: true,
            message: "Docker/Podman not installed; skipped".into(),
        };
    }
    let mut started = Vec::new();
    for unit in ["docker", "podman"] {
        if systemd_unit_active(unit) {
            started.push(format!("{unit} already active"));
            continue;
        }
        let ok = Command::new("systemctl")
            .args(["enable", "--now", unit])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            started.push(format!("started {unit}"));
        }
    }
    let _ = crate::panel_ops_docker_image_ref::heal_container_engine_podman_pull();
    let running = systemd_unit_active("docker")
        || systemd_unit_active("podman")
        || Path::new("/run/docker.sock").exists();
    HealResult {
        heal_id: "docker.engine".into(),
        ok: running || !started.is_empty(),
        message: if started.is_empty() {
            "could not start docker or podman".into()
        } else {
            started.join("; ")
        },
    }
}
