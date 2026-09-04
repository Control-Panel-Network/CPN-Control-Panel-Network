//! Host app lifecycle: detect, install, reinstall, uninstall via dnf/apt.
//!
//! Supported apps: MariaDB, MySQL, phpMyAdmin, Email (Postfix+Dovecot), RabbitMQ.
//! MariaDB and MySQL are treated as mutually exclusive on one host.

use crate::apps_pkg::{
    disable_now, enable_now, install_packages_dnf_or_apt, remove_packages_dnf_or_apt,
    rpm_or_dpkg_installed,
};
use crate::service_detect::{first_active_service, port_open, systemd_unit_active};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppId {
    Mariadb,
    Mysql,
    Phpmyadmin,
    Email,
    Rabbitmq,
}

impl AppId {
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "mariadb" => Ok(Self::Mariadb),
            "mysql" => Ok(Self::Mysql),
            "phpmyadmin" | "php-myadmin" => Ok(Self::Phpmyadmin),
            "email" | "mail" => Ok(Self::Email),
            "rabbitmq" => Ok(Self::Rabbitmq),
            other => Err(format!(
                "Unknown app `{other}`. Use: mariadb, mysql, phpmyadmin, email, rabbitmq"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mariadb => "mariadb",
            Self::Mysql => "mysql",
            Self::Phpmyadmin => "phpmyadmin",
            Self::Email => "email",
            Self::Rabbitmq => "rabbitmq",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Mariadb => "MariaDB",
            Self::Mysql => "MySQL",
            Self::Phpmyadmin => "phpMyAdmin",
            Self::Email => "Email (Postfix + Dovecot)",
            Self::Rabbitmq => "RabbitMQ",
        }
    }

