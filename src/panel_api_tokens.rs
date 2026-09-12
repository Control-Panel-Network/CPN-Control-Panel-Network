//! Panel API tokens (`cpn_` prefix). Secrets hashed with SHA256 at rest.
//! Store: `/var/lib/cpn/api-tokens.json` (mode 600). CPN session auth remains primary.

use crate::account::now_unix;
use crate::paths;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::PathBuf};

pub const TOKEN_PREFIX: &str = "cpn_";
pub const VALID_SCOPES: &[&str] = &["read", "dns", "admin"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiTokenRecord {
    pub id: String,
    pub label: String,
    pub username: String,
    pub scopes: Vec<String>,
    pub token_hash: String,
    pub created_at_unix: u64,
    #[serde(default)]
    pub last_used_at_unix: Option<u64>,
    #[serde(default)]
    pub revoked_at_unix: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ApiTokenStore {
    #[serde(default = "default_schema_version")]
    schema_version: u32,
    #[serde(default)]
    tokens: Vec<ApiTokenRecord>,
}

fn default_schema_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiTokenPublic {
    pub id: String,
    pub label: String,
    pub username: String,
    pub scopes: Vec<String>,
    pub created_at_unix: u64,
    pub last_used_at_unix: Option<u64>,
    pub revoked: bool,
}

pub fn api_tokens_path() -> PathBuf {
    paths::join_data("api-tokens.json")
}

fn hash_token(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    format!("{:x}", hasher.finalize())
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

fn load_store() -> ApiTokenStore {
    let path = api_tokens_path();
    let Ok(raw) = fs::read_to_string(&path) else {
        return ApiTokenStore::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn persist_store(store: &ApiTokenStore) -> Result<(), String> {
    let json = serde_json::to_string_pretty(store)
        .map_err(|e| format!("Could not serialize API tokens: {e}"))?;
    write_mode_600(&api_tokens_path(), json.as_bytes())
}

fn record_to_public(record: &ApiTokenRecord) -> ApiTokenPublic {
    ApiTokenPublic {
        id: record.id.clone(),
        label: record.label.clone(),
        username: record.username.clone(),
        scopes: record.scopes.clone(),
        created_at_unix: record.created_at_unix,
        last_used_at_unix: record.last_used_at_unix,
        revoked: record.revoked_at_unix.is_some(),
    }
}

pub fn normalize_scopes(raw: &[String]) -> Result<Vec<String>, String> {
    if raw.is_empty() {
        return Ok(vec!["read".into()]);
    }
    let mut out = Vec::new();
    for scope in raw {
        let s = scope.trim().to_ascii_lowercase();
        if s.is_empty() {
            continue;
        }
        if !VALID_SCOPES.contains(&s.as_str()) {
            return Err(format!(
                "Invalid scope `{s}`. Allowed: {}",
                VALID_SCOPES.join(", ")
            ));
        }
        if !out.iter().any(|x| x == &s) {
            out.push(s);
        }
    }
    if out.is_empty() {
        out.push("read".into());
    }
    Ok(out)
}

fn generate_secret() -> String {
    let mut rng = rand::rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.random()).collect();
    let body: String = bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>()
        .chars()
        .take(48)
        .collect();
    format!("{TOKEN_PREFIX}{body}")
}

/// Ensure the JSON token store exists (migration 0001 hook).
pub fn ensure_store_migrated() -> Result<(), String> {
    let path = api_tokens_path();
    if path.exists() {
        return Ok(());
    }
    persist_store(&ApiTokenStore::default())
}

pub fn issue_token(
    username: &str,
    label: &str,
    scopes: &[String],
) -> Result<(ApiTokenPublic, String), String> {
    ensure_store_migrated()?;
    let username = username.trim();
    if username.is_empty() {
        return Err("Username is required".into());
    }
    let label = label.trim();
    if label.is_empty() {
        return Err("Token label is required".into());
    }
    if label.len() > 128 {
        return Err("Token label is too long (max 128)".into());
    }
    let scopes = normalize_scopes(scopes)?;
    let secret = generate_secret();
    let record = ApiTokenRecord {
        id: uuid::Uuid::new_v4().to_string(),
        label: label.to_string(),
        username: username.to_string(),
        scopes,
        token_hash: hash_token(&secret),
        created_at_unix: now_unix(),
        last_used_at_unix: None,
        revoked_at_unix: None,
    };
    let public = record_to_public(&record);
    let mut store = load_store();
    store.schema_version = 1;
    store.tokens.push(record);
    persist_store(&store)?;
    Ok((public, secret))
}

pub fn list_tokens(viewer: &str, admin: bool) -> Vec<ApiTokenPublic> {
    let _ = ensure_store_migrated();
    let viewer = viewer.trim();
    load_store()
        .tokens
        .iter()
        .filter(|t| admin || t.username.eq_ignore_ascii_case(viewer))
        .map(record_to_public)
        .collect()
}

pub fn revoke_token(token_id: &str, viewer: &str, admin: bool) -> Result<(), String> {
    ensure_store_migrated()?;
    let token_id = token_id.trim();
    if token_id.is_empty() {
        return Err("Token id is required".into());
    }
    let mut store = load_store();
    let Some(record) = store.tokens.iter_mut().find(|t| t.id == token_id) else {
        return Err("Token not found".into());
    };
    if !admin && !record.username.eq_ignore_ascii_case(viewer.trim()) {
        return Err("You can only revoke your own tokens".into());
    }
    if record.revoked_at_unix.is_none() {
        record.revoked_at_unix = Some(now_unix());
    }
    persist_store(&store)
}

pub fn authenticate_bearer(raw: &str) -> Option<(String, Vec<String>)> {
    let token = raw.trim();
    if !token.starts_with(TOKEN_PREFIX) || token.len() < TOKEN_PREFIX.len() + 8 {
        return None;
    }
    let _ = ensure_store_migrated();
    let hash = hash_token(token);
    let mut store = load_store();
    let idx = store.tokens.iter().position(|t| {
        t.token_hash == hash && t.revoked_at_unix.is_none()
    })?;
    let record = &mut store.tokens[idx];
    record.last_used_at_unix = Some(now_unix());
    let username = record.username.clone();
    let scopes = record.scopes.clone();
    let _ = persist_store(&store);
    Some((username, scopes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn issue_list_revoke_roundtrip() {
        with_test_data_dir(|| {
            ensure_store_migrated().unwrap();
            let (pub1, secret) = issue_token("admin", "lab token", &["read".into(), "dns".into()])
                .unwrap();
            assert!(secret.starts_with(TOKEN_PREFIX));
            assert!(!pub1.revoked);
            assert_eq!(pub1.scopes, vec!["read", "dns"]);
            let listed = list_tokens("admin", false);
            assert_eq!(listed.len(), 1);
            let auth = authenticate_bearer(&secret).unwrap();
            assert_eq!(auth.0, "admin");
            revoke_token(&pub1.id, "admin", false).unwrap();
            assert!(authenticate_bearer(&secret).is_none());
        });
    }

    #[test]
    fn admin_sees_all_tokens() {
        with_test_data_dir(|| {
            issue_token("alice", "a", &[]).unwrap();
            issue_token("bob", "b", &[]).unwrap();
            assert_eq!(list_tokens("alice", false).len(), 1);
            assert_eq!(list_tokens("admin", true).len(), 2);
        });
    }

    #[test]
    fn reject_invalid_scope() {
        with_test_data_dir(|| {
            let err = issue_token("admin", "x", &["superuser".into()])
                .unwrap_err()
                .to_ascii_lowercase();
            assert!(err.contains("invalid scope"));
        });
    }
}
