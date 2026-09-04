//! MariaDB database listing helpers (CLI; no stored credentials).

use crate::service_detect::detect_database;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct DbListStatus {
    pub engine_label: String,
    pub listening: bool,
    pub databases: Vec<String>,
    pub detail: String,
}

fn mariadb_cli() -> Option<&'static str> {
    ["mariadb", "mysql"].into_iter().find(|&candidate| {
        Command::new(candidate)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

pub fn list_databases() -> DbListStatus {
    let detected = detect_database();
    let Some(bin) = mariadb_cli() else {
        return DbListStatus {
            engine_label: detected.service_label,
            listening: detected.listening_3306,
            databases: vec![],
            detail: "MariaDB/MySQL client not found. Install MariaDB to manage databases from the panel."
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
    let bin = mariadb_cli().ok_or_else(|| "MariaDB/MySQL client not found".to_string())?;
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

pub fn drop_database(name: &str) -> Result<String, String> {
    let name = sanitize_db_ident(name)?;
    if matches!(
        name.as_str(),
        "mysql" | "information_schema" | "performance_schema" | "sys"
    ) {
        return Err("Refusing to drop a system database".into());
    }
    let bin = mariadb_cli().ok_or_else(|| "MariaDB/MySQL client not found".to_string())?;
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
}
