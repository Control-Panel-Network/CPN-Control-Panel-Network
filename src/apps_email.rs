//! Postfix + Dovecot host package heal, start, and fast health probes.

use crate::apps_pkg::{
    enable_now, install_packages_dnf_or_apt, package_manager, rpm_or_dpkg_all_installed,
};
use crate::install_mail_backend::apply_local_mail_configuration;
use crate::service_detect::{port_open, systemd_unit_active, systemd_unit_file_exists};

const DNF_MAIL_PKGS: &[&str] = &["postfix", "dovecot"];
const APT_MAIL_PKGS: &[&str] = &["postfix", "dovecot-core", "dovecot-imapd"];

/// True when every mail backend package for the host PM is present.
pub fn email_packages_installed() -> bool {
    if package_manager().is_ok_and(|pm| pm == "dnf") {
        rpm_or_dpkg_all_installed(DNF_MAIL_PKGS)
    } else {
        rpm_or_dpkg_all_installed(APT_MAIL_PKGS)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailStackPublic {
    pub postfix_running: bool,
    pub dovecot_running: bool,
    pub imap_listening: bool,
    pub packages_ok: bool,
    pub detail: String,
}

/// Fast local checks only (no outbound network, no IMAP auth).
pub fn detect_mail_stack_public() -> MailStackPublic {
    let packages_ok = email_packages_installed();
    let postfix_running = systemd_unit_active("postfix") || port_open("127.0.0.1:25", 250);
    let dovecot_running = systemd_unit_active("dovecot");
    let imap_listening = port_open("127.0.0.1:143", 250);
    let detail = if postfix_running && dovecot_running && imap_listening {
        "Postfix and Dovecot are active; IMAP :143 accepts connections.".into()
    } else if !packages_ok {
        "Mail packages are missing or incomplete. Install the Email host package or use Start to heal.".into()
    } else if postfix_running && !dovecot_running {
        "Postfix is running but Dovecot is not active. Use Start on the Email host package.".into()
    } else if !postfix_running && dovecot_running {
        "Dovecot is running but Postfix is not active. Use Start on the Email host package.".into()
    } else if postfix_running && dovecot_running && !imap_listening {
        "Postfix and Dovecot units are up but IMAP :143 is not listening yet.".into()
    } else {
        "Mail stack is partially present. Use Start on the Email host package.".into()
    };
    MailStackPublic {
        postfix_running,
        dovecot_running,
        imap_listening,
        packages_ok,
        detail,
    }
}

pub fn mail_backend_ready_fast() -> bool {
    let st = detect_mail_stack_public();
    st.postfix_running && st.dovecot_running && st.imap_listening
}

/// Install missing packages, apply CPN mail config, enable postfix then dovecot.
pub fn start_email_stack() -> Result<String, String> {
    if !email_packages_installed() {
        install_packages_dnf_or_apt(DNF_MAIL_PKGS, APT_MAIL_PKGS)?;
    }
    apply_local_mail_configuration()?;
    let _ = crate::install_mail_sieve::ensure_dovecot_sieve_sync();
    let _ = crate::panel_ops_mailbox_folders::ensure_dovecot_system_mailboxes_conf();
    crate::install_selinux_mail::ensure_httpd_mail_ports();
    if !systemd_unit_file_exists("postfix") {
        return Err(
            "Postfix unit is missing after package install. Check dnf/yum logs on the host.".into(),
        );
    }
    enable_now(&["postfix"])?;
    if !systemd_unit_file_exists("dovecot") {
        return Err(
            "Dovecot unit is missing. On RHEL/AlmaLinux install the dovecot package, then retry Start."
                .into(),
        );
    }
    enable_now(&["dovecot"])?;
    let _ = std::process::Command::new("systemctl")
        .args(["reload", "postfix"])
        .status();
    let _ = std::process::Command::new("timeout")
        .args(["8", "systemctl", "reload", "dovecot"])
        .status();
    if !mail_backend_ready_fast() {
        let st = detect_mail_stack_public();
        return Err(format!(
            "Email stack started but health check failed: {}",
            st.detail
        ));
    }
    // Open the mail ports on the host firewall when firewalld is present (no-op otherwise).
    let _ = crate::panel_ops_firewall_heal::heal_firewalld("email stack start");
    let smtp587 = port_open("127.0.0.1:587", 250);
    let smtp465 = port_open("127.0.0.1:465", 250);
    Ok(format!(
        "Started Email stack (Postfix + Dovecot). IMAP :143 ready; submission :587 {}; SMTPS :465 {}.",
        if smtp587 {
            "listening"
        } else {
            "not listening yet"
        },
        if smtp465 {
            "listening"
        } else {
            "not listening yet"
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::detect_mail_stack_public;

    #[test]
    fn detect_mail_stack_returns_detail() {
        let st = detect_mail_stack_public();
        assert!(!st.detail.is_empty());
    }
}
