//! Audit retention and non-essential category opt-out (security-essential locked on).

use crate::panel_audit_types::AuditCategory;
use crate::paths::join_data;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

/// Default keep window for panel audit JSONL (CRA compliance window).
pub const DEFAULT_AUDIT_RETENTION_DAYS: u32 = 90;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditPolicy {
    /// Days to keep audit rows (default 90). Never silently wipe the compliance window.
    #[serde(default = "default_days")]
    pub retention_days: u32,
    /// Optional webhook URL for forward (no secrets in payloads). Empty disables.
    #[serde(default)]
    pub webhook_url: String,
    /// Non-essential categories the owner opted out of recording.
    #[serde(default)]
    pub disabled_categories: Vec<String>,
    /// When true, owner alerts fire for key security events.
    #[serde(default = "default_true")]
    pub alerts_enabled: bool,
}

fn default_days() -> u32 {
    DEFAULT_AUDIT_RETENTION_DAYS
}

fn default_true() -> bool {
    true
}

impl Default for AuditPolicy {
    fn default() -> Self {
        Self {
            retention_days: DEFAULT_AUDIT_RETENTION_DAYS,
            webhook_url: String::new(),
            disabled_categories: Vec::new(),
            alerts_enabled: true,
        }
    }
}

fn prefs_path() -> PathBuf {
    join_data("audit").join("policy.json")
}

pub fn load_audit_policy() -> AuditPolicy {
    let path = prefs_path();
    let Ok(raw) = fs::read_to_string(&path) else {
        return AuditPolicy::default();
    };
    let mut policy: AuditPolicy = serde_json::from_str(&raw).unwrap_or_default();
    sanitize_policy(&mut policy);
    policy
}

fn sanitize_policy(policy: &mut AuditPolicy) {
    if policy.retention_days == 0 {
        policy.retention_days = DEFAULT_AUDIT_RETENTION_DAYS;
    }
    if policy.retention_days > 3650 {
        policy.retention_days = 3650;
    }
    policy.disabled_categories.retain(|raw| {
        AuditCategory::parse(raw)
            .map(|c| !c.is_security_essential())
            .unwrap_or(false)
    });
    policy.webhook_url = policy.webhook_url.trim().to_string();
    if policy.webhook_url.len() > 500 {
        policy.webhook_url.truncate(500);
    }
}

pub fn save_audit_policy(policy: &AuditPolicy) -> Result<(), String> {
    let mut cleaned = policy.clone();
    sanitize_policy(&mut cleaned);
    let path = prefs_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create audit dir: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(&cleaned)
        .map_err(|e| format!("Could not serialize audit policy: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("Could not write audit policy: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Returns true when this category should be recorded under current policy.
pub fn category_enabled(category: AuditCategory) -> bool {
    if category.is_security_essential() {
        return true;
    }
    let policy = load_audit_policy();
    let disabled: HashSet<String> = policy
        .disabled_categories
        .iter()
        .map(|s| s.to_ascii_lowercase())
        .collect();
    !disabled.contains(category.as_str())
}

pub fn retention_seconds() -> u64 {
    let days = load_audit_policy().retention_days.max(1);
    u64::from(days).saturating_mul(86_400)
}

/// Categories owners may opt out of (non-essential only).
pub fn opt_outable_categories() -> &'static [AuditCategory] {
    &[
        AuditCategory::Ui,
        AuditCategory::Themes,
        AuditCategory::Notifications,
        AuditCategory::Other,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn cannot_disable_auth() {
        with_test_data_dir(|| {
            let mut policy = AuditPolicy::default();
            policy.disabled_categories = vec!["auth".into(), "ui".into()];
            save_audit_policy(&policy).unwrap();
            let loaded = load_audit_policy();
            assert!(
                !loaded
                    .disabled_categories
                    .iter()
                    .any(|c| c.eq_ignore_ascii_case("auth"))
            );
            assert!(
                loaded
                    .disabled_categories
                    .iter()
                    .any(|c| c.eq_ignore_ascii_case("ui"))
            );
            assert!(category_enabled(AuditCategory::Auth));
            assert!(!category_enabled(AuditCategory::Ui));
        });
    }

    #[test]
    fn default_retention_is_ninety() {
        assert_eq!(AuditPolicy::default().retention_days, 90);
    }
}
