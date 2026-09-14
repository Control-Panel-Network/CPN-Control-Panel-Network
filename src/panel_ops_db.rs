//! MariaDB database listing helpers (CLI; no stored credentials).
//!
//! CPN's host database is MariaDB. Client/dump helpers prefer MariaDB binaries
//! (`mariadb`, `mariadb-dump`) and fall back to MySQL-compatible names that
//! MariaDB and cPanel-style tooling often ship as aliases (`mysql`, `mysqldump`).

use crate::service_detect::detect_database;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct DbListStatus {
    pub engine_label: String,
    pub listening: bool,
    pub databases: Vec<String>,
    pub detail: String,
}

/// Preferred MariaDB/MySQL-compatible client binaries (MariaDB first).
pub fn client_bin_candidates() -> &'static [&'static str] {
    &["mariadb", "mysql"]
}

/// Preferred dump binaries for CPN backups (MariaDB first).
pub fn dump_bin_candidates() -> &'static [&'static str] {
    &["mariadb-dump", "mysqldump"]
}

fn first_working_bin(candidates: &[&'static str]) -> Option<&'static str> {
    candidates.iter().copied().find(|&candidate| {
        Command::new(candidate)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

fn mariadb_cli() -> Option<&'static str> {
    first_working_bin(client_bin_candidates())
}

/// Public MariaDB client binary for restore/import paths (`mariadb`, else `mysql`).
pub fn mariadb_client_bin() -> Option<&'static str> {
    mariadb_cli()
}

/// Dump binary for selective backups (`mariadb-dump`, else `mysqldump`).
pub fn mariadb_dump_bin() -> Option<&'static str> {
    first_working_bin(dump_bin_candidates())
}

/// True when a local MariaDB-compatible server is detected for backup/restore SQL.
pub fn local_mariadb_ready() -> bool {
    let detected = detect_database();
    detected.listening_3306 || detected.service_label != "Not detected"
}

pub fn list_databases() -> DbListStatus {
    let detected = detect_database();
    let Some(bin) = mariadb_cli() else {
        return DbListStatus {
            engine_label: detected.service_label,
            listening: detected.listening_3306,
            databases: vec![],
            detail: "MariaDB client not found (`mariadb` or `mysql`). Install MariaDB from Host packages to manage databases from the panel."
                .into(),
        };
    };
    let out = Command::new(bin)
        .args(["-N", "-e", "SHOW DATABASES;"])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            let dbs: Vec<String> = String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            DbListStatus {
                engine_label: detected.service_label,
                listening: detected.listening_3306,
                databases: dbs,
                detail: format!("Listed via `{bin}` (socket/local auth)."),
            }
        }
        Ok(o) => DbListStatus {
            engine_label: detected.service_label,
            listening: detected.listening_3306,
            databases: vec![],
            detail: format!(
                "Client present but list failed: {}",
                String::from_utf8_lossy(&o.stderr).trim()
            ),
        },
        Err(e) => DbListStatus {
            engine_label: detected.service_label,
            listening: detected.listening_3306,
            databases: vec![],
            detail: format!("Failed to run client: {e}"),
        },
    }
}

