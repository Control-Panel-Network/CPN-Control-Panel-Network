//! Passkey (WebAuthn) credential storage under the CPN data directory.
//! Credential material lives only on disk (mode 0600). Never log secrets.

use crate::account::{data_dir, now_unix};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use webauthn_rs::prelude::Passkey;

const SCHEMA: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredPasskey {
    pub id: String,
    pub label: String,
    pub created_at_unix: u64,
    pub last_used_unix: u64,
    pub passkey: Passkey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasskeyStore {
    pub schema_version: u32,
    pub username: String,
    pub credentials: Vec<StoredPasskey>,
}

fn passkeys_dir() -> PathBuf {
    data_dir().join("passkeys")
}

fn user_key(username: &str) -> String {
    username
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn store_path(username: &str) -> PathBuf {
    passkeys_dir().join(format!("{}.json", user_key(username)))
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

fn empty_store(username: &str) -> PasskeyStore {
    PasskeyStore {
        schema_version: SCHEMA,
        username: username.to_string(),
        credentials: Vec::new(),
    }
}

pub fn load_passkeys(username: &str) -> PasskeyStore {
    let path = store_path(username);
    let Ok(raw) = fs::read_to_string(&path) else {
        return empty_store(username);
    };
    serde_json::from_str(&raw).unwrap_or_else(|_| empty_store(username))
}

pub fn save_passkeys(store: &PasskeyStore) -> Result<(), String> {
    let path = store_path(&store.username);
    let json = serde_json::to_string_pretty(store)
        .map_err(|err| format!("Could not serialize passkeys: {err}"))?;
    write_secret_file(&path, json.as_bytes())
}

pub fn list_passkey_summaries(username: &str) -> Vec<(String, String, u64, u64)> {
    load_passkeys(username)
        .credentials
        .into_iter()
        .map(|c| (c.id, c.label, c.created_at_unix, c.last_used_unix))
        .collect()
}

pub fn passkey_count(username: &str) -> usize {
    load_passkeys(username).credentials.len()
}

pub fn has_passkeys(username: &str) -> bool {
    passkey_count(username) > 0
}

pub fn add_passkey(username: &str, label: &str, passkey: Passkey) -> Result<StoredPasskey, String> {
    let mut store = load_passkeys(username);
    let id = base64url(passkey.cred_id());
    if store.credentials.iter().any(|c| c.id == id) {
        return Err("This passkey is already registered".into());
    }
    let label = {
        let trimmed = label.trim();
        if trimmed.is_empty() {
            format!("Passkey {}", store.credentials.len() + 1)
        } else {
            trimmed.chars().take(64).collect()
        }
    };
    let entry = StoredPasskey {
        id: id.clone(),
        label,
        created_at_unix: now_unix(),
        last_used_unix: 0,
        passkey,
    };
    store.username = username.to_string();
    store.credentials.push(entry.clone());
    save_passkeys(&store)?;
    Ok(entry)
}

pub fn delete_passkey(username: &str, id: &str) -> Result<(), String> {
    let mut store = load_passkeys(username);
    let before = store.credentials.len();
    store.credentials.retain(|c| c.id != id);
    if store.credentials.len() == before {
        return Err("Passkey not found".into());
    }
    if store.credentials.is_empty() {
        return remove_passkey_store_file(username);
    }
    save_passkeys(&store)
}

/// Update the display label for a stored passkey (max 64 chars).
pub fn rename_passkey(username: &str, id: &str, new_label: &str) -> Result<(), String> {
    let trimmed = new_label.trim();
    if trimmed.is_empty() {
        return Err("Label is required".into());
    }
    let label: String = trimmed.chars().take(64).collect();
    let mut store = load_passkeys(username);
    let Some(entry) = store.credentials.iter_mut().find(|c| c.id == id) else {
        return Err("Passkey not found".into());
    };
    entry.label = label;
    save_passkeys(&store)
}

/// Format a unix timestamp as `dd/mm/yyyy HH:MM` (local when `date` is available).
pub fn format_passkey_timestamp(ts: u64) -> String {
    if ts == 0 {
        return "-".into();
    }
    #[cfg(unix)]
    {
        use std::process::Command;
        if let Ok(out) = Command::new("date")
            .args(["-d", &format!("@{ts}"), "+%d/%m/%Y %H:%M"])
            .output()
        {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !text.is_empty() {
                    return text;
                }
            }
        }
    }
    format_passkey_timestamp_utc(ts)
}

fn format_passkey_timestamp_utc(ts: u64) -> String {
    let days = ts / 86_400;
    let rem = ts % 86_400;
    let hours = rem / 3_600;
    let mins = (rem % 3_600) / 60;
    let (y, m, d) = civil_from_days(days as i64);
    format!("{d:02}/{m:02}/{y} {hours:02}:{mins:02}")
}

/// Howard Hinnant civil_from_days (proleptic Gregorian).
fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

/// Remove every registered passkey for an account. Returns how many were removed.
pub fn clear_all_passkeys(username: &str) -> Result<usize, String> {
    let store = load_passkeys(username);
    let count = store.credentials.len();
    remove_passkey_store_file(username)?;
    Ok(count)
}

fn remove_passkey_store_file(username: &str) -> Result<(), String> {
    let path = store_path(username);
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(&path).map_err(|err| format!("Could not remove {}: {err}", path.display()))
}

pub fn passkeys_for_auth(username: &str) -> Vec<Passkey> {
    load_passkeys(username)
        .credentials
        .into_iter()
        .map(|c| c.passkey)
        .collect()
}

/// Return every registered credential with its owning account. Login is RP-wide,
/// so the user does not have to type an account name before choosing a passkey.
pub fn all_passkeys_for_auth() -> Vec<(String, Passkey)> {
    let Ok(entries) = fs::read_dir(passkeys_dir()) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("json"))
        .filter_map(|entry| fs::read_to_string(entry.path()).ok())
        .filter_map(|raw| serde_json::from_str::<PasskeyStore>(&raw).ok())
        .flat_map(|store| {
            store
                .credentials
                .into_iter()
                .map(move |credential| (store.username.clone(), credential.passkey))
        })
        .collect()
}

pub fn passkey_owner(cred_id: &[u8]) -> Option<String> {
    let wanted = base64url(cred_id);
    let Ok(entries) = fs::read_dir(passkeys_dir()) else {
        return None;
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("json"))
        .filter_map(|entry| fs::read_to_string(entry.path()).ok())
        .filter_map(|raw| serde_json::from_str::<PasskeyStore>(&raw).ok())
        .find_map(|store| {
            store
                .credentials
                .iter()
                .any(|credential| credential.id == wanted)
                .then_some(store.username)
        })
}

pub fn update_passkey_after_auth(
    username: &str,
    cred_id: &[u8],
    updated: Passkey,
) -> Result<(), String> {
    let id = base64url(cred_id);
    let mut store = load_passkeys(username);
    let Some(entry) = store.credentials.iter_mut().find(|c| c.id == id) else {
        return Err("Passkey not found after authentication".into());
    };
    entry.passkey = updated;
    entry.last_used_unix = now_unix();
    save_passkeys(&store)
}

pub fn rename_passkey_store(old_username: &str, new_username: &str) -> Result<(), String> {
    let mut store = load_passkeys(old_username);
    if store.credentials.is_empty() {
        return Ok(());
    }
    store.username = new_username.to_string();
    save_passkeys(&store)?;
    if !old_username.eq_ignore_ascii_case(new_username) {
        let _ = fs::remove_file(store_path(old_username));
    }
    Ok(())
}

pub fn exclude_credential_ids(username: &str) -> Vec<webauthn_rs::prelude::CredentialID> {
    load_passkeys(username)
        .credentials
        .iter()
        .map(|c| c.passkey.cred_id().clone())
        .collect()
}

fn base64url(bytes: &[u8]) -> String {
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn empty_store_roundtrip() {
        with_test_data_dir(|| {
            let store = load_passkeys("Admin");
            assert!(store.credentials.is_empty());
            assert!(!has_passkeys("Admin"));
        });
    }

    #[test]
    fn clear_all_removes_empty_store_file() {
        with_test_data_dir(|| {
            let store = empty_store("admin");
            save_passkeys(&store).expect("save");
            let path = store_path("admin");
            assert!(path.is_file());
            let removed = clear_all_passkeys("admin").expect("clear");
            assert_eq!(removed, 0);
            assert!(!path.exists());
            assert!(!has_passkeys("admin"));
        });
    }

    #[test]
    fn rename_missing_and_empty_label_errors() {
        with_test_data_dir(|| {
            let missing = rename_passkey("admin", "no-such-id", "Laptop").unwrap_err();
            assert!(missing.contains("not found"));
            let empty = rename_passkey("admin", "any", "   ").unwrap_err();
            assert!(empty.contains("required"));
        });
    }

    #[test]
    fn format_passkey_timestamp_utc_style() {
        assert_eq!(format_passkey_timestamp(0), "-");
        // 15/09/2026 17:11:37 UTC (1789492297)
        let formatted = format_passkey_timestamp_utc(1_789_492_297);
        assert_eq!(formatted, "15/09/2026 17:11");
    }
}
