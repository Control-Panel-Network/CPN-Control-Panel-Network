//! Cloudflare OAuth for DNS link (not panel login). PKCE flow via dash.cloudflare.com.
//! Client/link JSON stores under `/var/lib/cpn/` (mode 600). Never log secrets.

use crate::account::now_unix;
use crate::panel_ops_cloudflare::{
    CloudflareAuthType, load_cloudflare, mask_token, persist_cloudflare, sanitize_cloudflare_secret,
};
use crate::panel_public_url::{load_panel_public_url, local_listen_base_url};
use crate::paths;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::PathBuf, process::Command};

pub const CF_OAUTH_AUTH_URL: &str = "https://dash.cloudflare.com/oauth2/auth";
pub const CF_OAUTH_TOKEN_URL: &str = "https://dash.cloudflare.com/oauth2/token";
pub const CF_OAUTH_REVOKE_URL: &str = "https://dash.cloudflare.com/oauth2/revoke";
pub const DEFAULT_SCOPES: &str = "zone.read dns.write offline_access";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CloudflareOauthClient {
    pub schema_version: u32,
    pub client_id: String,
    pub client_secret: String,
    pub updated_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CloudflareOauthLink {
    pub schema_version: u32,
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_at_unix: Option<u64>,
    pub scopes: String,
    pub connected_at_unix: u64,
    pub updated_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct OauthPending {
    pub state: String,
    pub code_verifier: String,
    pub created_at_unix: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CloudflareOauthPublic {
    pub client_configured: bool,
    pub client_id: String,
    pub client_secret_masked: String,
    pub linked: bool,
    pub scopes: String,
    pub expires_at_unix: Option<u64>,
    pub connected_at_unix: Option<u64>,
    pub redirect_uri: String,
}

fn oauth_client_path() -> PathBuf {
    paths::join_data("cloudflare-oauth-client.json")
}

fn oauth_link_path() -> PathBuf {
    paths::join_data("cloudflare-oauth-link.json")
}

fn oauth_pending_path() -> PathBuf {
    paths::join_data("cloudflare-oauth-pending.json")
}

fn write_mode_600(path: &PathBuf, contents: &[u8]) -> Result<(), String> {
    let dir = paths::default_data_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    file.write_all(contents)
        .map_err(|e| format!("Could not save {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de> + Default>(path: &PathBuf) -> T {
    let Ok(raw) = fs::read_to_string(path) else {
        return T::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn persist_json<T: Serialize>(path: &PathBuf, value: &T) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| format!("Could not serialize {}: {e}", path.display()))?;
    write_mode_600(path, json.as_bytes())
}

/// Migration 0002 hook: ensure OAuth JSON stores exist.
pub fn ensure_oauth_stores_migrated() -> Result<(), String> {
    if !oauth_client_path().exists() {
        persist_json(&oauth_client_path(), &CloudflareOauthClient::default())?;
    }
    if !oauth_link_path().exists() {
        persist_json(&oauth_link_path(), &CloudflareOauthLink::default())?;
    }
    Ok(())
}

pub fn load_oauth_client() -> CloudflareOauthClient {
    read_json(&oauth_client_path())
}

pub fn load_oauth_link() -> CloudflareOauthLink {
    read_json(&oauth_link_path())
}

pub fn oauth_redirect_uri(listen_port: u16) -> String {
    if let Some(base) = load_panel_public_url() {
        return format!("{base}/dns/cloudflare/oauth/callback");
    }
    format!(
        "{}/dns/cloudflare/oauth/callback",
        local_listen_base_url(listen_port)
    )
}

pub fn oauth_public(listen_port: u16) -> CloudflareOauthPublic {
    let client = load_oauth_client();
    let link = load_oauth_link();
    let client_configured =
        !client.client_id.trim().is_empty() && !client.client_secret.trim().is_empty();
    let linked = !link.access_token.trim().is_empty();
    CloudflareOauthPublic {
        client_configured,
        client_id: client.client_id.clone(),
        client_secret_masked: if client.client_secret.is_empty() {
            String::new()
        } else {
            mask_token(&client.client_secret)
        },
        linked,
        scopes: if link.scopes.is_empty() {
            DEFAULT_SCOPES.into()
        } else {
            link.scopes.clone()
        },
        expires_at_unix: link.expires_at_unix,
        connected_at_unix: if linked {
            Some(link.connected_at_unix)
        } else {
            None
        },
        redirect_uri: oauth_redirect_uri(listen_port),
    }
}

pub fn save_oauth_client(client_id: &str, client_secret: &str) -> Result<String, String> {
    ensure_oauth_stores_migrated()?;
    let client_id = client_id.trim();
    if client_id.is_empty() {
        return Err("OAuth client id is required".into());
    }
    let mut current = load_oauth_client();
    current.client_id = client_id.to_string();
    let incoming = sanitize_cloudflare_secret(client_secret);
    if !incoming.is_empty() {
        current.client_secret = incoming;
    } else {
        current.client_secret = sanitize_cloudflare_secret(&current.client_secret);
    }
    if current.client_secret.is_empty() {
        return Err("OAuth client secret is required".into());
    }
    current.schema_version = 1;
    current.updated_at_unix = now_unix();
    persist_json(&oauth_client_path(), &current)?;
    Ok("Cloudflare OAuth client saved".into())
}

fn base64url_no_pad(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn pkce_pair() -> (String, String) {
    let mut rng = rand::rng();
    let verifier_bytes: Vec<u8> = (0..32).map(|_| rng.random()).collect();
    let verifier = base64url_no_pad(&verifier_bytes);
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = base64url_no_pad(&hasher.finalize());
    (verifier, challenge)
}

fn random_state() -> String {
    let mut rng = rand::rng();
    let bytes: Vec<u8> = (0..16).map(|_| rng.random()).collect();
    base64url_no_pad(&bytes)
}

fn urlencode(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}

/// Start OAuth connect; returns redirect URL for the browser.
pub fn begin_oauth_connect(listen_port: u16) -> Result<String, String> {
    ensure_oauth_stores_migrated()?;
    let client = load_oauth_client();
    if client.client_id.trim().is_empty() || client.client_secret.trim().is_empty() {
        return Err("Save Cloudflare OAuth client id and secret first".into());
    }
    let (verifier, challenge) = pkce_pair();
    let state = random_state();
    let pending = OauthPending {
        state: state.clone(),
        code_verifier: verifier,
        created_at_unix: now_unix(),
    };
    persist_json(&oauth_pending_path(), &pending)?;
    let redirect_uri = oauth_redirect_uri(listen_port);
    let url = format!(
        "{CF_OAUTH_AUTH_URL}?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        urlencode(&client.client_id),
        urlencode(&redirect_uri),
        urlencode(DEFAULT_SCOPES),
        urlencode(&state),
        urlencode(&challenge),
    );
    Ok(url)
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    token_type: Option<String>,
    expires_in: Option<u64>,
    scope: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

fn curl_form_post(url: &str, fields: &[(&str, &str)]) -> Result<String, String> {
    let mut cmd = Command::new("curl");
    cmd.args([
        "--fail-with-body",
        "--silent",
        "--show-error",
        "--max-time",
        "60",
        "-X",
        "POST",
        url,
    ]);
    for (key, value) in fields {
        cmd.args(["--data-urlencode", &format!("{key}={value}")]);
    }
    let output = cmd
        .output()
        .map_err(|e| format!("Could not call Cloudflare OAuth (curl missing?): {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Cloudflare OAuth request failed: {}",
            stderr.trim().chars().take(200).collect::<String>()
        ));
    }
    Ok(stdout)
}

fn sync_access_token_to_cloudflare(access_token: &str, scopes: &str) -> Result<(), String> {
    let mut settings = load_cloudflare();
    settings.auth_type = CloudflareAuthType::Oauth;
    settings.api_token = sanitize_cloudflare_secret(access_token);
    settings.schema_version = 2;
    settings.updated_at_unix = now_unix();
    settings.last_verify_ok = None;
    settings.last_verify_message = Some(format!(
        "OAuth linked ({})",
        scopes.trim().chars().take(120).collect::<String>()
    ));
    persist_cloudflare(&settings)
}

/// Finish OAuth callback after user approves at Cloudflare.
pub fn finish_oauth_callback(code: &str, state: &str, listen_port: u16) -> Result<String, String> {
    let code = code.trim();
    let state = state.trim();
    if code.is_empty() {
        return Err("OAuth callback missing code".into());
    }
    if state.is_empty() {
        return Err("OAuth callback missing state".into());
    }
    let pending: OauthPending = read_json(&oauth_pending_path());
    if pending.state.is_empty() || pending.state != state {
        return Err("OAuth state mismatch (expired or invalid session)".into());
    }
    let client = load_oauth_client();
    let redirect_uri = oauth_redirect_uri(listen_port);
    let body = curl_form_post(
        CF_OAUTH_TOKEN_URL,
        &[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri.as_str()),
            ("client_id", client.client_id.as_str()),
            ("client_secret", client.client_secret.as_str()),
            ("code_verifier", pending.code_verifier.as_str()),
        ],
    )?;
    let parsed: TokenResponse =
        serde_json::from_str(&body).map_err(|e| format!("OAuth token response invalid: {e}"))?;
    if let Some(err) = parsed.error {
        let detail = parsed.error_description.unwrap_or_default();
        return Err(format!("Cloudflare OAuth error: {err} {detail}")
            .trim()
            .to_string());
    }
    let access = parsed
        .access_token
        .filter(|t| !t.trim().is_empty())
        .ok_or_else(|| "OAuth response missing access_token".to_string())?;
    let refresh = parsed.refresh_token.unwrap_or_default();
    let scopes = parsed
        .scope
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_SCOPES.into());
    let expires_at = parsed
        .expires_in
        .map(|secs| now_unix().saturating_add(secs));
    let link = CloudflareOauthLink {
        schema_version: 1,
        access_token: access.clone(),
        refresh_token: refresh,
        token_type: parsed.token_type.unwrap_or_else(|| "bearer".into()),
        expires_at_unix: expires_at,
        scopes: scopes.clone(),
        connected_at_unix: now_unix(),
        updated_at_unix: now_unix(),
    };
    persist_json(&oauth_link_path(), &link)?;
    let _ = fs::remove_file(oauth_pending_path());
    sync_access_token_to_cloudflare(&access, &scopes)?;
    Ok("Cloudflare OAuth connected".into())
}

pub fn disconnect_oauth() -> Result<String, String> {
    let link = load_oauth_link();
    if !link.access_token.trim().is_empty() {
        let client = load_oauth_client();
        let _ = curl_form_post(
            CF_OAUTH_REVOKE_URL,
            &[
                ("token", link.access_token.as_str()),
                ("client_id", client.client_id.as_str()),
                ("client_secret", client.client_secret.as_str()),
            ],
        );
    }
    persist_json(&oauth_link_path(), &CloudflareOauthLink::default())?;
    let mut settings = load_cloudflare();
    if settings.auth_type == CloudflareAuthType::Oauth {
        settings.api_token.clear();
        settings.auth_type = CloudflareAuthType::ApiToken;
        settings.schema_version = 2;
        settings.updated_at_unix = now_unix();
        settings.last_verify_ok = None;
        settings.last_verify_message = Some("OAuth disconnected".into());
        persist_cloudflare(&settings)?;
    }
    Ok("Cloudflare OAuth disconnected".into())
}

/// Refresh OAuth access token when near expiry. Updates cloudflare.json.
pub fn refresh_oauth_access_if_needed() -> Result<(), String> {
    let settings = load_cloudflare();
    if settings.auth_type != CloudflareAuthType::Oauth {
        return Ok(());
    }
    let link = load_oauth_link();
    if link.refresh_token.trim().is_empty() {
        return Ok(());
    }
    let now = now_unix();
    let needs_refresh = match link.expires_at_unix {
        None => false,
        Some(exp) => now.saturating_add(120) >= exp,
    };
    if !needs_refresh {
        return Ok(());
    }
    let client = load_oauth_client();
    let body = curl_form_post(
        CF_OAUTH_TOKEN_URL,
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", link.refresh_token.as_str()),
            ("client_id", client.client_id.as_str()),
            ("client_secret", client.client_secret.as_str()),
        ],
    )?;
    let parsed: TokenResponse =
        serde_json::from_str(&body).map_err(|e| format!("OAuth refresh response invalid: {e}"))?;
    if let Some(err) = parsed.error {
        return Err(format!("Cloudflare OAuth refresh failed: {err}"));
    }
    let access = parsed
        .access_token
        .filter(|t| !t.trim().is_empty())
        .ok_or_else(|| "OAuth refresh missing access_token".to_string())?;
    let mut updated = link;
    if let Some(refresh) = parsed.refresh_token.filter(|t| !t.trim().is_empty()) {
        updated.refresh_token = refresh;
    }
    updated.access_token = access.clone();
    updated.expires_at_unix = parsed.expires_in.map(|secs| now.saturating_add(secs));
    updated.updated_at_unix = now;
    persist_json(&oauth_link_path(), &updated)?;
    sync_access_token_to_cloudflare(&access, &updated.scopes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn client_save_masks_secret_in_public() {
        with_test_data_dir(|| {
            save_oauth_client("cf-client-id", "super_secret_value_1234").unwrap();
            let pubv = oauth_public(2087);
            assert!(pubv.client_configured);
            assert_eq!(pubv.client_id, "cf-client-id");
            assert_eq!(pubv.client_secret_masked, "****1234");
            assert!(!pubv.client_secret_masked.contains("super_secret"));
        });
    }

    #[test]
    fn begin_oauth_requires_client() {
        with_test_data_dir(|| {
            ensure_oauth_stores_migrated().unwrap();
            assert!(begin_oauth_connect(2087).is_err());
        });
    }
}
