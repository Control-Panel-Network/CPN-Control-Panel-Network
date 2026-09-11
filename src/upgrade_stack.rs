//! Best-effort host stack package refresh during CPN upgrade/repair.
//!
//! Updates already-installed CPN-managed services (MariaDB, OpenLiteSpeed, PHP)
//! via the system package manager. Never drops databases or wipes data dirs.
//! Skips packages that are not installed. Failures are logged as notes, not
//! hard errors, so a transient mirror issue does not undo a successful CPN package op.

use crate::apps_pkg::{package_manager, rpm_or_dpkg_installed};
use crate::service_detect;
use std::process::Command;

fn upgrade_named_packages(dnf_pkgs: &[&str], apt_pkgs: &[&str]) -> Result<String, String> {
    let pm = package_manager()?;
    if pm == "dnf" {
        let present: Vec<&str> = dnf_pkgs
            .iter()
            .copied()
            .filter(|name| rpm_or_dpkg_installed(&[name]))
            .collect();
        if present.is_empty() {
            return Ok("skipped (not installed)".into());
        }
        let mut args = vec!["upgrade", "-y"];
        args.extend(present.iter().copied());
        let status = Command::new("dnf")
            .args(&args)
            .status()
            .map_err(|error| format!("Could not start dnf upgrade: {error}"))?;
        if !status.success() {
            return Err(format!("dnf upgrade {} failed", present.join(" ")));
        }
        Ok(format!("upgraded {}", present.join(", ")))
    } else {
        let present: Vec<&str> = apt_pkgs
            .iter()
            .copied()
            .filter(|name| rpm_or_dpkg_installed(&[name]))
            .collect();
        if present.is_empty() {
            return Ok("skipped (not installed)".into());
        }
        let update = Command::new("apt-get")
            .args(["update", "-y"])
            .status()
            .map_err(|error| format!("Could not start apt-get update: {error}"))?;
        if !update.success() {
            return Err("apt-get update failed".into());
        }
        let mut args = vec!["install", "--only-upgrade", "-y"];
        args.extend(present.iter().copied());
        let status = Command::new("apt-get")
            .args(&args)
            .status()
            .map_err(|error| format!("Could not start apt-get upgrade: {error}"))?;
        if !status.success() {
            return Err(format!("apt-get --only-upgrade {} failed", present.join(" ")));
        }
        Ok(format!("upgraded {}", present.join(", ")))
    }
}

fn restart_if_active(unit: &str) -> String {
    if !service_detect::systemd_unit_active(unit) {
        return format!("{unit}: not active (left stopped)");
    }
    match Command::new("systemctl").args(["restart", unit]).status() {
        Ok(status) if status.success() => format!("{unit}: restarted"),
        Ok(_) => format!("{unit}: restart failed"),
        Err(error) => format!("{unit}: restart error ({error})"),
    }
}

fn health_mariadb() -> String {
    let active = service_detect::systemd_unit_active("mariadb")
        || service_detect::systemd_unit_active("mysqld")
        || service_detect::systemd_unit_active("mysql");
    if active {
        "MariaDB/MySQL unit active after refresh (databases preserved)".into()
    } else if rpm_or_dpkg_installed(&["mariadb-server", "MariaDB-server", "mysql-server"]) {
        "MariaDB/MySQL packages present but unit not active".into()
    } else {
        "MariaDB/MySQL not installed (skipped)".into()
    }
}

/// Refresh already-installed CPN-managed stack packages after a successful core upgrade.
pub fn refresh_managed_stack() -> Vec<String> {
    let mut notes = Vec::new();
    if cfg!(windows) {
        notes.push("managed stack refresh skipped on Windows".into());
        return notes;
    }

    notes.push(
        "Refreshing CPN-managed host packages when already installed (MariaDB, OpenLiteSpeed, PHP). Databases and docroots are not dropped."
            .into(),
    );

    match upgrade_named_packages(
        &["mariadb-server", "MariaDB-server"],
        &["mariadb-server"],
    ) {
        Ok(msg) => notes.push(format!("MariaDB: {msg}")),
        Err(error) => notes.push(format!("MariaDB: {error} (continuing)")),
    }
    if rpm_or_dpkg_installed(&["mariadb-server", "MariaDB-server"]) {
        notes.push(restart_if_active("mariadb"));
    }

    match upgrade_named_packages(&["openlitespeed"], &["openlitespeed"]) {
        Ok(msg) => notes.push(format!("OpenLiteSpeed: {msg}")),
        Err(error) => notes.push(format!("OpenLiteSpeed: {error} (continuing)")),
    }
    if rpm_or_dpkg_installed(&["openlitespeed"]) {
        notes.push(restart_if_active("openlitespeed"));
        notes.push(restart_if_active("lshttpd"));
    }

    match upgrade_named_packages(
        &["php", "php-fpm", "php-cli", "php-mysqlnd", "php-gd", "php-xml"],
        &["php", "php-fpm", "php-cli", "php-mysql", "php-gd", "php-xml"],
    ) {
        Ok(msg) => notes.push(format!("PHP: {msg}")),
        Err(error) => notes.push(format!("PHP: {error} (continuing)")),
    }
    if rpm_or_dpkg_installed(&["php-fpm"]) {
        notes.push(restart_if_active("php-fpm"));
    }

    notes.push(health_mariadb());
    notes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_avoid_em_and_en_dash() {
        for note in [
            "Refreshing CPN-managed host packages when already installed (MariaDB, OpenLiteSpeed, PHP). Databases and docroots are not dropped.",
            "MariaDB/MySQL unit active after refresh (databases preserved)",
        ] {
            assert!(!note.contains('\u{2014}'));
            assert!(!note.contains('\u{2013}'));
            assert!(!note.to_lowercase().contains("cyberpanel"));
        }
    }

    #[test]
    fn windows_path_returns_skip_note() {
        if cfg!(windows) {
            let notes = refresh_managed_stack();
            assert!(notes.iter().any(|n| n.contains("Windows")));
        }
    }
}
