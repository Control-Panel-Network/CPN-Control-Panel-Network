//! Pre-map SnappyMail-family system folders (settings_local) so send is not blocked.

use crate::install_snappymail_lineage::{chown_data_tree, lineage_data_dirs};
use crate::install_webmail_runtime::SNAPPYMAIL_DATA_DIR;
use serde_json::Value;
use std::path::Path;

const MARKER_FOLDERS: &str = "_data_/_default_/.cpn-system-folders-v1";

/// Default IMAP folder names CPN creates (see `panel_ops_mailbox_folders`).
const DEFAULT_SENT: &str = "Sent";
const DEFAULT_DRAFTS: &str = "Drafts";
const DEFAULT_JUNK: &str = "Junk";
const DEFAULT_TRASH: &str = "Trash";
const DEFAULT_ARCHIVE: &str = "Archive";

/// Ensure IMAP folders exist on disk and webmail `settings_local` mappings are filled.
pub fn ensure_snappymail_system_folders() -> Result<(), String> {
    let _ = crate::panel_ops_mailbox_folders::ensure_dovecot_system_mailboxes_conf();
    let _ = crate::panel_ops_mailbox_folders::heal_all_maildir_system_folders();
    for data_dir in lineage_data_dirs() {
        let _ = ensure_settings_local_mappings_in(&data_dir);
    }
    Ok(())
}

/// Seed `settings_local` for one mailbox address (create / password / heal).
pub fn seed_settings_local_for_address(address: &str) -> Result<(), String> {
    let address = address.trim().to_ascii_lowercase();
    let Some((local, domain)) = address.split_once('@') else {
        return Ok(());
    };
    if local.is_empty() || domain.is_empty() {
        return Ok(());
    }
    for data_dir in lineage_data_dirs() {
        let dir = Path::new(&data_dir)
            .join("_data_/_default_/storage")
            .join(domain)
            .join(local);
        // Only create under trees that already exist (or SnappyMail default always).
        if data_dir != SNAPPYMAIL_DATA_DIR && !Path::new(&data_dir).is_dir() {
            continue;
        }
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("settings_local");
        patch_settings_local_file(&path, true)?;
        let _ = chown_data_tree(&data_dir);
    }
    let _ = crate::panel_ops_mailbox_folders::ensure_imap_system_folders(local);
    Ok(())
}

fn ensure_settings_local_mappings_in(data_dir: &str) -> Result<(), String> {
    let storage = Path::new(data_dir).join("_data_/_default_/storage");
    if !storage.is_dir() {
        return Ok(());
    }
    let marker = Path::new(data_dir).join(MARKER_FOLDERS);
    let force_once = !marker.is_file();
    walk_settings_local(&storage, force_once)?;
    // Also seed settings_local next to any existing `settings` account dir.
    walk_settings_siblings(&storage, force_once)?;
    if force_once {
        if let Some(parent) = marker.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(
            &marker,
            "cpn snappymail system folders v1: Sent/Drafts/Junk/Trash/Archive mappings\n",
        );
    }
    let _ = chown_data_tree(data_dir);
    Ok(())
}

fn walk_settings_local(dir: &Path, force: bool) -> Result<(), String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_settings_local(&path, force)?;
            continue;
        }
        if path.file_name().and_then(|n| n.to_str()) != Some("settings_local") {
            continue;
        }
        patch_settings_local_file(&path, force)?;
    }
    Ok(())
}

fn walk_settings_siblings(dir: &Path, force: bool) -> Result<(), String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_settings_siblings(&path, force)?;
            continue;
        }
        if path.file_name().and_then(|n| n.to_str()) != Some("settings") {
            continue;
        }
        let local = path.with_file_name("settings_local");
        if !local.is_file() {
            patch_settings_local_file(&local, true)?;
        } else {
            patch_settings_local_file(&local, force)?;
        }
    }
    Ok(())
}

fn patch_settings_local_file(path: &Path, force: bool) -> Result<(), String> {
    let raw = if path.is_file() {
        std::fs::read_to_string(path).unwrap_or_else(|_| "{}".into())
    } else {
        "{}".into()
    };
    let mut data: Value = serde_json::from_str(&raw).unwrap_or_else(|_| serde_json::json!({}));
    let Some(obj) = data.as_object_mut() else {
        return Ok(());
    };
    let mut changed = false;
    // Only fill empty mappings (never overwrite a user-chosen folder name).
    let _ = force;
    changed |= set_folder(obj, "SentFolder", DEFAULT_SENT);
    changed |= set_folder(obj, "DraftsFolder", DEFAULT_DRAFTS);
    changed |= set_junk_folder(obj);
    changed |= set_folder(obj, "TrashFolder", DEFAULT_TRASH);
    changed |= set_folder(obj, "ArchiveFolder", DEFAULT_ARCHIVE);
    if changed || !path.is_file() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let pretty = serde_json::to_string(&data).map_err(|e| e.to_string())?;
        std::fs::write(path, pretty).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn set_folder(obj: &mut serde_json::Map<String, Value>, key: &str, default: &str) -> bool {
    let empty = match obj.get(key) {
        None => true,
        Some(Value::String(s)) => s.trim().is_empty(),
        Some(Value::Null) => true,
        _ => false,
    };
    if empty {
        obj.insert(key.into(), Value::String(default.into()));
        return true;
    }
    false
}

/// Prefer `Junk` (SPECIAL-USE); leave non-empty custom values (including Spam) alone.
fn set_junk_folder(obj: &mut serde_json::Map<String, Value>) -> bool {
    let current = obj
        .get("JunkFolder")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if !current.is_empty() {
        return false;
    }
    obj.insert("JunkFolder".into(), Value::String(DEFAULT_JUNK.into()));
    true
}

#[cfg(test)]
mod tests {
    use super::{set_folder, set_junk_folder};
    use serde_json::{Map, Value};

    #[test]
    fn fills_empty_sent() {
        let mut obj = Map::new();
        obj.insert("SentFolder".into(), Value::String("".into()));
        assert!(set_folder(&mut obj, "SentFolder", "Sent"));
        assert_eq!(obj.get("SentFolder").and_then(|v| v.as_str()), Some("Sent"));
    }

    #[test]
    fn preserves_custom_sent() {
        let mut obj = Map::new();
        obj.insert("SentFolder".into(), Value::String("Sent Items".into()));
        assert!(!set_folder(&mut obj, "SentFolder", "Sent"));
        assert_eq!(
            obj.get("SentFolder").and_then(|v| v.as_str()),
            Some("Sent Items")
        );
    }

    #[test]
    fn junk_defaults_to_junk() {
        let mut obj = Map::new();
        assert!(set_junk_folder(&mut obj));
        assert_eq!(obj.get("JunkFolder").and_then(|v| v.as_str()), Some("Junk"));
    }

    #[test]
    fn junk_preserves_spam_mapping() {
        let mut obj = Map::new();
        obj.insert("JunkFolder".into(), Value::String("Spam".into()));
        assert!(!set_junk_folder(&mut obj));
        assert_eq!(obj.get("JunkFolder").and_then(|v| v.as_str()), Some("Spam"));
    }
}
