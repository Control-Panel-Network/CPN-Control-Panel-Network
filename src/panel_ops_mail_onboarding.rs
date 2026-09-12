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

pub fn mail_client_server_host(domain: &str) -> String {
    let cfg = load_mail_onboarding();
    match cfg.mail_mode {
        MailMode::Local => {
            if !cfg.hostname.trim().is_empty() {
                let host = cfg.hostname.trim().to_ascii_lowercase();
                let domain = domain.trim().to_ascii_lowercase();
                if host == domain || host.ends_with(&format!(".{domain}")) {
                    return host;
                }
            }
            domain.trim().to_ascii_lowercase()
        }
        MailMode::External => {
            if !cfg.external_imap_host.trim().is_empty() {
                cfg.external_imap_host.trim().to_ascii_lowercase()
            } else {
                domain.trim().to_ascii_lowercase()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

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
