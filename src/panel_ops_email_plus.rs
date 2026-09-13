//! Plus-addressing (user+tag@domain) via Postfix recipient_delimiter.

use crate::panel_ops_security::{cmd_stdout, which_exists};
use crate::paths::join_data;
use crate::postfix_fallback::postfix_is_ready;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlusSettings {
    pub enabled: bool,
    /// Usually `+`.
    pub delimiter: String,
}

impl Default for PlusSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            delimiter: "+".into(),
        }
    }
}

fn store_path() -> PathBuf {
    join_data("mail-plus-addressing.json")
}

pub fn load_plus_settings() -> PlusSettings {
    fs::read_to_string(store_path())
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save_plus_settings(settings: &PlusSettings) -> Result<(), String> {
    let path = store_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(&path, raw).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn current_postfix_delimiter() -> String {
    if which_exists("postconf") {
        if let Some(v) = cmd_stdout("postconf", &["-h", "recipient_delimiter"]) {
            return v;
        }
    }
    "(unknown)".into()
}

pub fn set_plus_addressing(enabled: bool, delimiter: &str) -> Result<String, String> {
    let delim = delimiter.trim();
    if delim != "+" && delim != "-" && delim != "=" {
        return Err("Delimiter must be +, -, or =".into());
    }
    let settings = PlusSettings {
        enabled,
        delimiter: delim.to_string(),
    };
    save_plus_settings(&settings)?;

    if !postfix_is_ready() && !which_exists("postconf") {
        return Ok(format!(
            "Saved panel preference (enabled={enabled}, delimiter={delim}). Postfix not ready to apply."
        ));
    }

    let value = if enabled { delim } else { "" };
    let out = Command::new("postconf")
        .args(["-e", &format!("recipient_delimiter={value}")])
        .output()
        .map_err(|e| format!("postconf failed: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("Could not set recipient_delimiter: {}", err.trim()));
    }
    let _ = Command::new("systemctl")
        .args(["reload", "postfix"])
        .status();
    Ok(format!(
        "Plus-addressing {} (recipient_delimiter={}). Postfix reloaded when available.",
        if enabled { "enabled" } else { "disabled" },
        if enabled { delim } else { "(empty)" }
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn settings_persist() {
        with_test_data_dir(|| {
            // May fail postconf on Windows/CI; still persist JSON.
            let _ = set_plus_addressing(true, "+");
            let s = load_plus_settings();
            assert!(s.enabled);
            assert_eq!(s.delimiter, "+");
        });
    }
}
