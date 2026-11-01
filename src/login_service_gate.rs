//! Gate panel password/passkey sign-in until critical host services are ready.
//!
//! Critical by default: web server + MariaDB when those stacks were installed
//! (CPN manifest, OpenLiteSpeed tree, enabled systemd units, or server binaries).
//! Mail is informational only (never blocks login). Installer/bootstrap token
//! routes stay reachable outside this gate.
//!
//! Policy:
//! - Known unhealthy critical services: block sign-in.
//! - Probe timeout with a recent confirmed-ready cache: reuse that status
//!   (stale-while-revalidate) so a brief systemd stall does not lock owners out.
//! - Probe timeout with no recent ready status: **fail closed** (do not invite
//!   sign-in). Operators recover with `CPN_LOGIN_SERVICE_GATE=0` or by waiting
//!   until probes succeed.
//!
//! Set `CPN_LOGIN_SERVICE_GATE=0` to disable the gate (tests / recovery).
//!
//! Results are cached (30s TTL) so `/login` and `/api/login/services` never spam
//! `systemctl` on every poll (worker starvation → browser `408` blank page).

use crate::manifest::load_manifest;
use crate::service_detect::{
    database_health_label, detect_database, detect_mail_service_label, detect_web_server_label,
    openlitespeed_tree_present, port_open, systemd_unit_enabled,
};
use serde::Serialize;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Reuse the last evaluation long enough to outlive the 4s login poll interval.
const LOGIN_GATE_CACHE_TTL: Duration = Duration::from_secs(30);

/// Hard wall clock for a fresh gate evaluation (many `systemctl` probes).
const LOGIN_GATE_EVAL_BUDGET: Duration = Duration::from_millis(4000);

/// Operator-facing text when probes exceed the budget and no recent ready cache exists.
pub const LOGIN_SERVICE_TIMEOUT_MESSAGE: &str = "Could not confirm MariaDB/web health in time. Sign-in is disabled until panel services are confirmed running. Wait a few seconds and try again.";

/// Alias kept for older call sites and session-log filters.
pub const LOGIN_SERVICE_TIMEOUT_WARNING: &str = LOGIN_SERVICE_TIMEOUT_MESSAGE;

static LOGIN_GATE_CACHE: Mutex<Option<(Instant, LoginServiceStatus)>> = Mutex::new(None);

/// Last ready status without a probe-timeout block (stale-while-revalidate on budget expiry).
static LOGIN_GATE_LAST_GOOD: Mutex<Option<LoginServiceStatus>> = Mutex::new(None);

/// Serializes tests that mutate `CPN_LOGIN_SERVICE_GATE` (process-wide env).
#[cfg(test)]
pub(crate) static LOGIN_SERVICE_GATE_ENV_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(crate) fn lock_login_service_gate_env() -> std::sync::MutexGuard<'static, ()> {
    LOGIN_SERVICE_GATE_ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

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

fn gate_disabled_by_env() -> bool {
    match std::env::var("CPN_LOGIN_SERVICE_GATE") {
        Ok(raw) => matches!(
            raw.trim().to_ascii_lowercase().as_str(),
            "0" | "off" | "false" | "no" | "disable" | "disabled"
        ),
        Err(_) => false,
    }
}

fn ready_status_unrestricted(message: &str) -> LoginServiceStatus {
    LoginServiceStatus {
        ready: true,
        message: message.to_string(),
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
        warnings: if message.is_empty() {
            Vec::new()
        } else {
            vec![message.to_string()]
        },
    }
}

fn timed_out_fail_closed_status() -> LoginServiceStatus {
    LoginServiceStatus {
        ready: false,
        message: LOGIN_SERVICE_TIMEOUT_MESSAGE.to_string(),
        web: ServiceSlice {
            expected: true,
            running: false,
            label: "check timed out".into(),
        },
        database: ServiceSlice {
            expected: true,
            running: false,
            label: "check timed out".into(),
        },
        mail: ServiceSlice {
            expected: false,
            running: true,
            label: "n/a".into(),
        },
        blocking: Vec::new(),
        warnings: Vec::new(),
    }
}

