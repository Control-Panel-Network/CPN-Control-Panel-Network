//! Shared dnf/apt and systemd helpers for host app recipes.

use crate::service_detect::systemd_unit_file_exists;
use std::process::{Command, Stdio};

/// Probe a package manager binary quietly (no stdout/stderr leak into the panel journal).
fn manager_runs(bin: &str) -> bool {
    Command::new(bin)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

/// Detect dnf or apt-get once per process. Host app status checks call this for
/// every card on each page load; spawning `dnf --version` each time cost seconds
/// per request and flooded the journal. Only a successful probe is cached so a
/// package manager installed later is still picked up.
pub fn package_manager() -> Result<&'static str, String> {
    static DETECTED: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();
    if let Some(pm) = DETECTED.get() {
        return Ok(*pm);
    }
    let found = if manager_runs("dnf") {
        Some("dnf")
    } else if manager_runs("apt-get") {
        Some("apt")
    } else {
        None
    };
    match found {
        Some(pm) => Ok(*DETECTED.get_or_init(|| pm)),
        None => Err("No supported package manager found (need dnf or apt-get).".into()),
    }
}

pub fn run_pkg(args: &[&str]) -> Result<(), String> {
    let pm = package_manager()?;
    let status = if pm == "dnf" {
        Command::new("dnf")
            .args(args)
            .status()
            .map_err(|error| format!("Could not start dnf: {error}"))?
    } else {
        if args.first() == Some(&"install") || args.first() == Some(&"remove") {
            let update = Command::new("apt-get")
                .args(["update", "-y"])
                .status()
                .map_err(|error| format!("Could not start apt-get update: {error}"))?;
            if !update.success() {
                return Err("apt-get update failed".into());
            }
        }
        let mut apt_args: Vec<&str> = Vec::new();
        match args.first().copied() {
            Some("install") => {
                apt_args.push("install");
                apt_args.push("-y");
                apt_args.extend_from_slice(&args[1..]);
            }
            Some("remove") => {
                apt_args.push("remove");
                apt_args.push("-y");
                apt_args.extend_from_slice(&args[1..]);
            }
            _ => {
                apt_args.extend_from_slice(args);
            }
        }
        Command::new("apt-get")
            .args(&apt_args)
            .status()
            .map_err(|error| format!("Could not start apt-get: {error}"))?
    };
    if !status.success() {
        return Err(format!("{pm} {} failed", args.join(" ")));
    }
    Ok(())
}

fn sanitize_unit_log_snippet(raw: &str) -> String {
    let mut out = String::new();
    for line in raw.lines().take(8) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.contains("password") || lower.contains("passphrase") {
            continue;
        }
        if out.len() + trimmed.len() > 480 {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(trimmed);
    }
    out
}

fn unit_enable_failure_hint(unit: &str) -> String {
    if !systemd_unit_file_exists(unit) {
        return format!(
            "Unit file {unit}.service is not installed. Install the Email host package first."
        );
    }
    let journal = Command::new("timeout")
        .args(["5", "journalctl", "-u", unit, "-n", "8", "--no-pager"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| sanitize_unit_log_snippet(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default();
    if journal.is_empty() {
        format!("Check: systemctl status {unit} and journalctl -u {unit} -n 20")
    } else {
        format!("Recent log: {journal}")
    }
}

pub fn enable_now(units: &[&str]) -> Result<(), String> {
    for unit in units {
        if !systemd_unit_file_exists(unit) {
            return Err(format!(
                "systemctl enable --now {unit} failed. {}",
                unit_enable_failure_hint(unit)
            ));
        }
        let status = Command::new("systemctl")
            .args(["enable", "--now", unit])
            .status()
            .map_err(|error| format!("Could not start systemctl for {unit}: {error}"))?;
        if !status.success() {
            return Err(format!(
                "systemctl enable --now {unit} failed. {}",
                unit_enable_failure_hint(unit)
            ));
        }
    }
    Ok(())
}

pub fn disable_now(units: &[&str]) -> Result<(), String> {
    for unit in units {
        let _ = Command::new("systemctl")
            .args(["disable", "--now", unit])
            .status();
    }
    Ok(())
}

pub fn start_units(units: &[&str]) -> Result<(), String> {
    for unit in units {
        let status = Command::new("systemctl")
            .args(["start", unit])
            .status()
            .map_err(|error| format!("Could not start systemctl for {unit}: {error}"))?;
        if !status.success() {
            return Err(format!("systemctl start {unit} failed"));
        }
    }
    Ok(())
}

pub fn stop_units(units: &[&str]) -> Result<(), String> {
    for unit in units {
        let _ = Command::new("systemctl").args(["stop", unit]).status();
    }
    Ok(())
}

pub fn install_packages_dnf_or_apt(dnf_pkgs: &[&str], apt_pkgs: &[&str]) -> Result<(), String> {
    let pm = package_manager()?;
    if pm == "dnf" {
        let mut args = vec!["install", "-y"];
        args.extend_from_slice(dnf_pkgs);
        run_pkg(&args)
    } else {
        let mut args = vec!["install"];
        args.extend_from_slice(apt_pkgs);
        run_pkg(&args)
    }
}

pub fn remove_packages_dnf_or_apt(dnf_pkgs: &[&str], apt_pkgs: &[&str]) -> Result<(), String> {
    let pm = package_manager()?;
    if pm == "dnf" {
        let mut args = vec!["remove", "-y"];
        args.extend_from_slice(dnf_pkgs);
        run_pkg(&args)
    } else {
        let mut args = vec!["remove"];
        args.extend_from_slice(apt_pkgs);
        run_pkg(&args)
    }
}

pub fn package_installed(name: &str) -> bool {
    let rpm = Command::new("rpm")
        .args(["-q", name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if rpm {
        return true;
    }
    Command::new("dpkg-query")
        .args(["-W", "-f=${Status}", name])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()
        .map(|out| {
            let text = String::from_utf8_lossy(&out.stdout);
            text.contains("install ok installed")
        })
        .unwrap_or(false)
}

/// True when at least one listed package is installed (legacy helper).
pub fn rpm_or_dpkg_installed(names: &[&str]) -> bool {
    names.iter().any(|name| package_installed(name))
}

/// True when every listed package is installed.
pub fn rpm_or_dpkg_all_installed(names: &[&str]) -> bool {
    !names.is_empty() && names.iter().all(|name| package_installed(name))
}
