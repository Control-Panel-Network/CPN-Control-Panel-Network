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
    /// Last Test connection result (never stores secrets).
    #[serde(default)]
    pub last_verify_ok: Option<bool>,
    #[serde(default)]
    pub last_verify_at_unix: Option<u64>,
    #[serde(default)]
    pub last_verify_message: Option<String>,
    #[serde(default)]
    pub last_zone_count: Option<u32>,
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
            last_verify_ok: None,
            last_verify_at_unix: None,
            last_verify_message: None,
            last_zone_count: None,
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
    pub last_verify_ok: Option<bool>,
    pub last_verify_at_unix: Option<u64>,
    pub last_verify_message: Option<String>,
    pub last_zone_count: Option<u32>,
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
    let t = sanitize_cloudflare_secret(token);
    if t.is_empty() {
        return String::new();
    }
    if t.len() <= 4 {
        return "****".into();
    }
    format!("****{}", &t[t.len().saturating_sub(4)..])
}

/// Strip BOM, quotes, accidental `Bearer ` prefix, and all whitespace from a secret.
/// Never log the returned value.
pub fn sanitize_cloudflare_secret(raw: &str) -> String {
    let mut t = raw.trim().trim_start_matches('\u{feff}').to_string();
    if (t.starts_with('"') && t.ends_with('"') && t.len() >= 2)
        || (t.starts_with('\'') && t.ends_with('\'') && t.len() >= 2)
    {
        t = t[1..t.len() - 1].trim().to_string();
    }
    if t.len() >= 7 && t[..7].eq_ignore_ascii_case("bearer ") {
        t = t[7..].trim().to_string();
    }
    t.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Cloudflare Global API Keys are 37 hex characters. API Tokens are not.
pub fn looks_like_global_api_key(token: &str) -> bool {
    let t = sanitize_cloudflare_secret(token);
    t.len() == 37 && t.chars().all(|c| c.is_ascii_hexdigit())
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
        last_verify_ok: s.last_verify_ok,
        last_verify_at_unix: s.last_verify_at_unix,
        last_verify_message: s.last_verify_message,
        last_zone_count: s.last_zone_count,
    }
}

/// Persist a Test connection outcome without touching the secret token.
pub fn record_cloudflare_verify(
    ok: bool,
    message: &str,
    zone_count: Option<u32>,
) -> Result<(), String> {
    let mut current = load_cloudflare();
    if current.api_token.trim().is_empty() {
        return Err("Cloudflare API token is not configured".into());
    }
    let msg = message.trim().chars().take(240).collect::<String>();
    current.last_verify_ok = Some(ok);
    current.last_verify_at_unix = Some(now_unix());
    current.last_verify_message = if msg.is_empty() { None } else { Some(msg) };
    current.last_zone_count = zone_count;
    persist_cloudflare(&current)
}

/// Format unix seconds as `dd/mm/yyyy HH:MM` for operator UI.
pub fn format_verify_time(ts: u64) -> String {
    #[cfg(unix)]
    {
        use std::process::Command;
        let output = Command::new("date")
            .args(["-d", &format!("@{ts}"), "+%d/%m/%Y %H:%M"])
            .output();
        if let Ok(out) = output
            && out.status.success()
        {
            let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !text.is_empty() {
                return text;
            }
        }
    }
    format!("{ts} (unix)")
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
    let incoming = sanitize_cloudflare_secret(api_token);
    if !incoming.is_empty() {
        if incoming.chars().any(|c| c.is_control()) {
            return Err("API token cannot include control characters".into());
        }
        current.api_token = incoming;
    } else {
        current.api_token = sanitize_cloudflare_secret(&current.api_token);
    }
    if current.api_token.is_empty() {
        return Err("API token is required".into());
    }
    // Auto-correct common mis-label: Global API Key pasted under API Token.
    if current.auth_type == CloudflareAuthType::ApiToken
        && looks_like_global_api_key(&current.api_token)
    {
        if current.email.trim().is_empty() {
            return Err(
                "That secret looks like a Cloudflare Global API Key (37 hex chars). Switch Authentication type to Global API Key and enter the account email, or paste a scoped API Token instead."
                    .into(),
            );
        }
        current.auth_type = CloudflareAuthType::GlobalKey;
    }
    if current.auth_type == CloudflareAuthType::GlobalKey && current.email.trim().is_empty() {
        return Err("Cloudflare email is required when using a Global API Key".into());
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

/// Validate record content for common types before calling Cloudflare.
pub fn validate_record_content(record_type: &str, content: &str) -> Result<(), String> {
    let content = content.trim();
    if content.is_empty() {
        return Err("Value is required".into());
    }
    match record_type {
        "A" => {
            if content.parse::<std::net::Ipv4Addr>().is_err() {
                return Err(
                    "A records require a valid IPv4 address (for example 192.0.2.1)".into(),
                );
            }
        }
        "AAAA" => {
            if content.parse::<std::net::Ipv4Addr>().is_ok() {
                return Err(
                    "AAAA records require an IPv6 address, not IPv4 (for example 2001:db8::1)"
                        .into(),
                );
            }
            if content.parse::<std::net::Ipv6Addr>().is_err() {
                return Err(
                    "AAAA records require a valid IPv6 address (for example 2001:db8::1)".into(),
                );
            }
        }
        _ => {}
    }
    Ok(())
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
    fn sanitize_strips_bearer_and_whitespace() {
        assert_eq!(
            sanitize_cloudflare_secret("  Bearer abcd\n1234  "),
            "abcd1234"
        );
        assert_eq!(sanitize_cloudflare_secret("\"tok_value\""), "tok_value");
        assert!(looks_like_global_api_key(
            "0123456789abcdef0123456789abcdef0123456"
        ));
        assert!(!looks_like_global_api_key(
            "QAht_not_a_global_key_value_xxxxxx"
        ));
    }

    #[test]
    #[test]
    fn aaaa_rejects_ipv4_and_accepts_ipv6() {
        assert!(validate_record_content("AAAA", "192.168.1.1").is_err());
        assert!(validate_record_content("AAAA", "2001:db8::1").is_ok());
        assert!(validate_record_content("A", "192.168.1.1").is_ok());
        assert!(validate_record_content("A", "2001:db8::1").is_err());
    }

    #[test]
    fn reject_global_key_as_api_token_without_email() {
        with_test_data_dir(|| {
            let err = save_cloudflare_settings(
                "api_token",
                "",
                "0123456789abcdef0123456789abcdef0123456",
                true,
            )
            .unwrap_err();
            assert!(err.to_ascii_lowercase().contains("global api key"));
        });
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