fn is_usable_last_good(status: &LoginServiceStatus) -> bool {
    status.ready
        && status.blocking.is_empty()
        && status.message.as_str() != LOGIN_SERVICE_TIMEOUT_MESSAGE
        && !status
            .warnings
            .iter()
            .any(|w| w.as_str() == LOGIN_SERVICE_TIMEOUT_MESSAGE)
}

fn remember_last_good(status: &LoginServiceStatus) {
    if !is_usable_last_good(status) {
        return;
    }
    if let Ok(mut guard) = LOGIN_GATE_LAST_GOOD.lock() {
        *guard = Some(status.clone());
    }
}

fn peek_usable_last_good() -> Option<LoginServiceStatus> {
    if let Ok(guard) = LOGIN_GATE_LAST_GOOD.lock()
        && let Some(ref status) = *guard
        && is_usable_last_good(status)
    {
        return Some(status.clone());
    }
    // Cache may still hold a good status after TTL expiry (before overwrite).
    if let Ok(guard) = LOGIN_GATE_CACHE.lock()
        && let Some((_, ref status)) = *guard
        && is_usable_last_good(status)
    {
        return Some(status.clone());
    }
    None
}

/// Prefer a prior ready status; otherwise fail closed (do not invite sign-in).
pub(crate) fn fail_closed_or_stale() -> LoginServiceStatus {
    if let Some(status) = peek_usable_last_good() {
        return status;
    }
    timed_out_fail_closed_status()
}

fn log_gate_timeout(where_: &str) {
    // Main Log + Error logs; no credentials or usernames.
    crate::upgrade_tip_log::log_failure(
        format!("login_service_gate: {where_} timed out; sign-in blocked until health is confirmed (fail-closed)"),
        None,
    );
}

/// Run a fresh evaluation with a hard wall clock so login never blocks indefinitely.
fn evaluate_login_services_budgeted() -> LoginServiceStatus {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("cpn-login-gate".into())
        .spawn(move || {
            let _ = tx.send(evaluate_login_services_fresh());
        })
        .ok();
    match rx.recv_timeout(LOGIN_GATE_EVAL_BUDGET) {
        Ok(status) => {
            remember_last_good(&status);
            status
        }
        Err(_) => {
            log_gate_timeout("evaluate");
            fail_closed_or_stale()
        }
    }
}

/// True when CPN or the host clearly installed a web stack meant to run.
fn web_stack_installed() -> bool {
    if load_manifest().and_then(|m| m.selected_server).is_some() {
        return true;
    }
    if openlitespeed_tree_present() {
        return true;
    }
    // Enabled (not merely present) units: GitHub CI images ship unused unit files.
    [
        "nginx",
        "lsws",
        "lshttpd",
        "openlitespeed",
        "caddy",
        "httpd",
    ]
    .iter()
    .any(|unit| systemd_unit_enabled(unit))
}

fn database_stack_installed() -> bool {
    if Path::new("/usr/sbin/mariadbd").is_file()
        || Path::new("/usr/libexec/mariadbd").is_file()
        || Path::new("/usr/sbin/mysqld").is_file()
    {
        return true;
    }
    ["mariadb", "mysql", "mysqld"]
        .iter()
        .any(|unit| systemd_unit_enabled(unit))
}

fn mail_stack_expected() -> bool {
    // Manifest only on the login path: avoid extra systemd probes for a
    // non-blocking warning when CPN never selected mail.
    load_manifest().and_then(|m| m.selected_mail).is_some()
}

fn is_running_label(label: &str) -> bool {
    label == "Running"
}

