//! Host app lifecycle: detect, install, reinstall, uninstall via dnf/apt.
//!
//! Supported apps: MariaDB, MySQL, phpMyAdmin, Email (Postfix+Dovecot), RabbitMQ.
//! MariaDB and MySQL are treated as mutually exclusive on one host.

use crate::service_detect::{first_active_service, port_open, systemd_unit_active};
use std::process::Command;

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

fn package_manager() -> Result<&'static str, String> {
    if Command::new("dnf").arg("--version").status().is_ok() {
        return Ok("dnf");
    }
    if Command::new("apt-get").arg("--version").status().is_ok() {
        return Ok("apt");
    }
    Err("No supported package manager found (need dnf or apt-get).".into())
}

fn run_pkg(args: &[&str]) -> Result<(), String> {
    let pm = package_manager()?;
    let status = if pm == "dnf" {
        Command::new("dnf")
            .args(args)
            .status()
            .map_err(|error| format!("Could not start dnf: {error}"))?
    } else {
        if args.first() == Some(&"install") || args.first() == Some(&"remove") {
            let update = Command::new("apt-get")
                .args(["update", "-y"])
                .status()
                .map_err(|error| format!("Could not start apt-get update: {error}"))?;
            if !update.success() {
                return Err("apt-get update failed".into());
            }
        }
        let mut apt_args: Vec<&str> = Vec::new();
        match args.first().copied() {
            Some("install") => {
                apt_args.push("install");
                apt_args.push("-y");
                apt_args.extend_from_slice(&args[1..]);
            }
            Some("remove") => {
                apt_args.push("remove");
                apt_args.push("-y");
                apt_args.extend_from_slice(&args[1..]);
            }
            _ => {
                apt_args.extend_from_slice(args);
            }
        }
        Command::new("apt-get")
            .args(&apt_args)
            .status()
            .map_err(|error| format!("Could not start apt-get: {error}"))?
    };
    if !status.success() {
        return Err(format!("{pm} {} failed", args.join(" ")));
    }
    Ok(())
}

fn enable_now(units: &[&str]) -> Result<(), String> {
    for unit in units {
        let status = Command::new("systemctl")
            .args(["enable", "--now", unit])
            .status()
            .map_err(|error| format!("Could not start systemctl for {unit}: {error}"))?;
        if !status.success() {
            return Err(format!("systemctl enable --now {unit} failed"));
        }
    }
    Ok(())
}

fn disable_now(units: &[&str]) -> Result<(), String> {
    for unit in units {
        let _ = Command::new("systemctl")
            .args(["disable", "--now", unit])
            .status();
    }
    Ok(())
}

fn rpm_or_dpkg_installed(names: &[&str]) -> bool {
    for name in names {
        let rpm = Command::new("rpm")
            .args(["-q", name])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if rpm {
            return true;
        }
        let dpkg = Command::new("dpkg-query")
            .args(["-W", "-f=${Status}", name])
            .output()
            .ok()
            .map(|out| {
                let text = String::from_utf8_lossy(&out.stdout);
                text.contains("install ok installed")
            })
            .unwrap_or(false);
        if dpkg {
            return true;
        }
    }
    false
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

fn install_packages_dnf_or_apt(dnf_pkgs: &[&str], apt_pkgs: &[&str]) -> Result<(), String> {
    let pm = package_manager()?;
    if pm == "dnf" {
        let mut args = vec!["install", "-y"];
        args.extend_from_slice(dnf_pkgs);
        run_pkg(&args)
    } else {
        let mut args = vec!["install"];
        args.extend_from_slice(apt_pkgs);
        run_pkg(&args)
    }
}

fn remove_packages_dnf_or_apt(dnf_pkgs: &[&str], apt_pkgs: &[&str]) -> Result<(), String> {
    let pm = package_manager()?;
    if pm == "dnf" {
        let mut args = vec!["remove", "-y"];
        args.extend_from_slice(dnf_pkgs);
        run_pkg(&args)
    } else {
        let mut args = vec!["remove"];
        args.extend_from_slice(apt_pkgs);
        run_pkg(&args)
    }
}

pub fn install_app(id: AppId) -> Result<String, String> {
    enforce_db_xor(id)?;
    let current = detect_app(id);
    if current.state == AppStateKind::Running {
        return Ok(format!("{} is already running.", id.label()));
    }
    match id {
        AppId::Mariadb => {
            install_packages_dnf_or_apt(&["mariadb-server"], &["mariadb-server"])?;
            enable_now(&["mariadb"])?;
            Ok("Installed and started MariaDB.".into())
        }
        AppId::Mysql => {
            install_packages_dnf_or_apt(&["mysql-server"], &["mysql-server"])?;
            let _ = enable_now(&["mysqld"]);
            let _ = enable_now(&["mysql"]);
            Ok("Installed and started MySQL.".into())
        }
        AppId::Phpmyadmin => {
            install_packages_dnf_or_apt(&["phpMyAdmin"], &["phpmyadmin"])?;
            Ok("Installed phpMyAdmin packages. Configure a web vhost to expose the UI.".into())
        }
        AppId::Email => {
            install_packages_dnf_or_apt(
                &["postfix", "dovecot"],
                &["postfix", "dovecot-core", "dovecot-imapd"],
            )?;
            enable_now(&["postfix", "dovecot"])?;
            Ok("Installed and started Email stack (Postfix + Dovecot).".into())
        }
        AppId::Rabbitmq => {
            install_packages_dnf_or_apt(&["rabbitmq-server"], &["rabbitmq-server"])?;
            enable_now(&["rabbitmq-server"])?;
            Ok("Installed and started RabbitMQ.".into())
        }
    }
}

pub fn reinstall_app(id: AppId) -> Result<String, String> {
    let _ = uninstall_app(id);
    install_app(id).map(|msg| format!("Reinstall: {msg}"))
}

pub fn uninstall_app(id: AppId) -> Result<String, String> {
    match id {
        AppId::Mariadb => {
            disable_now(&["mariadb"])?;
            remove_packages_dnf_or_apt(&["mariadb-server"], &["mariadb-server"])?;
            Ok("Uninstalled MariaDB.".into())
        }
        AppId::Mysql => {
            disable_now(&["mysqld"])?;
            disable_now(&["mysql"])?;
            remove_packages_dnf_or_apt(
                &["mysql-server", "mysql-community-server"],
                &["mysql-server"],
            )?;
            Ok("Uninstalled MySQL.".into())
        }
        AppId::Phpmyadmin => {
            remove_packages_dnf_or_apt(&["phpMyAdmin"], &["phpmyadmin"])?;
            Ok("Uninstalled phpMyAdmin.".into())
        }
        AppId::Email => {
            disable_now(&["postfix", "dovecot"])?;
            remove_packages_dnf_or_apt(
                &["postfix", "dovecot"],
                &["postfix", "dovecot-core", "dovecot-imapd"],
            )?;
            Ok("Uninstalled Email stack (Postfix + Dovecot).".into())
        }
        AppId::Rabbitmq => {
            disable_now(&["rabbitmq-server"])?;
            remove_packages_dnf_or_apt(&["rabbitmq-server"], &["rabbitmq-server"])?;
            Ok("Uninstalled RabbitMQ.".into())
        }
    }
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
