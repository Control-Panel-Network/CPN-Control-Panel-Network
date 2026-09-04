//! Outbound SMTP settings for panel setup and password reset.
//! Secrets live only under the CPN data directory with restricted permissions.

use crate::paths;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SmtpTlsMode {
    #[default]
    Starttls,
    Tls,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpSettings {
    pub schema_version: u32,
    pub host: String,
    pub port: u16,
    pub tls_mode: SmtpTlsMode,
    pub from_address: String,
    pub username: String,
    /// Stored only on disk; never returned from public APIs.
    pub password: String,
    pub updated_at_unix: u64,
}

/// Safe summary for `/api/status` (no passwords or SMTP usernames).
#[derive(Debug, Clone, Serialize)]
pub struct SmtpPublic {
    pub configured: bool,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub tls_mode: Option<SmtpTlsMode>,
    pub from_address: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SmtpSetupInput {
    pub host: String,
    pub port: Option<u16>,
    pub tls_mode: Option<SmtpTlsMode>,
    pub from_address: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

pub fn smtp_path() -> PathBuf {
    paths::join_data("smtp.json")
}

pub fn load_smtp() -> Option<SmtpSettings> {
    let raw = fs::read_to_string(smtp_path()).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn smtp_public_from_disk() -> SmtpPublic {
    match load_smtp() {
        Some(settings)
            if !settings.host.trim().is_empty() && !settings.from_address.trim().is_empty() =>
        {
            SmtpPublic {
                configured: true,
                host: Some(settings.host),
                port: Some(settings.port),
                tls_mode: Some(settings.tls_mode),
                from_address: Some(settings.from_address),
            }
        }
        _ => SmtpPublic {
            configured: false,
            host: None,
            port: None,
            tls_mode: None,
            from_address: None,
        },
    }
}

pub fn validate_smtp_input(input: &SmtpSetupInput) -> Result<SmtpSettings, String> {
    let host = input.host.trim().to_string();
    if host.is_empty() {
        return Err("SMTP host is required when configuring outbound mail".into());
    }
    if host.chars().any(|ch| ch.is_control()) {
        return Err("SMTP host cannot include control characters".into());
    }
    let from_address = input.from_address.trim().to_string();
    if from_address.is_empty() || !from_address.contains('@') {
        return Err("SMTP from address must be a valid email".into());
    }
    let port = input.port.unwrap_or(587);
    if port == 0 {
        return Err("SMTP port must be between 1 and 65535".into());
    }
    let tls_mode = input.tls_mode.unwrap_or_default();
    let username = input.username.as_deref().unwrap_or("").trim().to_string();
    let password = input.password.clone().unwrap_or_default();
    Ok(SmtpSettings {
        schema_version: 1,
        host,
        port,
        tls_mode,
        from_address,
        username,
        password,
        updated_at_unix: now_unix(),
    })
}

pub fn persist_smtp(settings: &SmtpSettings) -> Result<(), String> {
    let dir = paths::default_data_dir();
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Could not create {}: {error}", dir.display()))?;
    let json = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("Could not serialize SMTP settings: {error}"))?;
    let path = smtp_path();
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write;
    let mut file = options
        .open(&path)
        .map_err(|error| format!("Could not write {}: {error}", path.display()))?;
    file.write_all(json.as_bytes())
        .map_err(|error| format!("Could not save SMTP settings: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Match a forgot-password identifier against username or recovery email.
pub fn identifier_matches_account(username: &str, recovery_email: &str, identifier: &str) -> bool {
    let id = identifier.trim();
    if id.is_empty() {
        return false;
    }
    if username == id || username.eq_ignore_ascii_case(id) {
        return true;
    }
    !recovery_email.is_empty() && recovery_email.eq_ignore_ascii_case(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_username_or_email() {
        assert!(identifier_matches_account(
            "Admin",
            "ops@example.com",
            "admin"
        ));
        assert!(identifier_matches_account(
            "Admin",
            "ops@example.com",
            "OPS@example.com"
        ));
        assert!(!identifier_matches_account(
            "Admin",
            "ops@example.com",
            "other"
        ));
    }
}
