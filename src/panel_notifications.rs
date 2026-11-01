//! Panel notification store under `/var/lib/cpn/notifications/<user>.json`.
//!
//! Categories: `ssl`, `plugins`, `panel`, `generic`. Prefer
//! [`notify_best_effort`] / [`notify_panel_admins`] from event sites so the inbox
//! fills automatically without a parallel system.

use crate::account::{data_dir, load_bootstrap};
use crate::account_mgmt::list_accounts;
use crate::packages::is_panel_admin;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// SSL certificate events (issue, renew, expiry).
pub const CATEGORY_SSL: &str = "ssl";
/// Plugin install / update / failure.
pub const CATEGORY_PLUGINS: &str = "plugins";
/// Panel install / upgrade / version events.
pub const CATEGORY_PANEL: &str = "panel";
/// Site create/delete and other panel events.
pub const CATEGORY_GENERIC: &str = "generic";
/// Toast / inbox severity: `error`, `warning`, `success`, or `info`.
pub const SEVERITIES: &[&str] = &["error", "warning", "success", "info"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PanelNotification {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub category: String,
    /// Optional explicit level. Empty on older JSON; resolved at read time.
    #[serde(default, alias = "level")]
    pub severity: String,
    pub created_at: u64,
    #[serde(default)]
    pub read: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NotificationStore {
    #[serde(default)]
    pub items: Vec<PanelNotification>,
}

fn safe_username_key(username: &str) -> String {
    let trimmed = username.trim();
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "user".into()
    } else {
        out.chars().take(128).collect()
    }
}

fn store_path(username: &str) -> PathBuf {
    data_dir()
        .join("notifications")
        .join(format!("{}.json", safe_username_key(username)))
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn write_store(path: &PathBuf, store: &NotificationStore) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create notifications dir: {error}"))?;
    }
    let raw = serde_json::to_string_pretty(store)
        .map_err(|error| format!("Could not serialize notifications: {error}"))?;
    fs::write(path, raw).map_err(|error| format!("Could not write notifications: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Normalize a caller-supplied severity / level string.
pub fn normalize_severity(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "error" | "err" | "danger" | "critical" | "fail" | "failed" => Some("error"),
        "warning" | "warn" | "caution" => Some("warning"),
        "success" | "ok" | "done" => Some("success"),
        "info" | "information" | "neutral" | "notice" => Some("info"),
        _ => None,
    }
}

/// Infer toast type from category + title/body keywords (backward compatible).
pub fn infer_severity(category: &str, title: &str, body: &str) -> &'static str {
    let cat = category.trim().to_ascii_lowercase();
    let title_l = title.to_ascii_lowercase();
    let body_l = body.to_ascii_lowercase();
    let blob = format!("{cat} {title_l} {body_l}");

    let has_fail = blob.contains("fail")
        || blob.contains("error")
        || blob.contains("denied")
        || blob.contains("critical")
        || blob.contains("unable")
        || blob.contains("could not");
    let has_warn = blob.contains("expir")
        || blob.contains("warn")
        || blob.contains("attention")
        || blob.contains("soon")
        || blob.contains("degrad");
    let has_success = blob.contains("renew")
        || blob.contains("success")
        || blob.contains("installed")
        || blob.contains("completed")
        || blob.contains("created")
        || blob.contains("ready");

    if has_fail {
        return "error";
    }
    if has_warn && !has_success {
        return "warning";
    }
    if has_success {
        return "success";
    }

    match cat.as_str() {
        "ssl" | "plugin" | "upgrade" | "site" | "ops" | "panel" | "" => "info",
        _ => "info",
    }
}

/// Resolve stored or inferred severity for display / API.
pub fn resolve_severity(item: &PanelNotification) -> &'static str {
    if let Some(s) = normalize_severity(&item.severity) {
        return s;
    }
    infer_severity(&item.category, &item.title, &item.body)
}

