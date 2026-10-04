//! OpenLiteSpeed package install, repair, upgrade, and downgrade operations.

use crate::litespeed_stack::{openlitespeed_installed, restart_litespeed};
use crate::service_detect::systemd_unit_active;
use std::process::Command;

/// Repair the selected OpenLiteSpeed package repository and restart an existing install.
///
/// This deliberately does not install OLS on hosts where it is absent. Operators select or
/// switch the web server through the installer so CPN can safely handle port conflicts.
pub fn repair_openlitespeed() -> Result<String, String> {
    if !openlitespeed_installed() {
        return Ok(
            "OpenLiteSpeed is not installed; skipped. Select it in the installer to switch web servers."
                .into(),
        );
    }

    let guest = crate::os_support::detect_guest_os()?;
    crate::install_recipes::prepare_openlitespeed_repository(&guest)?;
    match guest.family {
        crate::os_support::PackageFamily::Apt => {
            let bootstrap = crate::install_recipes::prepare_openlitespeed_apt_command();
            let status = Command::new(bootstrap.program)
                .args(bootstrap.args)
                .status()
                .map_err(|error| format!("OpenLiteSpeed apt repository repair failed: {error}"))?;
            if !status.success() {
                return Err("OpenLiteSpeed apt repository repair failed.".into());
            }
            let status = Command::new("apt-get")
                .args(["install", "-y", "openlitespeed"])
                .status()
                .map_err(|error| format!("apt-get install openlitespeed failed: {error}"))?;
            if !status.success() {
                return Err("apt-get could not repair the OpenLiteSpeed package.".into());
            }
        }
        crate::os_support::PackageFamily::Dnf => {
            let status = Command::new("dnf")
                .args(["install", "-y", "openlitespeed"])
                .status()
                .map_err(|error| format!("dnf install openlitespeed failed: {error}"))?;
            if !status.success() {
                return Err("dnf could not repair the OpenLiteSpeed package.".into());
            }
        }
        crate::os_support::PackageFamily::Windows => {
            return Err(crate::os_support::windows_linux_recipe_blocked_message(
                "OpenLiteSpeed repair",
            ));
        }
    }

    let restart = restart_litespeed();
    if !["openlitespeed", "lsws", "lshttpd"]
        .iter()
        .any(|unit| systemd_unit_active(unit))
    {
        return Err(format!(
            "OpenLiteSpeed package repair completed, but its service is not active. {restart}"
        ));
    }
    Ok(format!(
        "OpenLiteSpeed package repository repaired. {restart}"
    ))
}

/// Soft package refresh for OpenLiteSpeed (best-effort).
pub fn upgrade_openlitespeed_packages() -> Result<String, String> {
    if !openlitespeed_installed() {
        return Err("OpenLiteSpeed is not installed on this host.".into());
    }
    if Command::new("dnf").arg("--version").status().is_ok() {
        let status = Command::new("dnf")
            .args(["upgrade", "-y", "openlitespeed"])
            .status()
            .map_err(|e| format!("dnf: {e}"))?;
        if !status.success() {
            return Err("dnf upgrade openlitespeed failed.".into());
        }
        let _ = restart_litespeed();
        return Ok("OpenLiteSpeed packages upgraded via dnf.".into());
    }
    if Command::new("apt-get").arg("--version").status().is_ok() {
        let _ = Command::new("apt-get").args(["update", "-y"]).status();
        let status = Command::new("apt-get")
            .args(["install", "--only-upgrade", "-y", "openlitespeed"])
            .status()
            .map_err(|e| format!("apt-get: {e}"))?;
        if !status.success() {
            return Err("apt-get upgrade openlitespeed failed.".into());
        }
        let _ = restart_litespeed();
        return Ok("OpenLiteSpeed packages upgraded via apt.".into());
    }
    Err("No supported package manager (dnf/apt-get).".into())
}

/// Best-effort downgrade of `openlitespeed` to an explicit version string.
pub fn downgrade_openlitespeed_to(version: &str) -> Result<String, String> {
    let version = version.trim();
    if version.is_empty()
        || version.contains(' ')
        || version.contains(';')
        || version.contains('&')
        || version.contains('|')
        || version.contains('`')
    {
        return Err("Provide a clean package version (example: 1.8.2).".into());
    }
    if !openlitespeed_installed() {
        return Err("OpenLiteSpeed is not installed on this host.".into());
    }
    let pkg = format!("openlitespeed-{version}");
    if Command::new("dnf").arg("--version").status().is_ok() {
        let status = Command::new("dnf")
            .args(["downgrade", "-y", &pkg])
            .status()
            .map_err(|e| format!("dnf: {e}"))?;
        if !status.success() {
            let status2 = Command::new("dnf")
                .args(["install", "-y", "--allowerasing", &pkg])
                .status()
                .map_err(|e| format!("dnf: {e}"))?;
            if !status2.success() {
                return Err(format!(
                    "dnf could not downgrade/install {pkg}. Check repo mirrors for that version."
                ));
            }
        }
        let _ = restart_litespeed();
        return Ok(format!("OpenLiteSpeed moved toward {pkg} via dnf."));
    }
    if Command::new("apt-get").arg("--version").status().is_ok() {
        let status = Command::new("apt-get")
            .args([
                "install",
                "-y",
                "--allow-downgrades",
                &format!("openlitespeed={version}"),
            ])
            .status()
            .map_err(|e| format!("apt-get: {e}"))?;
        if !status.success() {
            return Err(format!(
                "apt-get could not install openlitespeed={version}."
            ));
        }
        let _ = restart_litespeed();
        return Ok(format!(
            "OpenLiteSpeed moved toward openlitespeed={version} via apt."
        ));
    }
    Err("No supported package manager (dnf/apt-get).".into())
}
