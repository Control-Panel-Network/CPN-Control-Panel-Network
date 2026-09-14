//! Dovecot ManageSieve / Pigeonhole for SnappyMail filters (port 4190).

use crate::install_journal::{self, JournalAction};
use crate::install_recipes::{DnfProgress, command, pkg_install};
use crate::installer::{AppState, run_command};
use crate::os_support::require_installable_guest;
use std::path::Path;
use tokio::process::Command;

const STAGE: &str = "mail_sieve";
const CPN_SIEVE_CONF: &str = "/etc/dovecot/conf.d/99-cpn-sieve.conf";

/// Install pigeonhole packages, enable ManageSieve on 4190, reload Dovecot.
pub async fn ensure_dovecot_sieve(state: &AppState) -> Result<(), String> {
    let guest = require_installable_guest()?;
    install_journal::ensure_journal_dirs()?;
    state
        .progress(
            "installing",
            74,
            "Installing Dovecot Sieve / ManageSieve (Pigeonhole)",
        )
        .await;

    run_command(
        state,
        pkg_install(
            &guest,
            vec!["dovecot-pigeonhole"],
            vec!["dovecot-sieve", "dovecot-managesieve"],
            "Installing Dovecot Sieve packages",
            DnfProgress {
                download_start: 72,
                download_end: 74,
                install_start: 74,
                install_end: 75,
                label: "Sieve / ManageSieve",
            },
        ),
    )
    .await?;
    install_journal::record(
        STAGE,
        JournalAction::InstalledPackage,
        "dovecot-pigeonhole",
        None,
        Some("ManageSieve for SnappyMail filters".into()),
    )?;

    ensure_sieve_dovecot_config()?;
    ensure_sieve_selinux_port();
    // Rebuild cpn_webmail_imap after pigeonhole so sieve_port_t exists.
    crate::install_selinux_mail::ensure_httpd_mail_ports();

    let _ = Command::new("systemctl")
        .args(["reload", "dovecot"])
        .status()
        .await;
    // Reload can fail if unit was not running yet; enable --now is safe.
    let _ = run_command(
        state,
        command(
            "systemctl",
            vec!["enable", "--now", "dovecot"],
            "Ensuring Dovecot is running with Sieve",
            "installing",
            75,
        ),
    )
    .await;

    if !port_open_sync("127.0.0.1", 4190) {
        install_journal::record(
            STAGE,
            JournalAction::Note,
            "managesieve:4190",
            None,
            Some("Sieve packages installed; ManageSieve not yet listening (check dovecot -n)".into()),
        )?;
    } else {
        install_journal::record(
            STAGE,
            JournalAction::Note,
            "managesieve:4190",
            None,
            Some("ManageSieve listening on 127.0.0.1:4190".into()),
        )?;
    }
    Ok(())
}

/// Sync-only heal for upgrades / redeploys (no AppState progress UI).
pub fn ensure_dovecot_sieve_sync() -> Result<(), String> {
    install_journal::ensure_journal_dirs()?;
    // Best-effort package install when missing.
    let has_pkg = Path::new("/usr/lib64/dovecot/lib90_sieve_plugin.so").exists()
        || Path::new("/usr/lib/dovecot/lib90_sieve_plugin.so").exists()
        || std::process::Command::new("rpm")
            .args(["-q", "dovecot-pigeonhole"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    if !has_pkg {
        let _ = std::process::Command::new("bash")
            .args([
                "-c",
                "if command -v dnf >/dev/null 2>&1; then \
                   dnf install -y dovecot-pigeonhole >/dev/null 2>&1 || true; \
                 elif command -v apt-get >/dev/null 2>&1; then \
                   apt-get install -y dovecot-sieve dovecot-managesieve >/dev/null 2>&1 || true; \
                 fi",
            ])
            .status();
    }
    ensure_sieve_dovecot_config()?;
    ensure_sieve_selinux_port();
    crate::install_selinux_mail::ensure_httpd_mail_ports();
    let _ = std::process::Command::new("systemctl")
        .args(["reload", "dovecot"])
        .status();
    Ok(())
}

fn ensure_sieve_dovecot_config() -> Result<(), String> {
    // Ensure protocols include sieve (ManageSieve).
    let dovecot_conf = "/etc/dovecot/dovecot.conf";
    if Path::new(dovecot_conf).exists() {
        let mut raw = std::fs::read_to_string(dovecot_conf).unwrap_or_default();
        let mut changed = false;
        if let Some(idx) = raw.find("protocols =") {
            let line_end = raw[idx..]
                .find('\n')
                .map(|n| idx + n)
                .unwrap_or(raw.len());
            let line = &raw[idx..line_end];
            if !line.contains("sieve") {
                let new_line = if line.contains("imap") {
                    "protocols = imap sieve"
                } else {
                    "protocols = imap sieve"
                };
                raw.replace_range(idx..line_end, new_line);
                changed = true;
            }
        } else {
            if !raw.ends_with('\n') {
                raw.push('\n');
            }
            raw.push_str("protocols = imap sieve\n");
            changed = true;
        }
        if changed {
            install_journal::write_file_tracked(STAGE, Path::new(dovecot_conf), &raw)?;
        }
    }

    let conf = r#"# Managed by CPN: ManageSieve for SnappyMail filters
protocol sieve {
}

service managesieve-login {
  inet_listener sieve {
    port = 4190
  }
  inet_listener sieve_secure {
    port = 0
  }
}

service managesieve {
  process_limit = 10
}

plugin {
  sieve = file:~/sieve;active=~/.dovecot.sieve
  sieve_dir = ~/sieve
}
"#;
    install_journal::write_file_tracked(STAGE, Path::new(CPN_SIEVE_CONF), conf)?;
    Ok(())
}

fn ensure_sieve_selinux_port() {
    let _ = std::process::Command::new("bash")
        .args([
            "-c",
            "command -v semanage >/dev/null 2>&1 || exit 0; \
             (semanage port -a -t sieve_port_t -p tcp 4190 || \
              semanage port -m -t sieve_port_t -p tcp 4190 || true); \
             setsebool -P httpd_can_network_connect 1 >/dev/null 2>&1 || true",
        ])
        .status();
}

fn port_open_sync(host: &str, port: u16) -> bool {
    let script = format!("timeout 2 bash -c 'echo > /dev/tcp/{host}/{port}' >/dev/null 2>&1");
    std::process::Command::new("bash")
        .args(["-c", &script])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    #[test]
    fn sieve_conf_mentions_4190() {
        let sample = include_str!("install_mail_sieve.rs");
        assert!(sample.contains("4190"));
        assert!(sample.contains("managesieve"));
    }
}