pub fn load_notifications(username: &str) -> NotificationStore {
    let path = store_path(username);
    let Ok(raw) = fs::read_to_string(&path) else {
        return NotificationStore::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn unread_count(store: &NotificationStore) -> usize {
    store.items.iter().filter(|item| !item.read).count()
}

/// Push a panel notification for a user (install events, SSL, plugins, etc.).
pub fn push_notification(
    username: &str,
    title: &str,
    body: &str,
    category: &str,
) -> Result<PanelNotification, String> {
    push_notification_with_severity(username, title, body, category, None)
}

/// Push with an optional explicit severity (`error`|`warning`|`success`|`info`).
pub fn push_notification_with_severity(
    username: &str,
    title: &str,
    body: &str,
    category: &str,
    severity: Option<&str>,
) -> Result<PanelNotification, String> {
    let path = store_path(username);
    let mut store = load_notifications(username);
    let category = category.trim().to_string();
    let title = title.trim().to_string();
    let body = body.trim().to_string();
    if title.is_empty() {
        return Err("title is required".into());
    }
    let severity = severity
        .and_then(normalize_severity)
        .unwrap_or_else(|| infer_severity(&category, &title, &body))
        .to_string();
    let item = PanelNotification {
        id: format!("n-{}-{}", now_unix(), store.items.len() + 1),
        title,
        body,
        category,
        severity,
        created_at: now_unix(),
        read: false,
    };
    store.items.insert(0, item.clone());
    // Cap growth so the file stays small.
    if store.items.len() > 100 {
        store.items.truncate(100);
    }
    write_store(&path, &store)?;
    Ok(item)
}

pub fn mark_read(username: &str, ids: &[String], all: bool) -> Result<NotificationStore, String> {
    let path = store_path(username);
    let mut store = load_notifications(username);
    for item in &mut store.items {
        if all || ids.iter().any(|id| id == &item.id) {
            item.read = true;
        }
    }
    write_store(&path, &store)?;
    Ok(store)
}

fn item_public_json(item: &PanelNotification) -> serde_json::Value {
    let severity = resolve_severity(item);
    serde_json::json!({
        "id": item.id,
        "title": item.title,
        "body": item.body,
        "category": item.category,
        "severity": severity,
        "level": severity,
        "created_at": item.created_at,
        "read": item.read,
    })
}

pub fn notifications_public_json(store: &NotificationStore) -> serde_json::Value {
    let items: Vec<serde_json::Value> = store.items.iter().map(item_public_json).collect();
    serde_json::json!({
        "ok": true,
        "unread_count": unread_count(store),
        "items": items,
    })
}

/// Push without failing the caller (event hooks must stay best-effort).
pub fn notify_best_effort(username: &str, title: &str, body: &str, category: &str) {
    let _ = push_notification(username, title, body, category);
}

fn unique_usernames(names: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out = Vec::new();
    for name in names {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            continue;
        }
        if out
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(trimmed))
        {
            continue;
        }
        out.push(trimmed.to_string());
    }
    out
}

/// Bootstrap owner plus any account that counts as panel admin.
pub fn panel_admin_usernames() -> Vec<String> {
    let mut names = Vec::new();
    if let Some(boot) = load_bootstrap() {
        names.push(boot.username);
    }
    if let Ok(accounts) = list_accounts() {
        for acct in accounts {
            if is_panel_admin(&acct.username) {
                names.push(acct.username);
            }
        }
    }
    unique_usernames(names)
}

/// Notify every panel admin (install events, SSL, plugins, version).
pub fn notify_panel_admins(title: &str, body: &str, category: &str) {
    for user in panel_admin_usernames() {
        notify_best_effort(&user, title, body, category);
    }
}

