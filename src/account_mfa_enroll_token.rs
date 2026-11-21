//! Short-lived, single-use operator tokens that open HTTPS MFA enrollment in a browser.
//! Raw tokens are only returned to root CLI output once; on disk we store SHA-256 hashes only.

use crate::account::{data_dir, now_unix};
use crate::account_mgmt::find_account;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Operator CLI enroll link TTL (short-lived).
pub const MFA_ENROLL_TOKEN_TTL_SECS: u64 = 15 * 60;

static STORE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EnrollTokenRecord {
    token_hash: String,
    username: String,
    expires_at_unix: u64,
    created_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct EnrollTokenStore {
    tokens: Vec<EnrollTokenRecord>,
}

fn tokens_path() -> PathBuf {
    data_dir().join("mfa-enroll-tokens.json")
}

fn write_secret_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Could not create {}: {err}", parent.display()))?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|err| format!("Could not write {}: {err}", path.display()))?;
    file.write_all(bytes)
        .map_err(|err| format!("Could not save {}: {err}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn hash_token(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (left, right) in a.iter().zip(b.iter()) {
        diff |= left ^ right;
    }
    diff == 0
}

fn load_token_store() -> EnrollTokenStore {
    let Ok(raw) = fs::read_to_string(tokens_path()) else {
        return EnrollTokenStore::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn save_token_store(store: &EnrollTokenStore) -> Result<(), String> {
    let json = serde_json::to_string_pretty(store)
        .map_err(|err| format!("Could not serialize MFA enroll tokens: {err}"))?;
    write_secret_file(&tokens_path(), json.as_bytes())
}

fn purge_expired(store: &mut EnrollTokenStore, now: u64) {
    store.tokens.retain(|t| t.expires_at_unix > now);
}

/// Create a single-use enroll token for `username`. Returns `(raw_token, expires_at_unix)`.
pub fn create_mfa_enroll_token(username: &str) -> Result<(String, u64), String> {
    let username = username.trim();
    if username.is_empty() {
        return Err("Username is required".into());
    }
    let _ = find_account(username)?;
    let bytes: [u8; 32] = rand::rng().random();
    let raw: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let token_hash = hash_token(&raw);
    let now = now_unix();
    let expires = now.saturating_add(MFA_ENROLL_TOKEN_TTL_SECS);
    let _guard = STORE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let mut store = load_token_store();
    purge_expired(&mut store, now);
    store
        .tokens
        .retain(|t| !t.username.eq_ignore_ascii_case(username));
    store.tokens.push(EnrollTokenRecord {
        token_hash,
        username: username.to_string(),
        expires_at_unix: expires,
        created_at_unix: now,
    });
    save_token_store(&store)?;
    Ok((raw, expires))
}

/// Peek whether a raw token is valid (does not consume).
pub fn peek_mfa_enroll_token(raw_token: &str) -> Option<String> {
    let raw = raw_token.trim();
    if raw.is_empty() {
        return None;
    }
    let want = hash_token(raw);
    let now = now_unix();
    let _guard = STORE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let mut store = load_token_store();
    purge_expired(&mut store, now);
    let _ = save_token_store(&store);
    store
        .tokens
        .iter()
        .find(|t| constant_time_eq(&t.token_hash, &want) && t.expires_at_unix > now)
        .map(|t| t.username.clone())
}

/// Consume a valid token and return the username.
pub fn consume_mfa_enroll_token(raw_token: &str) -> Result<String, String> {
    let raw = raw_token.trim();
    if raw.is_empty() {
        return Err("Enroll link is missing or incomplete.".into());
    }
    let want = hash_token(raw);
    let now = now_unix();
    let _guard = STORE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let mut store = load_token_store();
    purge_expired(&mut store, now);
    let mut found_idx = None;
    for (idx, entry) in store.tokens.iter().enumerate() {
        if constant_time_eq(&entry.token_hash, &want) {
            if entry.expires_at_unix <= now {
                store.tokens.remove(idx);
                let _ = save_token_store(&store);
                return Err("This enroll link has expired. Ask an operator for a new one.".into());
            }
            found_idx = Some(idx);
            break;
        }
    }
    let Some(idx) = found_idx else {
        let _ = save_token_store(&store);
        return Err("This enroll link is invalid or already used.".into());
    };
    let username = store.tokens[idx].username.clone();
    store.tokens.remove(idx);
    save_token_store(&store)?;
    Ok(username)
}

/// Whether a pending (unused) enroll token exists for the account.
pub fn has_pending_mfa_enroll_token(username: &str) -> bool {
    let now = now_unix();
    let _guard = STORE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let store = load_token_store();
    store
        .tokens
        .iter()
        .any(|t| t.username.eq_ignore_ascii_case(username.trim()) && t.expires_at_unix > now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::{
        PanelBootstrap, default_password_policy, new_password_salt, persist_bootstrap,
        with_test_data_dir,
    };

    fn seed_admin() {
        let salt = new_password_salt();
        let boot = PanelBootstrap {
            schema_version: 1,
            username: "admin".into(),
            password_hash: "x".into(),
            password_salt: salt,
            recovery_email: "admin@example.com".into(),
            language: "en".into(),
            password_policy: default_password_policy(),
            created_at_unix: 1,
            must_change_password: false,
            totp_required: true,
            disabled: false,
            role: String::new(),
            parent_reseller: String::new(),
            parent_owner: String::new(),
        };
        persist_bootstrap(&boot).expect("bootstrap");
    }

    #[test]
    fn enroll_token_single_use() {
        with_test_data_dir(|| {
            seed_admin();
            let (raw, _exp) = create_mfa_enroll_token("admin").expect("create");
            assert!(peek_mfa_enroll_token(&raw).is_some());
            let user = consume_mfa_enroll_token(&raw).expect("consume");
            assert_eq!(user, "admin");
            assert!(consume_mfa_enroll_token(&raw).is_err());
            assert!(!has_pending_mfa_enroll_token("admin"));
        });
    }
}
