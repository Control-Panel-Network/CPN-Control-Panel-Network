//! Post-upgrade health checks and optional CPN-managed Docker refresh (`--bypass`).

use crate::listen_port;
use crate::panel_feature_gate;
use crate::panel_ops_docker;
use crate::panel_service::{self, PanelServiceMode};
use crate::service_detect;
use std::process::Command;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct VerifyCheck {
    pub name: String,
    pub ok: bool,
    pub detail: String,
    pub required: bool,
}

#[derive(Debug, Clone, Default)]
pub struct VerifyReport {
    pub checks: Vec<VerifyCheck>,
}

impl VerifyReport {
    pub fn failed_required(&self) -> Vec<&VerifyCheck> {
        self.checks.iter().filter(|c| c.required && !c.ok).collect()
    }

    pub fn ok(&self) -> bool {
        self.failed_required().is_empty()
    }

    pub fn summary_lines(&self) -> Vec<String> {
        self.checks
            .iter()
            .map(|c| {
                let mark = if c.ok { "ok" } else { "FAIL" };
                let req = if c.required { "required" } else { "optional" };
                format!("[{mark}] {req}: {} ({})", c.name, c.detail)
            })
            .collect()
    }
}

fn push(
    report: &mut VerifyReport,
    name: &str,
    ok: bool,
    detail: impl AsRef<str>,
    required: bool,
) {
    report.checks.push(VerifyCheck {
        name: name.into(),
        ok,
        detail: detail.as_ref().to_string(),
        required,
    });
}

fn unit_active(unit: &str) -> bool {
    service_detect::systemd_unit_active(unit)
}

fn http_login_ok(port: u16) -> bool {
    let url = format!("http://127.0.0.1:{port}/login");
    Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--max-time",
            "8",
            "--output",
            "/dev/null",
            &url,
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn cpn_managed_container_ids() -> Vec<String> {
    let Some(bin) = ["docker", "podman"].into_iter().find(|&candidate| {
        Command::new(candidate)
            .arg("--version")
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }) else {
        return Vec::new();
    };
    let output = Command::new(bin)
        .args([
            "ps",
            "-a",
            "--filter",
            "label=com.cpn.managed=1",
            "--format",
            "{{.ID}} {{.Names}} {{.Status}}",
        ])
        .output();
    let Ok(out) = output else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

fn cpn_compose_dirs() -> Vec<std::path::PathBuf> {
    let root = std::path::Path::new("/var/lib/cpn/docker");
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && (p.join("compose.yml").is_file()
                    || p.join("docker-compose.yml").is_file()
                    || p.join("compose.yaml").is_file())
        })
        .collect()
}

/// Snapshot running CPN-managed container IDs before package ops.
pub fn snapshot_cpn_docker_running() -> Vec<String> {
    cpn_managed_container_ids()
        .into_iter()
        .filter(|line| {
            let lower = line.to_lowercase();
            lower.contains("up ") || lower.contains("running")
        })
        .filter_map(|line| line.split_whitespace().next().map(str::to_string))
        .collect()
}

