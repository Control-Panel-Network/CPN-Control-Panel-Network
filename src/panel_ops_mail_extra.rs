//! Extra mail features: forwarding, catch-all, DKIM file stores when Postfix exists.

use crate::panel_ops_dkim_keys::{dkim_root, dkim_status_detail, ensure_dkim_root};
use crate::paths::join_data;
use crate::postfix_fallback::postfix_is_ready;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MailForward {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CatchAll {
    pub domain: String,
    pub target: String,
}

fn forwards_path() -> PathBuf {
    join_data("mail-forwards.json")
}

fn catchall_path() -> PathBuf {
    join_data("mail-catchall.json")
}

pub fn mail_stack_note() -> String {
    if postfix_is_ready() {
        "Postfix is ready. Panel stores forwarding/catch-all JSON under the CPN data dir; map to Postfix maps in a follow-up."
            .into()
    } else {
        "Postfix not detected. Configure Postfix (or external SMTP) before enabling deliverability tools."
            .into()
    }
}

pub fn load_forwards() -> Vec<MailForward> {
    fs::read_to_string(forwards_path())
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn save_forwards(rows: &[MailForward]) -> Result<(), String> {
    if let Some(parent) = forwards_path().parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_string_pretty(rows).map_err(|e| e.to_string())?;
    fs::write(forwards_path(), raw).map_err(|e| e.to_string())
}

pub fn load_catchall() -> Vec<CatchAll> {
    fs::read_to_string(catchall_path())
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn save_catchall(rows: &[CatchAll]) -> Result<(), String> {
    if let Some(parent) = catchall_path().parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_string_pretty(rows).map_err(|e| e.to_string())?;
    fs::write(catchall_path(), raw).map_err(|e| e.to_string())
}

pub fn dkim_status() -> (bool, String) {
    dkim_status_detail()
}

pub fn ensure_dkim_dir() -> Result<PathBuf, String> {
    ensure_dkim_root()
}

pub fn ensure_dkim_store_ready() -> Result<String, String> {
    let dir = ensure_dkim_root()?;
    Ok(format!(
        "DKIM directory ready at {} (keys are generated per domain on site create).",
        dir.display()
    ))
}

pub fn dkim_store_path_display() -> String {
    dkim_root().display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn forwards_roundtrip() {
        with_test_data_dir(|| {
            save_forwards(&[MailForward {
                from: "a@example.com".into(),
                to: "b@example.com".into(),
            }])
            .unwrap();
            let rows = load_forwards();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].from, "a@example.com");
        });
    }
}
