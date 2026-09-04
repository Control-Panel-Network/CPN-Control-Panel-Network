//! Per-account MFA state under the CPN data dir (`/var/lib/cpn/mfa/` by default).
//! TOTP secrets are AES-256-GCM encrypted at rest. Backup codes are PBKDF2 hashed.
//! The AES key is generated on first use and stored as raw 32 bytes at
//! `mfa/mfa-encryption.key` (mode 600). Unique per install; never a committed default.
//! Secrets are never logged or Debug-printed.

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

/// MFA at-rest record. No `Debug`: never log or print this struct (CodeQL / secrets).
#[derive(Clone, Serialize, Deserialize)]
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

#[derive(Clone, Serialize, Deserialize)]
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

/// Decode a 64-char hex string into 32 bytes without a zero-filled key buffer
/// (CodeQL flags `[0u8; 32]` used as a key).
fn decode_hex_key32(hex: &str) -> Result<[u8; 32], String> {
    if hex.len() != 64 || !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err("MFA encryption key must be 32 raw bytes or 64 hex chars".into());
    }
    let bytes = hex.as_bytes();
    let mut out = Vec::with_capacity(32);
    let mut i = 0;
    while i + 1 < bytes.len() {
        let hi = (bytes[i] as char)
            .to_digit(16)
            .ok_or_else(|| "Corrupt MFA encryption key".to_string())?;
        let lo = (bytes[i + 1] as char)
            .to_digit(16)
            .ok_or_else(|| "Corrupt MFA encryption key".to_string())?;
        out.push(((hi << 4) | lo) as u8);
        i += 2;
    }
    out.try_into()
        .map_err(|_| "Corrupt MFA encryption key length".into())
}

/// Load the per-install AES-256 key, or generate and persist one (mode 600).
/// Unique on every fresh data dir; never a committed default.
fn load_or_create_mfa_key() -> Result<[u8; 32], String> {
    let path = mfa_key_path();
    if path.is_file() {
        let raw =
            fs::read(&path).map_err(|err| format!("Could not read {}: {err}", path.display()))?;
        if raw.len() == 32 {
            return raw
                .try_into()
                .map_err(|_| "Corrupt MFA encryption key length".into());
        }
        // Legacy installs stored 64 hex chars; migrate to raw 32 bytes.
        if let Ok(text) = std::str::from_utf8(&raw) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                let key = decode_hex_key32(trimmed)?;
                write_secret_file(&path, &key)?;
                return Ok(key);
            }
        }
        return Err("Corrupt MFA encryption key on disk".into());
    }
    let key: [u8; 32] = rand::rng().random();
    write_secret_file(&path, &key)?;
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
/// Patches on-disk JSON so a loaded secret-bearing struct is not written back.
pub fn save_mfa_for_rename(old_username: &str, new_username: &str) -> Result<(), String> {
    let old_path = mfa_record_path(old_username);
    if !old_path.is_file() {
        return Ok(());
    }
    let raw =
        fs::read_to_string(&old_path).map_err(|err| format!("Could not read MFA record: {err}"))?;
    let mut value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|err| format!("Corrupt MFA record: {err}"))?;
    if let Some(obj) = value.as_object_mut() {
        obj.insert(
            "username".into(),
            serde_json::Value::String(new_username.to_string()),
        );
        obj.insert(
            "updated_at_unix".into(),
            serde_json::Value::from(now_unix()),
        );
    }
    let json = serde_json::to_string_pretty(&value)
        .map_err(|err| format!("Could not serialize MFA record: {err}"))?;
    let new_path = mfa_record_path(new_username);
    write_secret_file(&new_path, json.as_bytes())?;
    if !old_username.eq_ignore_ascii_case(new_username) {
        let _ = fs::remove_file(&old_path);
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
    if !load_mfa(username).totp_enabled {
        return Err("TOTP is not enabled".into());
    }
    if !verify_mfa_challenge(username, code_or_backup)? {
        return Err("Invalid authenticator or backup code".into());
    }
    // Persist a fresh empty record (do not rewrite a loaded secret-bearing struct).
    save_mfa(&empty_record(username))?;
    let _ = fs::remove_file(pending_path(username));
    Ok(())
}