    pub fn all() -> &'static [AppId] {
        &[
            Self::Mariadb,
            Self::Mysql,
            Self::Phpmyadmin,
            Self::Email,
            Self::Rabbitmq,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppStateKind {
    NotInstalled,
    Installed,
    Running,
}

impl AppStateKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotInstalled => "not_installed",
            Self::Installed => "installed",
            Self::Running => "running",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::NotInstalled => "Not installed",
            Self::Installed => "Installed (not running)",
            Self::Running => "Running",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppStatus {
    pub id: AppId,
    pub state: AppStateKind,
    pub detail: String,
    pub warning: Option<String>,
}

fn mariadb_present() -> bool {
    systemd_unit_active("mariadb")
        || rpm_or_dpkg_installed(&["mariadb-server", "MariaDB-server"])
        || first_active_service(&[("mariadb", "x")]).is_some()
}

fn mysql_present() -> bool {
    systemd_unit_active("mysqld")
        || systemd_unit_active("mysql")
        || rpm_or_dpkg_installed(&["mysql-server", "mysql-community-server"])
        || first_active_service(&[("mysqld", "x"), ("mysql", "x")]).is_some()
}

fn conflict_warning(id: AppId) -> Option<String> {
    match id {
        AppId::Mariadb if mysql_present() => Some(
            "MySQL appears installed on this host. MariaDB and MySQL typically conflict; uninstall MySQL first or keep only one."
                .into(),
        ),
        AppId::Mysql if mariadb_present() => Some(
            "MariaDB appears installed on this host. MySQL and MariaDB typically conflict; uninstall MariaDB first or keep only one."
                .into(),
        ),
        _ => None,
    }
}

pub fn detect_app(id: AppId) -> AppStatus {
    let warning = conflict_warning(id);
    match id {
        AppId::Mariadb => {
            let running = systemd_unit_active("mariadb") && port_open("127.0.0.1:3306", 250);
            let installed = mariadb_present() || port_open("127.0.0.1:3306", 250);
            let (state, detail) = if running {
                (
                    AppStateKind::Running,
                    "mariadb unit active and :3306 accepting connections.".into(),
                )
            } else if installed {
                (
                    AppStateKind::Installed,
                    "MariaDB packages or unit present, but not fully running on :3306.".into(),
                )
            } else {
                (AppStateKind::NotInstalled, "MariaDB not detected.".into())
            };
            AppStatus {
                id,
                state,
                detail,
                warning,
            }
        }
        AppId::Mysql => {
            let running = (systemd_unit_active("mysqld") || systemd_unit_active("mysql"))
                && port_open("127.0.0.1:3306", 250);
            let installed = mysql_present();
            let (state, detail) = if running {
                (
                    AppStateKind::Running,
                    "MySQL unit active and :3306 accepting connections.".into(),
                )
            } else if installed {
                (
                    AppStateKind::Installed,
                    "MySQL packages or unit present, but not fully running on :3306.".into(),
                )
            } else {
                (AppStateKind::NotInstalled, "MySQL not detected.".into())
            };
            AppStatus {
                id,
                state,
                detail,
                warning,
            }
        }
        AppId::Phpmyadmin => {
            let installed = rpm_or_dpkg_installed(&["phpMyAdmin", "phpmyadmin"]);
            let path_hint = std::path::Path::new("/usr/share/phpMyAdmin").exists()
                || std::path::Path::new("/usr/share/phpmyadmin").exists();
            let (state, detail) = if installed || path_hint {
                (
                    AppStateKind::Installed,
                    "phpMyAdmin package or share path found. Wire a vhost to expose it.".into(),
                )
            } else {
                (
                    AppStateKind::NotInstalled,
                    "phpMyAdmin not detected.".into(),
                )
            };
            AppStatus {
                id,
                state,
                detail,
                warning: None,
            }
        }
        AppId::Email => {
            let postfix = systemd_unit_active("postfix");
            let dovecot = systemd_unit_active("dovecot");
            let pkgs = rpm_or_dpkg_installed(&["postfix", "dovecot"]);
            let (state, detail) = if postfix && dovecot {
                (
                    AppStateKind::Running,
                    "Postfix and Dovecot units are active.".into(),
                )
            } else if pkgs || postfix || dovecot {
                (
                    AppStateKind::Installed,
                    "Mail packages or units partially present.".into(),
                )
            } else {
                (
                    AppStateKind::NotInstalled,
                    "Email stack (Postfix + Dovecot) not detected.".into(),
                )
            };
            AppStatus {
                id,
                state,
                detail,
                warning: None,
            }
        }
        AppId::Rabbitmq => {
            let running =
                systemd_unit_active("rabbitmq-server") || port_open("127.0.0.1:5672", 250);
            let installed = rpm_or_dpkg_installed(&["rabbitmq-server"]);
            let (state, detail) = if running {
                (
                    AppStateKind::Running,
                    "RabbitMQ active (unit or AMQP :5672).".into(),
                )
            } else if installed {
                (
                    AppStateKind::Installed,
                    "rabbitmq-server package present but not running.".into(),
                )
            } else {
                (AppStateKind::NotInstalled, "RabbitMQ not detected.".into())
            };
            AppStatus {
                id,
                state,
                detail,
                warning: None,
            }
        }
    }
}

pub fn list_apps() -> Vec<AppStatus> {
    AppId::all().iter().copied().map(detect_app).collect()
}

fn enforce_db_xor(id: AppId) -> Result<(), String> {
    match id {
        AppId::Mariadb if mysql_present() => Err(
            "Refuse to install MariaDB while MySQL is present. Uninstall MySQL first (hosts typically run MariaDB XOR MySQL)."
                .into(),
        ),
        AppId::Mysql if mariadb_present() => Err(
            "Refuse to install MySQL while MariaDB is present. Uninstall MariaDB first (hosts typically run MariaDB XOR MySQL)."
                .into(),
        ),
        _ => Ok(()),
    }
}

pub fn install_app(id: AppId) -> Result<String, String> {
    install_app_on(id, None)
}

/// Install host packages and optionally associate/drop site-scoped pieces under a domain home.
pub fn install_app_on(id: AppId, domain: Option<&str>) -> Result<String, String> {
    enforce_db_xor(id)?;
    let current = detect_app(id);
    let mut messages = Vec::new();
    if current.state != AppStateKind::Running {
        let msg = match id {
            AppId::Mariadb => {
                install_packages_dnf_or_apt(&["mariadb-server"], &["mariadb-server"])?;
                enable_now(&["mariadb"])?;
                "Installed and started MariaDB.".to_string()
            }
            AppId::Mysql => {
                install_packages_dnf_or_apt(&["mysql-server"], &["mysql-server"])?;
                let _ = enable_now(&["mysqld"]);
                let _ = enable_now(&["mysql"]);
                "Installed and started MySQL.".to_string()
            }
            AppId::Phpmyadmin => {
                install_packages_dnf_or_apt(&["phpMyAdmin"], &["phpmyadmin"])?;
                "Installed phpMyAdmin packages.".to_string()
            }
            AppId::Email => {
                install_packages_dnf_or_apt(
                    &["postfix", "dovecot"],
                    &["postfix", "dovecot-core", "dovecot-imapd"],
                )?;
                enable_now(&["postfix", "dovecot"])?;
                "Installed and started Email stack (Postfix + Dovecot).".to_string()
            }
            AppId::Rabbitmq => {
                install_packages_dnf_or_apt(&["rabbitmq-server"], &["rabbitmq-server"])?;
                enable_now(&["rabbitmq-server"])?;
                "Installed and started RabbitMQ.".to_string()
            }
        };
        messages.push(msg);
    } else {
        messages.push(format!("{} is already running on the host.", id.label()));
    }
    if let Some(domain) = domain.map(str::trim).filter(|v| !v.is_empty()) {
        if crate::apps_site::is_associable(id) {
            messages.push(crate::apps_site::apply_site_scope(id, domain)?);
        }
    } else if crate::apps_site::is_site_scoped(id) {
        messages.push(
            "No domain selected: host packages only. Choose a domain or subdomain to drop site paths under /home/<domain>/apps/."
                .into(),
        );
    }
    Ok(messages.join(" "))
}

pub fn reinstall_app(id: AppId) -> Result<String, String> {
    reinstall_app_on(id, None)
}

pub fn reinstall_app_on(id: AppId, domain: Option<&str>) -> Result<String, String> {
    let _ = uninstall_app_on(id, domain);
    install_app_on(id, domain).map(|msg| format!("Reinstall: {msg}"))
}

pub fn uninstall_app(id: AppId) -> Result<String, String> {
    uninstall_app_on(id, None)
}

pub fn uninstall_app_on(id: AppId, domain: Option<&str>) -> Result<String, String> {
    let mut messages = Vec::new();
    if let Some(domain) = domain.map(str::trim).filter(|v| !v.is_empty()) {
        if crate::apps_site::is_associable(id) {
            messages.push(crate::apps_site::clear_site_scope(id, domain)?);
        }
        if crate::apps_site::is_site_scoped(id) {
            // Site-scoped uninstall clears domain paths only; leave host packages unless no domain.
            return Ok(messages.join(" "));
        }
    }
    let msg = match id {
        AppId::Mariadb => {
            disable_now(&["mariadb"])?;
            remove_packages_dnf_or_apt(&["mariadb-server"], &["mariadb-server"])?;
            "Uninstalled MariaDB.".to_string()
        }
        AppId::Mysql => {
            disable_now(&["mysqld"])?;
            disable_now(&["mysql"])?;
            remove_packages_dnf_or_apt(
                &["mysql-server", "mysql-community-server"],
                &["mysql-server"],
            )?;
            "Uninstalled MySQL.".to_string()
        }
        AppId::Phpmyadmin => {
            remove_packages_dnf_or_apt(&["phpMyAdmin"], &["phpmyadmin"])?;
            "Uninstalled phpMyAdmin.".to_string()
        }
        AppId::Email => {
            disable_now(&["postfix", "dovecot"])?;
            remove_packages_dnf_or_apt(
                &["postfix", "dovecot"],
                &["postfix", "dovecot-core", "dovecot-imapd"],
            )?;
            "Uninstalled Email stack (Postfix + Dovecot).".to_string()
        }
        AppId::Rabbitmq => {
            disable_now(&["rabbitmq-server"])?;
            remove_packages_dnf_or_apt(&["rabbitmq-server"], &["rabbitmq-server"])?;
            "Uninstalled RabbitMQ.".to_string()
        }
    };
    messages.push(msg);
    Ok(messages.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_app_ids() {
        assert_eq!(AppId::parse("MariaDB").unwrap(), AppId::Mariadb);
        assert_eq!(AppId::parse("php-myadmin").unwrap(), AppId::Phpmyadmin);
        assert!(AppId::parse("nginx").is_err());
    }
}
