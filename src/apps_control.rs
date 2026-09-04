//! Start / stop helpers for host apps with systemd units.

use crate::apps::{AppId, AppStateKind, detect_app};
use crate::apps_pkg::{enable_now, start_units, stop_units};
use crate::apps_postgresql::{start_postgresql, stop_postgresql};

pub fn start_app(id: AppId) -> Result<String, String> {
    if !id.supports_service_control() {
        return Err(format!(
            "{} does not support Start/Stop from Apps.",
            id.label()
        ));
    }
    let current = detect_app(id);
    if current.state == AppStateKind::NotInstalled {
        return Err(format!(
            "{} is not installed. Use Install first.",
            id.label()
        ));
    }
    if current.state == AppStateKind::Running {
        return Ok(format!("{} is already running.", id.label()));
    }
    match id {
        AppId::Mariadb => {
            enable_now(&["mariadb"])?;
            Ok("Started MariaDB.".into())
        }
        AppId::Mysql => {
            let _ = start_units(&["mysqld"]);
            let _ = start_units(&["mysql"]);
            Ok("Started MySQL.".into())
        }
        AppId::Postgresql => start_postgresql(),
        AppId::Email => {
            enable_now(&["postfix", "dovecot"])?;
            Ok("Started Email stack (Postfix + Dovecot).".into())
        }
        AppId::Rabbitmq => {
            enable_now(&["rabbitmq-server"])?;
            Ok("Started RabbitMQ.".into())
        }
        AppId::Phpmyadmin => Err("phpMyAdmin does not support Start/Stop from Apps.".into()),
    }
}

pub fn stop_app(id: AppId) -> Result<String, String> {
    if !id.supports_service_control() {
        return Err(format!(
            "{} does not support Start/Stop from Apps.",
            id.label()
        ));
    }
    let current = detect_app(id);
    if current.state == AppStateKind::NotInstalled {
        return Err(format!("{} is not installed.", id.label()));
    }
    match id {
        AppId::Mariadb => {
            stop_units(&["mariadb"])?;
            Ok("Stopped MariaDB.".into())
        }
        AppId::Mysql => {
            let _ = stop_units(&["mysqld"]);
            let _ = stop_units(&["mysql"]);
            Ok("Stopped MySQL.".into())
        }
        AppId::Postgresql => stop_postgresql(),
        AppId::Email => {
            stop_units(&["postfix", "dovecot"])?;
            Ok("Stopped Email stack (Postfix + Dovecot).".into())
        }
        AppId::Rabbitmq => {
            stop_units(&["rabbitmq-server"])?;
            Ok("Stopped RabbitMQ.".into())
        }
        AppId::Phpmyadmin => Err("phpMyAdmin does not support Start/Stop from Apps.".into()),
    }
}
