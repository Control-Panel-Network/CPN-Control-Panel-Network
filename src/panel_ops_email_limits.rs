//! Per-mailbox / per-domain sending limits (CPN metadata + Postfix policy hint file).

use crate::paths::join_data;
use crate::postfix_fallback::postfix_is_ready;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendLimit {
    pub id: String,
    /// Mailbox address or `@domain` scope.
    pub scope: String,
    /// Max messages per rolling window.
    pub max_messages: u32,
    /// Window length in minutes.
    pub window_minutes: u32,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct LimitsFile {
    #[serde(default)]
    limits: Vec<SendLimit>,
}

fn store_path() -> PathBuf {
    join_data("mail-send-limits.json")
}

fn policy_path() -> PathBuf {
    join_data("mail/send_limits.policy")
}

pub fn load_send_limits() -> Vec<SendLimit> {
    fs::read_to_string(store_path())
        .ok()
        .and_then(|raw| serde_json::from_str::<LimitsFile>(&raw).ok())
        .map(|f| f.limits)
        .unwrap_or_default()
}

fn save_send_limits(limits: &[SendLimit]) -> Result<(), String> {
    let path = store_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_string_pretty(&LimitsFile {
        limits: limits.to_vec(),
    })
    .map_err(|e| e.to_string())?;
    fs::write(&path, raw).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn validate_scope(scope: &str) -> Result<String, String> {
    let s = scope.trim().to_ascii_lowercase();
    if s.is_empty() || s.len() > 253 {
        return Err("Scope must be mailbox@domain or @domain".into());
    }
    if s.starts_with('@') {
        if s.len() < 3 || s.contains(' ') {
            return Err("Domain scope must look like @example.com".into());
        }
        return Ok(s);
    }
    if !s.contains('@') || s.contains(' ') {
        return Err("Mailbox scope must be a valid email".into());
    }
    Ok(s)
}

pub fn add_send_limit(
    scope: &str,
    max_messages: u32,
    window_minutes: u32,
    note: &str,
) -> Result<String, String> {
    if max_messages == 0 || max_messages > 100_000 {
        return Err("max_messages must be 1..100000".into());
    }
    if window_minutes == 0 || window_minutes > 10_080 {
        return Err("window_minutes must be 1..10080 (one week)".into());
    }
    let scope = validate_scope(scope)?;
    let mut limits = load_send_limits();
    let id = format!("lim-{}", crate::account::now_unix());
    limits.push(SendLimit {
        id: id.clone(),
        scope,
        max_messages,
        window_minutes,
        note: note.trim().chars().take(200).collect(),
    });
    save_send_limits(&limits)?;
    let apply = write_policy_file()?;
    Ok(format!("Limit {id} saved. {apply}"))
}

pub fn remove_send_limit(id: &str) -> Result<String, String> {
    let id = id.trim();
    let mut limits = load_send_limits();
    let before = limits.len();
    limits.retain(|l| l.id != id);
    if limits.len() == before {
        return Err(format!("Limit `{id}` not found"));
    }
    save_send_limits(&limits)?;
    let apply = write_policy_file()?;
    Ok(format!("Limit removed. {apply}"))
}

/// Write a human/policy hint file Postfix operators (or policyd) can consume.
pub fn write_policy_file() -> Result<String, String> {
    let limits = load_send_limits();
    let path = policy_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut body = String::from("# CPN send limits (messages / window_minutes)\n");
    body.push_str("# Format: scope max_messages window_minutes\n");
    for lim in &limits {
        body.push_str(&format!(
            "{} {} {}\n",
            lim.scope, lim.max_messages, lim.window_minutes
        ));
    }
    fs::write(&path, &body).map_err(|e| e.to_string())?;

    if postfix_is_ready() && !limits.is_empty() {
        let min_rate = limits.iter().map(|l| l.max_messages).min().unwrap_or(0);
        if min_rate > 0 {
            let _ = Command::new("postconf")
                .args([
                    "-e",
                    &format!("smtpd_client_message_rate_limit={min_rate}"),
                ])
                .status();
        }
    }

    Ok(format!(
        "Policy file at {} ({} rule(s)). CPN enforces metadata for panel marketing sends; Postfix rate hint updated when MTA is ready.",
        path.display(),
        limits.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn limits_roundtrip() {
        with_test_data_dir(|| {
            add_send_limit("user@example.com", 100, 60, "test").unwrap();
            assert_eq!(load_send_limits().len(), 1);
            let id = load_send_limits()[0].id.clone();
            remove_send_limit(&id).unwrap();
            assert!(load_send_limits().is_empty());
        });
    }
}