/// Fast path for OLS reverse-proxy hosts: TCP proves the web stack without systemd.
fn web_running_label_fast() -> String {
    if openlitespeed_tree_present()
        && (port_open("127.0.0.1:80", 200)
            || port_open("127.0.0.1:443", 200)
            || port_open("127.0.0.1:8088", 200))
    {
        return "Running".into();
    }
    detect_web_server_label()
}

/// Fast path: :3306 accepting connections is enough for login health.
fn database_running_label_fast() -> String {
    if port_open("127.0.0.1:3306", 200) {
        return "Running".into();
    }
    let db = detect_database();
    database_health_label(&db)
}

fn evaluate_login_services_fresh() -> LoginServiceStatus {
    if gate_disabled_by_env() {
        return ready_status_unrestricted("");
    }

    #[cfg(not(unix))]
    {
        return ready_status_unrestricted("");
    }

    #[cfg(unix)]
    {
        // Expected stacks first (path/manifest short-circuits before systemd).
        let web_expected = web_stack_installed();
        let db_expected = database_stack_installed();
        let mail_expected = mail_stack_expected();

        // TCP-first running checks; fall back to systemd-aware detectors only when needed.
        let web_label = if web_expected {
            web_running_label_fast()
        } else {
            "n/a".into()
        };
        let db_label = if db_expected {
            database_running_label_fast()
        } else {
            "n/a".into()
        };
        let mail_label = if mail_expected {
            detect_mail_service_label()
        } else {
            "n/a".into()
        };

        let web_running = !web_expected || is_running_label(&web_label);
        let db_running = !db_expected || is_running_label(&db_label);
        let mail_running = !mail_expected || is_running_label(&mail_label);

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
                "No managed web server or MariaDB install was detected. Sign-in is allowed.".into(),
            );
        }

        let ready = blocking.is_empty();
        let message = if ready {
            if warnings.is_empty() {
                String::new()
            } else {
                warnings.first().cloned().unwrap_or_default()
            }
        } else if blocking.len() == 1 {
            format!(
                "Panel services are still starting. Sign-in is disabled until {} is running.",
                blocking[0]
            )
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

/// Evaluate whether the login form may accept credentials.
///
/// On non-Unix hosts (no systemd services), always ready so Windows/dev builds
/// are not locked out. Cached briefly to keep `/login` fast under poll load.
///
/// `CPN_LOGIN_SERVICE_GATE=0` always bypasses the cache so parallel tests (and
/// operator recovery) cannot see a stale not-ready result from another thread.
pub fn evaluate_login_services() -> LoginServiceStatus {
    // Env disable must win over a hot cache (parallel tests / recovery).
    if gate_disabled_by_env() {
        return ready_status_unrestricted("");
    }

    if let Ok(guard) = LOGIN_GATE_CACHE.lock()
        && let Some((at, ref status)) = *guard
        && at.elapsed() < LOGIN_GATE_CACHE_TTL
    {
        return status.clone();
    }

    let status = evaluate_login_services_budgeted();
    if let Ok(mut guard) = LOGIN_GATE_CACHE.lock() {
        *guard = Some((Instant::now(), status.clone()));
    }
    status
}

/// Drop the cached gate result (tests / after service heal).
pub fn invalidate_login_services_cache() {
    if let Ok(mut guard) = LOGIN_GATE_CACHE.lock() {
        *guard = None;
    }
    if let Ok(mut guard) = LOGIN_GATE_LAST_GOOD.lock() {
        *guard = None;
    }
}

/// True when password/passkey POST handlers may proceed.
pub fn login_services_ready() -> bool {
    if gate_disabled_by_env() {
        return true;
    }
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
        assert!(
            json["message"]
                .as_str()
                .unwrap()
                .contains("Sign-in is disabled")
        );
    }

    #[test]
    fn evaluate_returns_struct_without_panic() {
        invalidate_login_services_cache();
        let status = evaluate_login_services();
        let _ = status.ready;
        let _ = serde_json::to_string(&status).expect("serialize");
    }

    #[test]
    fn env_can_disable_gate() {
        let _env = lock_login_service_gate_env();
        invalidate_login_services_cache();
        unsafe {
            std::env::set_var("CPN_LOGIN_SERVICE_GATE", "0");
        }
        let status = evaluate_login_services();
        unsafe {
            std::env::remove_var("CPN_LOGIN_SERVICE_GATE");
        }
        invalidate_login_services_cache();
        assert!(status.ready);
        assert!(status.blocking.is_empty());
    }

    #[test]
    fn env_disable_bypasses_stale_not_ready_cache() {
        let _env = lock_login_service_gate_env();
        invalidate_login_services_cache();
        if let Ok(mut guard) = LOGIN_GATE_CACHE.lock() {
            *guard = Some((
                Instant::now(),
                LoginServiceStatus {
                    ready: false,
                    message: "stale".into(),
                    web: ServiceSlice {
                        expected: true,
                        running: false,
                        label: "x".into(),
                    },
                    database: ServiceSlice {
                        expected: true,
                        running: false,
                        label: "x".into(),
                    },
                    mail: ServiceSlice {
                        expected: false,
                        running: false,
                        label: "x".into(),
                    },
                    blocking: vec!["Web server"],
                    warnings: Vec::new(),
                },
            ));
        }
        unsafe {
            std::env::set_var("CPN_LOGIN_SERVICE_GATE", "0");
        }
        assert!(login_services_ready());
        let status = evaluate_login_services();
        unsafe {
            std::env::remove_var("CPN_LOGIN_SERVICE_GATE");
        }
        invalidate_login_services_cache();
        assert!(status.ready);
        assert!(status.blocking.is_empty());
    }

    #[test]
    fn cache_reuses_recent_evaluation() {
        let _env = lock_login_service_gate_env();
        invalidate_login_services_cache();
        unsafe {
            std::env::set_var("CPN_LOGIN_SERVICE_GATE", "0");
        }
        let first = evaluate_login_services();
        let second = evaluate_login_services();
        unsafe {
            std::env::remove_var("CPN_LOGIN_SERVICE_GATE");
        }
        invalidate_login_services_cache();
        assert_eq!(first, second);
    }

    #[test]
    fn timeout_fail_closed_status_blocks_sign_in() {
        let status = timed_out_fail_closed_status();
        assert!(!status.ready);
        assert!(status.message.contains("Sign-in is disabled"));
        assert!(!status.message.contains("still sign in"));
        assert_eq!(status.message, LOGIN_SERVICE_TIMEOUT_MESSAGE);
    }

    #[test]
    fn timeout_prefers_last_ready_cache_over_fail_closed() {
        let _env = lock_login_service_gate_env();
        invalidate_login_services_cache();
        let good = LoginServiceStatus {
            ready: true,
            message: String::new(),
            web: ServiceSlice {
                expected: true,
                running: true,
                label: "Running".into(),
            },
            database: ServiceSlice {
                expected: true,
                running: true,
                label: "Running".into(),
            },
            mail: ServiceSlice {
                expected: false,
                running: true,
                label: "n/a".into(),
            },
            blocking: Vec::new(),
            warnings: Vec::new(),
        };
        // Seed a stale cache entry (past TTL) that is still a usable last-good.
        if let Ok(mut guard) = LOGIN_GATE_CACHE.lock() {
            *guard = Some((
                Instant::now()
                    .checked_sub(Duration::from_secs(120))
                    .unwrap_or_else(Instant::now),
                good.clone(),
            ));
        }
        let status = fail_closed_or_stale();
        invalidate_login_services_cache();
        assert_eq!(status, good);
        assert!(status.ready);
    }

    #[test]
    fn timeout_without_last_good_fails_closed() {
        let _env = lock_login_service_gate_env();
        invalidate_login_services_cache();
        let status = fail_closed_or_stale();
        assert!(!status.ready);
        assert_eq!(status.message, LOGIN_SERVICE_TIMEOUT_MESSAGE);
    }
}
