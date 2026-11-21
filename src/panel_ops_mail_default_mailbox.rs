//! Default first mailbox `info@<apex>` on Host email / mail-domain install.

use crate::account::{default_password_policy, generate_password};
use crate::mail_accounts::{MailAccountInput, MailSmtpMode, create_account, list_accounts};
use crate::panel_ops_mail_onboarding::{
    MailMode, apex_from_panel_hostname, load_mail_onboarding, mail_client_fallback_domains,
};
use crate::paths;
use crate::sites::normalize_domain;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

pub const DEFAULT_MAILBOX_LOCAL_PART: &str = "info";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefaultMailboxResult {
    Created { address: String },
    AlreadyExists { address: String },
    Skipped { reason: String },
}

impl DefaultMailboxResult {
    pub fn summary(&self) -> String {
        match self {
            Self::Created { address } => {
                format!("Created default mailbox `{address}` (password stored in panel registry).")
            }
            Self::AlreadyExists { address } => {
                format!("Default mailbox `{address}` already exists; password left unchanged.")
            }
            Self::Skipped { reason } => format!("Default info@ mailbox skipped: {reason}"),
        }
    }
}

/// True when the operator opted out of hosting a mail domain on this server.
pub fn mail_domain_opted_out() -> bool {
    let cfg = load_mail_onboarding();
    cfg.without_mail_domain || cfg.skip_rdns || matches!(cfg.mail_mode, MailMode::External)
}

fn info_address_for_apex(apex: &str) -> Result<String, String> {
    let apex = normalize_domain(apex)?;
    Ok(format!("{DEFAULT_MAILBOX_LOCAL_PART}@{apex}"))
}

fn mailbox_exists(address: &str) -> bool {
    let needle = address.trim().to_ascii_lowercase();
    list_accounts()
        .iter()
        .any(|a| a.address.eq_ignore_ascii_case(&needle))
}

fn initial_password_path(apex: &str) -> PathBuf {
    let safe = apex
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    paths::join_data(format!("mailbox-info-{safe}.initial.pass"))
}

fn write_initial_password_file(apex: &str, password: &str) -> Result<(), String> {
    let path = initial_password_path(apex);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Cannot create data dir: {e}"))?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut f = options
        .open(&path)
        .map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
    writeln!(f, "{password}").map_err(|e| format!("Cannot save initial mailbox password: {e}"))?;
    Ok(())
}

/// Create `info@<apex>` once with a generated password. Never overwrites an existing mailbox.
pub fn ensure_default_info_mailbox(apex_raw: &str) -> Result<DefaultMailboxResult, String> {
    if mail_domain_opted_out() {
        return Ok(DefaultMailboxResult::Skipped {
            reason: "operator chose install without a mail domain (or external mail)".into(),
        });
    }
    let apex = normalize_domain(apex_raw)?;
    let address = info_address_for_apex(&apex)?;
    if mailbox_exists(&address) {
        return Ok(DefaultMailboxResult::AlreadyExists { address });
    }
    let password = generate_password(&default_password_policy());
    // Prefer enabled when Postfix is up (Host email Start). Otherwise register disabled
    // so unit tests and early install still seed info@ without failing SMTP checks.
    let enabled = crate::postfix_fallback::postfix_is_ready();
    create_account(MailAccountInput {
        address: address.clone(),
        domain: apex.clone(),
        enabled,
        smtp_mode: MailSmtpMode::PostfixLocal,
        smtp_host: String::new(),
        smtp_port: Some(587),
        smtp_tls: None,
        smtp_username: String::new(),
        smtp_password: String::new(),
        mailbox_password: password.clone(),
    })?;
    // Operator-readable once file under the CPN data dir (mode 600). Not committed to git.
    let _ = write_initial_password_file(&apex, &password);
    Ok(DefaultMailboxResult::Created { address })
}

/// Resolve the preferred apex for Host email install and ensure `info@`.
pub fn ensure_default_info_mailbox_for_host_email() -> Result<DefaultMailboxResult, String> {
    if mail_domain_opted_out() {
        return Ok(DefaultMailboxResult::Skipped {
            reason: "operator chose install without a mail domain (or external mail)".into(),
        });
    }
    let cfg = load_mail_onboarding();
    if let Some(apex) = apex_from_panel_hostname(cfg.hostname.trim()) {
        return ensure_default_info_mailbox(&apex);
    }
    let domains = mail_client_fallback_domains();
    let Some(apex) = domains.into_iter().next() else {
        return Ok(DefaultMailboxResult::Skipped {
            reason: "no mail apex domain yet (add a website or set Server hostname)".into(),
        });
    };
    ensure_default_info_mailbox(&apex)
}

/// Ensure default `info@` for every known site/panel apex (Host email heal / Start).
pub fn ensure_default_info_mailboxes_for_known_domains() -> Vec<DefaultMailboxResult> {
    if mail_domain_opted_out() {
        return vec![DefaultMailboxResult::Skipped {
            reason: "operator chose install without a mail domain (or external mail)".into(),
        }];
    }
    let mut out = Vec::new();
    for apex in mail_client_fallback_domains() {
        match ensure_default_info_mailbox(&apex) {
            Ok(r) => out.push(r),
            Err(e) => out.push(DefaultMailboxResult::Skipped {
                reason: format!("`{apex}`: {e}"),
            }),
        }
    }
    if out.is_empty() {
        out.push(DefaultMailboxResult::Skipped {
            reason: "no mail apex domain yet (add a website or set Server hostname)".into(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::panel_ops_mail_onboarding::{MailOnboarding, save_mail_onboarding};

    #[test]
    fn opt_out_skips_create() {
        with_test_data_dir(|| {
            let mut cfg = MailOnboarding::default();
            cfg.without_mail_domain = true;
            cfg.hostname = "mail.example.com".into();
            save_mail_onboarding(cfg).unwrap();
            let r = ensure_default_info_mailbox("example.com").unwrap();
            assert!(matches!(r, DefaultMailboxResult::Skipped { .. }));
            assert!(list_accounts().is_empty());
        });
    }

    #[test]
    fn creates_info_once_and_keeps_password() {
        with_test_data_dir(|| {
            let mut cfg = MailOnboarding::default();
            cfg.hostname = "mail.example.com".into();
            cfg.mail_mode = MailMode::Local;
            save_mail_onboarding(cfg).unwrap();
            let first = ensure_default_info_mailbox("example.com").unwrap();
            assert!(matches!(first, DefaultMailboxResult::Created { .. }));
            let accounts = list_accounts();
            assert_eq!(accounts.len(), 1);
            assert_eq!(accounts[0].address, "info@example.com");
            assert!(!accounts[0].mailbox_password.is_empty());
            let saved = accounts[0].mailbox_password.clone();
            let second = ensure_default_info_mailbox("example.com").unwrap();
            assert!(matches!(second, DefaultMailboxResult::AlreadyExists { .. }));
            let again = list_accounts();
            assert_eq!(again[0].mailbox_password, saved);
        });
    }

    #[test]
    fn skip_rdns_counts_as_opt_out() {
        with_test_data_dir(|| {
            let mut cfg = MailOnboarding::default();
            cfg.skip_rdns = true;
            save_mail_onboarding(cfg).unwrap();
            assert!(mail_domain_opted_out());
        });
    }
}
