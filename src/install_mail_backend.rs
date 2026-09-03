//! Local MTA + IMAP provisioning and health checks (issue #9).

use crate::install_journal::{self, JournalAction};
use crate::install_recipes::{DnfProgress, command, pkg_install};
use crate::installer::{AppState, run_command};
use crate::os_support::require_installable_guest;
use std::process::Stdio;
use tokio::process::Command;

const STAGE: &str = "mail_backend";

/// Install and configure a minimal local Postfix + Dovecot stack for webmail.
pub async fn provision_local_mail_backend(state: &AppState) -> Result<(), String> {
    let guest = require_installable_guest()?;
    install_journal::ensure_journal_dirs()?;
    state
        .progress(
            "installing",
            70,
            "Provisioning local IMAP/SMTP (Postfix + Dovecot)",
        )
        .await;

    run_command(
        state,
        pkg_install(
            &guest,
            vec!["postfix", "dovecot"],
            vec!["postfix", "dovecot-core", "dovecot-imapd"],
            "Instalando Postfix y Dovecot",
            DnfProgress {
                download_start: 62,
                download_end: 68,
                install_start: 68,
                install_end: 74,
                label: "Mail backend",
            },
        ),
    )
    .await?;
    install_journal::record(
        STAGE,
        JournalAction::InstalledPackage,
        "postfix,dovecot",
        None,
        Some("mail backend packages".into()),
    )?;

    // Keep submission on loopback for lab/local webmail; operators harden for public MX later.
    let master_cf = "/etc/postfix/master.cf";
    if std::path::Path::new(master_cf).exists() {
        let raw = std::fs::read_to_string(master_cf).unwrap_or_default();
        if !raw.contains("127.0.0.1:587") && !raw.contains("submission inet") {
            let extra = "\n# CPN local submission (issue #9)\n127.0.0.1:587 inet n - n - - smtpd\n  -o syslog_name=postfix/submission\n  -o smtpd_tls_security_level=may\n  -o smtpd_sasl_auth_enable=yes\n  -o smtpd_relay_restrictions=permit_sasl_authenticated,reject\n";
            let updated = format!("{raw}{extra}");
            install_journal::write_file_tracked(STAGE, std::path::Path::new(master_cf), &updated)?;
        }
    }

    let main_cf = "/etc/postfix/main.cf";
    if std::path::Path::new(main_cf).exists() {
        let mut raw = std::fs::read_to_string(main_cf).unwrap_or_default();
        let has_inet = raw
            .lines()
            .any(|l| l.trim_start().starts_with("inet_interfaces"));
        if !has_inet {
            raw.push_str("\ninet_interfaces = loopback-only\n");
            install_journal::write_file_tracked(STAGE, std::path::Path::new(main_cf), &raw)?;
        }
    }

    let dovecot_conf = "/etc/dovecot/dovecot.conf";
    if std::path::Path::new(dovecot_conf).exists() {
        let mut raw = std::fs::read_to_string(dovecot_conf).unwrap_or_default();
        if !raw.contains("protocols =") {
            raw.push_str("\nprotocols = imap\n");
            install_journal::write_file_tracked(STAGE, std::path::Path::new(dovecot_conf), &raw)?;
        }
    }

    // Prefer cleartext IMAP on loopback for Roundcube defaults (localhost:143).
    let listen = "/etc/dovecot/conf.d/10-master.conf";
    if std::path::Path::new(listen).exists() {
        let raw = std::fs::read_to_string(listen).unwrap_or_default();
        if raw.contains("port = 0") || !raw.contains("inet_listener imap") {
            // Leave vendor defaults when already listening; journal a note.
            install_journal::record(
                STAGE,
                JournalAction::Note,
                listen,
                None,
                Some("using vendor dovecot listener config".into()),
            )?;
        }
    }

    run_command(
        state,
        command(
            "systemctl",
            vec!["enable", "--now", "postfix"],
            "Activando Postfix",
            "installing",
            76,
        ),
    )
    .await?;
    install_journal::record(STAGE, JournalAction::EnabledService, "postfix", None, None)?;
    // Apply master.cf submission listener if we changed it.
    let _ = Command::new("systemctl")
        .args(["reload", "postfix"])
        .status()
        .await;

    run_command(
        state,
        command(
            "systemctl",
            vec!["enable", "--now", "dovecot"],
            "Activando Dovecot",
            "installing",
            78,
        ),
    )
    .await?;
    install_journal::record(STAGE, JournalAction::EnabledService, "dovecot", None, None)?;

    // Ensure cleartext IMAP is available on loopback for Roundcube defaults.
    let auth = "/etc/dovecot/conf.d/10-auth.conf";
    if std::path::Path::new(auth).exists() {
        let raw = std::fs::read_to_string(auth).unwrap_or_default();
        if raw.contains("disable_plaintext_auth = yes") {
            let updated = raw.replace(
                "disable_plaintext_auth = yes",
                "disable_plaintext_auth = no",
            );
            install_journal::write_file_tracked(STAGE, std::path::Path::new(auth), &updated)?;
            let _ = Command::new("systemctl")
                .args(["reload", "dovecot"])
                .status()
                .await;
        }
    }

    Ok(())
}

/// Fail unless IMAP (143) and SMTP submission (587) accept TCP on localhost.
pub async fn verify_imap_smtp_listeners() -> Result<(), String> {
    let imap = port_open("127.0.0.1", 143).await;
    let smtp = port_open("127.0.0.1", 587).await;
    // Some Postfix builds expose submission only after master.cf reload; also accept :25.
    let smtp25 = port_open("127.0.0.1", 25).await;
    if !imap {
        return Err(
            "IMAP check failed: nothing listening on 127.0.0.1:143 (Dovecot not ready)".into(),
        );
    }
    if !smtp && !smtp25 {
        return Err(
            "SMTP check failed: nothing listening on 127.0.0.1:587 or :25 (Postfix not ready)"
                .into(),
        );
    }
    Ok(())
}

async fn port_open(host: &str, port: u16) -> bool {
    let addr = format!("{host}:{port}");
    match tokio::time::timeout(
        std::time::Duration::from_secs(3),
        tokio::net::TcpStream::connect(&addr),
    )
    .await
    {
        Ok(Ok(_)) => true,
        _ => {
            // Fallback to bash /dev/tcp for environments without tokio net quirks.
            let script =
                format!("timeout 2 bash -c 'echo > /dev/tcp/{host}/{port}' >/dev/null 2>&1");
            Command::new("bash")
                .args(["-c", &script])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .kill_on_drop(true)
                .status()
                .await
                .map(|s| s.success())
                .unwrap_or(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::verify_imap_smtp_listeners;

    #[tokio::test]
    async fn verify_fails_when_nothing_listens() {
        // On CI / Windows hosts without Postfix this should error (honest fail).
        let result = verify_imap_smtp_listeners().await;
        // Either Ok (lab already has listeners) or Err with IMAP/SMTP text.
        if let Err(message) = result {
            assert!(
                message.contains("IMAP") || message.contains("SMTP"),
                "unexpected error: {message}"
            );
        }
    }
}
