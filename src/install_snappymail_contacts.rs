//! SnappyMail-family Contacts storage on local MariaDB (PDO type "MySQL" in the admin UI).
//!
//! Secrets live under `/var/lib/cpn/webmail-contacts/` (mode 600). Passwords are never
//! committed to the repo. The SnappyMail UI labels the driver "MySQL"; the server is MariaDB.

use crate::install_snappymail_lineage::{
    application_ini_path, client_id_for_data_dir, replace_ini_bool, replace_ini_quoted,
};
use crate::panel_ops_db::{create_database_with_user_tcp, local_mariadb_ready};
use rand::{Rng, distr::Alphanumeric};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const SECRETS_DIR: &str = "/var/lib/cpn/webmail-contacts";
const MARKER_MARIADB: &str = "_data_/_default_/.cpn-contacts-mariadb-v1";
const MARKER_SQLITE: &str = "_data_/_default_/.cpn-contacts-db-v1";
const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ContactsDbSecret {
    schema_version: u32,
    client_id: String,
    db_name: String,
    db_user: String,
    db_password: String,
    pdo_dsn: String,
}

/// Enable Contacts and provision MariaDB PDO storage for one lineage data root.
///
/// Falls back to SQLite only when local MariaDB is unavailable so heal stays non-fatal.
pub fn ensure_contacts_defaults(data_dir: &str) -> Result<(), String> {
    let ini = application_ini_path(data_dir);
    if !ini.is_file() {
        return Ok(());
    }
    if local_mariadb_ready() {
        match ensure_contacts_mariadb(data_dir) {
            Ok(()) => return Ok(()),
            Err(err) => {
                ensure_contacts_sqlite_fallback(data_dir).map_err(|fallback_err| {
                    format!(
                        "contacts MariaDB heal failed ({err}); sqlite fallback also failed: {fallback_err}"
                    )
                })?;
                return Ok(());
            }
        }
    }
    ensure_contacts_sqlite_fallback(data_dir)
}

fn ensure_contacts_mariadb(data_dir: &str) -> Result<(), String> {
    let check = Command::new("php")
        .args(["-r", "exit(extension_loaded('pdo_mysql') ? 0 : 1);"])
        .status()
        .map_err(|e| format!("php pdo_mysql check failed: {e}"))?;
    if !check.success() {
        return Err("PHP pdo_mysql extension required for webmail Contacts on MariaDB".into());
    }

    let client_id = client_id_for_data_dir(data_dir);
    let secret = load_or_create_secret(&client_id)?;
    let _ = create_database_with_user_tcp(&secret.db_name, &secret.db_user, &secret.db_password)?;

    let ini = application_ini_path(data_dir);
    let raw = fs::read_to_string(&ini).map_err(|e| e.to_string())?;
    let updated = patch_contacts_mysql_section(&raw, &secret);
    if updated != raw {
        fs::write(&ini, updated).map_err(|e| e.to_string())?;
    }

    init_mariadb_schema(&secret)?;
    let _ = maybe_migrate_sqlite_into_mariadb(data_dir, &secret);

    let marker = Path::new(data_dir).join(MARKER_MARIADB);
    if let Some(parent) = marker.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(
        &marker,
        format!(
            "cpn snappymail-family contacts mariadb v{SCHEMA_VERSION}: client={} db={}\n",
            secret.client_id, secret.db_name
        ),
    );
    Ok(())
}

fn secrets_path(client_id: &str) -> PathBuf {
    PathBuf::from(SECRETS_DIR).join(format!("{client_id}.json"))
}

fn load_or_create_secret(client_id: &str) -> Result<ContactsDbSecret, String> {
    let path = secrets_path(client_id);
    if path.is_file() {
        let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        if let Ok(existing) = serde_json::from_str::<ContactsDbSecret>(&raw) {
            if !existing.db_password.is_empty()
                && existing.db_name.starts_with("cpn_")
                && existing.client_id == client_id
            {
                return Ok(existing);
            }
        }
    }

    let db_name = format!("cpn_{client_id}_ab");
    let db_user = db_name.clone();
    let db_password = random_db_password();
    let secret = ContactsDbSecret {
        schema_version: SCHEMA_VERSION,
        client_id: client_id.to_string(),
        db_name: db_name.clone(),
        db_user,
        db_password,
        pdo_dsn: format!("host=127.0.0.1;port=3306;dbname={db_name}"),
    };
    write_secret(&path, &secret)?;
    Ok(secret)
}

