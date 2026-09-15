//! Gate panel password/passkey sign-in until critical host services are ready.
//!
//! Critical by default: web server + MariaDB when those stacks were installed.
//! Mail is informational only (never blocks login). Installer/bootstrap token
//! routes stay reachable outside this gate.

use crate::manifest::load_manifest;
use crate::service_detect::{
    database_health_label, detect_database, detect_mail_service_label, detect_web_server_label,
    openlitespeed_tree_present, systemd_unit_file_exists,
};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LoginServiceStatus {
    /// When false, password and passkey sign-in must be rejected server-side.
    pub ready: bool,
    /// Operator-facing summary (English; UI may localize later).
    pub message: String,
    pub web: ServiceSlice,
    pub database: ServiceSlice,
    pub mail: ServiceSlice,
    /// Short labels of services that currently block sign-in.
    pub blocking: Vec<&'static str>,
    /// Non-blocking notes (for example mail still starting).
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ServiceSlice {
    pub expected: bool,
    pub running: bool,
    pub label: String,
}

fn web_stack_installed() -> bool {
    if load_manifest()
        .and_then(|m| m.selected_server)
        .is_some()
    {
        return true;
    }
    if openlitespeed_tree_present() {
        return true;
    }
    [
        "nginx",
        "lsws",
        "lshttpd",
        "openlitespeed",
        "caddy",
        "httpd",
    ]
    .iter()
    .any(|unit| systemd_unit_file_exists(unit))
}

fn database_stack_installed() -> bool {
    ["mariadb", "mysql", "mysqld"]
        .iter()
        .any(|unit| systemd_unit_file_exists(unit))
        || Path::new("/usr/sbin/mariadbd").is_file()
        || Path::new("/usr/libexec/mariadbd").is_file()
        || Path::new("/usr/sbin/mysqld").is_file()
}

fn mail_stack_expected() -> bool {
    load_manifest()
        .and_then(|m| m.selected_mail)
        .is_some()
        || ["postfix", "exim4", "exim", "dovecot"]
            .iter()
            .any(|unit| systemd_unit_file_exists(unit))
}

fn is_running_label(label: &str) -> bool {
    label == "Running"
}

/// Evaluate whether the login form may accept credentials.
///
/// On non-Unix hosts (no systemd services), always ready so Windows/dev builds
/// are not locked out.
pub fn evaluate_login_services() -> LoginServiceStatus {
    #[cfg(not(unix))]
    {
        return LoginServiceStatus {
            ready: true,
            message: String::new(),
            web: ServiceSlice {
                expected: false,
                running: true,
                label: "n/a".into(),
            },
            database: ServiceSlice {
                expected: false,
                running: true,
                label: "n/a".into(),
            },
            mail: ServiceSlice {
                expected: false,
                running: true,
                label: "n/a".into(),
            },
            blocking: Vec::new(),
            warnings: Vec::new(),
        };
    }

    #[cfg(unix)]
    {
        let web_label = detect_web_server_label();
        let db = detect_database();
        let db_label = database_health_label(&db);
        let mail_label = detect_mail_service_label();

        let web_expected = web_stack_installed();
        let db_expected = database_stack_installed();
        let mail_expected = mail_stack_expected();

        let web_running = is_running_label(&web_label);
        let db_running = is_running_label(&db_label);
        let mail_running = is_running_label(&mail_label);

        let mut blocking: Vec<&'static str> = Vec::new();
        if web_expected && !web_running {
            blocking.push("Web server");
        }
        if db_expected && !db_running {
            blocking.push("MariaDB");
        }

        let mut warnings = Vec::new();
        if mail_expected && !mail_running {
            warnings.push(
                "Mail services are not running yet. Sign-in is still allowed; mail features may be limited."
                    .into(),
            );
        }
        if !web_expected && !db_expected {
            warnings.push(
                "No managed web server or MariaDB install was detected. Sign-in is allowed."
                    .into(),
            );
        }

        let ready = blocking.is_empty();
        let message = if ready {
            if warnings.is_empty() {
                String::new()
            } else {
                warnings.first().cloned().unwrap_or_default()
            }
        } else {
            let list = blocking.join(" and ");
            format!(
                "Panel services are still starting. Sign-in is disabled until {list} are running."
            )
        };

        LoginServiceStatus {
            ready,
            message,
            web: ServiceSlice {
                expected: web_expected,
                running: web_running,
                label: web_label,
            },
            database: ServiceSlice {
                expected: db_expected,
                running: db_running,
                label: db_label,
            },
            mail: ServiceSlice {
                expected: mail_expected,
                running: mail_running,
                label: mail_label,
            },
            blocking,
            warnings,
        }
    }
}

/// True when password/passkey POST handlers may proceed.
pub fn login_services_ready() -> bool {
    evaluate_login_services().ready
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_serializes_for_poll_api() {
        let status = LoginServiceStatus {
            ready: false,
            message: "Panel services are still starting. Sign-in is disabled until Web server and MariaDB are running.".into(),
            web: ServiceSlice {
                expected: true,
                running: false,
                label: "Not detected".into(),
            },
            database: ServiceSlice {
                expected: true,
                running: false,
                label: "Not detected".into(),
            },
            mail: ServiceSlice {
                expected: false,
                running: false,
                label: "Not detected".into(),
            },
            blocking: vec!["Web server", "MariaDB"],
            warnings: Vec::new(),
        };
        let json = serde_json::to_value(&status).expect("json");
        assert_eq!(json["ready"], false);
        assert!(json["blocking"].as_array().unwrap().len() == 2);
        assert!(json["message"].as_str().unwrap().contains("Sign-in is disabled"));
    }

    #[test]
    fn evaluate_returns_struct_without_panic() {
        let status = evaluate_login_services();
        // On the developer Windows host this is always ready; on Linux labs it
        // reflects live detection. Either way the call must succeed.
        let _ = status.ready;
        let _ = serde_json::to_string(&status).expect("serialize");
    }
}
