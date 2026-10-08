//! Privacy-safe Mr Agent usage statistics (no message bodies, no secrets).

use crate::mr_agent_install::HOST_DOMAIN_SENTINEL;
use crate::plugins_settings::load_plugin_settings;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize)]
pub struct MrAgentStats {
    pub ok: bool,
    pub scope: String,
    pub domain: String,
    pub conversations_total: u64,
    pub messages_total: u64,
    pub conversations_7d: u64,
    pub messages_7d: u64,
    pub conversations_30d: u64,
    pub messages_30d: u64,
    pub distinct_users: u64,
    pub storage_bytes: u64,
    pub storage_mb: f64,
    pub storage_limit_mb: u64,
    pub last_activity: String,
    pub empty: bool,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn secret_dir(domain: &str) -> PathBuf {
    let d = domain.trim();
    if d.is_empty() || d.eq_ignore_ascii_case(HOST_DOMAIN_SENTINEL) {
        PathBuf::from("/var/lib/cpn/mr-agent/_host")
    } else {
        PathBuf::from("/var/lib/cpn/mr-agent").join(d)
    }
}

fn chats_dir(domain: &str) -> PathBuf {
    secret_dir(domain).join("chats")
}

fn format_eu_datetime(ts: u64) -> String {
    if ts == 0 {
        return String::new();
    }
    // UTC wall clock as dd/mm/yyyy HH:mm (European display).
    let s = crate::panel_feedback_mail::format_eu_datetime_utc(ts);
    s.trim_end_matches(" UTC").trim().to_string()
}

fn dir_size_bytes(path: &PathBuf) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    let mut total = 0u64;
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file() {
            total += entry.metadata().map(|m| m.len()).unwrap_or(0);
        } else if p.is_dir() {
            total += dir_size_bytes(&p);
        }
    }
    total
}

fn storage_limit_mb(domain: &str) -> u64 {
    let settings = load_plugin_settings(domain, "mrAgent").unwrap_or_default();
    settings
        .fields
        .get("max_chat_disk_mb")
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(50)
}

/// Collect privacy-safe stats for host (`_host`) or a site domain.
pub fn collect_stats(domain_raw: &str) -> MrAgentStats {
    let domain = {
        let d = domain_raw.trim();
        if d.is_empty() {
            HOST_DOMAIN_SENTINEL
        } else {
            d
        }
    };
    let is_host = domain.eq_ignore_ascii_case(HOST_DOMAIN_SENTINEL);
    let dir = chats_dir(domain);
    let now = now_unix();
    let cut7 = now.saturating_sub(7 * 86400);
    let cut30 = now.saturating_sub(30 * 86400);
    let mut users: HashSet<String> = HashSet::new();
    let mut last_ts = 0u64;
    let mut conversations_total = 0u64;
    let mut messages_total = 0u64;
    let mut conversations_7d = 0u64;
    let mut messages_7d = 0u64;
    let mut conversations_30d = 0u64;
    let mut messages_30d = 0u64;
    let storage_bytes = if dir.is_dir() {
        dir_size_bytes(&dir)
    } else {
        0
    };
    let limit_mb = storage_limit_mb(domain);

    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Ok(raw) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(data) = serde_json::from_str::<Value>(&raw) else {
                continue;
            };
            conversations_total += 1;
            if let Some(user) = data.get("user").and_then(|v| v.as_str()) {
                let u = user.trim().to_ascii_lowercase();
                if !u.is_empty() {
                    users.insert(u);
                }
            }
            let updated = data
                .get("updated_at")
                .and_then(|v| v.as_u64())
                .or_else(|| {
                    path.metadata()
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                })
                .unwrap_or(0);
            if updated > last_ts {
                last_ts = updated;
            }
            let messages = data
                .get("messages")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            messages_total += messages.len() as u64;
            let mut msg7 = 0u64;
            let mut msg30 = 0u64;
            for m in &messages {
                let ts = m.get("ts").and_then(|v| v.as_u64()).unwrap_or(updated);
                if ts >= cut7 {
                    msg7 += 1;
                }
                if ts >= cut30 {
                    msg30 += 1;
                }
            }
            messages_7d += msg7;
            messages_30d += msg30;
            if updated >= cut7 {
                conversations_7d += 1;
            }
            if updated >= cut30 {
                conversations_30d += 1;
            }
        }
    }

    let empty = conversations_total == 0 && storage_bytes == 0;
    MrAgentStats {
        ok: true,
        scope: if is_host {
            "host".into()
        } else {
            "site".into()
        },
        domain: if is_host {
            HOST_DOMAIN_SENTINEL.into()
        } else {
            domain.to_string()
        },
        conversations_total,
        messages_total,
        conversations_7d,
        messages_7d,
        conversations_30d,
        messages_30d,
        distinct_users: users.len() as u64,
        storage_bytes,
        storage_mb: (storage_bytes as f64) / 1_048_576.0,
        storage_limit_mb: limit_mb,
        last_activity: format_eu_datetime(last_ts),
        empty,
    }
}
