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

fn push(report: &mut VerifyReport, name: &str, ok: bool, detail: impl AsRef<str>, required: bool) {
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

/// True when curl's `%{http_code}` means the panel process is answering.
///
/// `200` is the normal login page. `503` is the upgrade maintenance page
/// (`PanelMaintenanceGuard`); the process is healthy but visitors are gated
/// until maintenance clears. Using `curl --fail` alone false-fails mid-upgrade.
pub(crate) fn http_code_means_panel_up(code: &str) -> bool {
    matches!(code.trim(), "200" | "503")
}

/// Probe `/login` without `--fail` so maintenance `503` counts as up.
fn http_login_ok(port: u16) -> bool {
    let url = format!("http://127.0.0.1:{port}/login");
    let output = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--max-time",
            "5",
            "--output",
            "/dev/null",
            "--write-out",
            "%{http_code}",
            &url,
        ])
        .output();
    match output {
        Ok(out) => {
            // Connection refused / timeout: non-zero status and often empty body.
            if !out.status.success() && out.stdout.is_empty() {
                return false;
            }
            let code = String::from_utf8_lossy(&out.stdout);
            http_code_means_panel_up(&code)
        }
        Err(_) => false,
    }
}

fn path_is_executable(path: &str) -> bool {
    let p = std::path::Path::new(path);
    if !p.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        p.metadata()
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn local_bin_override_present(name: &str) -> bool {
    let path = format!("/usr/local/bin/{name}");
    let p = std::path::Path::new(&path);
    if p.exists() {
        return true;
    }
    std::fs::symlink_metadata(p)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

/// Ensure RPM CLI paths exist and warn when `/usr/local/bin` shadows them.
fn check_cli_binaries(report: &mut VerifyReport) {
    let installer_ok = path_is_executable(crate::paths::UNIX_INSTALLER_BIN);
    push(
        report,
        "cli.cpn_installer",
        installer_ok,
        if installer_ok {
            format!(
                "{} present and executable",
                crate::paths::UNIX_INSTALLER_BIN
            )
        } else {
            format!(
                "{} missing or not executable (reinstall RPM or run cpn-installer --repair)",
                crate::paths::UNIX_INSTALLER_BIN
            )
        },
        true,
    );

    let cpn_ok = path_is_executable(crate::paths::UNIX_CLI_BIN);
    push(
        report,
        "cli.cpn",
        cpn_ok,
        if cpn_ok {
            format!("{} present and executable", crate::paths::UNIX_CLI_BIN)
        } else {
            format!(
                "{} missing or not executable (dnf reinstall cpn-installer, or cpn-installer --repair)",
                crate::paths::UNIX_CLI_BIN
            )
        },
        true,
    );

    let local_cpn = local_bin_override_present("cpn");
    let local_installer = local_bin_override_present("cpn-installer");
    if local_cpn || local_installer {
        push(
            report,
            "cli.local_bin_override",
            false,
            "hot-deploy override under /usr/local/bin/cpn or cpn-installer still present; run cpn doctor --heal or upgrade cleanup so PATH uses /usr/bin",
            false,
        );
    } else {
        push(
            report,
            "cli.local_bin_override",
            true,
            "no /usr/local/bin/cpn override shadowing RPM binaries",
            false,
        );
    }
}

fn cpn_managed_container_ids() -> Vec<String> {
    let Some(bin) = ["docker", "podman"].into_iter().find(|&candidate| {
        crate::panel_ops_docker_probe::probe_ok(
            candidate,
            &["--version"],
            crate::panel_ops_docker_probe::VERSION_TIMEOUT,
        )
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
        crate::panel_ops_docker_probe::probe_ok(
            candidate,
            &["--version"],
            crate::panel_ops_docker_probe::VERSION_TIMEOUT,
        )
    });
    let Some(bin) = bin else {
        notes.push("docker/podman not installed; bypass docker refresh skipped".into());
        return notes;
    };

    let compose_notes = crate::panel_ops_docker_compose::refresh_all_cpn_compose_projects();
    let no_compose = compose_notes
        .first()
        .is_some_and(|s| s.starts_with("no CPN compose projects found"));
    if no_compose && cpn_managed_container_ids().is_empty() {
        notes.extend(compose_notes);
        return notes;
    }
    notes.extend(compose_notes);

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

/// Default wait for `/login` after a Sync restart (or skip-rebuild listen check).
/// About 20 × 2s = 40s of sleep, plus curl time, covers brief post-restart flaps.
pub(crate) const LOGIN_WAIT_ATTEMPTS: u32 = 20;
pub(crate) const LOGIN_WAIT_SLEEP_SECS: u64 = 2;

fn wait_panel_unit_active(attempts: u32, sleep_secs: u64) -> bool {
    for _ in 0..attempts {
        if unit_active("cpn-installer.service") {
            return true;
        }
        thread::sleep(Duration::from_secs(sleep_secs));
    }
    unit_active("cpn-installer.service")
}

fn wait_http_login_ok(port: u16, attempts: u32, sleep_secs: u64) -> bool {
    if attempts == 0 {
        return http_login_ok(port);
    }
    for i in 0..attempts {
        if http_login_ok(port) {
            return true;
        }
        if i + 1 < attempts {
            thread::sleep(Duration::from_secs(sleep_secs));
        }
    }
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PanelRestartKind {
    /// CLI / out-of-band: unit restarted in-process.
    Sync,
    /// Web UI under systemd: restart scheduled via systemd-run / nohup.
    Deferred,
    /// Tip skip-rebuild (binary already on tip): do not bounce MainPID.
    SkipRestart,
}

fn ensure_panel_service_ready(binary_replaced: bool) -> Result<PanelRestartKind, String> {
    let allow_remote = panel_service::allow_remote_requested();
    panel_service::ensure_panel_service_after_install(
        PanelServiceMode::EnableAndStart,
        allow_remote,
    )
    .map(|_| ())?;

    // Already on tip: enable/ensure only. Restarting would bounce /login and race
    // maintenance 503 checks for no binary gain.
    if !binary_replaced {
        return Ok(PanelRestartKind::SkipRestart);
    }

    // UI upgrades run as the systemd MainPID. Do not stop/restart here; the caller
    // schedules a detached reload after writing phase=completed so migrations finish.
    if panel_service::running_under_systemd() {
        return Ok(PanelRestartKind::Deferred);
    }

    // Labs often leave a foreground `sudo cpn-installer --web` holding :2087.
    // `systemctl restart` alone then fails with AddrInUse and verification marks
    // the upgrade as failed even when package apply succeeded.
    // stop_orphan_panel_listeners must spare this process (CLI --upgrade).
    let _ = panel_service::stop_orphan_panel_listeners();
    let _ = Command::new("systemctl")
        .args(["reset-failed", "cpn-installer.service"])
        .status();
    // Prefer an explicit restart so the new package binary is live.
    let _ = Command::new("systemctl")
        .args(["restart", "cpn-installer.service"])
        .status();
    if !wait_panel_unit_active(6, 1) {
        // One more cleanup pass when restart raced a lingering listener.
        let _ = panel_service::stop_orphan_panel_listeners();
        let _ = Command::new("systemctl")
            .args(["reset-failed", "cpn-installer.service"])
            .status();
        let _ = Command::new("systemctl")
            .args(["start", "cpn-installer.service"])
            .status();
        let _ = wait_panel_unit_active(6, 1);
    }
    Ok(PanelRestartKind::Sync)
}

fn push_http_login_check(
    report: &mut VerifyReport,
    port: u16,
    required: bool,
    detail_suffix: &str,
) {
    let login_ok = wait_http_login_ok(port, LOGIN_WAIT_ATTEMPTS, LOGIN_WAIT_SLEEP_SECS);
    let detail = if login_ok {
        format!("GET http://127.0.0.1:{port}/login (200 or maintenance 503){detail_suffix}")
    } else {
        format!(
            "GET http://127.0.0.1:{port}/login not answering after {LOGIN_WAIT_ATTEMPTS} attempts (~{}s){detail_suffix}",
            LOGIN_WAIT_ATTEMPTS as u64 * LOGIN_WAIT_SLEEP_SECS
        )
    };
    push(report, "panel.http_login", login_ok, detail, required);
}

/// Verify critical services after upgrade. Fails when required checks fail.
///
/// `binary_replaced`: false when tip/commit skip-rebuild left the on-disk binary
/// unchanged. In that case the panel is not restarted mid-verify.
pub fn verify_after_upgrade(
    bypass_docker: bool,
    previously_running_docker_ids: &[String],
    binary_replaced: bool,
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

    let restart_kind = match ensure_panel_service_ready(binary_replaced) {
        Ok(kind) => Some(kind),
        Err(error) => {
            push(
                &mut report,
                "panel.service",
                false,
                format!("could not enable/restart cpn-installer.service: {error}"),
                true,
            );
            None
        }
    };

    match restart_kind {
        Some(PanelRestartKind::Deferred) => {
            push(
                &mut report,
                "panel.service",
                true,
                "cpn-installer.service reload scheduled (detached; avoids in-process self-stop)",
                true,
            );
            let port =
                listen_port::load_preferred_listen_port().unwrap_or(listen_port::DEFAULT_PORT);
            push(
                &mut report,
                "panel.http_login",
                true,
                format!(
                    "GET http://127.0.0.1:{port}/login deferred until detached reload finishes"
                ),
                false,
            );
        }
        Some(PanelRestartKind::Sync) => {
            // Unit can be briefly inactive right after restart; wait before failing.
            let active = wait_panel_unit_active(12, 1);
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
            let port =
                listen_port::load_preferred_listen_port().unwrap_or(listen_port::DEFAULT_PORT);
            push_http_login_check(&mut report, port, true, "");
        }
        Some(PanelRestartKind::SkipRestart) => {
            push(
                &mut report,
                "panel.service",
                true,
                "binary already on tip; skipped panel restart during verify",
                true,
            );
            let port =
                listen_port::load_preferred_listen_port().unwrap_or(listen_port::DEFAULT_PORT);
            // Still probe /login with retries: maintenance begin or cleanup can
            // briefly flap the listener even when the binary was not replaced.
            push_http_login_check(&mut report, port, true, "; skip-rebuild path");
        }
        None => {}
    }

    check_cli_binaries(&mut report);

    // Web server: probe only units that exist on disk. Missing optional engines
    // (httpd/caddy on OLS labs) must be skipped quietly with no systemctl stderr.
    let web_units = [
        ("nginx", "nginx"),
        ("lsws", "OpenLiteSpeed"),
        ("lshttpd", "LiteSpeed / OpenLiteSpeed"),
        ("openlitespeed", "OpenLiteSpeed"),
        ("httpd", "Apache httpd"),
        ("caddy", "Caddy"),
    ];
    let mut saw_web = false;
    for (unit, label) in web_units {
        if !service_detect::systemd_unit_file_exists(unit) {
            continue;
        }
        let enabled = service_detect::systemd_unit_enabled(unit);
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
        let ols_bin = std::path::Path::new("/usr/local/lsws/bin/openlitespeed").is_file()
            || std::path::Path::new("/usr/local/lsws/bin/lshttpd").is_file();
        push(
            &mut report,
            "web.server",
            true,
            if ols_bin {
                "OpenLiteSpeed binaries present; no enabled httpd/caddy/nginx unit (optional engines skipped)"
            } else {
                "no enabled CPN-related web server unit detected; optional engines skipped"
            },
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
            "MariaDB/MySQL not detected; skipped",
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
        let detail = if ok {
            if bypass_docker {
                "CPN-managed containers healthy after bypass refresh".to_string()
            } else {
                "previously running CPN-managed containers still running (user stacks untouched)"
                    .to_string()
            }
        } else {
            format!(
                "CPN-managed containers no longer running: {}",
                missing.join(", ")
            )
        };
        push(
            &mut report,
            "docker.cpn_managed",
            ok,
            detail,
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

    #[test]
    fn missing_optional_web_units_are_not_required_failures() {
        let mut report = VerifyReport::default();
        push(
            &mut report,
            "web.server",
            true,
            "OpenLiteSpeed binaries present; no enabled httpd/caddy/nginx unit (optional engines skipped)",
            false,
        );
        assert!(report.ok());
        assert!(report.failed_required().is_empty());
    }

    #[test]
    fn login_http_200_and_maintenance_503_mean_panel_up() {
        assert!(http_code_means_panel_up("200"));
        assert!(http_code_means_panel_up("503"));
        assert!(http_code_means_panel_up(" 503\n"));
        assert!(!http_code_means_panel_up("000"));
        assert!(!http_code_means_panel_up("404"));
        assert!(!http_code_means_panel_up("500"));
        assert!(!http_code_means_panel_up(""));
    }

    #[test]
    fn login_wait_constants_cover_short_flaps() {
        // Product intent: several attempts over tens of seconds, not a single probe.
        assert!(LOGIN_WAIT_ATTEMPTS >= 15);
        assert!(LOGIN_WAIT_SLEEP_SECS >= 1);
        assert!(LOGIN_WAIT_ATTEMPTS as u64 * LOGIN_WAIT_SLEEP_SECS >= 30);
    }
}
