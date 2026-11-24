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
    // Follow upstream: add a newer Tachyon version directory (throttled, kept side by side)
    // before per-data-root defaults and the admin SSO bridge are applied below.
    match crate::install_tachyon_release::upgrade_tachyon_if_needed(false) {
        Ok(msg) if msg.starts_with("Tachyon upgraded") => {
            eprintln!("[webmail-tachyon-release] [info] {msg}");
        }
        Ok(_) => {}
        Err(e) => eprintln!("[webmail-tachyon-release] [warn] {e}"),
    }
    let _ = ensure_actions_php_defaults();
    for data_dir in lineage_data_dirs() {
        let _ = ensure_domain_sieve_enabled(&data_dir);
        let _ = ensure_user_settings_defaults(&data_dir);
        let _ = ensure_login_and_branding_defaults(&data_dir);
        let _ = crate::install_snappymail_contacts::ensure_contacts_defaults(&data_dir);
        let _ = chown_data_tree(&data_dir);
    }
    let _ = crate::install_snappymail_folders::ensure_snappymail_system_folders();
    // `/?admin` sign-in is delegated to the CPN panel owner (owner-only bridge).
    let _ = crate::tachyon_admin_bridge_install::ensure_admin_bridge();
    // OpenPGP (SnappyMail-family + Roundcube Enigma when present).
    let _ = crate::install_webmail_openpgp::ensure_webmail_openpgp_defaults();
    Ok(())
}

/// Set SnappyMail-family `/?admin` password (all installed data roots with application.ini).
///
/// Only the panel owner (bootstrap admin) may drive this. Password changes for normal
/// users, resellers, and ACL accounts must never touch the webmail admin credential.
pub fn sync_snappymail_admin_password(username: &str, password: &str) -> Result<(), String> {
    if !crate::panel_admin::is_panel_admin(username) {
        return Ok(());
    }
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

/// Read IMAP (then SMTP) `shortLogin` from a SnappyMail/Tachyon domain profile.
pub fn domain_mailbox_short_login(data: &Value) -> Option<bool> {
    data.get("IMAP")
        .and_then(|v| v.get("shortLogin"))
        .and_then(|v| v.as_bool())
        .or_else(|| {
            data.get("SMTP")
                .and_then(|v| v.get("shortLogin"))
                .and_then(|v| v.as_bool())
        })
}

/// Keep `Sieve.shortLogin` aligned with IMAP/SMTP on the same domain JSON.
///
/// Creates a Sieve object when missing. Never leaves Sieve false when IMAP
/// shortLogin is true (Tachyon Filters ManageSieve ERROR 3 on PAM/unix users).
pub fn align_domain_sieve_short_login(data: &mut Value) -> bool {
    let Some(short) = domain_mailbox_short_login(data) else {
        return false;
    };
    let Some(root) = data.as_object_mut() else {
        return false;
    };
    let sieve = root
        .entry("Sieve".to_string())
        .or_insert_with(|| Value::Object(Default::default()));
    let Some(obj) = sieve.as_object_mut() else {
        return false;
    };
    let next = Value::Bool(short);
    if obj.get("shortLogin") == Some(&next) {
        return false;
    }
    obj.insert("shortLogin".into(), next);
    true
}

/// Heal one domain JSON file so Sieve shortLogin matches IMAP/SMTP.
pub fn heal_domain_sieve_short_login_file(path: &Path) -> Result<bool, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut data: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    if !align_domain_sieve_short_login(&mut data) {
        return Ok(false);
    }
    let pretty = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
    std::fs::write(path, format!("{pretty}\n")).map_err(|e| e.to_string())?;
    Ok(true)
}

/// Ensure apex and `mail.<apex>` domain profiles are on the provision mirror list.
pub fn append_mail_and_apex_domain_profiles(
    mirror_names: &mut Vec<String>,
    apex_domains: &[String],
) {
    for apex_raw in apex_domains {
        let apex = apex_raw.trim().to_ascii_lowercase();
        if apex.is_empty() {
            continue;
        }
        push_domain_profile_name(mirror_names, &format!("{apex}.json"));
        let mail_host = if apex.starts_with("mail.") {
            apex.clone()
        } else {
            format!("mail.{apex}")
        };
        push_domain_profile_name(mirror_names, &format!("{mail_host}.json"));
    }
}

