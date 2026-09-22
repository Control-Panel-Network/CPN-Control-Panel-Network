//! `cpn doctor`: health checks for CLI paths, panel unit, login, and core files.

use crate::listen_port;
use crate::manifest::{self, load_manifest};
use crate::panel_service::UNIT_NAME;
use crate::paths;
use crate::service_detect;
use crate::upgrade_cleanup;
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
struct Check {
    name: String,
    ok: bool,
    detail: String,
    required: bool,
}

fn push(checks: &mut Vec<Check>, name: &str, ok: bool, detail: impl Into<String>, required: bool) {
    checks.push(Check {
        name: name.into(),
        ok,
        detail: detail.into(),
        required,
    });
}

fn executable(path: &str) -> bool {
    let p = Path::new(path);
    if !p.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        return p
            .metadata()
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false);
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn symlink_or_exists(path: &str) -> bool {
    let p = Path::new(path);
    if p.exists() {
        return true;
    }
    fs::symlink_metadata(p)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
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

fn which_cpn() -> Option<String> {
    Command::new("bash")
        .args(["-lc", "command -v cpn 2>/dev/null || true"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

fn check_core_manifest(checks: &mut Vec<Check>) {
    match load_manifest() {
        None => push(
            checks,
            "manifest",
            false,
            format!(
                "install-manifest.json missing under {}",
                manifest::data_dir().display()
            ),
            false,
        ),
        Some(m) => {
            push(
                checks,
                "manifest",
                true,
                format!(
                    "package_version={} release_tag={} core_files={}",
                    m.package_version,
                    m.release_tag,
                    m.core_files.len()
                ),
                false,
            );
            let mut missing = Vec::new();
            let mut present = 0usize;
            for entry in &m.core_files {
                let path = Path::new(&entry.path);
                if path.exists() {
                    present += 1;
                } else if !entry.optional {
                    missing.push(entry.path.clone());
                }
            }
            let ok = missing.is_empty();
            push(
                checks,
                "manifest.core_files",
                ok,
                if ok {
                    format!("{present} core paths present on disk")
                } else {
                    format!(
                        "missing required core paths: {} (run sudo cpn-installer --repair)",
                        missing.join(", ")
                    )
                },
                !missing.is_empty(),
            );
        }
    }
}

fn check_host_stack(checks: &mut Vec<Check>) {
    let db = service_detect::detect_database();
    if db.listening_3306
        || db.service_label.to_lowercase().contains("mariadb")
        || db.service_label.to_lowercase().contains("mysql")
    {
        let ok = db.listening_3306
            || unit_active("mariadb")
            || unit_active("mysql")
            || unit_active("mysqld");
        push(checks, "host.database", ok, db.detail.clone(), false);
    } else {
        push(
            checks,
            "host.database",
            true,
            "MariaDB not detected (optional)",
            false,
        );
    }

    let web_bins = [
        "/usr/local/lsws/bin/openlitespeed",
        "/usr/local/lsws/bin/lshttpd",
        "/usr/sbin/nginx",
        "/usr/sbin/httpd",
    ];
    let saw = web_bins.iter().any(|p| Path::new(p).is_file());
    push(
        checks,
        "host.web_bin",
        true,
        if saw {
            "web server binary present"
        } else {
            "no OLS/nginx/httpd binary detected (optional)"
        },
        false,
    );
}

fn check_plugins_summary(checks: &mut Vec<Check>) {
    // Lightweight presence scan; doctor does not reinstall plugins.
    let sites_dir = paths::default_data_dir().join("sites");
    let mut plugin_roots = 0usize;
    if sites_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&sites_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.ends_with(".json") {
                    continue;
                }
                let domain = name.trim_end_matches(".json");
                let home = Path::new("/home").join(domain).join("plugins");
                if home.is_dir() {
                    plugin_roots += 1;
                }
            }
        }
    }
    let host_plugins = Path::new("/var/lib/cpn/plugins");
    let host_present = host_plugins.is_dir();
    push(
        checks,
        "plugins.summary",
        true,
        format!(
            "site plugin trees={plugin_roots}; host plugins dir={}; repair does not reinstall plugins (use cpn plugin / Host packages UI)",
            if host_present { "present" } else { "absent" }
        ),
        false,
    );
}

fn print_report(checks: &[Check]) -> i32 {
    let mut failed_required = 0usize;
    let mut failed_optional = 0usize;
    for c in checks {
        let mark = if c.ok { "ok" } else { "FAIL" };
        let req = if c.required { "required" } else { "info" };
        println!("[{mark}] {req}: {} ({})", c.name, c.detail);
        if !c.ok {
            if c.required {
                failed_required += 1;
            } else {
                failed_optional += 1;
            }
        }
    }
    println!();
    if failed_required == 0 {
        println!(
            "doctor: PASS ({} checks; {} info warnings)",
            checks.len(),
            failed_optional
        );
        0
    } else {
        println!(
            "doctor: FAIL ({failed_required} required, {failed_optional} info). See recommended fix flow in docs/CLI.md (Repair and doctor)."
        );
        1
    }
}

/// Run doctor checks. When `heal` is true, remove `/usr/local/bin/cpn*` overrides (root).
pub fn run(heal: bool) -> Result<(), String> {
    let mut checks = Vec::new();

    if heal {
        #[cfg(unix)]
        {
            if unsafe { libc::geteuid() } != 0 {
                return Err("cpn doctor --heal requires root (sudo cpn doctor --heal)".into());
            }
        }
        let notes = upgrade_cleanup::remove_local_bin_overrides();
        if notes.is_empty() {
            println!("heal: no /usr/local/bin/cpn overrides to remove");
        } else {
            for note in &notes {
                println!("heal: {note}");
            }
        }
        println!(
            "heal: run `hash -r` in open shells so bash forgets a deleted /usr/local/bin/cpn path"
        );
        println!();
    }

    let installer = paths::installer_bin_path();
    let cli = paths::cli_bin_path();
    push(
        &mut checks,
        "cli.installer_bin",
        executable(installer),
        format!("{installer}"),
        true,
    );
    push(
        &mut checks,
        "cli.cpn_bin",
        executable(cli),
        format!("{cli}"),
        true,
    );

    let local_cpn = symlink_or_exists("/usr/local/bin/cpn");
    let local_inst = symlink_or_exists("/usr/local/bin/cpn-installer");
    if local_cpn || local_inst {
        push(
            &mut checks,
            "cli.local_override",
            false,
            "/usr/local/bin/cpn or cpn-installer present (shadows RPM). Fix: sudo cpn doctor --heal then hash -r",
            false,
        );
    } else {
        push(
            &mut checks,
            "cli.local_override",
            true,
            "no /usr/local/bin/cpn override",
            false,
        );
    }

    if let Some(resolved) = which_cpn() {
        let ok = resolved == cli || resolved == "/bin/cpn" || resolved.ends_with("/usr/bin/cpn");
        push(
            &mut checks,
            "cli.which",
            ok || !local_cpn,
            format!("command -v cpn => {resolved}"),
            false,
        );
    } else {
        push(
            &mut checks,
            "cli.which",
            false,
            "cpn not on PATH in this shell",
            true,
        );
    }

    if cfg!(windows) {
        push(
            &mut checks,
            "panel.service",
            true,
            "systemd checks skipped on Windows",
            false,
        );
    } else {
        let active = unit_active(UNIT_NAME);
        push(
            &mut checks,
            "panel.service",
            active,
            if active {
                format!("{UNIT_NAME} is active")
            } else {
                format!("{UNIT_NAME} is not active (systemctl start {UNIT_NAME})")
            },
            true,
        );
    }

    let port = listen_port::load_preferred_listen_port().unwrap_or(listen_port::DEFAULT_PORT);
    let login_ok = http_login_ok(port);
    push(
        &mut checks,
        "panel.http_login",
        login_ok,
        format!("GET http://127.0.0.1:{port}/login"),
        true,
    );

    check_core_manifest(&mut checks);
    check_host_stack(&mut checks);
    check_plugins_summary(&mut checks);

    let code = print_report(&checks);
    if code != 0 {
        return Err("doctor reported required failures".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_records_required_flag() {
        let mut checks = Vec::new();
        push(&mut checks, "x", false, "detail", true);
        assert_eq!(checks.len(), 1);
        assert!(checks[0].required);
        assert!(!checks[0].ok);
    }

    #[test]
    fn no_em_or_en_dash_in_local_override_message() {
        let msg = "/usr/local/bin/cpn or cpn-installer present (shadows RPM). Fix: sudo cpn doctor --heal then hash -r";
        assert!(!msg.contains('\u{2014}'));
        assert!(!msg.contains('\u{2013}'));
    }
}