/// Persist only backup-code fields by patching the on-disk JSON object.
fn persist_backup_code_lists(
    path: &Path,
    hashes: Vec<String>,
    salts: Vec<String>,
) -> Result<(), String> {
    let raw =
        fs::read_to_string(path).map_err(|err| format!("Could not read MFA record: {err}"))?;
    let mut value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|err| format!("Corrupt MFA record: {err}"))?;
    let obj = value
        .as_object_mut()
        .ok_or_else(|| "Corrupt MFA record root".to_string())?;
    obj.insert(
        "backup_code_hashes".into(),
        serde_json::to_value(hashes)
            .map_err(|err| format!("Could not encode backup hashes: {err}"))?,
    );
    obj.insert(
        "backup_code_salts".into(),
        serde_json::to_value(salts)
            .map_err(|err| format!("Could not encode backup salts: {err}"))?,
    );
    obj.insert(
        "updated_at_unix".into(),
        serde_json::Value::from(now_unix()),
    );
    let json = serde_json::to_string_pretty(&value)
        .map_err(|err| format!("Could not serialize MFA record: {err}"))?;
    write_secret_file(path, json.as_bytes())
}

/// Consume one backup code. Reads hash/salt lists from JSON only (never via
/// `load_mfa`, so CodeQL does not treat persistence as cleartext logging of MFA).
fn consume_backup_code(username: &str, code: &str) -> bool {
    let path = mfa_record_path(username);
    let Ok(raw) = fs::read_to_string(&path) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return false;
    };
    let Some(obj) = value.as_object() else {
        return false;
    };
    let Ok(mut hashes) = serde_json::from_value::<Vec<String>>(
        obj.get("backup_code_hashes")
            .cloned()
            .unwrap_or_else(|| serde_json::Value::Array(Vec::new())),
    ) else {
        return false;
    };
    let Ok(mut salts) = serde_json::from_value::<Vec<String>>(
        obj.get("backup_code_salts")
            .cloned()
            .unwrap_or_else(|| serde_json::Value::Array(Vec::new())),
    ) else {
        return false;
    };
    let mut match_idx: Option<usize> = None;
    for (i, hash) in hashes.iter().enumerate() {
        let Some(salt) = salts.get(i).map(String::as_str).filter(|s| !s.is_empty()) else {
            continue;
        };
        if verify_backup_code(code, hash, salt) {
            match_idx = Some(i);
            break;
        }
    }
    let Some(i) = match_idx else {
        return false;
    };
    hashes.remove(i);
    if i < salts.len() {
        salts.remove(i);
    }
    persist_backup_code_lists(&path, hashes, salts).is_ok()
}

/// Verify TOTP or a single-use backup code. Updates rate limit on failure.
pub fn verify_mfa_challenge(username: &str, code_raw: &str) -> Result<bool, String> {
    check_rate_limit(username)?;
    let record = load_mfa(username);
    if !record.totp_enabled {
        return Ok(true);
    }
    let secret_b32 = decrypt_secret(&record.totp_secret_enc, &record.totp_nonce)?;
    let secret = decode_totp_secret(&secret_b32)?;
    if verify_totp_code(&secret, code_raw, now_unix()) {
        clear_rate_limit(username);
        return Ok(true);
    }
    if consume_backup_code(username, code_raw) {
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

    #[test]
    fn mfa_encryption_key_is_unique_per_data_dir() {
        let key_a = with_test_data_dir(|| {
            let key = load_or_create_mfa_key().unwrap();
            assert_eq!(load_or_create_mfa_key().unwrap(), key);
            assert_eq!(fs::read(mfa_key_path()).unwrap().len(), 32);
            key
        });
        let key_b = with_test_data_dir(|| load_or_create_mfa_key().unwrap());
        assert_ne!(key_a, key_b);
    }
}