/// Notify a site owner and panel admins (deduped).
pub fn notify_owner_and_admins(owner: &str, title: &str, body: &str, category: &str) {
    let mut names = panel_admin_usernames();
    if !owner.trim().is_empty() {
        names.push(owner.trim().to_string());
    }
    for user in unique_usernames(names) {
        notify_best_effort(&user, title, body, category);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::default_password_policy;
    use crate::account::with_test_data_dir;
    use crate::account_mgmt::create_account;

    #[test]
    fn push_and_mark_read_round_trip() {
        with_test_data_dir(|| {
            let item = push_notification(
                "Admin",
                "SSL renewed",
                "Certificate for example.com was renewed.",
                CATEGORY_SSL,
            )
            .expect("push");
            let store = load_notifications("Admin");
            assert_eq!(store.items.len(), 1);
            assert_eq!(unread_count(&store), 1);
            assert_eq!(store.items[0].id, item.id);
            assert_eq!(store.items[0].category, CATEGORY_SSL);
            assert_eq!(resolve_severity(&store.items[0]), "success");

            let updated = mark_read("Admin", std::slice::from_ref(&item.id), false).expect("mark");
            assert_eq!(unread_count(&updated), 0);
            assert!(updated.items[0].read);
        });
    }

    #[test]
    fn empty_store_is_ok() {
        with_test_data_dir(|| {
            let store = load_notifications("nobody");
            assert!(store.items.is_empty());
            assert_eq!(unread_count(&store), 0);
        });
    }

    #[test]
    fn notify_admins_reaches_bootstrap_owner() {
        with_test_data_dir(|| {
            unsafe {
                std::env::set_var("CPN_RESERVED_USERNAMES_OFFLINE", "1");
            }
            let created = create_account(
                "panelowner",
                None,
                true,
                "owner@example.com",
                default_password_policy(),
                "en",
            )
            .expect("create");
            assert!(created.generated_password.is_some());
            notify_panel_admins(
                "Plugin installed",
                "demoPlugin is ready on the host.",
                CATEGORY_PLUGINS,
            );
            let store = load_notifications("panelowner");
            assert_eq!(store.items.len(), 1);
            assert_eq!(store.items[0].category, CATEGORY_PLUGINS);
            assert!(store.items[0].title.contains("Plugin"));
            unsafe {
                std::env::remove_var("CPN_RESERVED_USERNAMES_OFFLINE");
            }
        });
    }

    #[test]
    fn severity_inference_maps_keywords() {
        assert_eq!(infer_severity("ssl", "SSL failure", ""), "error");
        assert_eq!(
            infer_severity("plugin", "Plugin failure", "install failed"),
            "error"
        );
        assert_eq!(infer_severity("upgrade", "Upgrade failed", ""), "error");
        assert_eq!(
            infer_severity("ssl", "SSL expiring", "expires soon"),
            "warning"
        );
        assert_eq!(infer_severity("ssl", "SSL renewed", ""), "success");
        assert_eq!(infer_severity("plugin", "Plugin installed", ""), "success");
        assert_eq!(
            infer_severity("upgrade", "Upgrade completed", ""),
            "success"
        );
        assert_eq!(infer_severity("site", "Site created", ""), "success");
        assert_eq!(infer_severity("panel", "Hello", "neutral note"), "info");
        assert_eq!(normalize_severity("WARN"), Some("warning"));
        assert_eq!(normalize_severity("nope"), None);
    }

    #[test]
    fn explicit_severity_wins_over_keywords() {
        with_test_data_dir(|| {
            let item = push_notification_with_severity(
                "Admin",
                "Upgrade failed",
                "details",
                "upgrade",
                Some("info"),
            )
            .expect("push");
            assert_eq!(item.severity, "info");
            assert_eq!(resolve_severity(&item), "info");
        });
    }

    #[test]
    fn public_json_includes_severity_and_level() {
        with_test_data_dir(|| {
            let _ = push_notification("Admin", "SSL failure", "x", "ssl").expect("push");
            let store = load_notifications("Admin");
            let json = notifications_public_json(&store);
            let sev = json["items"][0]["severity"].as_str().unwrap_or("");
            let level = json["items"][0]["level"].as_str().unwrap_or("");
            assert_eq!(sev, "error");
            assert_eq!(level, "error");
        });
    }

    #[test]
    fn legacy_json_without_severity_still_loads() {
        with_test_data_dir(|| {
            let path = store_path("Legacy");
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("dir");
            }
            fs::write(
                &path,
                r#"{
  "items": [
    {
      "id": "n-1",
      "title": "SSL expiring",
      "body": "example.com",
      "category": "ssl",
      "created_at": 1,
      "read": false
    }
  ]
}"#,
            )
            .expect("write");
            let store = load_notifications("Legacy");
            assert_eq!(store.items.len(), 1);
            assert!(store.items[0].severity.is_empty());
            assert_eq!(resolve_severity(&store.items[0]), "warning");
        });
    }
}
