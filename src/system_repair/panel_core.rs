//! Core panel / CLI path checks and heals (extends legacy `cpn doctor`).

use super::{CheckStatus, HealResult, push};
use crate::listen_port;
use crate::manifest::{self, load_manifest};
use crate::panel_service::UNIT_NAME;
use crate::paths;
use crate::service_detect;
use crate::upgrade_cleanup;
use std::fs;
use std::path::Path;
use std::process::Command;

pub const CORE_HEAL_IDS: &[&str] = &["cli.local_override", "panel.service"];

fn executable(path: &str) -> bool {
    let p = Path::new(path);
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

fn symlink_or_exists(path: &str) -> bool {
    let p = Path::new(path);
    if p.exists() {
        return true;
    }
    fs::symlink_metadata(p)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
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

pub fn collect(checks: &mut Vec<super::RepairCheck>) {
    let installer = paths::installer_bin_path();
    let cli = paths::cli_bin_path();
    push(
        checks,
        "cli.installer_bin",
        "cli",
        "Installer binary",
        if executable(installer) {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        installer.to_string(),
        true,
        None,
    );
    push(
        checks,
        "cli.cpn_bin",
        "cli",
        "cpn CLI binary",
        if executable(cli) {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        cli.to_string(),
        true,
        None,
    );

    let local_cpn = symlink_or_exists("/usr/local/bin/cpn");
    let local_inst = symlink_or_exists("/usr/local/bin/cpn-installer");
    if local_cpn || local_inst {
        push(
            checks,
            "cli.local_override",
            "cli",
            "Local bin override",
            CheckStatus::Warn,
            "/usr/local/bin/cpn or cpn-installer present (shadows RPM). Fix: sudo cpn doctor --heal then hash -r",
            false,
            Some("cli.local_override"),
        );
    } else {
        push(
            checks,
            "cli.local_override",
            "cli",
            "Local bin override",
            CheckStatus::Pass,
            "no /usr/local/bin/cpn override",
            false,
            None,
        );
    }

    if let Some(resolved) = which_cpn() {
        let ok = resolved == cli || resolved == "/bin/cpn" || resolved.ends_with("/usr/bin/cpn");
        push(
            checks,
            "cli.which",
            "cli",
            "PATH resolution",
            if ok || !local_cpn {
                CheckStatus::Pass
            } else {
                CheckStatus::Warn
            },
            format!("command -v cpn => {resolved}"),
            false,
            if local_cpn {
                Some("cli.local_override")
            } else {
                None
            },
        );
    } else {
        push(
            checks,
            "cli.which",
            "cli",
            "PATH resolution",
            CheckStatus::Fail,
            "cpn not on PATH in this shell",
            true,
            None,
        );
    }

    if cfg!(windows) {
        push(
            checks,
            "panel.service",
            "panel",
            "Panel service",
            CheckStatus::Pass,
            "systemd checks skipped on Windows",
            false,
            None,
        );
    } else {
        let active = service_detect::systemd_unit_active(UNIT_NAME);
        push(
            checks,
            "panel.service",
            "panel",
            "Panel service",
            if active {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            if active {
                format!("{UNIT_NAME} is active")
            } else {
                format!("{UNIT_NAME} is not active (systemctl start {UNIT_NAME})")
            },
            true,
            if active { None } else { Some("panel.service") },
        );
    }

    let port = listen_port::load_preferred_listen_port().unwrap_or(listen_port::DEFAULT_PORT);
    let login_ok = http_login_ok(port);
    push(
        checks,
        "panel.http_login",
        "panel",
        "Login HTTP",
        if login_ok {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        format!("GET http://127.0.0.1:{port}/login"),
        true,
        if login_ok {
            None
        } else {
            Some("panel.service")
        },
    );

    match load_manifest() {
        None => push(
            checks,
            "manifest",
            "panel",
            "Install manifest",
            CheckStatus::Warn,
            format!(
                "install-manifest.json missing under {}",
                manifest::data_dir().display()
            ),
            false,
            None,
        ),
        Some(m) => {
            push(
                checks,
                "manifest",
                "panel",
                "Install manifest",
                CheckStatus::Pass,
                format!(
                    "package_version={} release_tag={} core_files={}",
                    m.package_version,
                    m.release_tag,
                    m.core_files.len()
                ),
                false,
                None,
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
                "panel",
                "Core packaged files",
                if ok {
                    CheckStatus::Pass
                } else {
                    CheckStatus::Fail
                },
                if ok {
                    format!("{present} core paths present on disk")
                } else {
                    format!(
                        "missing required core paths: {} (run sudo cpn-installer --repair)",
                        missing.join(", ")
                    )
                },
                !missing.is_empty(),
                None,
            );
        }
    }
}

pub fn heal_local_override() -> HealResult {
    #[cfg(unix)]
    {
        if unsafe { libc::geteuid() } != 0 {
            return HealResult {
                heal_id: "cli.local_override".into(),
                ok: false,
                message: "requires root (sudo cpn doctor --heal)".into(),
            };
        }
    }
    let notes = upgrade_cleanup::remove_local_bin_overrides();
    if notes.is_empty() {
        HealResult {
            heal_id: "cli.local_override".into(),
            ok: true,
            message: "no /usr/local/bin/cpn overrides to remove; run hash -r in open shells".into(),
        }
    } else {
        HealResult {
            heal_id: "cli.local_override".into(),
            ok: true,
            message: format!(
                "{}; run hash -r in open shells",
                notes.join("; ")
            ),
        }
    }
}

pub fn heal_panel_service() -> HealResult {
    #[cfg(unix)]
    {
        if unsafe { libc::geteuid() } != 0 {
            return HealResult {
                heal_id: "panel.service".into(),
                ok: false,
                message: "requires root to start the panel unit".into(),
            };
        }
    }
    if service_detect::systemd_unit_active(UNIT_NAME) {
        return HealResult {
            heal_id: "panel.service".into(),
            ok: true,
            message: format!("{UNIT_NAME} already active"),
        };
    }
    let ok = Command::new("systemctl")
        .args(["start", UNIT_NAME])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    HealResult {
        heal_id: "panel.service".into(),
        ok,
        message: if ok {
            format!("started {UNIT_NAME}")
        } else {
            format!("could not start {UNIT_NAME}")
        },
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn override_message_has_no_em_dash() {
        let msg = "/usr/local/bin/cpn or cpn-installer present (shadows RPM). Fix: sudo cpn doctor --heal then hash -r";
        assert!(!msg.contains('\u{2014}'));
        assert!(!msg.contains('\u{2013}'));
    }
}
