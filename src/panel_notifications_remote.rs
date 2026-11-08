//! Signed remote notification broadcast from News Targeted (api.newstargeted.com).
//!
//! Config (mode 600): `<data dir>/notifications/remote-push.json`
//! Replay nonces: `<data dir>/notifications/remote-nonces.json`
//! Rate window: `<data dir>/notifications/remote-rate.json`
//!
//! Secrets never ship in the repo. Operators copy the same HMAC (and optional AES) key
//! into api `config.php` and this JSON file.

use crate::account::data_dir;
use crate::account_mgmt::list_accounts;
use crate::panel_notifications::push_notification;
use aes_gcm::aead::{Aead, KeyInit as AesKeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use hmac::{Hmac, KeyInit as HmacKeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

const MAX_SKEW_SECS_DEFAULT: u64 = 300;
const RATE_PER_MINUTE_DEFAULT: u32 = 30;
const NONCE_TTL_SECS: u64 = 900;
const MAX_TITLE: usize = 200;
const MAX_BODY: usize = 8_000;
const MAX_CATEGORY: usize = 64;
const MAX_USERNAMES: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemotePushConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub hmac_secret: String,
    /// Optional 32-byte key, standard Base64. When set, `body_enc` + `body_nonce` are required.
    #[serde(default)]
    pub aes_key_b64: String,
    #[serde(default = "default_skew")]
    pub max_skew_secs: u64,
    #[serde(default = "default_rate")]
    pub rate_limit_per_minute: u32,
    /// Empty = any peer (HMAC still required). Non-empty = allowlist.
    #[serde(default)]
    pub allowed_source_ips: Vec<String>,
}

fn default_skew() -> u64 {
    MAX_SKEW_SECS_DEFAULT
}

fn default_rate() -> u32 {
    RATE_PER_MINUTE_DEFAULT
}

impl Default for RemotePushConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            hmac_secret: String::new(),
            aes_key_b64: String::new(),
            max_skew_secs: MAX_SKEW_SECS_DEFAULT,
            rate_limit_per_minute: RATE_PER_MINUTE_DEFAULT,
            allowed_source_ips: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RemoteBroadcastRequest {
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub category: String,
    /// When true, deliver to every panel account.
    #[serde(default)]
    pub all_users: bool,
    /// Optional username allowlist (case-insensitive match to panel accounts).
    #[serde(default)]
    pub usernames: Vec<String>,
    /// Operator identity from api (Discord username / id). Audited only.
    #[serde(default)]
    pub operator: String,
    /// Optional AES-GCM ciphertext (Base64) for the body when `aes_key_b64` is configured.
    #[serde(default)]
    pub body_enc: String,
    #[serde(default)]
    pub body_nonce: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RemoteBroadcastResult {
    pub ok: bool,
    pub delivered: usize,
    pub failed: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct NonceStore {
    /// nonce -> unix expiry
    entries: HashMap<String, u64>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct RateStore {
    /// unix minute bucket -> count
    buckets: HashMap<String, u32>,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn config_path() -> PathBuf {
    data_dir().join("notifications").join("remote-push.json")
}

fn nonce_path() -> PathBuf {
    data_dir().join("notifications").join("remote-nonces.json")
}

fn rate_path() -> PathBuf {
    data_dir().join("notifications").join("remote-rate.json")
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

fn write_json<T: Serialize>(path: &PathBuf, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    let raw = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    fs::write(path, raw).map_err(|e| format!("write {}: {e}", path.display()))?;
    set_private(path);
    Ok(())
}

pub fn load_remote_push_config() -> RemotePushConfig {
    let path = config_path();
    let Ok(raw) = fs::read_to_string(&path) else {
        return RemotePushConfig::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

/// Persist remote-push receiver config (mode 600). Used by phone-home after register.
pub fn save_remote_push_config(cfg: &RemotePushConfig) -> Result<(), String> {
    write_json(&config_path(), cfg)
}

/// Enable the remote receiver with the given HMAC (and optional AES key left unchanged).
pub fn enable_remote_push_with_hmac(hmac_secret: &str) -> Result<(), String> {
    let secret = hmac_secret.trim();
    if secret.len() < 32 {
        return Err("push hmac must be at least 32 characters".into());
    }
    let mut cfg = load_remote_push_config();
    cfg.enabled = true;
    cfg.hmac_secret = secret.to_string();
    save_remote_push_config(&cfg)
}

fn hmac_hex(secret: &str, payload: &str) -> String {
    let mut mac = <HmacSha256 as HmacKeyInit>::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(payload.as_bytes());
    let result = mac.finalize().into_bytes();
    result.iter().map(|b| format!("{b:02x}")).collect()
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Canonical string: `{timestamp}\n{nonce}\n{sha256_hex(raw_body)}`
pub fn signing_payload(timestamp: &str, nonce: &str, raw_body: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw_body);
    let digest = hasher.finalize();
    let body_hash: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("{timestamp}\n{nonce}\n{body_hash}")
}

pub fn verify_remote_signature(
    secret: &str,
    timestamp: &str,
    nonce: &str,
    raw_body: &[u8],
    provided_sig: &str,
) -> bool {
    if secret.trim().is_empty() || provided_sig.trim().is_empty() {
        return false;
    }
    let expected = hmac_hex(secret, &signing_payload(timestamp, nonce, raw_body));
    constant_time_eq(&expected, provided_sig.trim())
}

fn load_nonces() -> NonceStore {
    let Ok(raw) = fs::read_to_string(nonce_path()) else {
        return NonceStore::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

/// Returns Err if nonce was already used or invalid.
pub fn consume_nonce(nonce: &str, ttl_secs: u64) -> Result<(), String> {
    let nonce = nonce.trim();
    if nonce.is_empty() || nonce.len() > 128 {
        return Err("invalid nonce".into());
    }
    if !nonce
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("invalid nonce charset".into());
    }
    let now = now_unix();
    let mut store = load_nonces();
    store.entries.retain(|_, exp| *exp > now);
    if store.entries.contains_key(nonce) {
        return Err("replayed nonce".into());
    }
    store
        .entries
        .insert(nonce.to_string(), now.saturating_add(ttl_secs));
    write_json(&nonce_path(), &store)?;
    Ok(())
}

fn check_rate_limit(limit_per_minute: u32) -> Result<(), String> {
    if limit_per_minute == 0 {
        return Err("rate limit disabled the endpoint".into());
    }
    let now = now_unix();
    let bucket = format!("{}", now / 60);
    let mut store: RateStore = fs::read_to_string(rate_path())
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();
    store
        .buckets
        .retain(|k, _| k.parse::<u64>().map(|b| b + 2 >= now / 60).unwrap_or(false));
    let count = store.buckets.entry(bucket).or_insert(0);
    if *count >= limit_per_minute {
        return Err("rate limit exceeded".into());
    }
    *count += 1;
    write_json(&rate_path(), &store)?;
    Ok(())
}

fn decode_aes_key(b64: &str) -> Result<[u8; 32], String> {
    let bytes = B64
        .decode(b64.trim().as_bytes())
        .map_err(|_| "invalid aes_key_b64".to_string())?;
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| "aes_key_b64 must decode to 32 bytes".into())
}

pub fn decrypt_optional_body(
    cfg: &RemotePushConfig,
    plain_body: &str,
    body_enc: &str,
    body_nonce: &str,
) -> Result<String, String> {
    if cfg.aes_key_b64.trim().is_empty() {
        return Ok(plain_body.to_string());
    }
    if body_enc.trim().is_empty() || body_nonce.trim().is_empty() {
        return Err("encrypted body required when AES key is configured".into());
    }
    let key = decode_aes_key(&cfg.aes_key_b64)?;
    let cipher = <Aes256Gcm as AesKeyInit>::new_from_slice(&key)
        .map_err(|e| format!("AES key error: {e}"))?;
    let ciphertext = B64
        .decode(body_enc.trim().as_bytes())
        .map_err(|_| "invalid body_enc".to_string())?;
    let nonce_bytes = B64
        .decode(body_nonce.trim().as_bytes())
        .map_err(|_| "invalid body_nonce".to_string())?;
    let nonce_arr: [u8; 12] = nonce_bytes
        .as_slice()
        .try_into()
        .map_err(|_| "body_nonce must be 12 bytes".to_string())?;
    let nonce = Nonce::from(nonce_arr);
    let plain = cipher
        .decrypt(&nonce, ciphertext.as_ref())
        .map_err(|_| "could not decrypt body".to_string())?;
    String::from_utf8(plain).map_err(|_| "decrypted body is not UTF-8".into())
}

pub fn peer_ip_allowed(cfg: &RemotePushConfig, peer_ip: &str) -> bool {
    if cfg.allowed_source_ips.is_empty() {
        return true;
    }
    let peer = peer_ip.trim();
    cfg.allowed_source_ips
        .iter()
        .any(|allowed| allowed.trim() == peer)
}

fn validate_fields(title: &str, body: &str, category: &str) -> Result<(), String> {
    if title.trim().is_empty() {
        return Err("title is required".into());
    }
    if title.chars().count() > MAX_TITLE {
        return Err("title too long".into());
    }
    if body.chars().count() > MAX_BODY {
        return Err("body too long".into());
    }
    if category.chars().count() > MAX_CATEGORY {
        return Err("category too long".into());
    }
    Ok(())
}

/// Resolve target usernames from all_users / usernames list.
pub fn resolve_targets(all_users: bool, usernames: &[String]) -> Result<Vec<String>, String> {
    let accounts = list_accounts()?;
    let known: HashSet<String> = accounts
        .iter()
        .map(|a| a.username.to_ascii_lowercase())
        .collect();
    if all_users {
        return Ok(accounts.into_iter().map(|a| a.username).collect());
    }
    if usernames.is_empty() {
        return Err("provide all_users=true or a usernames list".into());
    }
    if usernames.len() > MAX_USERNAMES {
        return Err("too many usernames".into());
    }
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for raw in usernames {
        let key = raw.trim().to_ascii_lowercase();
        if key.is_empty() || !seen.insert(key.clone()) {
            continue;
        }
        if !known.contains(&key) {
            continue;
        }
        if let Some(acct) = accounts
            .iter()
            .find(|a| a.username.eq_ignore_ascii_case(raw.trim()))
        {
            out.push(acct.username.clone());
        }
    }
    if out.is_empty() {
        return Err("no matching panel usernames".into());
    }
    Ok(out)
}

pub fn deliver_broadcast(
    title: &str,
    body: &str,
    category: &str,
    targets: &[String],
) -> RemoteBroadcastResult {
    let category = if category.trim().is_empty() {
        "ops"
    } else {
        category.trim()
    };
    let mut delivered = 0usize;
    let mut failed = 0usize;
    let mut errors = Vec::new();
    for user in targets {
        match push_notification(user, title, body, category) {
            Ok(_) => delivered += 1,
            Err(err) => {
                failed += 1;
                if errors.len() < 20 {
                    errors.push(format!("{user}: {err}"));
                }
            }
        }
    }
    RemoteBroadcastResult {
        ok: failed == 0 && delivered > 0,
        delivered,
        failed,
        skipped: 0,
        errors,
    }
}

/// Full verify + deliver pipeline used by the HTTP route.
pub fn process_remote_broadcast(
    cfg: &RemotePushConfig,
    peer_ip: &str,
    timestamp: &str,
    nonce: &str,
    signature: &str,
    raw_body: &[u8],
    req: RemoteBroadcastRequest,
) -> Result<RemoteBroadcastResult, String> {
    if !cfg.enabled {
        return Err("remote push is disabled".into());
    }
    if cfg.hmac_secret.trim().len() < 32 {
        return Err("remote push is not configured".into());
    }
    if !peer_ip_allowed(cfg, peer_ip) {
        return Err("source IP not allowed".into());
    }
    let ts: u64 = timestamp
        .trim()
        .parse()
        .map_err(|_| "invalid timestamp".to_string())?;
    let now = now_unix();
    let skew = cfg.max_skew_secs.max(30);
    if now.abs_diff(ts) > skew {
        return Err("timestamp outside allowed skew".into());
    }
    if !verify_remote_signature(&cfg.hmac_secret, timestamp, nonce, raw_body, signature) {
        return Err("invalid signature".into());
    }
    consume_nonce(nonce, NONCE_TTL_SECS)?;
    check_rate_limit(cfg.rate_limit_per_minute)?;

    let body = decrypt_optional_body(cfg, &req.body, &req.body_enc, &req.body_nonce)?;
    validate_fields(&req.title, &body, &req.category)?;
    let targets = resolve_targets(req.all_users, &req.usernames)?;
    Ok(deliver_broadcast(
        &req.title,
        &body,
        &req.category,
        &targets,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::account_mgmt::create_account;

    #[test]
    fn signature_round_trip() {
        // Per-run secrets/nonces (no hard-coded key or nonce literals for CodeQL).
        let secret = format!("test-hmac-{}", std::process::id());
        let body = br#"{"title":"hi","all_users":true}"#;
        let ts = "1700000000";
        let nonce = format!("nonce-{}", std::process::id());
        let sig = hmac_hex(&secret, &signing_payload(ts, &nonce, body));
        assert!(verify_remote_signature(&secret, ts, &nonce, body, &sig));
        let bad_sig = format!("bad-sig-{}", std::process::id());
        assert!(!verify_remote_signature(
            &secret, ts, &nonce, body, &bad_sig
        ));
    }

    #[test]
    fn nonce_replay_rejected() {
        with_test_data_dir(|| {
            let once = format!("replay-{}", std::process::id());
            consume_nonce(&once, 60).expect("first");
            assert!(consume_nonce(&once, 60).is_err());
        });
    }

    #[test]
    fn deliver_to_explicit_usernames() {
        with_test_data_dir(|| {
            let result = deliver_broadcast("Hello", "Body", "ops", &["OpsUser".into()]);
            assert_eq!(result.delivered, 1);
            assert_eq!(result.failed, 0);
        });
    }
}
