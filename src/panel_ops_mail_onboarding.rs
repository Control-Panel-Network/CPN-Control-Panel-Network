//! Mail onboarding: hostname / rDNS guidance and local vs external mail.

use crate::paths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MailMode {
    #[default]
    Local,
    External,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailOnboarding {
    pub schema_version: u32,
    pub hostname: String,
    pub skip_rdns: bool,
    /// When true, skip mail-domain DNS and the default `info@` mailbox on install.
    #[serde(default)]
    pub without_mail_domain: bool,
    pub mail_mode: MailMode,
    #[serde(default)]
    pub external_imap_host: String,
    #[serde(default)]
    pub external_smtp_host: String,
    #[serde(default)]
    pub updated_at_unix: u64,
}

impl Default for MailOnboarding {
    fn default() -> Self {
        Self {
            schema_version: 1,
            hostname: String::new(),
            skip_rdns: false,
            without_mail_domain: false,
            mail_mode: MailMode::Local,
            external_imap_host: String::new(),
            external_smtp_host: String::new(),
            updated_at_unix: 0,
        }
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|v| v.as_secs())
        .unwrap_or(0)
}

pub fn mail_onboarding_path() -> PathBuf {
    paths::join_data("mail-onboarding.json")
}

pub fn load_mail_onboarding() -> MailOnboarding {
    let Ok(raw) = fs::read_to_string(mail_onboarding_path()) else {
        return MailOnboarding::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save_mail_onboarding(mut cfg: MailOnboarding) -> Result<(), String> {
    cfg.schema_version = 1;
    cfg.updated_at_unix = now_unix();
    let dir = paths::default_data_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create data dir: {e}"))?;
    let json = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    let path = mail_onboarding_path();
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
    f.write_all(json.as_bytes())
        .map_err(|e| format!("Cannot save mail onboarding: {e}"))?;
    Ok(())
}

/// Placeholder when no hosted mailbox domain, site, or panel hostname exists yet.
pub const MAIL_CLIENT_PLACEHOLDER_DOMAIN: &str = "your-domain.example";

const PANEL_HOST_PREFIXES: &[&str] = &[
    "panel.", "cpn.", "mail.", "webmail.", "smtp.", "imap.", "mx.",
];

fn host_matches_mailbox_domain(host: &str, domain: &str) -> bool {
    let host = host.trim().to_ascii_lowercase();
    let domain = domain.trim().to_ascii_lowercase();
    if domain.is_empty() {
        return false;
    }
    host == domain || host.ends_with(&format!(".{domain}"))
}

fn valid_display_domain(domain: &str) -> bool {
    let domain = domain.trim().to_ascii_lowercase();
    !domain.is_empty()
        && domain != MAIL_CLIENT_PLACEHOLDER_DOMAIN
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains("..")
        && domain.len() <= 253
        && domain
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
}

/// Strip common panel/mail service labels (`panel.`, `mail.`, …) to the site apex.
pub fn apex_from_panel_hostname(hostname: &str) -> Option<String> {
    let host = hostname.trim().to_ascii_lowercase();
    if !valid_display_domain(&host) {
        return None;
    }
    for prefix in PANEL_HOST_PREFIXES {
        if let Some(rest) = host.strip_prefix(prefix)
            && valid_display_domain(rest)
        {
            return Some(rest.to_string());
        }
    }
    Some(host)
}

/// CPN mail DNS / client convention: `mail.<apex>` (matches MX plans in `panel_ops_mail_dns`).
pub fn preferred_mail_service_hostname(domain: &str) -> String {
    let domain = domain.trim().to_ascii_lowercase();
    if !valid_display_domain(&domain) || domain == MAIL_CLIENT_PLACEHOLDER_DOMAIN {
        return MAIL_CLIENT_PLACEHOLDER_DOMAIN.into();
    }
    if domain.starts_with("mail.") {
        return domain;
    }
    format!("mail.{domain}")
}

fn push_unique_domain(out: &mut Vec<String>, domain: &str) {
    let domain = domain.trim().to_ascii_lowercase();
    if !valid_display_domain(&domain) {
        return;
    }
    if !out.iter().any(|d| d == &domain) {
        out.push(domain);
    }
}

/// Apex domains inferred from panel hostname, website inventory, and mail onboarding.
pub fn mail_client_fallback_domains() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Ok(sites) = crate::sites::list_sites() {
        for site in sites {
            if site.enabled {
                push_unique_domain(&mut out, &site.domain);
            }
        }
    }
    if let Some(panel_host) = crate::panel_network::load_panel_hostname()
        && let Some(apex) = apex_from_panel_hostname(&panel_host)
    {
        push_unique_domain(&mut out, &apex);
    }
    let cfg = load_mail_onboarding();
    if let Some(apex) = apex_from_panel_hostname(&cfg.hostname) {
        push_unique_domain(&mut out, &apex);
    }
    out
}

/// Prefer mailbox domains; otherwise site/panel/onboarding apex defaults (never invent secrets).
pub fn mail_client_display_domains(mailbox_addresses: &[String]) -> Vec<String> {
    let from_mailboxes =
        mail_client_domains_from_addresses(mailbox_addresses.iter().map(String::as_str));
    if !from_mailboxes.is_empty() {
        return from_mailboxes;
    }
    mail_client_fallback_domains()
}

/// Seed local mail onboarding hostname once as `mail.<apex>` (Host email / webmail install).
pub fn seed_mail_onboarding_hostname_if_empty() -> Result<bool, String> {
    let mut cfg = load_mail_onboarding();
    if !cfg.hostname.trim().is_empty() {
        return Ok(false);
    }
    let domains = mail_client_fallback_domains();
    let Some(apex) = domains.into_iter().next() else {
        return Ok(false);
    };
    cfg.hostname = preferred_mail_service_hostname(&apex);
    cfg.mail_mode = MailMode::Local;
    save_mail_onboarding(cfg)?;
    Ok(true)
}

pub fn mail_client_server_host(domain: &str) -> String {
    let domain = domain.trim().to_ascii_lowercase();
    if domain.is_empty() || domain == MAIL_CLIENT_PLACEHOLDER_DOMAIN {
        return MAIL_CLIENT_PLACEHOLDER_DOMAIN.into();
    }
    let cfg = load_mail_onboarding();
    match cfg.mail_mode {
        MailMode::Local => {
            if !cfg.hostname.trim().is_empty() {
                let host = cfg.hostname.trim().to_ascii_lowercase();
                if host_matches_mailbox_domain(&host, &domain) {
                    // Bare apex in onboarding still displays as mail.<apex> (MX convention).
                    if host == domain {
                        return preferred_mail_service_hostname(&domain);
                    }
                    return host;
                }
            }
            preferred_mail_service_hostname(&domain)
        }
        MailMode::External => {
            if !cfg.external_imap_host.trim().is_empty() {
                let ext = cfg.external_imap_host.trim().to_ascii_lowercase();
                if host_matches_mailbox_domain(&ext, &domain) {
                    return ext;
                }
            }
            preferred_mail_service_hostname(&domain)
        }
    }
}

/// Distinct mailbox domains for client settings (enabled accounts first, then all).
pub fn mail_client_domains_from_addresses<'a, I>(addresses: I) -> Vec<String>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut out: Vec<String> = Vec::new();
    for addr in addresses {
        let Some(domain) = addr.split('@').nth(1) else {
            continue;
        };
        push_unique_domain(&mut out, domain);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::panel_network::{clear_panel_hostname, save_panel_hostname};

    #[test]
    fn mail_client_host_ignores_unrelated_onboarding_hostname() {
        with_test_data_dir(|| {
            let mut cfg = MailOnboarding::default();
            cfg.hostname = "newstargeted.com".into();
            save_mail_onboarding(cfg).unwrap();
            // Unrelated onboarding must not leak; fall back to mail.<mailbox-domain>.
            assert_eq!(
                mail_client_server_host("discord-bot-network.com"),
                "mail.discord-bot-network.com"
            );
        });
    }

    #[test]
    fn mail_client_domains_dedupe_addresses() {
        let list =
            mail_client_domains_from_addresses(["a@example.com", "b@example.com", "c@other.test"]);
        assert_eq!(list, vec!["example.com", "other.test"]);
    }

    #[test]
    fn apex_strips_panel_prefix() {
        assert_eq!(
            apex_from_panel_hostname("panel.threebrickstudio.com").as_deref(),
            Some("threebrickstudio.com")
        );
        assert_eq!(
            apex_from_panel_hostname("mail.example.com").as_deref(),
            Some("example.com")
        );
        assert_eq!(
            apex_from_panel_hostname("shop.example.com").as_deref(),
            Some("shop.example.com")
        );
    }

    #[test]
    fn display_domains_prefer_mailboxes_then_panel_apex() {
        with_test_data_dir(|| {
            clear_panel_hostname().unwrap();
            save_panel_hostname("panel.example.com").unwrap();
            let from_mail =
                mail_client_display_domains(&["info@site.test".into(), "ops@site.test".into()]);
            assert_eq!(from_mail, vec!["site.test".to_string()]);
            let fallback = mail_client_display_domains(&[]);
            assert!(
                fallback.iter().any(|d| d == "example.com"),
                "expected panel apex in {fallback:?}"
            );
            clear_panel_hostname().unwrap();
        });
    }

    #[test]
    fn seed_onboarding_fills_empty_hostname_once() {
        with_test_data_dir(|| {
            clear_panel_hostname().unwrap();
            save_panel_hostname("panel.example.com").unwrap();
            assert!(seed_mail_onboarding_hostname_if_empty().unwrap());
            assert_eq!(load_mail_onboarding().hostname, "mail.example.com");
            assert!(!seed_mail_onboarding_hostname_if_empty().unwrap());
            clear_panel_hostname().unwrap();
        });
    }

    #[test]
    fn client_server_prefers_mail_subdomain() {
        with_test_data_dir(|| {
            clear_panel_hostname().unwrap();
            // No onboarding hostname: still show mail.<apex>, not bare apex / placeholder.
            assert_eq!(
                mail_client_server_host("threebrickstudio.com"),
                "mail.threebrickstudio.com"
            );
            let mut cfg = MailOnboarding::default();
            cfg.hostname = "threebrickstudio.com".into();
            save_mail_onboarding(cfg).unwrap();
            assert_eq!(
                mail_client_server_host("threebrickstudio.com"),
                "mail.threebrickstudio.com"
            );
            let mut cfg = load_mail_onboarding();
            cfg.hostname = "mail.threebrickstudio.com".into();
            save_mail_onboarding(cfg).unwrap();
            assert_eq!(
                mail_client_server_host("threebrickstudio.com"),
                "mail.threebrickstudio.com"
            );
            let mut cfg = load_mail_onboarding();
            cfg.hostname = "mx.threebrickstudio.com".into();
            save_mail_onboarding(cfg).unwrap();
            assert_eq!(
                mail_client_server_host("threebrickstudio.com"),
                "mx.threebrickstudio.com"
            );
        });
    }

    #[test]
    fn roundtrip_onboarding() {
        with_test_data_dir(|| {
            let mut cfg = MailOnboarding::default();
            cfg.hostname = "mail.example.com".into();
            save_mail_onboarding(cfg).unwrap();
            assert_eq!(load_mail_onboarding().hostname, "mail.example.com");
        });
    }
}
