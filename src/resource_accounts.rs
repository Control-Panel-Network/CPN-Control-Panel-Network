//! Lightweight database and FTP account registries for package quota counting.
//!
//! Full DB/FTP management UI lands later; these JSON registries let package ACL
//! enforce limits as soon as create APIs are used (panel, CLI, or future tools).

use crate::account::{data_dir, now_unix};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseRecord {
    pub id: String,
    pub name: String,
    pub owner: String,
    #[serde(default)]
    pub domain: String,
    pub created_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FtpAccountRecord {
    pub id: String,
    pub username: String,
    pub owner: String,
    #[serde(default)]
    pub domain: String,
    pub created_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DatabasesFile {
    #[serde(default)]
    schema_version: u32,
    #[serde(default)]
    databases: Vec<DatabaseRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct FtpFile {
    #[serde(default)]
    schema_version: u32,
    #[serde(default)]
    accounts: Vec<FtpAccountRecord>,
}

fn databases_path() -> PathBuf {
    data_dir().join("databases-registry.json")
}

fn ftp_path() -> PathBuf {
    data_dir().join("ftp-accounts.json")
}

fn write_json(path: &Path, raw: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create data dir: {e}"))?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write;
    let mut file = options
        .open(path)
        .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    file.write_all(raw.as_bytes())
        .map_err(|e| format!("Could not save {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn load_databases_file() -> DatabasesFile {
    let Ok(raw) = fs::read_to_string(databases_path()) else {
        return DatabasesFile {
            schema_version: SCHEMA_VERSION,
            databases: Vec::new(),
        };
    };
    serde_json::from_str(&raw).unwrap_or(DatabasesFile {
        schema_version: SCHEMA_VERSION,
        databases: Vec::new(),
    })
}

fn load_ftp_file() -> FtpFile {
    let Ok(raw) = fs::read_to_string(ftp_path()) else {
        return FtpFile {
            schema_version: SCHEMA_VERSION,
            accounts: Vec::new(),
        };
    };
    serde_json::from_str(&raw).unwrap_or(FtpFile {
        schema_version: SCHEMA_VERSION,
        accounts: Vec::new(),
    })
}

fn has_control_chars(value: &str) -> bool {
    value.chars().any(|ch| ch.is_control())
}

pub fn list_databases() -> Vec<DatabaseRecord> {
    load_databases_file().databases
}

pub fn list_ftp_accounts() -> Vec<FtpAccountRecord> {
    load_ftp_file().accounts
}

pub fn create_database(
    owner: &str,
    name_raw: &str,
    domain: &str,
) -> Result<DatabaseRecord, String> {
    let owner = owner.trim();
    let name = name_raw.trim().to_ascii_lowercase();
    if owner.is_empty() {
        return Err("Owner is required".into());
    }
    if name.is_empty() {
        return Err("Database name is required".into());
    }
    if has_control_chars(owner) || has_control_chars(&name) {
        return Err("Owner and database name cannot include control characters".into());
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(
            "Database name may only contain letters, digits, underscore, and hyphen".into(),
        );
    }
    let mut file = load_databases_file();
    if file
        .databases
        .iter()
        .any(|d| d.name.eq_ignore_ascii_case(&name))
    {
        return Err(format!("Database `{name}` already exists"));
    }
    let record = DatabaseRecord {
        id: format!("db-{}", now_unix()),
        name,
        owner: owner.to_string(),
        domain: domain.trim().to_ascii_lowercase(),
        created_at_unix: now_unix(),
    };
    file.schema_version = SCHEMA_VERSION;
    file.databases.push(record.clone());
    let raw = serde_json::to_string_pretty(&file)
        .map_err(|e| format!("Could not serialize databases: {e}"))?;
    write_json(&databases_path(), &raw)?;
    Ok(record)
}

pub fn create_ftp_account(
    owner: &str,
    username_raw: &str,
    domain: &str,
) -> Result<FtpAccountRecord, String> {
    let owner = owner.trim();
    let username = username_raw.trim().to_ascii_lowercase();
    if owner.is_empty() {
        return Err("Owner is required".into());
    }
    if username.is_empty() {
        return Err("FTP username is required".into());
    }
    if has_control_chars(owner) || has_control_chars(&username) {
        return Err("Owner and FTP username cannot include control characters".into());
    }
    let mut file = load_ftp_file();
    if file
        .accounts
        .iter()
        .any(|a| a.username.eq_ignore_ascii_case(&username))
    {
        return Err(format!("FTP account `{username}` already exists"));
    }
    let record = FtpAccountRecord {
        id: format!("ftp-{}", now_unix()),
        username,
        owner: owner.to_string(),
        domain: domain.trim().to_ascii_lowercase(),
        created_at_unix: now_unix(),
    };
    file.schema_version = SCHEMA_VERSION;
    file.accounts.push(record.clone());
    let raw = serde_json::to_string_pretty(&file)
        .map_err(|e| format!("Could not serialize FTP accounts: {e}"))?;
    write_json(&ftp_path(), &raw)?;
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn create_database_and_ftp_roundtrip() {
        with_test_data_dir(|| {
            let db = create_database("admin", "app_db", "example.com").unwrap();
            assert_eq!(db.name, "app_db");
            assert_eq!(list_databases().len(), 1);
            let ftp = create_ftp_account("admin", "siteftp", "example.com").unwrap();
            assert_eq!(ftp.username, "siteftp");
            assert_eq!(list_ftp_accounts().len(), 1);
            assert!(create_database("admin", "app_db", "").is_err());
        });
    }
}
