//! Pre-map SnappyMail-family system folders (settings_local) so send is not blocked.
//!
//! Folder values must match IMAP LIST full names. On classic CyberPanel Dovecot that means
//! `INBOX.Sent` (not bare `Sent`). Empty mappings are filled; CPN bare defaults are migrated
//! to the prefixed form when a namespace prefix is detected. Other custom values are kept.

use crate::install_snappymail_lineage::{chown_data_tree, lineage_data_dirs};
use crate::install_webmail_runtime::SNAPPYMAIL_DATA_DIR;
use crate::panel_ops_mailbox_folders::{imap_namespace_prefix, imap_system_folder_name};
use serde_json::Value;
use std::path::Path;

const MARKER_FOLDERS: &str = "_data_/_default_/.cpn-system-folders-v2";
const MARKER_FOLDERS_V1: &str = "_data_/_default_/.cpn-system-folders-v1";

const DEFAULT_SENT: &str = "Sent";
const DEFAULT_DRAFTS: &str = "Drafts";
const DEFAULT_JUNK: &str = "Junk";
const DEFAULT_TRASH: &str = "Trash";
const DEFAULT_ARCHIVE: &str = "Archive";

/// Ensure IMAP folders exist on disk and webmail `settings_local` mappings are filled.
pub fn ensure_snappymail_system_folders() -> Result<(), String> {
    let _ = crate::panel_ops_mailbox_folders::ensure_dovecot_system_mailboxes_conf();
    let _ = crate::panel_ops_mailbox_folders::heal_all_maildir_system_folders();
    seed_known_mailbox_addresses();
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
        if data_dir != SNAPPYMAIL_DATA_DIR && !Path::new(&data_dir).is_dir() {
            continue;
        }
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("settings_local");
        patch_settings_local_file(&path)?;
        let _ = chown_data_tree(&data_dir);
    }
    let _ = crate::panel_ops_mailbox_folders::ensure_imap_system_folders_for_email(&address);
    Ok(())
}

fn seed_known_mailbox_addresses() {
    for row in crate::mail_legacy_sql::list_legacy_mailboxes() {
        let _ = seed_settings_local_for_address(&row.email);
    }
    for acct in crate::mail_accounts::list_accounts() {
        let _ = seed_settings_local_for_address(&acct.address);
    }
}

fn ensure_settings_local_mappings_in(data_dir: &str) -> Result<(), String> {
    let storage = Path::new(data_dir).join("_data_/_default_/storage");
    if !storage.is_dir() {
        return Ok(());
    }
    let marker = Path::new(data_dir).join(MARKER_FOLDERS);
    let need_marker = !marker.is_file();
    walk_settings_local(&storage)?;
    walk_settings_siblings(&storage)?;
    if need_marker {
        if let Some(parent) = marker.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let prefix = imap_namespace_prefix();
        let note = if prefix.is_empty() {
            "cpn snappymail system folders v2: Sent/Drafts/Junk/Trash/Archive\n".to_string()
        } else {
            format!(
                "cpn snappymail system folders v2: {prefix}Sent/{prefix}Drafts/{prefix}Junk/{prefix}Trash/{prefix}Archive\n"
            )
        };
        let _ = std::fs::write(&marker, note);
        // Keep v1 marker so older heals do not re-force unrelated paths.
        let v1 = Path::new(data_dir).join(MARKER_FOLDERS_V1);
        if !v1.is_file() {
            let _ = std::fs::write(&v1, "superseded by .cpn-system-folders-v2\n");
        }
    }
    let _ = chown_data_tree(data_dir);
    Ok(())
}

fn walk_settings_local(dir: &Path) -> Result<(), String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_settings_local(&path)?;
            continue;
        }
        if path.file_name().and_then(|n| n.to_str()) != Some("settings_local") {
            continue;
        }
        patch_settings_local_file(&path)?;
    }
    Ok(())
}

fn walk_settings_siblings(dir: &Path) -> Result<(), String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_settings_siblings(&path)?;
            continue;
        }
        if path.file_name().and_then(|n| n.to_str()) != Some("settings") {
            continue;
        }
        let local = path.with_file_name("settings_local");
        patch_settings_local_file(&local)?;
    }
    Ok(())
}

fn patch_settings_local_file(path: &Path) -> Result<(), String> {
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

/// Fill empty values, or migrate CPN bare defaults to the IMAP-prefixed name.
fn set_folder(obj: &mut serde_json::Map<String, Value>, key: &str, bare: &str) -> bool {
    let desired = imap_system_folder_name(bare);
    let current = obj
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if current.is_empty() {
        obj.insert(key.into(), Value::String(desired));
        return true;
    }
    if current == desired {
        return false;
    }
    // Migrate only exact CPN bare defaults (never "Sent Items" or other customs).
    if current == bare && desired != bare {
        obj.insert(key.into(), Value::String(desired));
        return true;
    }
    false
}

/// Prefer prefixed `Junk`. Migrate bare `Junk` when a namespace prefix is set.
/// Leave other non-empty values (including Spam / INBOX.Spam) alone.
fn set_junk_folder(obj: &mut serde_json::Map<String, Value>) -> bool {
    let desired = imap_system_folder_name(DEFAULT_JUNK);
    let current = obj
        .get("JunkFolder")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if current.is_empty() {
        obj.insert("JunkFolder".into(), Value::String(desired));
        return true;
    }
    if current == desired {
        return false;
    }
    if current == DEFAULT_JUNK && desired != DEFAULT_JUNK {
        obj.insert("JunkFolder".into(), Value::String(desired));
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_JUNK, set_folder, set_junk_folder};
    use serde_json::{Map, Value};

    #[test]
    fn fills_empty_sent_with_runtime_name() {
        let mut obj = Map::new();
        obj.insert("SentFolder".into(), Value::String("".into()));
        assert!(set_folder(&mut obj, "SentFolder", "Sent"));
        let got = obj.get("SentFolder").and_then(|v| v.as_str()).unwrap_or("");
        assert!(got.ends_with("Sent"));
        assert!(!got.is_empty());
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
    fn junk_defaults_when_empty() {
        let mut obj = Map::new();
        assert!(set_junk_folder(&mut obj));
        let got = obj.get("JunkFolder").and_then(|v| v.as_str()).unwrap_or("");
        assert!(got.ends_with(DEFAULT_JUNK));
    }

    #[test]
    fn junk_preserves_spam_mapping() {
        let mut obj = Map::new();
        obj.insert("JunkFolder".into(), Value::String("Spam".into()));
        assert!(!set_junk_folder(&mut obj));
        assert_eq!(obj.get("JunkFolder").and_then(|v| v.as_str()), Some("Spam"));
    }

    #[test]
    fn junk_preserves_inbox_spam() {
        let mut obj = Map::new();
        obj.insert("JunkFolder".into(), Value::String("INBOX.Spam".into()));
        assert!(!set_junk_folder(&mut obj));
        assert_eq!(
            obj.get("JunkFolder").and_then(|v| v.as_str()),
            Some("INBOX.Spam")
        );
    }
}
