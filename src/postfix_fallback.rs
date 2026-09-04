//! Postfix as the default local MTA when outbound SMTP is skipped.

use crate::apps_pkg::{enable_now, install_packages_dnf_or_apt, rpm_or_dpkg_installed};
use crate::os_support::{PackageFamily, detect_guest_os};
use crate::service_detect::{port_open, systemd_unit_active};
use crate::smtp_settings::{SmtpSettings, SmtpTlsMode};

/// True when local Postfix is running (unit active or SMTP ports listening).
pub fn postfix_is_ready() -> bool {
    if systemd_unit_active("postfix") {
        return true;
    }
    port_open("127.0.0.1:25", 250) || port_open("127.0.0.1:587", 250)
}

/// Localhost SMTP settings used for the Postfix fallback path.
pub fn postfix_local_smtp(from_address: &str) -> SmtpSettings {
    let from = if from_address.trim().is_empty() || !from_address.contains('@') {
        "cpn-panel@localhost".to_string()
    } else {
        from_address.trim().to_string()
    };
    let (host, port) = if port_open("127.0.0.1:587", 250) {
        ("127.0.0.1".into(), 587)
    } else {
        ("127.0.0.1".into(), 25)
    };
    SmtpSettings {
        schema_version: 1,
        host,
        port,
        tls_mode: SmtpTlsMode::None,
        from_address: from,
        username: String::new(),
        password: String::new(),
        updated_at_unix: crate::account::now_unix(),
    }
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
