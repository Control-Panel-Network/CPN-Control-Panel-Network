//! Heal SnappyMail-family notification i18n placeholders.
//!
//! Tachyon / SnappyMail `i18n(key, { COUNT: n })` substitutes the literal
//! `%COUNT%` token (case-sensitive). Upstream Norwegian `nb` shipped
//! `Du har %count% nye meldinger`, so browser push toasts showed the raw
//! placeholder. Normalize `NEW_MESSAGE_NOTIFICATION` across all locales.

use std::fs;
use std::path::{Path, PathBuf};

const WEBMAIL_ROOTS: &[&str] = &["/opt/cpn-webmail/tachyon", "/opt/cpn-webmail/snappymail"];

/// Walk installed Tachyon/SnappyMail trees and fix wrong-case `%count%`.
pub fn ensure_notification_count_placeholders() -> Result<usize, String> {
    let mut fixed = 0usize;
    for root in WEBMAIL_ROOTS {
        let root = Path::new(root);
        if !root.is_dir() {
            continue;
        }
        for path in localization_user_json_files(root) {
            match fix_new_message_notification_file(&path) {
                Ok(true) => fixed += 1,
                Ok(false) => {}
                Err(err) => {
                    eprintln!("cpn webmail i18n: skip {}: {err}", path.display());
                }
            }
        }
    }
    Ok(fixed)
}

fn localization_user_json_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk_collect(root, &mut out);
    out
}

fn walk_collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_collect(&path, out);
            continue;
        }
        if path.file_name().and_then(|n| n.to_str()) != Some("user.json") {
            continue;
        }
        let parent = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str());
        let grand = path
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str());
        // …/app/localization/<locale>/user.json
        if grand == Some("localization") && parent.is_some() {
            out.push(path);
        }
    }
}

fn fix_new_message_notification_file(path: &Path) -> Result<bool, String> {
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let Some(updated) = normalize_new_message_notification_json(&raw) else {
        return Ok(false);
    };
    if updated == raw {
        return Ok(false);
    }
    // Validate JSON before writing so a bad regex never corrupts the locale file.
    let _: serde_json::Value =
        serde_json::from_str(&updated).map_err(|e| format!("invalid json after normalize: {e}"))?;
    fs::write(path, updated).map_err(|e| e.to_string())?;
    Ok(true)
}

/// Returns `Some(new_text)` when the file contains `NEW_MESSAGE_NOTIFICATION`
/// (even if already correct); callers compare to detect a real change.
/// Returns `None` when the key is absent.
fn normalize_new_message_notification_json(raw: &str) -> Option<String> {
    let key = "\"NEW_MESSAGE_NOTIFICATION\"";
    let key_pos = raw.find(key)?;
    let after_key = &raw[key_pos + key.len()..];
    let colon = after_key.find(':')?;
    let after_colon = &after_key[colon + 1..];
    let value_start_rel = after_colon.find('"')?;
    let value_body_start = key_pos + key.len() + colon + 1 + value_start_rel + 1;
    let value_body = &raw[value_body_start..];
    let value_end_rel = find_json_string_end(value_body)?;
    let value = &value_body[..value_end_rel];
    let fixed = normalize_count_placeholder(value);
    if fixed == value {
        return Some(raw.to_string());
    }
    let mut out = String::with_capacity(raw.len() + 8);
    out.push_str(&raw[..value_body_start]);
    out.push_str(&fixed);
    out.push_str(&raw[value_body_start + value_end_rel..]);
    Some(out)
}

fn find_json_string_end(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return Some(i),
            _ => i += 1,
        }
    }
    None
}

fn normalize_count_placeholder(value: &str) -> String {
    // Any case of %count% becomes the exact token i18n substitutes.
    let mut out = String::with_capacity(value.len());
    let lower = value.to_ascii_lowercase();
    let needle = "%count%";
    let mut idx = 0usize;
    while let Some(rel) = lower[idx..].find(needle) {
        let at = idx + rel;
        out.push_str(&value[idx..at]);
        out.push_str("%COUNT%");
        idx = at + needle.len();
    }
    out.push_str(&value[idx..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_lowercase_nb_placeholder() {
        let raw = r#"{
  "MESSAGE_LIST": {
    "NEW_MESSAGE_NOTIFICATION": "Du har %count% nye meldinger"
  }
}"#;
        let updated = normalize_new_message_notification_json(raw).expect("key present");
        assert!(updated.contains("%COUNT%"));
        assert!(!updated.contains("%count%"));
        let v: serde_json::Value = serde_json::from_str(&updated).unwrap();
        assert_eq!(
            v["MESSAGE_LIST"]["NEW_MESSAGE_NOTIFICATION"],
            "Du har %COUNT% nye meldinger"
        );
    }

    #[test]
    fn leaves_correct_english_alone() {
        let raw =
            r#"{"MESSAGE_LIST":{"NEW_MESSAGE_NOTIFICATION":"You have %COUNT% new messages!"}}"#;
        let updated = normalize_new_message_notification_json(raw).unwrap();
        assert_eq!(updated, raw);
    }

    #[test]
    fn missing_key_returns_none() {
        assert!(normalize_new_message_notification_json(r#"{"a":1}"#).is_none());
    }

    #[test]
    fn normalize_count_helper_mixed_case() {
        assert_eq!(
            normalize_count_placeholder("x %Count% y %COUNT% z %count%"),
            "x %COUNT% y %COUNT% z %COUNT%"
        );
    }

    #[test]
    fn i18n_replace_simulation_matches_app_js() {
        // Mirrors app.js: result.replace('%' + key + '%', value)
        fn i18n_replace(template: &str, key: &str, value: &str) -> String {
            template.replace(&format!("%{key}%"), value)
        }
        assert_eq!(
            i18n_replace("Du har %count% nye meldinger", "COUNT", "3"),
            "Du har %count% nye meldinger"
        );
        assert_eq!(
            i18n_replace("Du har %COUNT% nye meldinger", "COUNT", "3"),
            "Du har 3 nye meldinger"
        );
    }
}
