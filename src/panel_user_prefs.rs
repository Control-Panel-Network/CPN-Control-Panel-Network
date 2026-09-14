//! Per-user UI preferences (color mode, minimalist mode, SSH review snooze)
//! under `/var/lib/cpn/user-prefs/`.

use crate::account::data_dir;
use crate::panel_theme::ColorMode;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum days the Activity Board SSH security review may stay hidden.
pub const SSH_SECURITY_REVIEW_SNOOZE_MAX_DAYS: u32 = 30;

/// Default snooze when the operator picks "Hide for 1 month".
pub const SSH_SECURITY_REVIEW_SNOOZE_DEFAULT_DAYS: u32 = 30;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserUiPrefs {
    #[serde(default)]
    pub color_mode: ColorMode,
    /// When true, prefer static UI: no live metrics polling; refresh to update.
    #[serde(default)]
    pub minimalist_mode: bool,
    /// Unix epoch seconds until which the SSH security review card stays hidden.
    /// Cleared or past values mean the card is shown again.
    #[serde(default)]
    pub ssh_security_review_snooze_until: Option<i64>,
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

fn now_epoch_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
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

/// Active snooze expiry (unix seconds) when the review should stay hidden, else `None`.
pub fn ssh_security_review_snooze_until(username: &str) -> Option<i64> {
    let until = load_user_ui_prefs(username).ssh_security_review_snooze_until?;
    if until > now_epoch_secs() {
        Some(until)
    } else {
        None
    }
}

pub fn is_ssh_security_review_snoozed(username: &str) -> bool {
    ssh_security_review_snooze_until(username).is_some()
}

/// Hide the SSH security review for `days` (clamped to 1..=30). Returns expiry epoch.
pub fn snooze_ssh_security_review(username: &str, days: u32) -> Result<i64, String> {
    let days = days.clamp(1, SSH_SECURITY_REVIEW_SNOOZE_MAX_DAYS);
    let until = now_epoch_secs().saturating_add(i64::from(days) * 86_400);
    let mut prefs = load_user_ui_prefs(username);
    prefs.ssh_security_review_snooze_until = Some(until);
    write_json(&user_prefs_path(username), &prefs)?;
    Ok(until)
}

/// Clear any active or expired snooze so the review shows again.
pub fn clear_ssh_security_review_snooze(username: &str) -> Result<(), String> {
    let mut prefs = load_user_ui_prefs(username);
    if prefs.ssh_security_review_snooze_until.is_none() {
        return Ok(());
    }
    prefs.ssh_security_review_snooze_until = None;
    write_json(&user_prefs_path(username), &prefs)
}

/// Format a unix epoch as `dd/mm/yyyy` for panel UI (Norwegian numeric date).
pub fn format_epoch_dd_mm_yyyy(epoch: i64) -> String {
    // Civil date from epoch without pulling chrono: days since 1970-01-01.
    let days = epoch.div_euclid(86_400);
    let mut y = 1970i32;
    let mut rem = days;
    loop {
        let diy = if is_leap_year(y) { 366 } else { 365 };
        if rem >= diy {
            rem -= diy;
            y += 1;
        } else {
            break;
        }
    }
    let month_lens: [i64; 12] = if is_leap_year(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 1u32;
    for len in month_lens {
        if rem >= len {
            rem -= len;
            m += 1;
        } else {
            break;
        }
    }
    let d = (rem + 1) as u32;
    format!("{d:02}/{m:02}/{y}")
}

fn is_leap_year(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
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

    #[test]
    fn ssh_security_review_snooze_persists_and_clears() {
        with_test_data_dir(|| {
            assert!(!is_ssh_security_review_snoozed("Admin"));
            let until = snooze_ssh_security_review("Admin", 30).unwrap();
            assert!(until > now_epoch_secs());
            assert!(is_ssh_security_review_snoozed("Admin"));
            assert_eq!(ssh_security_review_snooze_until("Admin"), Some(until));
            // Other prefs survive.
            save_user_color_mode("Admin", ColorMode::Dark).unwrap();
            assert!(is_ssh_security_review_snoozed("Admin"));
            assert_eq!(load_user_color_mode("Admin"), ColorMode::Dark);
            clear_ssh_security_review_snooze("Admin").unwrap();
            assert!(!is_ssh_security_review_snoozed("Admin"));
            assert_eq!(load_user_color_mode("Admin"), ColorMode::Dark);
        });
    }

    #[test]
    fn ssh_security_review_snooze_clamps_to_max_days() {
        with_test_data_dir(|| {
            let until = snooze_ssh_security_review("Admin", 999).unwrap();
            let max_until =
                now_epoch_secs().saturating_add(i64::from(SSH_SECURITY_REVIEW_SNOOZE_MAX_DAYS) * 86_400);
            // Allow a few seconds of clock skew between calls.
            assert!(until <= max_until + 5);
            assert!(until >= max_until - 5);
        });
    }

    #[test]
    fn expired_snooze_is_not_active() {
        with_test_data_dir(|| {
            let mut prefs = load_user_ui_prefs("Admin");
            prefs.ssh_security_review_snooze_until = Some(now_epoch_secs() - 10);
            write_json(&user_prefs_path("Admin"), &prefs).unwrap();
            assert!(!is_ssh_security_review_snoozed("Admin"));
            assert_eq!(ssh_security_review_snooze_until("Admin"), None);
        });
    }

    #[test]
    fn epoch_formats_as_dd_mm_yyyy() {
        assert_eq!(format_epoch_dd_mm_yyyy(0), "01/01/1970");
        // 2026-03-19 00:00:00 UTC
        assert_eq!(format_epoch_dd_mm_yyyy(1_773_878_400), "19/03/2026");
    }
}
