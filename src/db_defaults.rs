//! MariaDB + phpMyAdmin as the default database stack for Linux installs.
//!
//! Operators may skip the database / phpMyAdmin. CPN installs MariaDB only
//! (MySQL-compatible). Legacy `mysql` CLI/UI values map to MariaDB.

use crate::apps::{AppId, install_app};
use crate::model::{DatabaseEngine, ServerEngine};
use crate::os_support::{GuestOs, PackageFamily, detect_guest_os};
use crate::service_detect::detect_database;

/// Install the default (or operator-selected) database stack after the web server stage.
///
/// - `DatabaseEngine::Mariadb` (default): install/start MariaDB unless already present.
/// - `DatabaseEngine::None`: skip the database engine.
/// - `install_phpmyadmin`: when true (default), install phpMyAdmin packages.
/// - `web_server`: when set, phpMyAdmin skips starting nginx under OLS/Caddy.
///
/// Windows Phase A skips package recipes; the caller should treat that as a soft skip.
pub fn ensure_database_defaults(
    database: DatabaseEngine,
    install_phpmyadmin: bool,
    web_server: Option<ServerEngine>,
) -> Result<Vec<String>, String> {
    let mut notes = Vec::new();
    match detect_guest_os() {
        Ok(guest) if guest.is_windows() => {
            notes.push(
                "Database defaults skipped on Windows Server Phase A (no dnf/apt recipes).".into(),
            );
            return Ok(notes);
        }
        Ok(guest) if !guest_supports_db_packages(&guest) => {
            notes.push(format!(
                "Database defaults skipped: guest {} has no dnf/apt package path.",
                guest.label
            ));
            return Ok(notes);
        }
        Err(error) => {
            return Err(format!(
                "Could not detect guest OS for database defaults: {error}"
            ));
        }
        Ok(_) => {}
    }

    match database {
        DatabaseEngine::None => {
            notes.push("Database engine skipped by operator (--database none).".into());
        }
        DatabaseEngine::Mariadb => {
            notes.push(ensure_mariadb()?);
        }
    }

    if install_phpmyadmin {
        notes.push(ensure_phpmyadmin(web_server)?);
    } else {
        notes.push("phpMyAdmin skipped by operator (--skip-phpmyadmin).".into());
    }

    Ok(notes)
}

fn guest_supports_db_packages(guest: &GuestOs) -> bool {
    matches!(guest.family, PackageFamily::Dnf | PackageFamily::Apt)
}

fn ensure_mariadb() -> Result<String, String> {
    let status = detect_database();
    if status.service_label.starts_with("MariaDB") && status.listening_3306 {
        return Ok("MariaDB already running on :3306.".into());
    }
    if status.service_label.starts_with("MySQL") || status.service_label.starts_with("mysqld") {
        return Err(
            "Oracle MySQL appears installed on this host. CPN supports MariaDB only; uninstall MySQL server packages first, then re-run with --database mariadb."
                .into(),
        );
    }
    install_app(AppId::Mariadb)
}

fn ensure_phpmyadmin(web_server: Option<ServerEngine>) -> Result<String, String> {
    crate::apps_phpmyadmin::install_and_expose_for(web_server)
}

#[cfg(test)]
mod tests {
    use crate::model::DatabaseEngine;

    #[test]
    fn database_engine_default_is_mariadb() {
        assert_eq!(DatabaseEngine::default(), DatabaseEngine::Mariadb);
    }

    #[test]
    fn parse_cli_database_values() {
        assert_eq!(
            DatabaseEngine::parse_cli("mariadb").unwrap(),
            DatabaseEngine::Mariadb
        );
        assert_eq!(
            DatabaseEngine::parse_cli("mysql").unwrap(),
            DatabaseEngine::Mariadb
        );
        assert_eq!(
            DatabaseEngine::parse_cli("none").unwrap(),
            DatabaseEngine::None
        );
        assert!(DatabaseEngine::parse_cli("postgres").is_err());
    }
}
