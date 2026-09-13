//! Per-user UI preferences (color mode, minimalist mode) under `/var/lib/cpn/user-prefs/`.

use crate::account::data_dir;
use crate::panel_theme::ColorMode;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserUiPrefs {
    #[serde(default)]
    pub color_mode: ColorMode,
    /// When true, prefer static UI: no live metrics polling; refresh to update.
    #[serde(default)]
    pub minimalist_mode: bool,
}

fn user_prefs_path(username: &str) -> PathBuf {
    data_dir()
        .join("user-prefs")
        .join(format!("{}.json", safe_username_key(username)))
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

fn write_json(path: &PathBuf, value: &impl Serialize) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create prefs dir: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(value)
        .map_err(|e| format!("Could not serialize prefs: {e}"))?;
    fs::write(path, raw).map_err(|e| format!("Could not write prefs: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn load_user_ui_prefs(username: &str) -> UserUiPrefs {
    let Ok(raw) = fs::read_to_string(user_prefs_path(username)) else {
        return UserUiPrefs::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn load_user_color_mode(username: &str) -> ColorMode {
    load_user_ui_prefs(username).color_mode
}

pub fn save_user_color_mode(username: &str, mode: ColorMode) -> Result<ColorMode, String> {
    let mut prefs = load_user_ui_prefs(username);
    prefs.color_mode = mode;
    write_json(&user_prefs_path(username), &prefs)?;
    Ok(mode)
}

pub fn load_user_minimalist_mode(username: &str) -> bool {
    load_user_ui_prefs(username).minimalist_mode
}

pub fn save_user_minimalist_mode(username: &str, enabled: bool) -> Result<bool, String> {
    let mut prefs = load_user_ui_prefs(username);
    prefs.minimalist_mode = enabled;
    write_json(&user_prefs_path(username), &prefs)?;
    Ok(enabled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn user_color_mode_persists() {
        with_test_data_dir(|| {
            assert_eq!(load_user_color_mode("Admin"), ColorMode::Light);
            save_user_color_mode("Admin", ColorMode::Dark).unwrap();
            assert_eq!(load_user_color_mode("Admin"), ColorMode::Dark);
            assert_eq!(safe_username_key("Admin/../x"), "admin_.._x");
        });
    }

    #[test]
    fn user_minimalist_mode_persists_without_wiping_color() {
        with_test_data_dir(|| {
            assert!(!load_user_minimalist_mode("Admin"));
            save_user_color_mode("Admin", ColorMode::Dark).unwrap();
            save_user_minimalist_mode("Admin", true).unwrap();
            assert!(load_user_minimalist_mode("Admin"));
            assert_eq!(load_user_color_mode("Admin"), ColorMode::Dark);
            save_user_color_mode("Admin", ColorMode::Light).unwrap();
            assert!(load_user_minimalist_mode("Admin"));
            assert_eq!(load_user_color_mode("Admin"), ColorMode::Light);
        });
    }
}
