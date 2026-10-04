//! Postfix as the default local MTA when outbound SMTP is skipped.

use crate::apps_pkg::{enable_now, install_packages_dnf_or_apt, rpm_or_dpkg_installed};
use crate::install_mail_listeners::{append_cpn_panel_outbound, master_cf_has_panel_outbound};
use crate::os_support::{PackageFamily, detect_guest_os};
use crate::service_detect::{port_open, systemd_unit_active};
use crate::smtp_settings::{SmtpSettings, SmtpTlsMode};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

/// True when local Postfix is running (unit active or SMTP ports listening).
pub fn postfix_is_ready() -> bool {
    if systemd_unit_active("postfix") {
        return true;
    }
    port_open("127.0.0.1:25", 250)
        || port_open("127.0.0.1:2525", 250)
        || port_open("127.0.0.1:587", 250)
}

/// Envelope From for unauthenticated local injection.
/// External recipient addresses must not be used as From (relay/SPF rejects).
pub fn postfix_envelope_from(from_hint: &str) -> String {
    let trimmed = from_hint.trim();
    let lower = trimmed.to_ascii_lowercase();
    if trimmed.contains('@')
        && (lower.ends_with("@localhost")
            || lower.ends_with("@localdomain")
            || lower.contains("@localhost."))
    {
        return trimmed.to_string();
    }
    "cpn-panel@localhost".to_string()
}

fn wait_for_panel_outbound_port(attempts: u32) -> bool {
    for _ in 0..attempts {
        if port_open("127.0.0.1:2525", 250) {
            return true;
        }
        thread::sleep(Duration::from_millis(80));
    }
    false
}

fn master_cf_declares_panel_outbound() -> bool {
    let path = Path::new("/etc/postfix/master.cf");
    fs::read_to_string(path)
        .ok()
        .map(|raw| master_cf_has_panel_outbound(&raw))
        .unwrap_or(false)
}

/// Localhost SMTP settings used for the Postfix fallback path.
///
/// Prefers the panel-outbound injector on :2525 (no virtual-mailbox reject).
/// Falls back to smtpd on :25 only when :2525 is not configured. Never uses :587 (SASL required).
pub fn postfix_local_smtp(from_address: &str) -> SmtpSettings {
    let port = if wait_for_panel_outbound_port(20) || master_cf_declares_panel_outbound() {
        2525
    } else {
        25
    };
    SmtpSettings {
        schema_version: 1,
        host: "127.0.0.1".into(),
        port,
        tls_mode: SmtpTlsMode::None,
        from_address: postfix_envelope_from(from_address),
        username: String::new(),
        password: String::new(),
        updated_at_unix: crate::account::now_unix(),
    }
}

/// Install a loopback smtpd that can relay panel Feedback to remote inboxes
/// even when those domains are hosted locally (virtual alias table).
pub fn ensure_postfix_panel_outbound() -> Result<(), String> {
    let path = Path::new("/etc/postfix/master.cf");
    if !path.is_file() {
        return Err("Postfix master.cf is missing".into());
    }
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("Could not read Postfix master.cf: {error}"))?;
    if let Some(updated) = append_cpn_panel_outbound(&raw)
        && updated != raw
    {
        fs::write(path, updated)
            .map_err(|error| format!("Could not update Postfix master.cf: {error}"))?;
        let reload = Command::new("postfix")
            .arg("reload")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if reload.map(|s| !s.success()).unwrap_or(true) {
            let _ = Command::new("systemctl")
                .args(["reload", "postfix"])
                .status();
        }
    }
    let _ = wait_for_panel_outbound_port(25);
    Ok(())
}

/// Install and enable Postfix on Linux when external SMTP was skipped.
///
/// Windows Phase A: returns a clear limitation error (does not break setup callers
/// that treat this as soft-fail).
pub fn ensure_postfix_default(from_address: &str) -> Result<SmtpSettings, String> {
    match detect_guest_os() {
        Ok(guest) if guest.is_windows() => {
            return Err(
                "Postfix fallback is not available on Windows Server Phase A. Configure external SMTP for outbound mail."
                    .into(),
            );
        }
        Ok(guest) if !matches!(guest.family, PackageFamily::Dnf | PackageFamily::Apt) => {
            return Err("Postfix fallback requires a supported Linux guest (dnf/apt).".into());
        }
        Err(error) => {
            #[cfg(windows)]
            {
                let _ = error;
                return Err(
                    "Postfix fallback is not available on Windows Server Phase A. Configure external SMTP for outbound mail."
                        .into(),
                );
            }
            #[cfg(not(windows))]
            {
                return Err(format!(
                    "Could not detect guest OS for Postfix fallback: {error}"
                ));
            }
        }
        Ok(_) => {}
    }

    if !rpm_or_dpkg_installed(&["postfix"]) && !postfix_is_ready() {
        install_packages_dnf_or_apt(&["postfix"], &["postfix"])?;
    }
    if !systemd_unit_active("postfix") {
        enable_now(&["postfix"])?;
    }
    if !postfix_is_ready() {
        return Err(
            "Postfix was installed but is not accepting mail yet. Check the postfix service."
                .into(),
        );
    }
    Ok(postfix_local_smtp(from_address))
}

/// Validate that Postfix local binding is usable for an enabled mailbox.
pub fn require_postfix_smtp_ready() -> Result<(), String> {
    if postfix_is_ready() {
        Ok(())
    } else {
        Err(
            "Local Postfix is not running. Install/enable Postfix (Apps > Email or installer fallback) before enabling accounts on the local MTA."
                .into(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postfix_from_rejects_external_recipients() {
        assert_eq!(
            postfix_envelope_from("info@newstargeted.com"),
            "cpn-panel@localhost"
        );
        assert_eq!(
            postfix_envelope_from("cpn@localhost"),
            "cpn@localhost"
        );
        let settings = postfix_local_smtp("info@newstargeted.com");
        assert_eq!(settings.host, "127.0.0.1");
        assert!(settings.port == 25 || settings.port == 2525);
        assert_eq!(settings.from_address, "cpn-panel@localhost");
        assert!(settings.username.is_empty());
    }
}
