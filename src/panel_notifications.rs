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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PanelNotification {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub category: String,
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
    let path = store_path(username);
    let mut store = load_notifications(username);
    let item = PanelNotification {
        id: format!("n-{}-{}", now_unix(), store.items.len() + 1),
        title: title.trim().to_string(),
        body: body.trim().to_string(),
        category: category.trim().to_string(),
        created_at: now_unix(),
        read: false,
    };
    if item.title.is_empty() {
        return Err("title is required".into());
    }
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

pub fn notifications_public_json(store: &NotificationStore) -> serde_json::Value {
    serde_json::json!({
        "ok": true,
        "unread_count": unread_count(store),
        "items": store.items,
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
}
