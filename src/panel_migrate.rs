//! Versioned SQL migrations for CPN panel data stores.
//! Ledger: `schema_migrations.json`. SQL files under `sql/` via include_str.
//! JSON store hooks run always; optional `sqlite3 panel.db` when CLI is present.

use crate::account::now_unix;
use crate::panel_api_tokens::ensure_store_migrated;
use crate::panel_ops_cloudflare::ensure_oauth_schema_migrated;
use crate::panel_ops_cloudflare_oauth::ensure_oauth_stores_migrated;
use crate::paths;
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::PathBuf, process::Command};

const SQL_0001: &str = include_str!("../sql/0001_panel_api_tokens.sql");
const SQL_0002: &str = include_str!("../sql/0002_cloudflare_oauth.sql");
const SQL_0003: &str = include_str!("../sql/0003_cloudflare_settings_oauth.sql");

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct MigrationLedger {
    #[serde(default)]
    applied: Vec<String>,
    #[serde(default)]
    last_run_unix: u64,
}

struct MigrationDef {
    id: &'static str,
    sql: &'static str,
    hook: fn() -> Result<(), String>,
}

const MIGRATIONS: &[MigrationDef] = &[
    MigrationDef {
        id: "0001_panel_api_tokens",
        sql: SQL_0001,
        hook: ensure_store_migrated,
    },
    MigrationDef {
        id: "0002_cloudflare_oauth",
        sql: SQL_0002,
        hook: ensure_oauth_stores_migrated,
    },
    MigrationDef {
        id: "0003_cloudflare_settings_oauth",
        sql: SQL_0003,
        hook: ensure_oauth_schema_migrated,
    },
];

pub fn schema_migrations_path() -> PathBuf {
    paths::join_data("schema_migrations.json")
}

pub fn panel_db_path() -> PathBuf {
    paths::join_data("panel.db")
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

fn load_ledger() -> MigrationLedger {
    let path = schema_migrations_path();
    let Ok(raw) = fs::read_to_string(&path) else {
        return MigrationLedger::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn persist_ledger(ledger: &MigrationLedger) -> Result<(), String> {
    let json = serde_json::to_string_pretty(ledger)
        .map_err(|e| format!("Could not serialize migration ledger: {e}"))?;
    write_mode_600(&schema_migrations_path(), json.as_bytes())
}

fn sqlite3_available() -> bool {
    Command::new("sqlite3")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn apply_sql_to_panel_db(sql: &str) -> Result<(), String> {
    if !sqlite3_available() {
        return Ok(());
    }
    let db = panel_db_path();
    let dir = paths::default_data_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    // Pipe SQL on stdin. Passing SQL as argv fails when the file starts with
    // `--` comments (sqlite3 treats those as CLI options).
    let mut child = Command::new("sqlite3")
        .arg(&db)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not run sqlite3: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(sql.as_bytes())
            .map_err(|e| format!("Could not write SQL to sqlite3: {e}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("Could not wait for sqlite3: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "sqlite3 migration failed: {}",
            stderr.trim().chars().take(240).collect::<String>()
        ))
    }
}

/// Apply pending migrations in order. Idempotent: already-applied ids are skipped.
pub fn run_pending_migrations() -> Result<Vec<String>, String> {
    let mut ledger = load_ledger();
    let mut applied_now = Vec::new();
    for migration in MIGRATIONS {
        if ledger.applied.iter().any(|id| id == migration.id) {
            continue;
        }
        (migration.hook)()?;
        apply_sql_to_panel_db(migration.sql)?;
        if !ledger.applied.iter().any(|id| id == migration.id) {
            ledger.applied.push(migration.id.to_string());
        }
        applied_now.push(migration.id.to_string());
    }
    if !applied_now.is_empty() {
        ledger.last_run_unix = now_unix();
        persist_ledger(&ledger)?;
    }
    Ok(applied_now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn migrations_apply_idempotently() {
        with_test_data_dir(|| {
            let first = run_pending_migrations().unwrap();
            assert_eq!(first.len(), 3);
            assert!(first.contains(&"0001_panel_api_tokens".to_string()));
            let second = run_pending_migrations().unwrap();
            assert!(second.is_empty());
            let ledger = load_ledger();
            assert_eq!(ledger.applied.len(), 3);
        });
    }

    #[test]
    fn hook_creates_api_token_store() {
        with_test_data_dir(|| {
            run_pending_migrations().unwrap();
            assert!(crate::panel_api_tokens::api_tokens_path().exists());
        });
    }
}
