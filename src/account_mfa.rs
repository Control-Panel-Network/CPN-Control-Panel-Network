//! Per-account MFA state under the CPN data dir (`/var/lib/cpn/mfa/` by default).
//! TOTP secrets are AES-256-GCM encrypted at rest. Backup codes are PBKDF2 hashed.
//! Secrets are never logged.

use crate::account::{data_dir, hash_password, new_password_salt, now_unix, verify_password};
use crate::account_totp::{
    decode_totp_secret, generate_totp_secret_base32, otpauth_qr_svg, otpauth_uri, verify_totp_code,
};
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

const MFA_SCHEMA: u32 = 1;
const BACKUP_CODE_COUNT: usize = 10;
const RATE_MAX_ATTEMPTS: u32 = 5;
const RATE_LOCK_SECS: u64 = 300;
const PENDING_TTL_SECS: u64 = 600;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MfaRecord {
    pub schema_version: u32,
    pub username: String,
    pub totp_enabled: bool,
    #[serde(default)]
    pub totp_secret_enc: String,
    #[serde(default)]
    pub totp_nonce: String,
    #[serde(default)]
    pub backup_code_hashes: Vec<String>,
    #[serde(default)]
    pub backup_code_salts: Vec<String>,
    pub updated_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingTotp {
    username: String,
    secret_base32: String,
    created_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RateStore {
    entries: HashMap<String, RateEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RateEntry {
    failures: u32,
    locked_until_unix: u64,
}

static RATE_LOCK: Mutex<()> = Mutex::new(());

fn mfa_dir() -> PathBuf {
    data_dir().join("mfa")
}

fn mfa_key_path() -> PathBuf {
    mfa_dir().join("mfa-encryption.key")
}

fn mfa_record_path(username: &str) -> PathBuf {
    let key: String = username
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    mfa_dir().join(format!("{key}.json"))
}

fn pending_path(username: &str) -> PathBuf {
    let key: String = username
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    mfa_dir().join(format!("pending-{key}.json"))
}

fn rate_path() -> PathBuf {
    mfa_dir().join("rate-limits.json")
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

fn load_or_create_mfa_key() -> Result<[u8; 32], String> {
    let path = mfa_key_path();
    if let Ok(raw) = fs::read_to_string(&path) {
        let trimmed = raw.trim();
        if trimmed.len() == 64 && trimmed.chars().all(|ch| ch.is_ascii_hexdigit()) {
            let mut key = [0u8; 32];
            for (i, chunk) in trimmed.as_bytes().chunks(2).enumerate() {
                let hi = (chunk[0] as char).to_digit(16).unwrap_or(0);
                let lo = (chunk[1] as char).to_digit(16).unwrap_or(0);
                key[i] = ((hi << 4) | lo) as u8;
            }
            return Ok(key);
        }
    }
    let key: [u8; 32] = rand::rng().random();
    let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
    write_secret_file(&path, hex.as_bytes())?;
    Ok(key)
}

fn encrypt_secret(plain: &str) -> Result<(String, String), String> {
    let key = load_or_create_mfa_key()?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|err| format!("AES key error: {err}"))?;
    let nonce_bytes: [u8; 12] = rand::rng().random();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plain.as_bytes())
        .map_err(|_| "Could not encrypt TOTP secret".to_string())?;
    Ok((B64.encode(ciphertext), B64.encode(nonce_bytes)))
}

fn decrypt_secret(enc_b64: &str, nonce_b64: &str) -> Result<String, String> {
    let key = load_or_create_mfa_key()?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|err| format!("AES key error: {err}"))?;
    let ciphertext = B64
        .decode(enc_b64.as_bytes())
        .map_err(|_| "Corrupt TOTP ciphertext".to_string())?;
    let nonce_bytes = B64
        .decode(nonce_b64.as_bytes())
        .map_err(|_| "Corrupt TOTP nonce".to_string())?;
    if nonce_bytes.len() != 12 {
        return Err("Corrupt TOTP nonce length".into());
    }
    let nonce = Nonce::from_slice(&nonce_bytes);
    let plain = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|_| "Could not decrypt TOTP secret".to_string())?;
    String::from_utf8(plain).map_err(|_| "Invalid TOTP secret encoding".into())
}

fn empty_record(username: &str) -> MfaRecord {
    MfaRecord {
        schema_version: MFA_SCHEMA,
        username: username.to_string(),
        totp_enabled: false,
        totp_secret_enc: String::new(),
        totp_nonce: String::new(),
        backup_code_hashes: Vec::new(),
        backup_code_salts: Vec::new(),
        updated_at_unix: now_unix(),
    }
}

