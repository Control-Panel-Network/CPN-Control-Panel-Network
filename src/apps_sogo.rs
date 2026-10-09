//! SOGo groupware host package (webmail + CalDAV/CardDAV): detect, install, heal, start, stop,
//! uninstall. Packages come from `apps_sogo_repo`, configuration from `apps_sogo_config`, and
//! the panel reverse-proxies `/SOGo` to the loopback `sogod` listener (`panel_sogo_proxy`).

use crate::apps::{AppId, AppStateKind, AppStatus};
use crate::apps_pkg::{disable_now, enable_now, stop_units};
use crate::apps_sogo_config::{
    SOGO_CONF, SOGO_LOOPBACK, conf_is_cpn_managed, ensure_database, load_or_create_secret,
    secret_exists, sync_users, write_sogo_conf,
};
use crate::apps_sogo_repo::{ensure_packages, remove_packages, sogod_binary_present};
use crate::os_support::{GuestOs, PackageFamily, detect_guest_os};
use crate::service_detect::{port_open, systemd_unit_active, systemd_unit_file_exists};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Candidate unit names (Inverse RPM: `sogod`; Debian/Ubuntu: `sogo`).
pub const SOGO_UNITS: &[&str] = &["sogod", "sogo"];

/// systemd unit that runs sogod on this host.
pub fn sogo_unit() -> &'static str {
    for unit in SOGO_UNITS {
        if systemd_unit_file_exists(unit) {
            return unit;
        }
    }
    match detect_guest_os().map(|g| g.family) {
        Ok(PackageFamily::Apt) => "sogo",
        _ => "sogod",
    }
}

/// True when SOGo packages are on disk (binary or systemd unit).
pub fn sogo_installed() -> bool {
    sogod_binary_present() || SOGO_UNITS.iter().any(|u| systemd_unit_file_exists(u))
}

/// True when sogod is active and the loopback listener answers.
pub fn sogo_running() -> bool {
    sogo_installed()
        && (systemd_unit_active(sogo_unit()) || port_open(SOGO_LOOPBACK, 250))
        && port_open(SOGO_LOOPBACK, 250)
}

pub fn detect_sogo() -> AppStatus {
    let id = AppId::Sogo;
    if !sogo_installed() {
        return AppStatus {
            id,
            state: AppStateKind::NotInstalled,
            detail: "SOGo is not installed on this host. Install provisions Inverse/distro packages, MariaDB storage, memcached, and the /SOGo panel proxy.".into(),
            warning: platform_warning(),
        };
    }
    let unit = sogo_unit();
    let active = systemd_unit_active(unit);
    let listening = port_open(SOGO_LOOPBACK, 250);
    let managed = conf_is_cpn_managed();
    let (state, detail) = if active && listening {
        (
            AppStateKind::Running,
            format!(
                "sogod ({unit}) active; loopback {SOGO_LOOPBACK} answering. Webmail at /SOGo/, CalDAV/CardDAV at /SOGo/dav/.{}",
                if managed {
                    ""
                } else {
                    " Configuration is operator-managed (not the CPN template)."
                }
            ),
        )
    } else if active {
        (
            AppStateKind::Installed,
            format!(
                "sogod ({unit}) is active but {SOGO_LOOPBACK} is not answering yet. Use Start to heal."
            ),
        )
    } else {
        (
            AppStateKind::Installed,
            format!("SOGo packages present but {unit} is not running. Use Start."),
        )
    };
    AppStatus {
        id,
        state,
        detail,
        warning: None,
    }
}

/// Honest note for guests without an upstream SOGo package source.
fn platform_warning() -> Option<String> {
    let guest = detect_guest_os().ok()?;
    match guest.family {
        PackageFamily::Dnf if !matches!(guest.major, 8 | 9) => {
            Some(crate::apps_sogo_repo::unsupported_message(&guest))
        }
        PackageFamily::Windows => Some(crate::os_support::windows_linux_recipe_blocked_message(
            "SOGo host package",
        )),
        _ => None,
    }
}

fn require_dependencies() -> Result<(), String> {
    if !crate::panel_ops_db::local_mariadb_ready() {
        return Err(
            "SOGo stores calendars, contacts, and profiles in MariaDB. Install and start the MariaDB host package first, then retry."
                .into(),
        );
    }
    if !port_open("127.0.0.1:143", 400) {
        return Err(
            "SOGo authenticates mailboxes over local IMAP (Dovecot :143). Install or Start the Email host package (Postfix + Dovecot) first, then retry."
                .into(),
        );
    }
    Ok(())
}

fn installable_guest() -> Result<GuestOs, String> {
    let guest = detect_guest_os()?;
    if guest.is_windows() {
        return Err(crate::os_support::windows_linux_recipe_blocked_message(
            "SOGo host package",
        ));
    }
    Ok(guest)
}

fn wait_for_loopback(timeout: Duration) -> bool {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if port_open(SOGO_LOOPBACK, 300) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    port_open(SOGO_LOOPBACK, 300)
}

