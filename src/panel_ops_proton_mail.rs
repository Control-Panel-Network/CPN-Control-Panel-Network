//! Proton Mail operator settings (external / Bridge). Stored under /var/lib/cpn/.
//!
//! CPN does not host Proton encryption. Passwords are never written here.

use crate::account::data_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const SETTINGS_FILE: &str = "proton-mail.json";
const DEFAULT_OPEN_URL: &str = "https://mail.proton.me";
const DEFAULT_BRIDGE_HOST: &str = "127.0.0.1";
const DEFAULT_IMAP_PORT: u16 = 1143;
const DEFAULT_SMTP_PORT: u16 = 1025;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtonMailSettings {
    #[serde(default)]
    pub account_email: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default = "default_open_enabled")]
    pub open_button_enabled: bool,
    #[serde(default)]
    pub open_button_label: String,
    #[serde(default = "default_open_url")]
    pub open_url: String,
    #[serde(default = "default_bridge_host")]
    pub bridge_host: String,
    #[serde(default = "default_imap_port")]
    pub bridge_imap_port: u16,
    #[serde(default = "default_smtp_port")]
    pub bridge_smtp_port: u16,
    #[serde(default)]
    pub notes: String,
}

fn default_open_enabled() -> bool {
    true
}

fn default_open_url() -> String {
    DEFAULT_OPEN_URL.into()
}

fn default_bridge_host() -> String {
    DEFAULT_BRIDGE_HOST.into()
}

fn default_imap_port() -> u16 {
    DEFAULT_IMAP_PORT
}

fn default_smtp_port() -> u16 {
    DEFAULT_SMTP_PORT
}

impl Default for ProtonMailSettings {
    fn default() -> Self {
        Self {
            account_email: String::new(),
            display_name: String::new(),
            open_button_enabled: true,
            open_button_label: String::new(),
            open_url: default_open_url(),
            bridge_host: default_bridge_host(),
            bridge_imap_port: DEFAULT_IMAP_PORT,
            bridge_smtp_port: DEFAULT_SMTP_PORT,
            notes: String::new(),
        }
    }
}

fn settings_path() -> PathBuf {
    data_dir().join(SETTINGS_FILE)
}

pub fn load_proton_mail_settings() -> ProtonMailSettings {
    let path = settings_path();
    if !path.is_file() {
        return ProtonMailSettings::default();
    }
    match fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => ProtonMailSettings::default(),
    }
}

pub fn save_proton_mail_settings(settings: &ProtonMailSettings) -> Result<(), String> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
    }
    let raw = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Could not serialize Proton Mail settings: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn open_button_label(settings: &ProtonMailSettings) -> String {
    let custom = settings.open_button_label.trim();
    if custom.is_empty() {
        "Open Proton Mail".into()
    } else {
        custom.to_string()
    }
}

pub fn sanitize_open_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return DEFAULT_OPEN_URL.into();
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("https://") || lower.starts_with("http://") {
        trimmed.to_string()
    } else {
        DEFAULT_OPEN_URL.into()
    }
}

pub fn parse_port(raw: &str, fallback: u16) -> u16 {
    raw.trim()
        .parse::<u16>()
        .ok()
        .filter(|p| *p > 0)
        .unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_honest_bridge_ports() {
        let s = ProtonMailSettings::default();
        assert!(s.open_button_enabled);
        assert_eq!(s.open_url, DEFAULT_OPEN_URL);
        assert_eq!(s.bridge_host, DEFAULT_BRIDGE_HOST);
        assert_eq!(s.bridge_imap_port, 1143);
        assert_eq!(s.bridge_smtp_port, 1025);
    }

    #[test]
    fn rejects_non_http_open_url() {
        assert_eq!(sanitize_open_url("javascript:alert(1)"), DEFAULT_OPEN_URL);
        assert_eq!(
            sanitize_open_url("https://mail.proton.me/u/0/inbox"),
            "https://mail.proton.me/u/0/inbox"
        );
    }
}
