//! Panel reachability modes when a hostname reverse proxy is configured.
//!
//! - **Domain-only** (default after HTTPS proxy): bind `127.0.0.1:<port>` (HTTP).
//!   Public entry is `https://hostname` via the web server on 443.
//! - **Domain + IP backup**: also bind `0.0.0.0:<port>` (HTTP) so operators can
//!   reach `http://SERVER_IP:<port>/login` if DNS/proxy fails.
//!
//! Never advertise `https://IP:<port>`: the listen port is plain HTTP.

use crate::panel_host_info::host_sidebar_info;
use crate::panel_network::load_panel_hostname;
use crate::panel_ops_firewall::{ensure_panel_port_open, panel_listen_port};
use crate::panel_service::{
    load_allow_remote_preference, save_allow_remote_preference, schedule_detached_panel_restart,
    set_allow_remote_dropin,
};
use crate::panel_public_url::local_listen_base_url;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelAccessMode {
    /// Loopback listen only; public HTTPS via hostname reverse proxy.
    DomainOnly,
    /// Public HTTPS hostname plus HTTP IP:port backup on all interfaces.
    DomainPlusIpBackup,
}

impl PanelAccessMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DomainOnly => "domain_only",
            Self::DomainPlusIpBackup => "domain_plus_ip_backup",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::DomainOnly => "Domain-only (loopback + HTTPS hostname)",
            Self::DomainPlusIpBackup => "Domain + IP backup (HTTP on IP:port)",
        }
    }

    pub fn ip_backup_enabled(self) -> bool {
        matches!(self, Self::DomainPlusIpBackup)
    }
}

pub fn load_panel_access_mode() -> PanelAccessMode {
    if load_allow_remote_preference() {
        PanelAccessMode::DomainPlusIpBackup
    } else {
        PanelAccessMode::DomainOnly
    }
}

/// HTTP backup URL using the detected public IPv4 when IP backup is on.
/// Always `http://`, never `https://IP:port`.
pub fn ip_backup_http_login_url(listen_port: u16) -> Option<String> {
    if !load_allow_remote_preference() {
        return None;
    }
    let ip = host_sidebar_info().ip;
    let ip = ip.trim();
    if ip.is_empty() || ip.eq_ignore_ascii_case("Unavailable") {
        return Some(format!(
            "http://SERVER_IP:{listen_port}/login (detect IP failed)"
        ));
    }
    if ip.parse::<std::net::IpAddr>().is_err() {
        return Some(format!("http://{ip}:{listen_port}/login"));
    }
    Some(format!("http://{ip}:{listen_port}/login"))
}

fn try_close_panel_firewall_port() -> Result<(), String> {
    let port = panel_listen_port();
    let port_arg = format!("{port}/tcp");
    let remove = format!("--remove-port={port_arg}");
    // Best-effort: firewalld may be absent or the port may not be listed.
    let _ = crate::panel_ops_firewall::firewall_cmd_timed(&["--permanent", &remove]);
    let _ = crate::panel_ops_firewall::firewall_cmd_timed(&[&remove]);
    Ok(())
}

/// Persist access mode, systemd drop-in, and firewall port for the panel listen port.
/// Schedules a panel restart so the new bind address takes effect.
pub fn set_panel_access_mode(mode: PanelAccessMode, restart: bool) -> Result<Vec<String>, String> {
    let enabled = mode.ip_backup_enabled();
    save_allow_remote_preference(enabled)?;
    set_allow_remote_dropin(enabled)?;
    let mut notes = vec![format!("access_mode={}", mode.as_str())];
    if enabled {
        match ensure_panel_port_open() {
            Ok(()) => notes.push(format!(
                "Firewall: panel port {} /tcp left open for HTTP IP backup",
                panel_listen_port()
            )),
            Err(err) => notes.push(format!("Firewall open skipped: {err}")),
        }
        notes.push(format!(
            "IP backup is HTTP only: http://SERVER_IP:{}/login (never https://IP:{})",
            panel_listen_port(),
            panel_listen_port()
        ));
        if let Some(url) = ip_backup_http_login_url(panel_listen_port()) {
            notes.push(format!("Backup login: {url}"));
        }
    } else {
        if load_panel_hostname().is_some() {
            let _ = try_close_panel_firewall_port();
            notes.push(
                "Firewall: panel listen port closed when possible (hostname HTTPS remains on 443)"
                    .into(),
            );
        }
        notes.push(format!(
            "Domain-only: panel listens on {} (loopback HTTP)",
            local_listen_base_url(panel_listen_port())
        ));
    }
    if restart {
        match schedule_detached_panel_restart("access-mode") {
            Ok(()) => notes.push(
                "Panel restart scheduled so the new bind address applies".into(),
            ),
            Err(err) => notes.push(format!(
                "Restart manually: systemctl restart cpn-installer.service ({err})"
            )),
        }
    }
    Ok(notes)
}

/// After a successful hostname reverse proxy, default to domain-only unless owner kept IP backup.
pub fn apply_domain_only_after_https_proxy(keep_ip_backup: bool) -> Vec<String> {
    if keep_ip_backup {
        return vec![
            "Kept Domain + IP backup (0.0.0.0 bind) per operator request".into(),
        ];
    }
    match set_panel_access_mode(PanelAccessMode::DomainOnly, true) {
        Ok(notes) => {
            let mut out = vec![
                "Default after HTTPS proxy: Domain-only (loopback). Enable IP backup later with: cpn network set-ip-backup --on".into(),
            ];
            out.extend(notes);
            out
        }
        Err(err) => vec![format!("Could not switch to Domain-only: {err}")],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::panel_service::save_allow_remote_preference;

    #[test]
    fn mode_follows_allow_remote_preference() {
        with_test_data_dir(|| {
            save_allow_remote_preference(false).unwrap();
            assert_eq!(load_panel_access_mode(), PanelAccessMode::DomainOnly);
            save_allow_remote_preference(true).unwrap();
            assert_eq!(
                load_panel_access_mode(),
                PanelAccessMode::DomainPlusIpBackup
            );
            save_allow_remote_preference(false).unwrap();
        });
    }

    #[test]
    fn ip_backup_url_is_http_never_https() {
        with_test_data_dir(|| {
            save_allow_remote_preference(false).unwrap();
            assert!(ip_backup_http_login_url(2087).is_none());
            save_allow_remote_preference(true).unwrap();
            let url = ip_backup_http_login_url(2087).unwrap();
            assert!(url.starts_with("http://"));
            assert!(!url.starts_with("https://"));
            assert!(url.contains(":2087/login"));
            save_allow_remote_preference(false).unwrap();
        });
    }

    #[test]
    fn labels_have_no_em_dash() {
        assert!(!PanelAccessMode::DomainOnly.label().contains('\u{2014}'));
        assert!(!PanelAccessMode::DomainPlusIpBackup.label().contains('\u{2014}'));
    }
}