pub fn load_mfa(username: &str) -> MfaRecord {
    let path = mfa_record_path(username);
    let Ok(raw) = fs::read_to_string(&path) else {
        return empty_record(username);
    };
    serde_json::from_str(&raw).unwrap_or_else(|_| empty_record(username))
}

fn save_mfa(record: &MfaRecord) -> Result<(), String> {
    let path = mfa_record_path(&record.username);
    let json = serde_json::to_string_pretty(record)
        .map_err(|err| format!("Could not serialize MFA record: {err}"))?;
    write_secret_file(&path, json.as_bytes())
}

/// Persist MFA under the new username and remove the old file (best-effort).
pub fn save_mfa_for_rename(record: &MfaRecord, old_username: &str) -> Result<(), String> {
    save_mfa(record)?;
    if !old_username.eq_ignore_ascii_case(&record.username) {
        let _ = fs::remove_file(mfa_record_path(old_username));
        let _ = fs::remove_file(pending_path(old_username));
    }
    Ok(())
}

pub fn totp_enabled_for(username: &str) -> bool {
    load_mfa(username).totp_enabled
}

fn generate_backup_codes() -> Vec<String> {
    let mut rng = rand::rng();
    let mut codes = Vec::with_capacity(BACKUP_CODE_COUNT);
    for _ in 0..BACKUP_CODE_COUNT {
        let a: u32 = rng.random_range(0..100_000_000);
        let left = a / 10_000;
        let right = a % 10_000;
        codes.push(format!("{left:04}-{right:04}"));
    }
    codes
}

fn hash_backup_code(code: &str) -> (String, String) {
    let salt = new_password_salt();
    let normalized = code.trim().to_uppercase().replace(' ', "");
    (hash_password(&normalized, &salt), salt)
}

fn verify_backup_code(code: &str, hash: &str, salt: &str) -> bool {
    let normalized = code.trim().to_uppercase().replace(' ', "");
    verify_password(&normalized, salt, hash)
}

/// Start TOTP enrollment: returns (secret_base32, otpauth_uri, qr_svg).
pub fn begin_totp_enroll(username: &str) -> Result<(String, String, String), String> {
    if username.trim().is_empty() {
        return Err("Username is required".into());
    }
    if load_mfa(username).totp_enabled {
        return Err("TOTP is already enabled for this account".into());
    }
    let secret = generate_totp_secret_base32();
    let pending = PendingTotp {
        username: username.to_string(),
        secret_base32: secret.clone(),
        created_at_unix: now_unix(),
    };
    let json = serde_json::to_string_pretty(&pending)
        .map_err(|err| format!("Could not serialize pending TOTP: {err}"))?;
    write_secret_file(&pending_path(username), json.as_bytes())?;
    let uri = otpauth_uri("CPN Panel", username, &secret);
    let svg = otpauth_qr_svg(&uri)?;
    Ok((secret, uri, svg))
}

pub fn load_pending_secret(username: &str) -> Result<String, String> {
    let path = pending_path(username);
    let raw = fs::read_to_string(&path).map_err(|_| "No pending TOTP enrollment".to_string())?;
    let pending: PendingTotp =
        serde_json::from_str(&raw).map_err(|_| "Invalid pending TOTP enrollment".to_string())?;
    if now_unix().saturating_sub(pending.created_at_unix) > PENDING_TTL_SECS {
        let _ = fs::remove_file(&path);
        return Err("Pending TOTP enrollment expired; start again".into());
    }
    if !pending.username.eq_ignore_ascii_case(username) {
        return Err("Pending TOTP enrollment mismatch".into());
    }
    Ok(pending.secret_base32)
}

/// Confirm enrollment with a TOTP code; returns plaintext backup codes once.
pub fn confirm_totp_enroll(username: &str, code: &str) -> Result<Vec<String>, String> {
    let secret_b32 = load_pending_secret(username)?;
    let secret = decode_totp_secret(&secret_b32)?;
    if !verify_totp_code(&secret, code, now_unix()) {
        return Err("Invalid authenticator code".into());
    }
    let (enc, nonce) = encrypt_secret(&secret_b32)?;
    let backup_codes = generate_backup_codes();
    let mut hashes = Vec::new();
    let mut salts = Vec::new();
    for code in &backup_codes {
        let (hash, salt) = hash_backup_code(code);
        hashes.push(hash);
        salts.push(salt);
    }
    let record = MfaRecord {
        schema_version: MFA_SCHEMA,
        username: username.to_string(),
        totp_enabled: true,
        totp_secret_enc: enc,
        totp_nonce: nonce,
        backup_code_hashes: hashes,
        backup_code_salts: salts,
        updated_at_unix: now_unix(),
    };
    save_mfa(&record)?;
    let _ = fs::remove_file(pending_path(username));
    Ok(backup_codes)
}