fn loopback_http_ok() -> bool {
    let out = Command::new("curl")
        .args([
            "--silent",
            "--output",
            "/dev/null",
            "--max-time",
            "10",
            "--write-out",
            "%{http_code}",
            &format!("http://{SOGO_LOOPBACK}/SOGo/"),
        ])
        .stdin(Stdio::null())
        .output()
        .ok();
    out.map(|o| {
        let code = String::from_utf8_lossy(&o.stdout).trim().to_string();
        matches!(code.as_str(), "200" | "302" | "303")
    })
    .unwrap_or(false)
}

fn apply_config_and_start() -> Result<String, String> {
    let secret = load_or_create_secret()?;
    ensure_database(&secret)?;
    write_sogo_conf(&secret)?;
    let synced = sync_users(&secret).unwrap_or(0);
    let _ = enable_now(&["memcached"]);
    let unit = sogo_unit();
    if !systemd_unit_file_exists(unit) {
        return Err(format!(
            "SOGo unit {unit}.service is missing after package install. Check the package manager log on the host."
        ));
    }
    let _ = Command::new("systemctl")
        .args(["daemon-reload"])
        .stdin(Stdio::null())
        .status();
    enable_now(&[unit])?;
    let _ = Command::new("systemctl")
        .args(["restart", unit])
        .stdin(Stdio::null())
        .status();
    if !wait_for_loopback(Duration::from_secs(25)) {
        return Err(format!(
            "sogod started but {SOGO_LOOPBACK} is not answering. Check: journalctl -u {unit} -n 40 and /var/log/sogo/sogo.log (MariaDB grants and {SOGO_CONF})."
        ));
    }
    let http = if loopback_http_ok() {
        "HTTP OK"
    } else {
        "HTTP not ready yet (first request warms the cache)"
    };
    Ok(format!(
        "sogod ({unit}) running on {SOGO_LOOPBACK} ({http}); {synced} mailbox login(s) synced to SOGo. Webmail: /SOGo/ ; CalDAV/CardDAV: /SOGo/dav/<user>/ ; config {SOGO_CONF}; secrets {}.",
        crate::apps_sogo_repo::SOGO_STATE_DIR
    ))
}

/// Install (or heal) SOGo: packages, MariaDB storage, config, user sync, services.
pub fn install_sogo() -> Result<String, String> {
    let guest = installable_guest()?;
    require_dependencies()?;
    let pkg_msg = if sogo_installed() {
        "SOGo packages already present; re-applied CPN configuration.".to_string()
    } else {
        ensure_packages(&guest)?
    };
    if !sogo_installed() {
        return Err(
            "Package install reported success but sogod is missing. Check the package manager log on the host."
                .into(),
        );
    }
    let run_msg = apply_config_and_start()?;
    crate::panel_feature_gate::invalidate_feature_cache();
    Ok(format!("{pkg_msg} {run_msg}"))
}

pub fn start_sogo() -> Result<String, String> {
    if !sogo_installed() {
        return Err("SOGo is not installed. Use Install first.".into());
    }
    require_dependencies()?;
    if !secret_exists() || !conf_is_cpn_managed() {
        // Operator-managed config: only start units, never overwrite their sogo.conf.
        let _ = enable_now(&["memcached"]);
        enable_now(&[sogo_unit()])?;
        if !wait_for_loopback(Duration::from_secs(20)) {
            return Err(format!(
                "sogod started but {SOGO_LOOPBACK} is not answering. Check journalctl -u {} -n 40.",
                sogo_unit()
            ));
        }
        return Ok("Started SOGo (operator-managed configuration kept).".into());
    }
    apply_config_and_start().map(|m| format!("Started SOGo. {m}"))
}

pub fn stop_sogo() -> Result<String, String> {
    stop_units(SOGO_UNITS)?;
    Ok("Stopped SOGo (sogod). memcached and MariaDB stay running.".into())
}

/// Remove SOGo packages and units. The MariaDB database and `/var/lib/cpn/sogo` secrets are
/// kept so a later reinstall restores calendars and contacts.
pub fn uninstall_sogo() -> Result<String, String> {
    if !sogo_installed() {
        return Err("SOGo is not installed on this host.".into());
    }
    let guest = installable_guest()?;
    let _ = disable_now(SOGO_UNITS);
    remove_packages(&guest)?;
    let _ = std::fs::remove_file(SOGO_CONF);
    crate::panel_feature_gate::invalidate_feature_cache();
    Ok(
        "Uninstalled SOGo (packages, sogod unit, /SOGo panel proxy). MariaDB database cpn_sogo and /var/lib/cpn/sogo were kept for reinstall; drop them from MariaDB Manager if you want a clean slate."
            .into(),
    )
}

/// Resync mailbox logins when SOGo is installed (proxy first-hit and mailbox hooks).
pub fn heal_sogo_users() {
    crate::apps_sogo_config::sync_users_if_installed();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_candidates_cover_rpm_and_deb() {
        assert!(SOGO_UNITS.contains(&"sogod"));
        assert!(SOGO_UNITS.contains(&"sogo"));
        assert!(SOGO_UNITS.contains(&sogo_unit()));
    }

    #[test]
    fn detect_returns_status_for_this_host() {
        let st = detect_sogo();
        assert_eq!(st.id, AppId::Sogo);
        assert!(!st.detail.is_empty());
    }
}
