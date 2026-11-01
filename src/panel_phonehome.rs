//! Phone-home registration with api.newstargeted.com so News Targeted can reach this install.
//!
//! State under `<data dir>/phonehome/` (mode 600):
//! - `install_id` stable UUID
//! - `config.json` enabled flag, optional local HMAC override, API URL, contact email
//!
//! Auth: HMAC-SHA256 over `{ts}\\n{nonce}\\n{sha256(body)}` using `CPN_PHONEHOME_HMAC_SECRET`
//! (env), config `hmac_secret`, build-time `CPN_PHONEHOME_BUILD_SECRET`, or remote-push HMAC.

use crate::account::data_dir;
use crate::panel_notifications_remote::{enable_remote_push_with_hmac, signing_payload};
use crate::panel_public_url::load_panel_public_url;
use hmac::{Hmac, KeyInit as HmacKeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

const DEFAULT_API_URL: &str = "https://api.newstargeted.com/api/cpn/phonehome/register";
/// How often the background worker attempts register when enabled (24h).
pub const PHONEHOME_INTERVAL_SECS: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhonehomeConfig {
    /// When false, never register (operator opt-out).
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Override HMAC (preferred for labs). Empty = resolve from env / build / remote-push.
    #[serde(default)]
    pub hmac_secret: String,
    #[serde(default = "default_api_url")]
    pub api_url: String,
    #[serde(default)]
    pub contact_email: String,
    /// Lab only: ask API to accept http:// public URLs.
    #[serde(default)]
    pub allow_http: bool,
    #[serde(default)]
    pub last_register_at: u64,
    #[serde(default)]
    pub last_register_ok: bool,
    #[serde(default)]
    pub last_error: String,
}

fn default_enabled() -> bool {
    // Official NT builds may bake a registration secret; default on when a secret resolves.
    true
}

fn default_api_url() -> String {
    DEFAULT_API_URL.to_string()
}

impl Default for PhonehomeConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            hmac_secret: String::new(),
            api_url: default_api_url(),
            contact_email: String::new(),
            allow_http: false,
            last_register_at: 0,
            last_register_ok: false,
            last_error: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PhonehomeStatus {
    pub enabled: bool,
    pub install_id: String,
    pub api_url: String,
    pub panel_public_url: String,
    pub has_hmac: bool,
    pub last_register_at: u64,
    pub last_register_ok: bool,
    pub last_error: String,
    pub allow_http: bool,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn phonehome_dir() -> PathBuf {
    data_dir().join("phonehome")
}

fn install_id_path() -> PathBuf {
    phonehome_dir().join("install_id")
}

fn config_path() -> PathBuf {
    phonehome_dir().join("config.json")
}

fn set_private(path: &PathBuf) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

fn ensure_dir() -> Result<(), String> {
    let dir = phonehome_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

pub fn load_phonehome_config() -> PhonehomeConfig {
    let Ok(raw) = fs::read_to_string(config_path()) else {
        return PhonehomeConfig::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save_phonehome_config(cfg: &PhonehomeConfig) -> Result<(), String> {
    ensure_dir()?;
    let path = config_path();
    let raw = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    fs::write(&path, raw).map_err(|e| format!("write {}: {e}", path.display()))?;
    set_private(&path);
    Ok(())
}

pub fn ensure_install_id() -> Result<String, String> {
    ensure_dir()?;
    let path = install_id_path();
    if let Ok(existing) = fs::read_to_string(&path) {
        let id = existing.trim();
        if Uuid::parse_str(id).is_ok() {
            return Ok(id.to_string());
        }
    }
    let id = Uuid::new_v4().to_string();
    let mut f = fs::File::create(&path).map_err(|e| format!("create install_id: {e}"))?;
    f.write_all(id.as_bytes())
        .map_err(|e| format!("write install_id: {e}"))?;
    set_private(&path);
    Ok(id)
}

fn resolve_hmac(cfg: &PhonehomeConfig) -> String {
    let from_cfg = cfg.hmac_secret.trim();
    if from_cfg.len() >= 32 {
        return from_cfg.to_string();
    }
    if let Ok(env) = std::env::var("CPN_PHONEHOME_HMAC_SECRET") {
        let t = env.trim().to_string();
        if t.len() >= 32 {
            return t;
        }
    }
    if let Some(built) = option_env!("CPN_PHONEHOME_BUILD_SECRET") {
        let t = built.trim();
        if t.len() >= 32 {
            return t.to_string();
        }
    }
    let remote = crate::panel_notifications_remote::load_remote_push_config();
    let t = remote.hmac_secret.trim();
    if t.len() >= 32 {
        return t.to_string();
    }
    String::new()
}

fn hmac_hex(secret: &str, payload: &str) -> String {
    let mut mac = <HmacSha256 as HmacKeyInit>::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(payload.as_bytes());
    let result = mac.finalize().into_bytes();
    result.iter().map(|b| format!("{b:02x}")).collect()
}

fn hostname_guess() -> String {
    if let Some(h) = crate::panel_network::load_panel_hostname()
        && !h.trim().is_empty()
    {
        return h;
    }
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_default()
}

pub fn phonehome_status() -> PhonehomeStatus {
    let cfg = load_phonehome_config();
    let install_id = ensure_install_id().unwrap_or_default();
    let public = load_panel_public_url().unwrap_or_default();
    let has_hmac = resolve_hmac(&cfg).len() >= 32;
    PhonehomeStatus {
        enabled: cfg.enabled,
        install_id,
        api_url: cfg.api_url.clone(),
        panel_public_url: public,
        has_hmac,
        last_register_at: cfg.last_register_at,
        last_register_ok: cfg.last_register_ok,
        last_error: cfg.last_error.clone(),
        allow_http: cfg.allow_http,
    }
}

pub fn set_phonehome_enabled(enabled: bool) -> Result<PhonehomeStatus, String> {
    let mut cfg = load_phonehome_config();
    cfg.enabled = enabled;
    if !enabled {
        cfg.last_error = "opted out locally".into();
    }
    save_phonehome_config(&cfg)?;
    Ok(phonehome_status())
}

#[derive(Debug, Deserialize)]
struct RegisterResponse {
    ok: bool,
    #[serde(default)]
    error: String,
    #[serde(default)]
    push_hmac: String,
    #[serde(default)]
    push_hmac_issued: bool,
}

/// Register (or refresh) this install with the News Targeted API.
pub fn register_now() -> Result<PhonehomeStatus, String> {
    let cfg = load_phonehome_config();
    if !cfg.enabled {
        return Err("phone-home is disabled (cpn network phone-home --on to enable)".into());
    }
    register_with_enabled_flag(true)
}

/// Push enabled=false to the API (used after local opt-out when HMAC is available).
pub fn register_disabled_remote() -> Result<PhonehomeStatus, String> {
    register_with_enabled_flag(false)
}

fn register_with_enabled_flag(remote_enabled: bool) -> Result<PhonehomeStatus, String> {
    let mut cfg = load_phonehome_config();
    let secret = resolve_hmac(&cfg);
    if secret.len() < 32 {
        return Err(
            "phone-home HMAC not configured (set CPN_PHONEHOME_HMAC_SECRET, phonehome/config.json hmac_secret, or remote-push.json)"
                .into(),
        );
    }
    let public = load_panel_public_url().unwrap_or_default();
    if public.trim().is_empty() {
        return Err(
            "panel_public_url is not set (cpn network set-public-url --url https://your-host)"
                .into(),
        );
    }
    if public.starts_with("http://") && !cfg.allow_http {
        return Err(
            "panel_public_url must be https (or set allow_http in phonehome/config.json for labs)"
                .into(),
        );
    }

    let install_id = ensure_install_id()?;
    let body = serde_json::json!({
        "install_id": install_id,
        "panel_public_url": public.trim().trim_end_matches('/'),
        "version": env!("CARGO_PKG_VERSION"),
        "hostname": hostname_guess(),
        "contact_email": cfg.contact_email.trim(),
        "enabled": remote_enabled,
        "allow_http": cfg.allow_http,
    });
    let raw = serde_json::to_vec(&body).map_err(|e| e.to_string())?;
    let timestamp = now_unix().to_string();
    let nonce = Uuid::new_v4().as_simple().to_string();
    let payload = signing_payload(&timestamp, &nonce, &raw);
    let signature = hmac_hex(&secret, &payload);

    let api_url = if cfg.api_url.trim().is_empty() {
        DEFAULT_API_URL.to_string()
    } else {
        cfg.api_url.trim().to_string()
    };

    let tmp = std::env::temp_dir().join(format!("cpn-phonehome-{}.json", std::process::id()));
    let _ = fs::remove_file(&tmp);
    fs::write(&tmp, &raw).map_err(|e| format!("temp body: {e}"))?;

    let output = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--location",
            "--max-time",
            "25",
            "-X",
            "POST",
            "-H",
            "Content-Type: application/json; charset=utf-8",
            "-H",
            &format!("X-CPN-Timestamp: {timestamp}"),
            "-H",
            &format!("X-CPN-Nonce: {nonce}"),
            "-H",
            &format!("X-CPN-Signature: {signature}"),
            "-H",
            &format!("User-Agent: CPN-Installer/{}", env!("CARGO_PKG_VERSION")),
            "--data-binary",
            &format!("@{}", tmp.display()),
            &api_url,
        ])
        .output()
        .map_err(|e| format!("curl failed to start: {e}"))?;
    let _ = fs::remove_file(&tmp);

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() && stdout.trim().is_empty() {
        cfg.last_register_at = now_unix();
        cfg.last_register_ok = false;
        cfg.last_error = format!("curl error: {}", stderr.trim());
        let _ = save_phonehome_config(&cfg);
        return Err(cfg.last_error.clone());
    }

    let parsed: RegisterResponse = serde_json::from_str(&stdout).unwrap_or(RegisterResponse {
        ok: false,
        error: if stdout.trim().is_empty() {
            stderr.trim().to_string()
        } else {
            "invalid JSON from API".into()
        },
        push_hmac: String::new(),
        push_hmac_issued: false,
    });

    cfg.last_register_at = now_unix();
    if !parsed.ok {
        cfg.last_register_ok = false;
        cfg.last_error = if parsed.error.is_empty() {
            "register rejected".into()
        } else {
            parsed.error.clone()
        };
        let _ = save_phonehome_config(&cfg);
        return Err(cfg.last_error.clone());
    }

    if remote_enabled
        && (parsed.push_hmac_issued || parsed.push_hmac.trim().len() >= 32)
        && let Err(e) = enable_remote_push_with_hmac(parsed.push_hmac.trim())
    {
        cfg.last_register_ok = false;
        cfg.last_error = format!("registered but remote-push enable failed: {e}");
        let _ = save_phonehome_config(&cfg);
        return Err(cfg.last_error.clone());
    }

    cfg.last_register_ok = true;
    cfg.last_error.clear();
    save_phonehome_config(&cfg)?;
    Ok(phonehome_status())
}

/// Background worker: register on start (after short delay) and daily while enabled.
pub async fn spawn_phonehome_worker() {
    tokio::spawn(async move {
        // Let the panel finish binding before the first call.
        tokio::time::sleep(std::time::Duration::from_secs(45)).await;
        loop {
            let cfg = load_phonehome_config();
            if cfg.enabled {
                match tokio::task::spawn_blocking(register_now).await {
                    Ok(Ok(_)) => {}
                    Ok(Err(err)) => {
                        eprintln!("cpn phone-home: {err}");
                    }
                    Err(err) => {
                        eprintln!("cpn phone-home join error: {err}");
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(PHONEHOME_INTERVAL_SECS)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_enabled() {
        let cfg = PhonehomeConfig::default();
        assert!(cfg.enabled);
        assert!(cfg.api_url.contains("phonehome/register"));
    }
}
