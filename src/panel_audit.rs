//! Full panel audit store: append-only JSONL under `/var/lib/cpn/audit/audit.jsonl`.
//!
//! Migrates legacy `panel-actions.jsonl` on first write. Retention is day-based
//! (default 90 days), not a silent 1 MiB wipe of the compliance window.

use crate::account::now_unix;
use crate::panel_audit_paths::{action_label, category_for_action, default_severity};
use crate::panel_audit_policy::{category_enabled, load_audit_policy, retention_seconds};
use crate::panel_audit_types::{AuditCategory, AuditEvent, AuditRecord, AuditSeverity};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

pub const AUDIT_DIR: &str = "audit";
pub const AUDIT_FILE: &str = "audit.jsonl";
pub const LEGACY_LOG_FILE: &str = "panel-actions.jsonl";
const MAX_FIELD_CHARS: usize = 300;
/// Soft size guide only: prefer retention prune; never drop rows newer than retention.
const SOFT_MAX_BYTES: u64 = 32 * 1024 * 1024;

static WRITE_LOCK: Mutex<()> = Mutex::new(());

pub use crate::panel_audit_paths::{action_for_path, is_noisy_read_like, synthesize_action};
pub use crate::panel_audit_types::{AuditEvent as Event, AuditRecord as Record};

fn audit_dir() -> PathBuf {
    crate::account::data_dir().join(AUDIT_DIR)
}

pub fn log_path() -> PathBuf {
    audit_dir().join(AUDIT_FILE)
}

pub fn legacy_log_path() -> PathBuf {
    crate::account::data_dir().join(LEGACY_LOG_FILE)
}

fn ensure_audit_dir() -> Result<(), String> {
    let dir = audit_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }
    link_legacy_compat();
    Ok(())
}

fn link_legacy_compat() {
    let legacy = legacy_log_path();
    let audit = log_path();
    if legacy.exists() && !audit.exists() {
        if let Ok(raw) = fs::read(&legacy) {
            let _ = fs::write(&audit, raw);
            set_private(&audit);
        }
    }
    #[cfg(unix)]
    {
        if audit.exists() && !legacy.exists() {
            let _ = std::os::unix::fs::symlink(&audit, &legacy);
        }
    }
    #[cfg(not(unix))]
    {
        if audit.exists() && !legacy.exists() {
            let _ = fs::copy(&audit, &legacy);
        }
    }
}

fn set_private(path: &PathBuf) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

fn clean_field(value: &str) -> String {
    let flat: String = value
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let redacted = crate::panel_ops_activity::sanitize_log_line(&flat);
    redacted.chars().take(MAX_FIELD_CHARS).collect()
}

fn prune_by_retention(path: &PathBuf) {
    let cutoff = now_unix().saturating_sub(retention_seconds());
    let Ok(raw) = fs::read_to_string(path) else {
        return;
    };
    let mut kept = Vec::new();
    let mut changed = false;
    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<AuditRecord>(line) {
            Ok(row) if row.ts >= cutoff => kept.push(line.to_string()),
            Ok(_) => changed = true,
            Err(_) => kept.push(line.to_string()),
        }
    }
    if !changed {
        if let Ok(meta) = fs::metadata(path) {
            if meta.len() > SOFT_MAX_BYTES && kept.len() > 5000 {
                let start = kept.len().saturating_sub(5000);
                if start > 0 {
                    kept = kept[start..].to_vec();
                    changed = true;
                }
            }
        }
    }
    if !changed {
        return;
    }
    let mut body = kept.join("\n");
    if !body.is_empty() {
        body.push('\n');
    }
    let tmp = path.with_extension("jsonl.tmp");
    if fs::write(&tmp, body).is_ok() {
        set_private(&tmp);
        let _ = fs::rename(&tmp, path);
    }
}

/// Append a full audit event. Honours non-essential category opt-out. Never panics.
pub fn record_event(event: AuditEvent<'_>) -> Result<(), String> {
    if !category_enabled(event.category) {
        return Ok(());
    }
    let entry = AuditRecord {
        ts: now_unix(),
        actor: clean_field(event.actor),
        action: clean_field(event.action),
        target: clean_field(event.target),
        ok: event.ok,
        message: clean_field(event.message),
        ip: clean_field(event.ip),
        ua: clean_field(event.ua),
        request_id: clean_field(event.request_id),
        category: event.category.as_str().to_string(),
        severity: event.severity.as_str().to_string(),
    };
    let line = serde_json::to_string(&entry).map_err(|e| e.to_string())?;
    let path = log_path();
    let _guard = WRITE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    ensure_audit_dir()?;
    prune_by_retention(&path);
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&path)
        .map_err(|e| format!("open {}: {e}", path.display()))?;
    writeln!(file, "{line}").map_err(|e| format!("write {}: {e}", path.display()))?;
    crate::panel_audit_export::maybe_forward_webhook(&entry);
    crate::panel_audit_alerts::maybe_alert(&entry);
    let _ = load_audit_policy();
    Ok(())
}

