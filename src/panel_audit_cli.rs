//! Privileged CLI mutations mirrored into the panel audit store (`actor=cli:<user>`).

use crate::panel_audit::record_full;
use crate::panel_audit_types::{AuditCategory, AuditSeverity};

fn cli_actor() -> String {
    let user = std::env::var("SUDO_USER")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "root".into());
    let cleaned: String = user
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .take(64)
        .collect();
    if cleaned.is_empty() {
        "cli:root".into()
    } else {
        format!("cli:{cleaned}")
    }
}

/// Record a CLI mutation. Never panics; never includes secrets.
pub fn record_cli(action: &str, target: &str, ok: bool, message: &str) {
    let actor = cli_actor();
    if let Err(error) = record_full(
        &actor,
        action,
        target,
        ok,
        message,
        "",
        "cpn-cli",
        "",
        Some(AuditCategory::Cli),
        Some(if ok {
            AuditSeverity::Notice
        } else {
            AuditSeverity::Warning
        }),
    ) {
        eprintln!("panel-audit(cli): {action}: {error}");
    }
}
