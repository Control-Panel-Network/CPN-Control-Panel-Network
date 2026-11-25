//! Owner alerts for key security audit events (CRA Annex I unauthorised-access reporting aid).

use crate::account::now_unix;
use crate::panel_audit_policy::load_audit_policy;
use crate::panel_audit_types::{AuditRecord, AuditSeverity};
use crate::panel_notifications::{CATEGORY_PANEL, notify_panel_admins};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

const AUTH_FAIL_WINDOW_SECS: u64 = 600;
const AUTH_FAIL_SPIKE: u32 = 8;

static AUTH_FAIL_LOCK: Mutex<()> = Mutex::new(());

fn auth_fail_path() -> PathBuf {
    crate::account::data_dir()
        .join("audit")
        .join("auth-fail-window.json")
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct AuthFailWindow {
    #[serde(default)]
    started_at: u64,
    #[serde(default)]
    count: u32,
    #[serde(default)]
    last_alert_at: u64,
}

fn load_window() -> AuthFailWindow {
    let Ok(raw) = fs::read_to_string(auth_fail_path()) else {
        return AuthFailWindow::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn save_window(window: &AuthFailWindow) {
    let path = auth_fail_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(raw) = serde_json::to_string(window) {
        let _ = fs::write(path, raw);
    }
}

fn note_auth_failure(entry: &AuditRecord) {
    let _guard = AUTH_FAIL_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let now = now_unix();
    let mut window = load_window();
    if window.started_at == 0 || now.saturating_sub(window.started_at) > AUTH_FAIL_WINDOW_SECS {
        window.started_at = now;
        window.count = 0;
    }
    window.count = window.count.saturating_add(1);
    if window.count >= AUTH_FAIL_SPIKE && now.saturating_sub(window.last_alert_at) > 120 {
        window.last_alert_at = now;
        notify_panel_admins(
            "Audit: repeated login failures",
            &format!(
                "{} failed auth events in the last {} minutes (latest actor {}, ip {}). Review Server > Logs audit trail.",
                window.count,
                AUTH_FAIL_WINDOW_SECS / 60,
                entry.actor,
                if entry.ip.is_empty() {
                    "unknown"
                } else {
                    &entry.ip
                }
            ),
            CATEGORY_PANEL,
        );
        window.count = 0;
        window.started_at = now;
    }
    save_window(&window);
}

fn should_alert(entry: &AuditRecord) -> bool {
    if !load_audit_policy().alerts_enabled {
        return false;
    }
    let action = entry.action.as_str();
    if matches!(
        action,
        "auth.login.fail"
            | "auth.mfa.fail"
            | "auth.lockout"
            | "firewall.stop"
            | "token.issue"
            | "identity.totp.disable"
            | "cli.mfa.clear"
            | "cli.uninstall"
            | "cli.password"
    ) {
        return true;
    }
    if !entry.ok
        && matches!(
            entry.severity_enum(),
            AuditSeverity::Warning | AuditSeverity::Critical
        )
        && (action.starts_with("account.") || action.starts_with("firewall."))
    {
        return true;
    }
    false
}

/// Best-effort owner inbox alert. Never panics; safe to call from the write path.
pub fn maybe_alert(entry: &AuditRecord) {
    if entry.action == "auth.login.fail" || entry.action == "auth.mfa.fail" {
        note_auth_failure(entry);
    }
    if !should_alert(entry) {
        return;
    }
    let title = match entry.action.as_str() {
        "auth.login.fail" | "auth.mfa.fail" => return,
        "auth.lockout" => "Audit: account lockout",
        "firewall.stop" => "Audit: firewall stopped",
        "token.issue" => "Audit: API token issued",
        "identity.totp.disable" => "Audit: TOTP disabled",
        "cli.mfa.clear" => "Audit: CLI cleared MFA",
        "cli.uninstall" => "Audit: uninstall started",
        "cli.password" => "Audit: CLI password reset",
        other => {
            if other.starts_with("firewall.") {
                "Audit: firewall change"
            } else {
                "Audit: security event"
            }
        }
    };
    let body = format!(
        "action={} actor={} ok={} target={} ip={}",
        entry.action,
        entry.actor,
        entry.ok,
        if entry.target.is_empty() {
            "-"
        } else {
            &entry.target
        },
        if entry.ip.is_empty() { "-" } else { &entry.ip }
    );
    notify_panel_admins(title, &body, CATEGORY_PANEL);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn auth_fail_spike_resets_window() {
        with_test_data_dir(|| {
            let entry = AuditRecord {
                ts: now_unix(),
                actor: "anon".into(),
                action: "auth.login.fail".into(),
                target: String::new(),
                ok: false,
                message: "bad".into(),
                ip: "203.0.113.9".into(),
                ua: String::new(),
                request_id: String::new(),
                category: "auth".into(),
                severity: "warning".into(),
            };
            for _ in 0..AUTH_FAIL_SPIKE {
                note_auth_failure(&entry);
            }
            let window = load_window();
            assert_eq!(window.count, 0);
            assert!(window.last_alert_at > 0);
        });
    }
}