/// Compatibility helper matching the legacy `record(actor, action, target, ok, message)` API.
pub fn record(
    actor: &str,
    action: &str,
    target: &str,
    ok: bool,
    message: &str,
) -> Result<(), String> {
    let category = category_for_action(action);
    let severity = default_severity(action, ok);
    record_event(
        AuditEvent::new(actor, action, ok, message)
            .target(target)
            .category(category)
            .severity(severity),
    )
}

/// Richer helper used by auth and middleware.
pub fn record_full(
    actor: &str,
    action: &str,
    target: &str,
    ok: bool,
    message: &str,
    ip: &str,
    ua: &str,
    request_id: &str,
    category: Option<AuditCategory>,
    severity: Option<AuditSeverity>,
) -> Result<(), String> {
    let category = category.unwrap_or_else(|| category_for_action(action));
    let severity = severity.unwrap_or_else(|| default_severity(action, ok));
    record_event(
        AuditEvent::new(actor, action, ok, message)
            .target(target)
            .ip(ip)
            .ua(ua)
            .request_id(request_id)
            .category(category)
            .severity(severity),
    )
}

/// Newest-first records. `actor_filter` limits rows to one account (non-admin viewers).
pub fn recent(limit: usize, actor_filter: Option<&str>) -> Vec<AuditRecord> {
    recent_filtered(limit, actor_filter, None, None)
}

pub fn recent_filtered(
    limit: usize,
    actor_filter: Option<&str>,
    category_filter: Option<&str>,
    ok_filter: Option<bool>,
) -> Vec<AuditRecord> {
    let path = log_path();
    let raw = fs::read_to_string(&path)
        .or_else(|_| fs::read_to_string(legacy_log_path()))
        .unwrap_or_default();
    raw.lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<AuditRecord>(line).ok())
        .filter(|row| match actor_filter {
            Some(actor) => row.actor.eq_ignore_ascii_case(actor),
            None => true,
        })
        .filter(|row| match category_filter {
            Some(cat) if !cat.is_empty() => row.category.eq_ignore_ascii_case(cat),
            _ => true,
        })
        .filter(|row| match ok_filter {
            Some(ok) => row.ok == ok,
            None => true,
        })
        .take(limit)
        .collect()
}

pub fn label_for(action: &str) -> &'static str {
    action_label(action)
}

/// Apply retention prune now (Settings save / cron-friendly).
pub fn apply_retention_now() -> Result<(), String> {
    let path = log_path();
    ensure_audit_dir()?;
    let _guard = WRITE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    prune_by_retention(&path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::panel_audit_policy::{AuditPolicy, save_audit_policy};

    #[test]
    fn record_round_trips_and_redacts() {
        with_test_data_dir(|| {
            record(
                "cpnowner",
                "plugin.install-host",
                "",
                true,
                "Installed MariaDB",
            )
            .unwrap();
            record_full(
                "cpnowner",
                "host.attach",
                "a.example.com",
                false,
                "failed password=hunter2 now",
                "198.51.100.10",
                "TestAgent/1.0",
                "req-1",
                None,
                None,
            )
            .unwrap();
            let rows = recent(10, None);
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0].action, "host.attach");
            assert!(!rows[0].message.contains("hunter2"));
            assert_eq!(rows[0].ip, "198.51.100.10");
            assert_eq!(rows[0].category, "packages");
            assert_eq!(recent(10, Some("other")).len(), 0);
        });
    }

    #[test]
    fn ui_opt_out_skips_non_essential() {
        with_test_data_dir(|| {
            let mut policy = AuditPolicy::default();
            policy.disabled_categories = vec!["ui".into()];
            save_audit_policy(&policy).unwrap();
            record("alice", "ui.prefs", "", true, "theme toggled").unwrap();
            record("alice", "auth.login", "", true, "ok").unwrap();
            let rows = recent(10, None);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].action, "auth.login");
        });
    }

    #[test]
    fn retention_drops_old_rows() {
        with_test_data_dir(|| {
            let path = log_path();
            ensure_audit_dir().unwrap();
            let old = AuditRecord {
                ts: 1,
                actor: "a".into(),
                action: "auth.login".into(),
                target: String::new(),
                ok: true,
                message: "ancient".into(),
                ip: String::new(),
                ua: String::new(),
                request_id: String::new(),
                category: "auth".into(),
                severity: "info".into(),
            };
            let fresh = AuditRecord {
                ts: now_unix(),
                actor: "a".into(),
                action: "auth.logout".into(),
                target: String::new(),
                ok: true,
                message: "now".into(),
                ip: String::new(),
                ua: String::new(),
                request_id: String::new(),
                category: "auth".into(),
                severity: "info".into(),
            };
            let body = format!(
                "{}\n{}\n",
                serde_json::to_string(&old).unwrap(),
                serde_json::to_string(&fresh).unwrap()
            );
            fs::write(&path, body).unwrap();
            apply_retention_now().unwrap();
            let rows = recent(10, None);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].action, "auth.logout");
        });
    }
}