pub fn create_database(name: &str) -> Result<String, String> {
    let name = sanitize_db_ident(name)?;
    let bin = mariadb_cli().ok_or_else(|| {
        "MariaDB client not found (`mariadb` or `mysql`). Install MariaDB from Host packages."
            .to_string()
    })?;
    let sql = format!("CREATE DATABASE IF NOT EXISTS `{name}`;");
    let out = Command::new(bin)
        .args(["-e", &sql])
        .output()
        .map_err(|e| format!("Failed to run {bin}: {e}"))?;
    if out.status.success() {
        Ok(format!("Created database `{name}` (or already existed)"))
    } else {
        Err(format!(
            "CREATE DATABASE failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

fn sanitize_db_password(raw: &str) -> Result<String, String> {
    let pass = raw.trim();
    if pass.is_empty() || pass.chars().count() > 128 {
        return Err("Database password must be 1-128 characters".into());
    }
    if pass.chars().any(|c| c.is_control()) {
        return Err("Database password cannot include control characters".into());
    }
    Ok(pass.to_string())
}

fn sql_escape_password(pass: &str) -> String {
    pass.replace('\\', "\\\\").replace('\'', "''")
}

/// Create database, dedicated user, and grants (local MariaDB socket auth).
pub fn create_database_with_user(
    db_name: &str,
    db_user: &str,
    db_password: &str,
) -> Result<String, String> {
    let name = sanitize_db_ident(db_name)?;
    let user = sanitize_db_ident(db_user)?;
    let pass = sanitize_db_password(db_password)?;
    let pass_sql = sql_escape_password(&pass);
    let bin = mariadb_cli().ok_or_else(|| {
        "MariaDB client not found (`mariadb` or `mysql`). Install MariaDB from Host packages."
            .to_string()
    })?;

    let modern_sql = format!(
        "CREATE DATABASE IF NOT EXISTS `{name}`; \
         CREATE USER IF NOT EXISTS '{user}'@'localhost' IDENTIFIED BY '{pass_sql}'; \
         GRANT ALL PRIVILEGES ON `{name}`.* TO '{user}'@'localhost'; \
         FLUSH PRIVILEGES;"
    );
    let out = Command::new(bin)
        .args(["-e", &modern_sql])
        .output()
        .map_err(|e| format!("Failed to run {bin}: {e}"))?;
    if out.status.success() {
        return Ok(format!(
            "Created database `{name}` with user `{user}`@localhost"
        ));
    }

    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let legacy_sql = format!(
        "CREATE DATABASE IF NOT EXISTS `{name}`; \
         CREATE USER '{user}'@'localhost' IDENTIFIED BY '{pass_sql}'; \
         GRANT ALL PRIVILEGES ON `{name}`.* TO '{user}'@'localhost'; \
         FLUSH PRIVILEGES;"
    );
    let legacy = Command::new(bin)
        .args(["-e", &legacy_sql])
        .output()
        .map_err(|e| format!("Failed to run {bin}: {e}"))?;
    if legacy.status.success() {
        return Ok(format!(
            "Created database `{name}` with user `{user}`@localhost (legacy user create)"
        ));
    }
    Err(format!(
        "CREATE DATABASE/USER failed: {} / legacy: {}",
        stderr,
        String::from_utf8_lossy(&legacy.stderr).trim()
    ))
}

pub fn drop_database(name: &str) -> Result<String, String> {
    let name = sanitize_db_ident(name)?;
    if matches!(
        name.as_str(),
        "mysql" | "information_schema" | "performance_schema" | "sys"
    ) {
        return Err("Refusing to drop a system database".into());
    }
    let bin = mariadb_cli().ok_or_else(|| {
        "MariaDB client not found (`mariadb` or `mysql`). Install MariaDB from Host packages."
            .to_string()
    })?;
    let sql = format!("DROP DATABASE IF EXISTS `{name}`;");
    let out = Command::new(bin)
        .args(["-e", &sql])
        .output()
        .map_err(|e| format!("Failed to run {bin}: {e}"))?;
    if out.status.success() {
        Ok(format!("Dropped database `{name}` (if it existed)"))
    } else {
        Err(format!(
            "DROP DATABASE failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

fn sanitize_db_ident(raw: &str) -> Result<String, String> {
    let name = raw.trim();
    if name.is_empty() || name.len() > 64 {
        return Err("Database name must be 1-64 characters".into());
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("Database name may only contain letters, digits, and underscore".into());
    }
    Ok(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_idents() {
        assert!(sanitize_db_ident("").is_err());
        assert!(sanitize_db_ident("a-b").is_err());
        assert!(sanitize_db_ident("ok_name").is_ok());
    }

    #[test]
    fn refuses_system_drop() {
        assert!(drop_database("mysql").is_err());
    }

    #[test]
    fn prefers_mariadb_binaries_over_mysql_aliases() {
        assert_eq!(client_bin_candidates(), &["mariadb", "mysql"]);
        assert_eq!(dump_bin_candidates(), &["mariadb-dump", "mysqldump"]);
        assert_eq!(client_bin_candidates()[0], "mariadb");
        assert_eq!(dump_bin_candidates()[0], "mariadb-dump");
    }
}
