//! Redis host package: detect, install, start, stop, uninstall.

use crate::apps::{AppId, AppStateKind, AppStatus};
use crate::apps_pkg::{
    disable_now, enable_now, install_packages_dnf_or_apt, remove_packages_dnf_or_apt,
    rpm_or_dpkg_installed, stop_units,
};
use crate::service_detect::{port_open, systemd_unit_active};

const UNITS: &[&str] = &["redis", "redis-server"];

pub fn detect_redis() -> AppStatus {
    let running = systemd_unit_active("redis")
        || systemd_unit_active("redis-server")
        || port_open("127.0.0.1:6379", 250);
    let installed = rpm_or_dpkg_installed(&["redis", "redis-server"]) || running;
    let (state, detail) = if running {
        (
            AppStateKind::Running,
            "Redis is running on this host (unit or :6379).".into(),
        )
    } else if installed {
        (
            AppStateKind::Installed,
            "Redis packages are present but the service is not running.".into(),
        )
    } else {
        (
            AppStateKind::NotInstalled,
            "Redis is not installed on this host.".into(),
        )
    };
    AppStatus {
        id: AppId::Redis,
        state,
        detail,
        warning: None,
    }
}

pub fn install_redis() -> Result<String, String> {
    install_packages_dnf_or_apt(&["redis"], &["redis-server"])?;
    let _ = enable_now(&["redis"]);
    let _ = enable_now(&["redis-server"]);
    Ok("Installed and started Redis on the host.".into())
}

pub fn start_redis() -> Result<String, String> {
    let first = enable_now(&["redis"]);
    if first.is_ok() {
        return Ok("Started Redis.".into());
    }
    enable_now(&["redis-server"]).map(|_| "Started Redis.".into())
}

pub fn stop_redis() -> Result<String, String> {
    stop_units(UNITS)?;
    Ok("Stopped Redis.".into())
}

pub fn uninstall_redis() -> Result<String, String> {
    let _ = disable_now(UNITS);
    remove_packages_dnf_or_apt(&["redis"], &["redis-server"])?;
    Ok("Uninstalled Redis from the host.".into())
}