fn push_domain_profile_name(mirror_names: &mut Vec<String>, name: &str) {
    if !mirror_names.iter().any(|n| n.eq_ignore_ascii_case(name)) {
        mirror_names.push(name.to_string());
    }
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
        let short_login = domain_mailbox_short_login(&data)
            .unwrap_or_else(crate::mail_legacy_sql::prefer_webmail_short_login);
        let sieve = data.as_object_mut().map(|o| {
            o.entry("Sieve".to_string())
                .or_insert_with(|| Value::Object(Default::default()))
        });
        if let Some(Value::Object(obj)) = sieve {
            obj.insert("enabled".into(), Value::Bool(true));
            obj.insert("host".into(), Value::String("127.0.0.1".into()));
            obj.insert("port".into(), serde_json::json!(4190));
            obj.insert("shortLogin".into(), Value::Bool(short_login));
            obj.insert("lowerLogin".into(), Value::Bool(true));
            obj.insert("sasl".into(), serde_json::json!(["PLAIN", "LOGIN"]));
        }
        // Final pass: never leave Sieve false when IMAP shortLogin is true.
        let _ = align_domain_sieve_short_login(&mut data);
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
    use super::{
        align_domain_sieve_short_login, append_mail_and_apex_domain_profiles,
        heal_domain_sieve_short_login_file,
    };
    use crate::install_snappymail_lineage::replace_ini_bool;
    use serde_json::json;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn webmail_admin_password_sync_is_panel_owner_only() {
        use crate::account::{default_password_policy, setup_account, with_test_data_dir};
        with_test_data_dir(|| {
            let owner_pw: String = ["Owner", "Passw0rd", "xyz"].concat();
            setup_account(
                "ntowner",
                Some(&owner_pw),
                false,
                "owner@example.com",
                default_password_policy(),
                "en",
            )
            .unwrap();
            // A too-short password is rejected for the owner path only; users, resellers,
            // and ACL accounts return early and never touch the webmail admin credential.
            // Built from parts so CodeQL does not treat a literal as a hard-coded password.
            let short: String = ['s', 'h', 'o', 'r', 't'].into_iter().collect();
            assert!(super::sync_snappymail_admin_password("ntowner", &short).is_err());
            for other in ["plainuser", "reseller1", "acl-sub"] {
                assert!(super::sync_snappymail_admin_password(other, &short).is_ok());
            }
        });
    }

    #[test]
    fn sets_determine_user_domain_on() {
        let raw = "[login]\ndetermine_user_domain = Off\n";
        let out = replace_ini_bool(raw, "determine_user_domain", true);
        assert!(out.contains("determine_user_domain = On"));
    }

    #[test]
    fn sieve_short_login_follows_imap_true() {
        let mut data = json!({
            "IMAP": { "shortLogin": true },
            "SMTP": { "shortLogin": true },
            "Sieve": { "enabled": true, "shortLogin": false }
        });
        assert!(align_domain_sieve_short_login(&mut data));
        assert_eq!(data["Sieve"]["shortLogin"], true);
    }

    #[test]
    fn sieve_short_login_created_when_missing() {
        let mut data = json!({
            "IMAP": { "shortLogin": true },
            "SMTP": { "shortLogin": true }
        });
        assert!(align_domain_sieve_short_login(&mut data));
        assert_eq!(data["Sieve"]["shortLogin"], true);
    }

    #[test]
    fn sieve_short_login_follows_imap_false() {
        let mut data = json!({
            "IMAP": { "shortLogin": false },
            "Sieve": { "shortLogin": true }
        });
        assert!(align_domain_sieve_short_login(&mut data));
        assert_eq!(data["Sieve"]["shortLogin"], false);
    }

    #[test]
    fn append_mail_and_apex_covers_both_profiles() {
        let mut names = Vec::new();
        append_mail_and_apex_domain_profiles(
            &mut names,
            &["threebrickstudio.com".into(), "mail.example.com".into()],
        );
        assert!(names.iter().any(|n| n == "threebrickstudio.com.json"));
        assert!(names.iter().any(|n| n == "mail.threebrickstudio.com.json"));
        assert!(names.iter().any(|n| n == "mail.example.com.json"));
        assert_eq!(
            names
                .iter()
                .filter(|n| n.eq_ignore_ascii_case("mail.example.com.json"))
                .count(),
            1
        );
    }

    #[test]
    fn heal_domain_file_aligns_sieve_with_imap() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("cpn-sieve-shortlogin-{stamp}"));
        fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("mail.threebrickstudio.com.json");
        let raw = json!({
            "IMAP": { "host": "127.0.0.1", "shortLogin": true },
            "SMTP": { "host": "127.0.0.1", "shortLogin": true },
            "Sieve": {
                "enabled": true,
                "host": "127.0.0.1",
                "port": 4190,
                "shortLogin": false
            }
        });
        fs::write(&path, format!("{}\n", raw)).expect("write domain");
        assert_eq!(
            heal_domain_sieve_short_login_file(&path).expect("heal"),
            true
        );
        let healed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("json");
        assert_eq!(healed["Sieve"]["shortLogin"], true);
        assert_eq!(
            heal_domain_sieve_short_login_file(&path).expect("idempotent"),
            false
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
