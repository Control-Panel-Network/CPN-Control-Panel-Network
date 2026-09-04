//! Panel notification store under `/var/lib/cpn/notifications/<user>.json`.

use crate::account::data_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn push_and_mark_read_round_trip() {
        with_test_data_dir(|| {
            let item = push_notification(
                "Admin",
                "SSL renewed",
                "Certificate for example.com was renewed.",
                "ssl",
            )
            .expect("push");
            let store = load_notifications("Admin");
            assert_eq!(store.items.len(), 1);
            assert_eq!(unread_count(&store), 1);
            assert_eq!(store.items[0].id, item.id);

            let updated = mark_read("Admin", &[item.id.clone()], false).expect("mark");
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
}
