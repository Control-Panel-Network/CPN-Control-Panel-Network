//! SnappyMail-family operator defaults: Markdown, AllowStyles, Sieve, branding, login, contacts.

use crate::install_snappymail_lineage::{
    self, application_ini_path, chown_data_tree, lineage_data_dirs, lineage_docroots,
    replace_ini_bool, replace_ini_quoted,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

const MARKER_DEFAULTS: &str = "_data_/_default_/.cpn-user-defaults-v1";
const MARKER_CONTACTS_DB: &str = "_data_/_default_/.cpn-contacts-db-v1";

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
        let _ = ensure_contacts_defaults(&data_dir);
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

fn ensure_contacts_defaults(data_dir: &str) -> Result<(), String> {
    let ini = application_ini_path(data_dir);
    if !ini.is_file() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(&ini).map_err(|e| e.to_string())?;
    let updated = patch_contacts_section(&raw, data_dir);
    if updated != raw {
        std::fs::write(&ini, updated).map_err(|e| e.to_string())?;
    }
    ensure_contacts_sqlite_ready(data_dir)?;
    Ok(())
}

fn patch_contacts_section(raw: &str, data_dir: &str) -> String {
    let Some(start) = raw.find("[contacts]") else {
        return raw.to_string();
    };
    let after = &raw[start + "[contacts]".len()..];
    let end_rel = after.find("\n[").map(|i| i + 1).unwrap_or(after.len());
    let section_body = &after[..end_rel];
    let rest = &after[end_rel..];
    let mut body = section_body.to_string();
    body = replace_ini_bool(&body, "enable", true);
    body = replace_ini_quoted(&body, "type", "sqlite");
    let use_global = !per_user_addressbooks_exist(data_dir);
    body = replace_ini_bool(&body, "sqlite_global", use_global);
    format!("{}[contacts]{}{}", &raw[..start], body, rest)
}

fn per_user_addressbooks_exist(data_dir: &str) -> bool {
    let storage = Path::new(data_dir).join("_data_/_default_/storage");
    if !storage.is_dir() {
        return false;
    }
    find_addressbook_sqlite(&storage)
}

fn find_addressbook_sqlite(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if find_addressbook_sqlite(&path) {
                return true;
            }
            continue;
        }
        if path.file_name().and_then(|n| n.to_str()) == Some("AddressBook.sqlite") {
            return true;
        }
    }
    false
}

fn ensure_contacts_sqlite_ready(data_dir: &str) -> Result<(), String> {
    let check = Command::new("php")
        .args(["-r", "exit(extension_loaded('pdo_sqlite') ? 0 : 1);"])
        .status()
        .map_err(|e| format!("php pdo_sqlite check failed: {e}"))?;
    if !check.success() {
        return Err("PHP pdo_sqlite extension required for webmail Contacts".into());
    }

    let db = Path::new(data_dir).join("_data_/_default_/AddressBook.sqlite");
    if let Some(parent) = db.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let output = Command::new("php")
        .args([
            "-r",
            r#"
$dbPath = getenv('CPN_SNAPPY_AB_DB');
if (!$dbPath) { fwrite(STDERR, "missing CPN_SNAPPY_AB_DB\n"); exit(1); }
$pdo = new PDO('sqlite:' . $dbPath);
$pdo->setAttribute(PDO::ATTR_ERRMODE, PDO::ERRMODE_EXCEPTION);
$pdo->exec("CREATE TABLE IF NOT EXISTS rainloop_system (sys_name text NOT NULL, value_int integer NOT NULL DEFAULT 0)");
$pdo->exec("CREATE UNIQUE INDEX IF NOT EXISTS ui_rainloop_system_sys_name ON rainloop_system (sys_name)");
$pdo->exec("CREATE TABLE IF NOT EXISTS rainloop_users (id_user integer NOT NULL PRIMARY KEY, rl_email text NOT NULL DEFAULT '')");
$pdo->exec("CREATE INDEX IF NOT EXISTS rl_email_rainloop_users_index ON rainloop_users (rl_email)");
$pdo->exec("CREATE TABLE IF NOT EXISTS rainloop_ab_contacts (id_contact integer NOT NULL PRIMARY KEY, id_contact_str text NOT NULL DEFAULT '', id_user integer NOT NULL, display text NOT NULL DEFAULT '', changed integer NOT NULL DEFAULT 0, deleted integer NOT NULL DEFAULT 0, etag text NOT NULL DEFAULT '')");
$pdo->exec("CREATE INDEX IF NOT EXISTS id_user_rainloop_ab_contacts_index ON rainloop_ab_contacts (id_user)");
$pdo->exec("CREATE TABLE IF NOT EXISTS rainloop_ab_properties (id_prop integer NOT NULL PRIMARY KEY, id_contact integer NOT NULL, id_user integer NOT NULL, prop_type integer NOT NULL, prop_type_str text NOT NULL DEFAULT '', prop_value text NOT NULL DEFAULT '', prop_value_custom text NOT NULL DEFAULT '', prop_frec integer NOT NULL DEFAULT 0)");
$pdo->exec("CREATE INDEX IF NOT EXISTS id_user_rainloop_ab_properties_index ON rainloop_ab_properties (id_user)");
$pdo->exec("CREATE INDEX IF NOT EXISTS id_user_id_contact_rainloop_ab_properties_index ON rainloop_ab_properties (id_user, id_contact)");
$haveLower = false;
foreach ($pdo->query("PRAGMA table_info(rainloop_ab_properties)") as $c) {
  if (($c['name'] ?? '') === 'prop_value_lower') { $haveLower = true; break; }
}
if (!$haveLower) {
  $pdo->exec("ALTER TABLE rainloop_ab_properties ADD COLUMN prop_value_lower text NOT NULL DEFAULT ''");
}
$st = $pdo->prepare("INSERT OR REPLACE INTO rainloop_system (sys_name, value_int) VALUES (?, ?)");
$st->execute(["sqlite-ab-version", 2]);
echo "ok";
"#,
        ])
        .env("CPN_SNAPPY_AB_DB", db.to_string_lossy().as_ref())
        .output()
        .map_err(|e| format!("contacts sqlite init failed to start: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "contacts sqlite init failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let marker = Path::new(data_dir).join(MARKER_CONTACTS_DB);
    let _ = std::fs::write(
        &marker,
        "cpn snappymail-family contacts sqlite v1: enable + AddressBook schema\n",
    );
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
    use super::patch_contacts_section;
    use crate::install_snappymail_lineage::replace_ini_bool;

    #[test]
    fn sets_determine_user_domain_on() {
        let raw = "[login]\ndetermine_user_domain = Off\n";
        let out = replace_ini_bool(raw, "determine_user_domain", true);
        assert!(out.contains("determine_user_domain = On"));
    }

    #[test]
    fn patches_contacts_enable_and_type() {
        let raw = r#"[webmail]
title = "SnappyMail Webmail"

[contacts]
; Enable contacts
enable = Off
type = "mysql"
sqlite_global = Off

[security]
admin_login = "admin"
"#;
        let out = patch_contacts_section(raw, "/tmp/cpn-missing-webmail-data");
        assert!(out.contains("[contacts]"));
        assert!(out.contains("enable = On"));
        assert!(out.contains("type = \"sqlite\""));
        assert!(out.contains("title = \"SnappyMail Webmail\""));
        assert!(out.contains("admin_login = \"admin\""));
    }
}
