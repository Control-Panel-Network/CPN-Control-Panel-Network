//! PostgreSQL host app helpers (opt-in; not part of the default MariaDB stack).

use std::path::Path;
use std::process::Command;

use crate::apps_pkg::{
    disable_now, enable_now, install_packages_dnf_or_apt, remove_packages_dnf_or_apt,
    rpm_or_dpkg_installed, stop_units,
};
use crate::service_detect::{first_active_service, port_open, systemd_unit_active};

pub fn postgresql_present() -> bool {
    rpm_or_dpkg_installed(&[
        "postgresql-server",
        "postgresql",
        "postgresql16-server",
        "postgresql15-server",
    ]) || systemd_unit_active("postgresql")
        || first_active_service(&[("postgresql", "x")]).is_some()
}

fn postgresql_data_initialized() -> bool {
    if Path::new("/var/lib/pgsql/data/PG_VERSION").exists() {
        return true;
    }
    let debian_root = Path::new("/var/lib/postgresql");
    if !debian_root.is_dir() {
        return false;
    }
    std::fs::read_dir(debian_root)
        .ok()
        .map(|mut entries| entries.any(|e| e.is_ok()))
        .unwrap_or(false)
}

/// RHEL/Alma/Rocky need `postgresql-setup --initdb` once before the unit can start.
pub fn ensure_postgresql_initialized() -> Result<(), String> {
    if postgresql_data_initialized() {
        return Ok(());
    }
    let setups = ["postgresql-setup", "/usr/bin/postgresql-setup"];
    for bin in setups {
        let Ok(status) = Command::new(bin).args(["--initdb"]).status() else {
            continue;
        };
        if status.success() {
            return Ok(());
        }
    }
    // Debian/Ubuntu packages usually initialize on install; allow enable --now to proceed.
    Ok(())
}

/// Returns (running, installed, detail).
pub fn detect_postgresql_flags() -> (bool, bool, String) {
    let unit_active =
        systemd_unit_active("postgresql") || first_active_service(&[("postgresql", "x")]).is_some();
    let listening = port_open("127.0.0.1:5432", 250);
    let running = unit_active && listening;
    let installed = postgresql_present() || listening;
    let detail = if running {
        "postgresql unit active and :5432 accepting connections.".into()
    } else if installed {
        "PostgreSQL packages or unit present, but not fully running on :5432.".into()
    } else {
        "PostgreSQL not detected. Opt-in only; default stack remains MariaDB + phpMyAdmin.".into()
    };
    (running, installed, detail)
}

pub fn install_postgresql() -> Result<String, String> {
    install_packages_dnf_or_apt(&["postgresql-server", "postgresql"], &["postgresql"])?;
    ensure_postgresql_initialized()?;
    enable_now(&["postgresql"])?;
    Ok("Installed and started PostgreSQL.".to_string())
}

pub fn start_postgresql() -> Result<String, String> {
    ensure_postgresql_initialized()?;
    enable_now(&["postgresql"])?;
    Ok("Started PostgreSQL.".into())
}

pub fn stop_postgresql() -> Result<String, String> {
    stop_units(&["postgresql"])?;
    Ok("Stopped PostgreSQL.".into())
}

pub fn uninstall_postgresql() -> Result<String, String> {
    disable_now(&["postgresql"])?;
    remove_packages_dnf_or_apt(
        &["postgresql-server", "postgresql"],
        &["postgresql", "postgresql-contrib"],
    )?;
    Ok("Uninstalled PostgreSQL.".to_string())
}