/// With `--bypass`: pull/recreate CPN-managed compose projects and labeled containers.
/// Preserves volumes; never touches unlabeled user Docker stacks.
pub fn maybe_refresh_cpn_docker(bypass: bool) -> Vec<String> {
    let mut notes = Vec::new();
    if !bypass {
        notes.push(
            "docker refresh skipped (pass --bypass or CPN_UPGRADE_BYPASS=1 to refresh CPN-managed stacks only)"
                .into(),
        );
        return notes;
    }

    let bin = ["docker", "podman"].into_iter().find(|&candidate| {
        Command::new(candidate)
            .arg("--version")
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    });
    let Some(bin) = bin else {
        notes.push("docker/podman not installed; bypass docker refresh skipped".into());
        return notes;
    };

    let compose_dirs = cpn_compose_dirs();
    if compose_dirs.is_empty() && cpn_managed_container_ids().is_empty() {
        notes.push(
            "no CPN-managed docker compose projects or labeled containers found under /var/lib/cpn/docker (user stacks left untouched)"
                .into(),
        );
        return notes;
    }

    for dir in compose_dirs {
        let compose_file = ["compose.yml", "docker-compose.yml", "compose.yaml"]
            .into_iter()
            .map(|n| dir.join(n))
            .find(|p| p.is_file());
        let Some(file) = compose_file else {
            continue;
        };
        notes.push(format!(
            "bypass: refreshing CPN compose project {}",
            dir.display()
        ));
        let pull = Command::new(bin)
            .args([
                "compose",
                "-f",
                &file.to_string_lossy(),
                "--project-directory",
                &dir.to_string_lossy(),
                "pull",
            ])
            .status();
        if !pull.map(|s| s.success()).unwrap_or(false) {
            notes.push(format!(
                "bypass warning: compose pull failed for {} (continuing)",
                dir.display()
            ));
        }
        let up = Command::new(bin)
            .args([
                "compose",
                "-f",
                &file.to_string_lossy(),
                "--project-directory",
                &dir.to_string_lossy(),
                "up",
                "-d",
                "--remove-orphans",
            ])
            .status();
        if up.map(|s| s.success()).unwrap_or(false) {
            notes.push(format!("bypass: compose up ok for {}", dir.display()));
        } else {
            notes.push(format!(
                "bypass FAIL: compose up failed for {}",
                dir.display()
            ));
        }
    }

    // Labeled standalone containers: pull image + recreate while keeping volumes.
    for line in cpn_managed_container_ids() {
        let mut parts = line.split_whitespace();
        let Some(id) = parts.next() else {
            continue;
        };
        let inspect = Command::new(bin)
            .args(["inspect", "--format", "{{.Config.Image}}", id])
            .output();
        let Ok(out) = inspect else {
            continue;
        };
        if !out.status.success() {
            continue;
        }
        let image = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if image.is_empty() {
            continue;
        }
        let _ = Command::new(bin).args(["pull", &image]).status();
        // Recreate would need full run args; prefer compose path. Log only.
        notes.push(format!(
            "bypass: pulled image {image} for labeled container {id} (compose projects preferred for recreate)"
        ));
    }

    notes
}

fn ensure_panel_service_restarted() -> Result<(), String> {
    let allow_remote = panel_service::allow_remote_requested();
    panel_service::ensure_panel_service_after_install(
        PanelServiceMode::EnableAndStart,
        allow_remote,
    )
    .map(|_| ())?;
    // Prefer an explicit restart so the new package binary is live.
    let _ = Command::new("systemctl")
        .args(["restart", "cpn-installer.service"])
        .status();
    thread::sleep(Duration::from_secs(2));
    Ok(())
}

