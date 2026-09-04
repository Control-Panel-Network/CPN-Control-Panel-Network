//! Cloudflare DNS preferences for CPN.
//! Token/email live under `/var/lib/cpn/cloudflare.json` (mode 600). Never log secrets.

use crate::paths;
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CloudflareAuthType {
    #[default]
    ApiToken,
    GlobalKey,
}

impl CloudflareAuthType {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "global_key" | "global" | "email_key" => Self::GlobalKey,
            _ => Self::ApiToken,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ApiToken => "api_token",
            Self::GlobalKey => "global_key",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudflareSettings {
    pub schema_version: u32,
    pub auth_type: CloudflareAuthType,
    pub email: String,
    /// API token or Global API Key. Never expose in public APIs or logs.
    pub api_token: String,
    pub sync_local: bool,
    pub updated_at_unix: u64,
}

impl Default for CloudflareSettings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            auth_type: CloudflareAuthType::ApiToken,
            email: String::new(),
            api_token: String::new(),
            sync_local: true,
            updated_at_unix: 0,
        }
    }
}

/// Safe summary for UI (token masked).
#[derive(Debug, Clone, Serialize)]
pub struct CloudflarePublic {
    pub configured: bool,
    pub auth_type: String,
    pub email: String,
    pub token_masked: String,
    pub sync_local: bool,
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|v| v.as_secs())
        .unwrap_or(0)
}

pub fn cloudflare_settings_path() -> PathBuf {
    paths::join_data("cloudflare.json")
}

pub fn mask_token(token: &str) -> String {
    let t = token.trim();
    if t.is_empty() {
        return String::new();
    }
    if t.len() <= 4 {
        return "****".into();
    }
    format!("****{}", &t[t.len().saturating_sub(4)..])
}

pub fn load_cloudflare() -> CloudflareSettings {
    let Ok(raw) = fs::read_to_string(cloudflare_settings_path()) else {
        return CloudflareSettings::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn cloudflare_public() -> CloudflarePublic {
    let s = load_cloudflare();
    let configured = !s.api_token.trim().is_empty();
    CloudflarePublic {
        configured,
        auth_type: s.auth_type.as_str().to_string(),
        email: s.email,
        token_masked: if configured {
            mask_token(&s.api_token)
        } else {
            String::new()
        },
        sync_local: s.sync_local,
    }
}

pub fn cloudflare_configured() -> bool {
    !load_cloudflare().api_token.trim().is_empty()
}

pub fn persist_cloudflare(settings: &CloudflareSettings) -> Result<(), String> {
    let dir = paths::default_data_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Could not serialize Cloudflare settings: {e}"))?;
    let path = cloudflare_settings_path();
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&path)
        .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    file.write_all(json.as_bytes())
        .map_err(|e| format!("Could not save Cloudflare settings: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Save API settings. Empty token keeps the existing secret.
pub fn save_cloudflare_settings(
    auth_type: &str,
    email: &str,
    api_token: &str,
    sync_local: bool,
) -> Result<String, String> {
    let mut current = load_cloudflare();
    current.auth_type = CloudflareAuthType::parse(auth_type);
    current.email = email.trim().to_string();
    let token = api_token.trim();
    if !token.is_empty() {
        if token.chars().any(|c| c.is_control()) {
            return Err("API token cannot include control characters".into());
        }
        current.api_token = token.to_string();
    }
    if current.api_token.trim().is_empty() {
        return Err("API token is required".into());
    }
    if current.auth_type == CloudflareAuthType::GlobalKey && current.email.trim().is_empty() {
        return Err("Cloudflare email is required when using a Global API Key".into());
    }
    if current.auth_type == CloudflareAuthType::ApiToken {
        // Email is optional for token auth.
    }
    current.sync_local = sync_local;
    current.schema_version = 1;
    current.updated_at_unix = now_unix();
    persist_cloudflare(&current)?;
    Ok("Cloudflare API configuration saved".into())
}

pub const RECORD_TYPES: &[&str] = &[
    "A", "AAAA", "CNAME", "MX", "TXT", "SPF", "NS", "SOA", "SRV", "CAA",
];

pub fn normalize_record_type(raw: &str) -> Result<String, String> {
    let t = raw.trim().to_ascii_uppercase();
    if RECORD_TYPES.iter().any(|x| *x == t) {
        Ok(t)
    } else {
        Err(format!("Unsupported DNS record type '{raw}'"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn mask_hides_token_body() {
        assert_eq!(mask_token(""), "");
        assert_eq!(mask_token("abcd"), "****");
        assert_eq!(mask_token("cfut_abcdefghijklmnop"), "****mnop");
    }

    #[test]
    fn save_and_load_roundtrip_masks_in_public() {
        with_test_data_dir(|| {
            save_cloudflare_settings(
                "api_token",
                "ops@example.com",
                "tok_secret_value_9999",
                true,
            )
            .unwrap();
            let pubv = cloudflare_public();
            assert!(pubv.configured);
            assert_eq!(pubv.email, "ops@example.com");
            assert_eq!(pubv.token_masked, "****9999");
            assert!(!pubv.token_masked.contains("secret"));
            let loaded = load_cloudflare();
            assert_eq!(loaded.api_token, "tok_secret_value_9999");
            // Empty token keeps previous
            save_cloudflare_settings("api_token", "ops@example.com", "", false).unwrap();
            assert_eq!(load_cloudflare().api_token, "tok_secret_value_9999");
            assert!(!load_cloudflare().sync_local);
        });
    }

    #[test]
    fn path_under_data_dir() {
        with_test_data_dir(|| {
            let p = cloudflare_settings_path();
            assert!(p.to_string_lossy().contains("cloudflare.json"));
        });
    }

    #[test]
    fn record_type_normalize() {
        assert_eq!(normalize_record_type("a").unwrap(), "A");
        assert!(normalize_record_type("bogus").is_err());
    }
}