fn write_secret(path: &Path, secret: &ContactsDbSecret) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create secrets dir: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
        }
    }
    let json = serde_json::to_string_pretty(secret).map_err(|e| e.to_string())?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    file.write_all(json.as_bytes())
        .map_err(|e| format!("Could not save {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn random_db_password() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

fn patch_contacts_mysql_section(raw: &str, secret: &ContactsDbSecret) -> String {
    let Some(start) = raw.find("[contacts]") else {
        return raw.to_string();
    };
    let after = &raw[start + "[contacts]".len()..];
    let end_rel = after.find("\n[").map(|i| i + 1).unwrap_or(after.len());
    let section_body = &after[..end_rel];
    let rest = &after[end_rel..];
    let mut body = section_body.to_string();
    body = replace_ini_bool(&body, "enable", true);
    // Admin UI label is "MySQL"; the backend is local MariaDB.
    body = replace_ini_quoted(&body, "type", "mysql");
    body = replace_ini_quoted(&body, "pdo_dsn", &secret.pdo_dsn);
    body = replace_ini_quoted(&body, "pdo_user", &secret.db_user);
    body = replace_ini_quoted(&body, "pdo_password", &secret.db_password);
    body = replace_ini_bool(&body, "sqlite_global", true);
    format!("{}[contacts]{}{}", &raw[..start], body, rest)
}

fn init_mariadb_schema(secret: &ContactsDbSecret) -> Result<(), String> {
    let output = Command::new("php")
        .args([
            "-r",
            r#"
$dsn = getenv('CPN_AB_DSN');
$user = getenv('CPN_AB_USER');
$pass = getenv('CPN_AB_PASS');
if (!$dsn || !$user) { fwrite(STDERR, "missing CPN_AB_*\n"); exit(1); }
$pdo = new PDO('mysql:' . $dsn, $user, $pass, [
  PDO::ATTR_ERRMODE => PDO::ERRMODE_EXCEPTION,
]);
$pdo->exec("CREATE TABLE IF NOT EXISTS rainloop_system (
  sys_name varchar(64) NOT NULL,
  value_int int UNSIGNED NOT NULL DEFAULT 0,
  PRIMARY KEY (sys_name)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_general_ci");
$pdo->exec("CREATE TABLE IF NOT EXISTS rainloop_users (
  id_user int UNSIGNED NOT NULL AUTO_INCREMENT,
  rl_email varchar(254) NOT NULL,
  PRIMARY KEY (id_user),
  UNIQUE KEY ui_rainloop_users_email (rl_email)
) CHARACTER SET utf8mb4 COLLATE utf8mb4_general_ci");
$pdo->exec("CREATE TABLE IF NOT EXISTS rainloop_ab_contacts (
  id_contact bigint UNSIGNED NOT NULL AUTO_INCREMENT,
  id_contact_str varchar(128) NOT NULL DEFAULT '',
  id_user int UNSIGNED NOT NULL,
  display varchar(255) NOT NULL DEFAULT '',
  changed int UNSIGNED NOT NULL DEFAULT 0,
  deleted tinyint UNSIGNED NOT NULL DEFAULT 0,
  etag varchar(128) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL DEFAULT '',
  PRIMARY KEY(id_contact),
  INDEX id_user_rainloop_ab_contacts_index (id_user)
) ENGINE=INNODB CHARACTER SET utf8mb4 COLLATE utf8mb4_general_ci");
$pdo->exec("CREATE TABLE IF NOT EXISTS rainloop_ab_properties (
  id_prop bigint UNSIGNED NOT NULL AUTO_INCREMENT,
  id_contact bigint UNSIGNED NOT NULL,
  id_user int UNSIGNED NOT NULL,
  prop_type tinyint UNSIGNED NOT NULL,
  prop_type_str varchar(255) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL DEFAULT '',
  prop_value MEDIUMTEXT NOT NULL,
  prop_value_custom MEDIUMTEXT NOT NULL,
  prop_frec int UNSIGNED NOT NULL DEFAULT 0,
  PRIMARY KEY(id_prop),
  INDEX id_user_rainloop_ab_properties_index (id_user),
  INDEX id_user_id_contact_rainloop_ab_properties_index (id_user, id_contact),
  INDEX id_contact_prop_type_rainloop_ab_properties_index (id_contact, prop_type)
) ENGINE=INNODB CHARACTER SET utf8mb4 COLLATE utf8mb4_general_ci");
$cols = $pdo->query("SHOW COLUMNS FROM rainloop_ab_properties LIKE 'prop_value_lower'")->fetchAll();
if (!$cols) {
  $pdo->exec("ALTER TABLE rainloop_ab_properties ADD prop_value_lower MEDIUMTEXT NOT NULL");
}
$pdo->prepare("INSERT INTO rainloop_system (sys_name, value_int) VALUES (?, ?)
  ON DUPLICATE KEY UPDATE value_int = VALUES(value_int)")
  ->execute(["mysql-ab-version", 4]);
echo "ok";
"#,
        ])
        .env("CPN_AB_DSN", &secret.pdo_dsn)
        .env("CPN_AB_USER", &secret.db_user)
        .env("CPN_AB_PASS", &secret.db_password)
        .output()
        .map_err(|e| format!("contacts MariaDB schema init failed to start: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "contacts MariaDB schema init failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

fn maybe_migrate_sqlite_into_mariadb(
    data_dir: &str,
    secret: &ContactsDbSecret,
) -> Result<(), String> {
    let sqlite = Path::new(data_dir).join("_data_/_default_/AddressBook.sqlite");
    if !sqlite.is_file() {
        return Ok(());
    }
    let output = Command::new("php")
        .args([
            "-r",
            r#"
$sqlitePath = getenv('CPN_AB_SQLITE');
$dsn = getenv('CPN_AB_DSN');
$user = getenv('CPN_AB_USER');
$pass = getenv('CPN_AB_PASS');
if (!$sqlitePath || !is_file($sqlitePath)) { echo "skip"; exit(0); }
if (!extension_loaded('pdo_sqlite')) { echo "skip"; exit(0); }
$mysql = new PDO('mysql:' . $dsn, $user, $pass, [PDO::ATTR_ERRMODE => PDO::ERRMODE_EXCEPTION]);
$count = (int)$mysql->query('SELECT COUNT(*) FROM rainloop_ab_contacts')->fetchColumn();
if ($count > 0) { echo "keep-mysql"; exit(0); }
$sqlite = new PDO('sqlite:' . $sqlitePath, null, null, [PDO::ATTR_ERRMODE => PDO::ERRMODE_EXCEPTION]);
$users = $sqlite->query('SELECT id_user, rl_email FROM rainloop_users')->fetchAll(PDO::FETCH_ASSOC) ?: [];
$contacts = $sqlite->query('SELECT id_contact, id_contact_str, id_user, display, changed, deleted, etag FROM rainloop_ab_contacts')->fetchAll(PDO::FETCH_ASSOC) ?: [];
$props = $sqlite->query('SELECT id_prop, id_contact, id_user, prop_type, prop_type_str, prop_value, prop_value_custom, prop_frec FROM rainloop_ab_properties')->fetchAll(PDO::FETCH_ASSOC) ?: [];
if (!$users && !$contacts) { echo "empty-sqlite"; exit(0); }
$mysql->beginTransaction();
$insUser = $mysql->prepare('INSERT IGNORE INTO rainloop_users (id_user, rl_email) VALUES (?, ?)');
foreach ($users as $row) {
  $insUser->execute([(int)$row['id_user'], (string)$row['rl_email']]);
}
$insC = $mysql->prepare('INSERT INTO rainloop_ab_contacts (id_contact, id_contact_str, id_user, display, changed, deleted, etag) VALUES (?, ?, ?, ?, ?, ?, ?)');
foreach ($contacts as $row) {
  $insC->execute([
    (int)$row['id_contact'], (string)$row['id_contact_str'], (int)$row['id_user'],
    (string)$row['display'], (int)$row['changed'], (int)$row['deleted'], (string)$row['etag']
  ]);
}
$insP = $mysql->prepare('INSERT INTO rainloop_ab_properties (id_prop, id_contact, id_user, prop_type, prop_type_str, prop_value, prop_value_custom, prop_frec) VALUES (?, ?, ?, ?, ?, ?, ?, ?)');
foreach ($props as $row) {
  $insP->execute([
    (int)$row['id_prop'], (int)$row['id_contact'], (int)$row['id_user'], (int)$row['prop_type'],
    (string)$row['prop_type_str'], (string)$row['prop_value'], (string)$row['prop_value_custom'],
    (int)$row['prop_frec']
  ]);
}
$mysql->commit();
echo "migrated";
"#,
        ])
        .env("CPN_AB_SQLITE", sqlite.to_string_lossy().as_ref())
        .env("CPN_AB_DSN", &secret.pdo_dsn)
        .env("CPN_AB_USER", &secret.db_user)
        .env("CPN_AB_PASS", &secret.db_password)
        .output()
        .map_err(|e| format!("contacts sqlite migrate failed to start: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "contacts sqlite migrate failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

fn ensure_contacts_sqlite_fallback(data_dir: &str) -> Result<(), String> {
    let ini = application_ini_path(data_dir);
    let raw = fs::read_to_string(&ini).map_err(|e| e.to_string())?;
    let updated = patch_contacts_sqlite_section(&raw, data_dir);
    if updated != raw {
        fs::write(&ini, updated).map_err(|e| e.to_string())?;
    }
    ensure_contacts_sqlite_ready(data_dir)
}

fn patch_contacts_sqlite_section(raw: &str, data_dir: &str) -> String {
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
    let Ok(entries) = fs::read_dir(dir) else {
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
        return Err("PHP pdo_sqlite extension required for webmail Contacts fallback".into());
    }

    let db = Path::new(data_dir).join("_data_/_default_/AddressBook.sqlite");
    if let Some(parent) = db.parent() {
        let _ = fs::create_dir_all(parent);
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

    let marker = Path::new(data_dir).join(MARKER_SQLITE);
    let _ = fs::write(
        &marker,
        "cpn snappymail-family contacts sqlite fallback v1\n",
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patches_contacts_to_mysql_mariadb() {
        let raw = r#"[webmail]
title = "SnappyMail Webmail"

[contacts]
; Enable contacts
enable = Off
type = "sqlite"
pdo_dsn = "host=127.0.0.1;port=3306;dbname=snappymail"
pdo_user = "root"
pdo_password = ""
sqlite_global = Off

[security]
admin_login = "admin"
"#;
        let secret = ContactsDbSecret {
            schema_version: 1,
            client_id: "snappymail".into(),
            db_name: "cpn_snappymail_ab".into(),
            db_user: "cpn_snappymail_ab".into(),
            db_password: "TestCredAlphaNum99".into(),
            pdo_dsn: "host=127.0.0.1;port=3306;dbname=cpn_snappymail_ab".into(),
        };
        let out = patch_contacts_mysql_section(raw, &secret);
        assert!(out.contains("enable = On"));
        assert!(out.contains("type = \"mysql\""));
        assert!(out.contains("pdo_dsn = \"host=127.0.0.1;port=3306;dbname=cpn_snappymail_ab\""));
        assert!(out.contains("pdo_user = \"cpn_snappymail_ab\""));
        assert!(out.contains("pdo_password = \"TestCredAlphaNum99\""));
        assert!(out.contains("sqlite_global = On"));
        assert!(out.contains("title = \"SnappyMail Webmail\""));
    }
}