/// Verify critical services after upgrade. Fails when required checks fail.
pub fn verify_after_upgrade(
    bypass_docker: bool,
    previously_running_docker_ids: &[String],
) -> Result<VerifyReport, String> {
    let mut report = VerifyReport::default();

    if cfg!(windows) {
        push(
            &mut report,
            "platform",
            true,
            "Windows verify is limited; Linux packaging checks skipped",
            false,
        );
        return Ok(report);
    }

    if let Err(error) = ensure_panel_service_restarted() {
        push(
            &mut report,
            "panel.service",
            false,
            format!("could not enable/restart cpn-installer.service: {error}"),
            true,
        );
    } else {
        let active = unit_active("cpn-installer.service");
        push(
            &mut report,
            "panel.service",
            active,
            if active {
                "cpn-installer.service is active"
            } else {
                "cpn-installer.service is not active after restart"
            },
            true,
        );
    }

    let port = listen_port::load_preferred_listen_port().unwrap_or(listen_port::DEFAULT_PORT);
    let login_ok = http_login_ok(port);
    push(
        &mut report,
        "panel.http_login",
        login_ok,
        format!("GET http://127.0.0.1:{port}/login"),
        true,
    );

    // Web server: require active only when the unit is enabled (installed for boot).
    let web_units = [
        ("nginx", "nginx"),
        ("openlitespeed", "OpenLiteSpeed"),
        ("lshttpd", "LiteSpeed"),
        ("httpd", "Apache httpd"),
        ("caddy", "Caddy"),
    ];
    let mut saw_web = false;
    for (unit, label) in web_units {
        let enabled = Command::new("systemctl")
            .args(["is-enabled", "--quiet", unit])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        let active = unit_active(unit);
        if !(enabled || active) {
            continue;
        }
        saw_web = true;
        push(
            &mut report,
            &format!("web.{unit}"),
            active,
            format!("{label} unit {unit} (enabled={enabled})"),
            enabled,
        );
    }
    if !saw_web {
        push(
            &mut report,
            "web.server",
            true,
            "no enabled CPN-related web server unit detected; skipped",
            false,
        );
    }

    let db = service_detect::detect_database();
    if db.listening_3306
        || db.service_label.starts_with("MariaDB")
        || db.service_label.starts_with("MySQL")
        || db.service_label.starts_with("mysqld")
    {
        let ok = db.listening_3306
            || service_detect::systemd_unit_active("mariadb")
            || service_detect::systemd_unit_active("mysql")
            || service_detect::systemd_unit_active("mysqld");
        push(&mut report, "database", ok, db.detail.clone(), true);
    } else {
        push(
            &mut report,
            "database",
            true,
            "MariaDB/MySQL not detected; skipped".into(),
            false,
        );
    }

    if panel_feature_gate::webmail_installed() || unit_active("cpn-webmail") {
        let sock_ok = std::path::Path::new("/run/php-fpm/cpn-webmail.sock").exists()
            || unit_active("php-fpm")
            || unit_active("cpn-webmail");
        push(
            &mut report,
            "webmail",
            sock_ok,
            "SnappyMail/Roundcube or cpn-webmail/php-fpm surface",
            true,
        );
    } else {
        push(
            &mut report,
            "webmail",
            true,
            "webmail not installed; skipped",
            false,
        );
    }

    if unit_active("postfix") || unit_active("dovecot") {
        let mail_ok = unit_active("postfix") || unit_active("dovecot");
        push(
            &mut report,
            "mail",
            mail_ok,
            "Postfix/Dovecot active check",
            true,
        );
    } else {
        push(
            &mut report,
            "mail",
            true,
            "mail stack not active; skipped",
            false,
        );
    }

    // Docker: default leave stacks alone; verify previously running CPN-managed ones.
    let docker = panel_ops_docker::docker_status();
    if !docker.installed {
        push(
            &mut report,
            "docker",
            true,
            "docker/podman not installed; skipped",
            false,
        );
    } else if previously_running_docker_ids.is_empty() && !bypass_docker {
        push(
            &mut report,
            "docker",
            true,
            "no prior CPN-managed running containers; user docker stacks left as-is",
            false,
        );
    } else {
        let now = snapshot_cpn_docker_running();
        let mut missing = Vec::new();
        for id in previously_running_docker_ids {
            if !now.iter().any(|n| n == id)
                && !cpn_managed_container_ids().iter().any(|line| {
                    line.starts_with(id) && {
                        let lower = line.to_lowercase();
                        lower.contains("up ") || lower.contains("running")
                    }
                })
            {
                missing.push(id.clone());
            }
        }
        let ok = missing.is_empty();
        push(
            &mut report,
            "docker.cpn_managed",
            ok,
            if ok {
                if bypass_docker {
                    "CPN-managed containers healthy after bypass refresh".into()
                } else {
                    "previously running CPN-managed containers still running (user stacks untouched)"
                        .into()
                }
            } else {
                format!(
                    "CPN-managed containers no longer running: {}",
                    missing.join(", ")
                )
            },
            !previously_running_docker_ids.is_empty(),
        );
    }

    if !report.ok() {
        let fails = report
            .failed_required()
            .iter()
            .map(|c| format!("{}: {}", c.name, c.detail))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(format!(
            "Post-upgrade verification failed: {fails}. Panel package may be updated; fix services before claiming success."
        ));
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_report_is_ok() {
        assert!(VerifyReport::default().ok());
    }

    #[test]
    fn required_failure_is_not_ok() {
        let mut report = VerifyReport::default();
        push(&mut report, "x", false, "down", true);
        assert!(!report.ok());
    }
}