pub fn disable_totp(username: &str, code_or_backup: &str) -> Result<(), String> {
    let mut record = load_mfa(username);
    if !record.totp_enabled {
        return Err("TOTP is not enabled".into());
    }
    if !verify_mfa_challenge(username, code_or_backup)? {
        return Err("Invalid authenticator or backup code".into());
    }
    record.totp_enabled = false;
    record.totp_secret_enc.clear();
    record.totp_nonce.clear();
    record.backup_code_hashes.clear();
    record.backup_code_salts.clear();
    record.updated_at_unix = now_unix();
    save_mfa(&record)?;
    let _ = fs::remove_file(pending_path(username));
    Ok(())
}

fn consume_backup_code(record: &mut MfaRecord, code: &str) -> bool {
    for i in 0..record.backup_code_hashes.len() {
        let hash = &record.backup_code_hashes[i];
        let salt = record
            .backup_code_salts
            .get(i)
            .map(String::as_str)
            .unwrap_or("");
        if verify_backup_code(code, hash, salt) {
            record.backup_code_hashes.remove(i);
            if i < record.backup_code_salts.len() {
                record.backup_code_salts.remove(i);
            }
            record.updated_at_unix = now_unix();
            let _ = save_mfa(record);
            return true;
        }
    }
    false
}

/// Verify TOTP or a single-use backup code. Updates rate limit on failure.
pub fn verify_mfa_challenge(username: &str, code_raw: &str) -> Result<bool, String> {
    check_rate_limit(username)?;
    let mut record = load_mfa(username);
    if !record.totp_enabled {
        return Ok(true);
    }
    let secret_b32 = decrypt_secret(&record.totp_secret_enc, &record.totp_nonce)?;
    let secret = decode_totp_secret(&secret_b32)?;
    if verify_totp_code(&secret, code_raw, now_unix()) {
        clear_rate_limit(username);
        return Ok(true);
    }
    if consume_backup_code(&mut record, code_raw) {
        clear_rate_limit(username);
        return Ok(true);
    }
    register_rate_failure(username);
    Ok(false)
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

pub fn check_rate_limit(username: &str) -> Result<(), String> {
    let _guard = RATE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let store = load_rate_store();
    let key = username.to_ascii_lowercase();
    if let Some(entry) = store.entries.get(&key)
        && entry.locked_until_unix > now_unix()
    {
        let wait = entry.locked_until_unix.saturating_sub(now_unix());
        return Err(format!(
            "Too many MFA attempts. Try again in {wait} seconds."
        ));
    }
    Ok(())
}

fn register_rate_failure(username: &str) {
    let _guard = RATE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let mut store = load_rate_store();
    let key = username.to_ascii_lowercase();
    let entry = store.entries.entry(key).or_default();
    entry.failures = entry.failures.saturating_add(1);
    if entry.failures >= RATE_MAX_ATTEMPTS {
        entry.locked_until_unix = now_unix().saturating_add(RATE_LOCK_SECS);
        entry.failures = 0;
    }
    save_rate_store(&store);
}

fn clear_rate_limit(username: &str) {
    let _guard = RATE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let mut store = load_rate_store();
    store.entries.remove(&username.to_ascii_lowercase());
    save_rate_store(&store);
}

/// Fingerprint for UI (not secret): short hash of ciphertext.
pub fn totp_fingerprint(record: &MfaRecord) -> String {
    if record.totp_secret_enc.is_empty() {
        return String::new();
    }
    let mut hasher = Sha256::new();
    hasher.update(record.totp_secret_enc.as_bytes());
    let dig = hasher.finalize();
    format!("{:02x}{:02x}{:02x}{:02x}", dig[0], dig[1], dig[2], dig[3])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::account_totp::totp_code_at;

    #[test]
    fn enroll_confirm_verify_disable() {
        with_test_data_dir(|| {
            let user = "Admin";
            let (secret_b32, _uri, svg) = begin_totp_enroll(user).unwrap();
            assert!(svg.contains("<svg"));
            let secret = decode_totp_secret(&secret_b32).unwrap();
            let code = format!("{:06}", totp_code_at(&secret, now_unix()));
            let backups = confirm_totp_enroll(user, &code).unwrap();
            assert_eq!(backups.len(), BACKUP_CODE_COUNT);
            assert!(totp_enabled_for(user));
            let code2 = format!("{:06}", totp_code_at(&secret, now_unix()));
            assert!(verify_mfa_challenge(user, &code2).unwrap());
            assert!(verify_mfa_challenge(user, &backups[0]).unwrap());
            // Backup consumed once.
            assert!(!verify_mfa_challenge(user, &backups[0]).unwrap());
            let code3 = format!("{:06}", totp_code_at(&secret, now_unix()));
            disable_totp(user, &code3).unwrap();
            assert!(!totp_enabled_for(user));
        });
    }
}
