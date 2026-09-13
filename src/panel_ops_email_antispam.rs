//! SpamAssassin / Rspamd / MailScanner detect, status, and admin enable/install.

use crate::apps_pkg::{enable_now, install_packages_dnf_or_apt, rpm_or_dpkg_installed};
use crate::panel_ops_security::{cmd_stdout, systemctl_active, which_exists};
use crate::service_detect::systemd_unit_active;

#[derive(Debug, Clone)]
pub struct FilterStatus {
    pub name: &'static str,
    pub installed: bool,
    pub active: bool,
    pub detail: String,
    pub web_ui: Option<String>,
    pub package_hint: &'static str,
}

fn unit_active_any(units: &[&str]) -> bool {
    units.iter().any(|u| systemd_unit_active(u) || systemctl_active(u) == "active")
}

pub fn spamassassin_status() -> FilterStatus {
    let bins = which_exists("spamassassin")
        || which_exists("spamd")
        || which_exists("spamc")
        || path_exists("/etc/mail/spamassassin")
        || path_exists("/etc/spamassassin");
    let pkgs = rpm_or_dpkg_installed(&["spamassassin"]);
    let installed = bins || pkgs;
    let active = unit_active_any(&["spamassassin", "spamd"]);
    let version = cmd_stdout("spamassassin", &["--version"]).unwrap_or_default();
    let detail = if !installed {
        "SpamAssassin is not installed on this host.".into()
    } else if active {
        format!(
            "Installed and active. {}",
            if version.is_empty() {
                String::new()
            } else {
                version.lines().next().unwrap_or("").to_string()
            }
        )
    } else {
        "Installed but service is not active. Use Enable to start spamassassin/spamd.".into()
    };
    FilterStatus {
        name: "SpamAssassin",
        installed,
        active,
        detail,
        web_ui: None,
        package_hint: "spamassassin",
    }
}

pub fn rspamd_status() -> FilterStatus {
    let installed =
        which_exists("rspamd") || rpm_or_dpkg_installed(&["rspamd"]) || path_exists("/etc/rspamd");
    let active = unit_active_any(&["rspamd"]);
    let web = if path_exists("/etc/rspamd") || installed {
        Some("http://127.0.0.1:11334/".into())
    } else {
        None
    };
    let detail = if !installed {
        "Rspamd is not installed on this host.".into()
    } else if active {
        "Installed and active. Worker web UI is typically on port 11334 (localhost).".into()
    } else {
        "Installed but not active. Use Enable to start rspamd.".into()
    };
    FilterStatus {
        name: "Rspamd",
        installed,
        active,
        detail,
        web_ui: web,
        package_hint: "rspamd",
    }
}

pub fn mailscanner_status() -> FilterStatus {
    let installed = which_exists("MailScanner")
        || which_exists("mailscanner")
        || rpm_or_dpkg_installed(&["MailScanner", "mailscanner"])
        || path_exists("/etc/MailScanner");
    let active = unit_active_any(&["mailscanner", "MailScanner"]);
    let detail = if !installed {
        "MailScanner is not available on this host (common on AlmaLinux 9). This page stays LIVE for detection and enable when a package exists."
            .into()
    } else if active {
        "MailScanner is installed and active.".into()
    } else {
        "MailScanner package detected but not active.".into()
    };
    FilterStatus {
        name: "MailScanner",
        installed,
        active,
        detail,
        web_ui: None,
        package_hint: "MailScanner",
    }
}

fn path_exists(path: &str) -> bool {
    std::path::Path::new(path).exists()
}

pub fn enable_spamassassin() -> Result<String, String> {
    let st = spamassassin_status();
    if !st.installed {
        install_packages_dnf_or_apt(&["spamassassin"], &["spamassassin"])?;
    }
    if let Err(e) = enable_now(&["spamassassin"]) {
        enable_now(&["spamd"]).map_err(|_| e)?;
    }
    Ok("SpamAssassin enable requested.".into())
}

pub fn enable_rspamd() -> Result<String, String> {
    let st = rspamd_status();
    if !st.installed {
        install_packages_dnf_or_apt(&["rspamd"], &["rspamd"])?;
    }
    enable_now(&["rspamd"])?;
    Ok("Rspamd enable requested.".into())
}

pub fn enable_mailscanner() -> Result<String, String> {
    let st = mailscanner_status();
    if !st.installed {
        return Err(
            "MailScanner package is not available for install on this OS. Status page remains available."
                .into(),
        );
    }
    if let Err(e1) = enable_now(&["mailscanner"]) {
        enable_now(&["MailScanner"]).map_err(|_| e1)?;
    }
    Ok("MailScanner enable requested.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_structs_construct() {
        let s = spamassassin_status();
        assert_eq!(s.name, "SpamAssassin");
        let r = rspamd_status();
        assert_eq!(r.name, "Rspamd");
        let m = mailscanner_status();
        assert_eq!(m.name, "MailScanner");
    }
}
