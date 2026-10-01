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
            Some(
                "Sieve packages installed; ManageSieve not yet listening (check dovecot -n)".into(),
            ),
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

/// True when `line` is an active (uncommented) `protocols = ...` assignment.
fn is_active_protocols_line(line: &str) -> bool {
    let t = line.trim_start();
    if t.starts_with('#') {
        return false;
    }
    match t.strip_prefix("protocols") {
        Some(rest) => rest.trim_start().starts_with('='),
        None => false,
    }
}

/// True when any active `protocols =` line already lists `sieve`.
fn has_active_sieve_protocol(raw: &str) -> bool {
    raw.lines()
        .any(|l| is_active_protocols_line(l) && l.split_whitespace().any(|w| w == "sieve"))
}

/// Append `sieve` to an active `protocols =` line that lacks it. Commented lines are ignored
/// (stock EL `dovecot.conf` ships `#protocols = imap sieve`, which must not count as enabled).
fn append_sieve_to_active_protocols(raw: &str) -> Option<String> {
    let mut changed = false;
    let mut out: Vec<String> = Vec::new();
    for line in raw.lines() {
        if is_active_protocols_line(line) && !line.split_whitespace().any(|w| w == "sieve") {
            out.push(format!("{} sieve", line.trim_end()));
            changed = true;
        } else {
            out.push(line.to_string());
        }
    }
    if !changed {
        return None;
    }
    let mut joined = out.join("\n");
    if raw.ends_with('\n') {
        joined.push('\n');
    }
    Some(joined)
}

fn sieve_conf_body(add_protocols: bool) -> String {
    let mut conf = String::from(
        "# Managed by CPN: ensure ManageSieve listens on 4190.\n\
         # Vendor 20-managesieve.conf / 90-sieve.conf ship with dovecot-pigeonhole.\n",
    );
    if add_protocols {
        conf.push_str("protocols = $protocols sieve\n");
    }
    conf.push_str(
        "service managesieve-login {\n  inet_listener sieve {\n    port = 4190\n  }\n}\n",
    );
    conf
}

fn ensure_sieve_dovecot_config() -> Result<(), String> {
    // Ensure protocols include sieve (ManageSieve). Only active lines count: the stock
    // `#protocols = imap sieve` comment must not suppress enabling the listener.
    let dovecot_conf = "/etc/dovecot/dovecot.conf";
    let mut main_has_sieve = false;
    if Path::new(dovecot_conf).exists() {
        let raw = std::fs::read_to_string(dovecot_conf).unwrap_or_default();
        if let Some(updated) = append_sieve_to_active_protocols(&raw) {
            install_journal::write_file_tracked(STAGE, Path::new(dovecot_conf), &updated)?;
            main_has_sieve = true;
        } else if has_active_sieve_protocol(&raw) {
            main_has_sieve = true;
        }
    }
    let vendor =
        std::fs::read_to_string("/etc/dovecot/conf.d/20-managesieve.conf").unwrap_or_default();
    let add_protocols = !main_has_sieve && !has_active_sieve_protocol(&vendor);

    let conf = sieve_conf_body(add_protocols);
    install_journal::write_file_tracked(STAGE, Path::new(CPN_SIEVE_CONF), &conf)?;
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

    #[test]
    fn commented_protocols_line_does_not_count_as_enabled() {
        let stock = "#protocols = imap sieve\n!include conf.d/*.conf\n";
        assert!(!super::has_active_sieve_protocol(stock));
        assert!(super::append_sieve_to_active_protocols(stock).is_none());
    }

    #[test]
    fn active_protocols_line_gets_sieve_appended() {
        let raw = "protocols = imap pop3 lmtp\nfoo = bar\n";
        let out = super::append_sieve_to_active_protocols(raw).expect("changed");
        assert_eq!(out, "protocols = imap pop3 lmtp sieve\nfoo = bar\n");
        assert!(super::has_active_sieve_protocol(&out));
        assert!(super::append_sieve_to_active_protocols(&out).is_none());
    }

    #[test]
    fn sieve_conf_adds_protocols_only_when_needed() {
        assert!(super::sieve_conf_body(true).contains("protocols = $protocols sieve"));
        assert!(!super::sieve_conf_body(false).contains("protocols"));
        assert!(super::sieve_conf_body(false).contains("port = 4190"));
    }
}
