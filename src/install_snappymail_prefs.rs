//! SnappyMail-family operator defaults: Markdown, AllowStyles, Sieve, branding, login, contacts.

use crate::install_snappymail_lineage::{
    self, application_ini_path, chown_data_tree, lineage_data_dirs, lineage_docroots,
    replace_ini_bool, replace_ini_quoted,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

const MARKER_DEFAULTS: &str = "_data_/_default_/.cpn-user-defaults-v1";

const CPN_PAGE_TITLE: &str = "CPN Webmail";
const CPN_LOADING_DESCRIPTION: &str = "CPN Panel";
const CPN_FAVICON_URL: &str = "/favicon.ico";

/// Apply IMAP/SMTP/Sieve domain defaults, Markdown + AllowStyles, branding, login, contacts.
///
/// Runs for every installed SnappyMail-family data root (SnappyMail, Tachyon, NextSnapMail).
pub fn ensure_snappymail_operator_defaults() -> Result<(), String> {
    let _ = ensure_actions_php_defaults();
    for data_dir in lineage_data_dirs() {
        let _ = ensure_domain_sieve_enabled(&data_dir);
        let _ = ensure_user_settings_defaults(&data_dir);
        let _ = ensure_login_and_branding_defaults(&data_dir);
        let _ = crate::install_snappymail_contacts::ensure_contacts_defaults(&data_dir);
        let _ = chown_data_tree(&data_dir);
    }
    let _ = crate::install_snappymail_folders::ensure_snappymail_system_folders();
    Ok(())
}

/// Set SnappyMail-family `/?admin` password (all installed data roots with application.ini).
pub fn sync_snappymail_admin_password(password: &str) -> Result<(), String> {
    let synced = install_snappymail_lineage::sync_lineage_admin_password(password)?;
    if synced == 0 {
        // No application.ini yet (first visit pending). Not a hard failure.
        return Ok(());
    }
    Ok(())
}

fn ensure_login_and_branding_defaults(data_dir: &str) -> Result<(), String> {
    let ini = application_ini_path(data_dir);
    if !ini.is_file() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(&ini).map_err(|e| e.to_string())?;
    let mut updated = raw.clone();
    updated = replace_ini_bool(&updated, "determine_user_domain", true);
    updated = replace_ini_bool(&updated, "allow_languages_on_login", true);
    updated = replace_ini_bool(&updated, "determine_user_language", true);
    updated = replace_ini_quoted(&updated, "title", CPN_PAGE_TITLE);
    updated = replace_ini_quoted(&updated, "loading_description", CPN_LOADING_DESCRIPTION);
    updated = replace_ini_quoted(&updated, "favicon_url", CPN_FAVICON_URL);
    if updated != raw {
        std::fs::write(&ini, updated).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn ensure_domain_sieve_enabled(data_dir: &str) -> Result<(), String> {
    let domains = Path::new(data_dir).join("_data_/_default_/domains");
    if !domains.is_dir() {
        return Ok(());
    }
    let entries: Vec<PathBuf> = std::fs::read_dir(&domains)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().and_then(|e| e.to_str()) == Some("json")
                && !p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .eq_ignore_ascii_case("gmail.com.json")
                && !p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .eq_ignore_ascii_case("hotmail.com.json")
                && !p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .eq_ignore_ascii_case("yahoo.com.json")
        })
        .collect();
    for path in entries {
        let raw = match std::fs::read_to_string(&path) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let Ok(mut data) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let sieve = data.as_object_mut().map(|o| {
            o.entry("Sieve".to_string())
                .or_insert_with(|| Value::Object(Default::default()))
        });
        if let Some(Value::Object(obj)) = sieve {
            obj.insert("enabled".into(), Value::Bool(true));
            obj.insert("host".into(), Value::String("127.0.0.1".into()));
            obj.insert("port".into(), serde_json::json!(4190));
            obj.insert("shortLogin".into(), Value::Bool(true));
            obj.insert("lowerLogin".into(), Value::Bool(true));
            obj.insert("sasl".into(), serde_json::json!(["PLAIN", "LOGIN"]));
        }
        if let Ok(pretty) = serde_json::to_string_pretty(&data) {
            let _ = std::fs::write(&path, format!("{pretty}\n"));
        }
    }
    Ok(())
}

/// Patch installed Actions.php so new sessions default markdown + AllowStyles On.
fn ensure_actions_php_defaults() -> Result<(), String> {
    for root in lineage_docroots() {
        let find_cmd = format!(
            "find {} -path '*/libraries/RainLoop/Actions.php' -type f 2>/dev/null",
            shell_single_quote(&root)
        );
        let output = Command::new("bash")
            .args(["-c", &find_cmd])
            .output()
            .map_err(|e| e.to_string())?;
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let path = Path::new(line.trim());
            if !path.is_file() {
                continue;
            }
            let Ok(raw) = std::fs::read_to_string(path) else {
                continue;
            };
            let updated = raw
                .replace("'AllowStyles' => false,", "'AllowStyles' => true,")
                .replace("'markdown' => false,", "'markdown' => true,");
            if updated != raw {
                let _ = std::fs::write(path, updated);
            }
        }
    }
    Ok(())
}

fn shell_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn ensure_user_settings_defaults(data_dir: &str) -> Result<(), String> {
    let storage = Path::new(data_dir).join("_data_/_default_/storage");
    if !storage.is_dir() {
        return Ok(());
    }
    let marker = Path::new(data_dir).join(MARKER_DEFAULTS);
    let force_once = !marker.is_file();
    walk_settings(&storage, force_once)?;
    if force_once {
        if let Some(parent) = marker.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(
            &marker,
            "cpn snappymail-family user defaults v1: markdown + AllowStyles\n",
        );
    }
    Ok(())
}

fn walk_settings(dir: &Path, force: bool) -> Result<(), String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_settings(&path, force)?;
            continue;
        }
        if path.file_name().and_then(|n| n.to_str()) != Some("settings") {
            continue;
        }
        patch_settings_file(&path, force)?;
    }
    Ok(())
}

fn patch_settings_file(path: &Path, force: bool) -> Result<(), String> {
    let raw = std::fs::read_to_string(path).unwrap_or_else(|_| "{}".into());
    let mut data: Value = serde_json::from_str(&raw).unwrap_or_else(|_| serde_json::json!({}));
    let Some(obj) = data.as_object_mut() else {
        return Ok(());
    };
    let mut changed = false;
    if force || !obj.contains_key("markdown") {
        obj.insert("markdown".into(), Value::Bool(true));
        changed = true;
    }
    if force || !obj.contains_key("AllowStyles") {
        obj.insert("AllowStyles".into(), Value::Bool(true));
        changed = true;
    }
    if changed {
        let pretty = serde_json::to_string(&data).map_err(|e| e.to_string())?;
        std::fs::write(path, pretty).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::install_snappymail_lineage::replace_ini_bool;

    #[test]
    fn sets_determine_user_domain_on() {
        let raw = "[login]\ndetermine_user_domain = Off\n";
        let out = replace_ini_bool(raw, "determine_user_domain", true);
        assert!(out.contains("determine_user_domain = On"));
    }
}
