//! Single-use, time-limited panel password reset tokens (forgot-password email flow).

use crate::account::{data_dir, now_unix};
use crate::account_mgmt::{find_account, list_accounts};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// How long a reset link remains valid.
pub const RESET_TOKEN_TTL_SECS: u64 = 60 * 60;
/// Max forgot-password requests per identifier inside the window.
const RATE_MAX_PER_ID: u32 = 5;
/// Max forgot-password requests per client key (IP) inside the window.
const RATE_MAX_PER_CLIENT: u32 = 20;
/// Sliding window for rate limits.
const RATE_WINDOW_SECS: u64 = 15 * 60;

static STORE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ResetTokenRecord {
    /// SHA-256 hex of the raw token (never store cleartext).
    token_hash: String,
    username: String,
    expires_at_unix: u64,
    created_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ResetTokenStore {
    tokens: Vec<ResetTokenRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RateEntry {
    hits: Vec<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RateStore {
    by_identifier: HashMap<String, RateEntry>,
    by_client: HashMap<String, RateEntry>,
}

fn tokens_path() -> PathBuf {
    data_dir().join("password-reset-tokens.json")
}

fn rate_path() -> PathBuf {
    data_dir().join("password-reset-rate.json")
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

fn load_token_store() -> ResetTokenStore {
    let Ok(raw) = fs::read_to_string(tokens_path()) else {
        return ResetTokenStore::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn save_token_store(store: &ResetTokenStore) -> Result<(), String> {
    let json = serde_json::to_string_pretty(store)
        .map_err(|err| format!("Could not serialize reset tokens: {err}"))?;
    write_secret_file(&tokens_path(), json.as_bytes())
}

fn load_rate_store() -> RateStore {
    let Ok(raw) = fs::read_to_string(rate_path()) else {
        return RateStore::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn save_rate_store(store: &RateStore) {
    if let Ok(json) = serde_json::to_string_pretty(store) {
        let _ = write_secret_file(&rate_path(), json.as_bytes());
    }
}

fn prune_hits(entry: &mut RateEntry, now: u64) {
    entry
        .hits
        .retain(|ts| now.saturating_sub(*ts) < RATE_WINDOW_SECS);
}

/// Rate-limit forgot-password by identifier and optional client key (e.g. IP).
/// Returns Ok when allowed; Err when throttled (caller still shows the same ack UX).
pub fn check_and_record_forgot_rate(
    identifier: &str,
    client_key: Option<&str>,
) -> Result<(), String> {
    let _guard = STORE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let now = now_unix();
    let mut store = load_rate_store();
    let id_key = identifier.trim().to_ascii_lowercase();
    if !id_key.is_empty() {
        let entry = store.by_identifier.entry(id_key).or_default();
        prune_hits(entry, now);
        if entry.hits.len() as u32 >= RATE_MAX_PER_ID {
            return Err("Too many password reset requests. Try again later.".into());
        }
        entry.hits.push(now);
    }
    if let Some(client) = client_key.map(str::trim).filter(|v| !v.is_empty()) {
        let client_key = client.to_ascii_lowercase();
        let entry = store.by_client.entry(client_key).or_default();
        prune_hits(entry, now);
        if entry.hits.len() as u32 >= RATE_MAX_PER_CLIENT {
            return Err("Too many password reset requests. Try again later.".into());
        }
        entry.hits.push(now);
    }
    save_rate_store(&store);
    Ok(())
}

fn purge_expired(store: &mut ResetTokenStore, now: u64) {
    store.tokens.retain(|t| t.expires_at_unix > now);
}

/// Resolve username or recovery email to a panel account with a recovery address.
pub fn find_account_for_reset(identifier: &str) -> Option<(String, String)> {
    let id = identifier.trim();
    if id.is_empty() {
        return None;
    }
    let accounts = list_accounts().ok()?;
    for account in accounts {
        let username_ok = account.username.eq_ignore_ascii_case(id);
        let email_ok = !account.recovery_email.trim().is_empty()
            && account.recovery_email.eq_ignore_ascii_case(id);
        if !username_ok && !email_ok {
            continue;
        }
        if account.recovery_email.trim().is_empty() {
            return None;
        }
        // Confirm the account file still loads.
        if find_account(&account.username).is_ok() {
            return Some((account.username, account.recovery_email));
        }
    }
    None
}

/// Create a single-use reset token for `username`. Returns the raw token (email only).
pub fn create_reset_token(username: &str) -> Result<String, String> {
    let username = username.trim();
    if username.is_empty() {
        return Err("Username is required".into());
    }
    let _ = find_account(username)?;
    let bytes: [u8; 32] = rand::rng().random();
    let raw: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let token_hash = hash_token(&raw);
    let now = now_unix();
    let _guard = STORE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let mut store = load_token_store();
    purge_expired(&mut store, now);
    // One active token per account: replace prior entries for this user.
    store
        .tokens
        .retain(|t| !t.username.eq_ignore_ascii_case(username));
    store.tokens.push(ResetTokenRecord {
        token_hash,
        username: username.to_string(),
        expires_at_unix: now.saturating_add(RESET_TOKEN_TTL_SECS),
        created_at_unix: now,
    });
    save_token_store(&store)?;
    Ok(raw)
}

/// Peek whether a raw token is currently valid (does not consume).
pub fn peek_reset_token(raw_token: &str) -> Option<String> {
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

/// Consume a valid token and return the username. Invalid or expired tokens fail.
pub fn consume_reset_token(raw_token: &str) -> Result<String, String> {
    let raw = raw_token.trim();
    if raw.is_empty() {
        return Err("Reset link is missing or incomplete.".into());
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
                return Err("This reset link has expired. Request a new one.".into());
            }
            found_idx = Some(idx);
            break;
        }
    }
    let Some(idx) = found_idx else {
        let _ = save_token_store(&store);
        return Err("This reset link is invalid or already used.".into());
    };
    let username = store.tokens[idx].username.clone();
    store.tokens.remove(idx);
    save_token_store(&store)?;
    Ok(username)
}

/// Invalidate all outstanding reset tokens for a username (e.g. after password change).
pub fn invalidate_tokens_for_user(username: &str) {
    let _guard = STORE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let mut store = load_token_store();
    store
        .tokens
        .retain(|t| !t.username.eq_ignore_ascii_case(username));
    let _ = save_token_store(&store);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::{
        PanelBootstrap, default_password_policy, hash_password, new_password_salt,
        with_test_data_dir, write_account_file,
    };
    use crate::account_mgmt::reset_account_password;

    fn write_admin(password: &str) {
        let salt = new_password_salt();
        let boot = PanelBootstrap {
            schema_version: 1,
            username: "Admin".into(),
            recovery_email: "admin@example.com".into(),
            password_hash: hash_password(password, &salt),
            password_salt: salt,
            password_policy: default_password_policy(),
            language: "en".into(),
            created_at_unix: 1,
        };
        write_account_file(&crate::account::bootstrap_path(), &boot).expect("write");
    }

    #[test]
    fn create_peek_consume_and_reject_reuse() {
        with_test_data_dir(|| {
            write_admin("Passw0rd!");
            let raw = create_reset_token("Admin").expect("create");
            assert_eq!(peek_reset_token(&raw).as_deref(), Some("Admin"));
            let user = consume_reset_token(&raw).expect("consume");
            assert_eq!(user, "Admin");
            assert!(consume_reset_token(&raw).is_err());
            assert!(peek_reset_token(&raw).is_none());
        });
    }

    #[test]
    fn find_by_email_and_username() {
        with_test_data_dir(|| {
            write_admin("Passw0rd!");
            let by_user = find_account_for_reset("admin").expect("user");
            assert_eq!(by_user.0, "Admin");
            let by_email = find_account_for_reset("ADMIN@example.com").expect("email");
            assert_eq!(by_email.1, "admin@example.com");
            assert!(find_account_for_reset("nobody").is_none());
        });
    }

    #[test]
    fn expired_token_rejected() {
        with_test_data_dir(|| {
            write_admin("Passw0rd!");
            let raw = create_reset_token("Admin").expect("create");
            // Force expiry by rewriting store.
            {
                let _guard = STORE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
                let mut store = load_token_store();
                for t in &mut store.tokens {
                    t.expires_at_unix = 1;
                }
                save_token_store(&store).unwrap();
            }
            assert!(consume_reset_token(&raw).is_err());
        });
    }

    #[test]
    fn reset_password_via_token_path() {
        with_test_data_dir(|| {
            write_admin("Passw0rd!");
            let raw = create_reset_token("Admin").expect("create");
            let user = consume_reset_token(&raw).expect("consume");
            reset_account_password(&user, Some("N3w-Pass!"), false).expect("reset");
            let (boot, _) = find_account("Admin").unwrap();
            assert!(crate::account::verify_password(
                "N3w-Pass!",
                &boot.password_salt,
                &boot.password_hash
            ));
        });
    }

    #[test]
    fn rate_limit_blocks_extra_hits() {
        with_test_data_dir(|| {
            for _ in 0..RATE_MAX_PER_ID {
                assert!(
                    check_and_record_forgot_rate("admin@example.com", Some("10.0.0.1")).is_ok()
                );
            }
            assert!(check_and_record_forgot_rate("admin@example.com", Some("10.0.0.1")).is_err());
        });
    }
}
