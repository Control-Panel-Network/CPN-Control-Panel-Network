//! SOGo configuration managed by CPN: MariaDB storage, `/etc/sogo/sogo.conf`, and the
//! mailbox user table SOGo authenticates against (`sogo_users`).
//!
//! Secrets live in `/var/lib/cpn/sogo/db.json` (mode 600). Mailbox logins reuse the CPN
//! system-mailbox credentials: the password hash is copied from `/etc/shadow` so SOGo's
//! `crypt` user source verifies the same password Dovecot / Postfix accept.

use crate::apps_sogo_repo::SOGO_STATE_DIR;
use crate::panel_ops_db::{create_database_with_user_tcp, mariadb_client_bin};
use rand::{Rng, distr::Alphanumeric};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const SOGO_CONF: &str = "/etc/sogo/sogo.conf";
/// sogod loopback listener (panel reverse-proxies `/SOGo` to it).
pub const SOGO_LOOPBACK: &str = "127.0.0.1:20000";
const DB_SECRET_FILE: &str = "db.json";
const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SogoDbSecret {
    pub schema_version: u32,
    pub db_name: String,
    pub db_user: String,
    pub db_password: String,
}

fn db_access_path() -> PathBuf {
    Path::new(SOGO_STATE_DIR).join(DB_SECRET_FILE)
}

fn random_password() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

fn write_private(path: &Path, body: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
        }
    }
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
    file.write_all(body.as_bytes())
        .map_err(|e| format!("Could not save {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Load the SOGo database access record (name, user, password) from
/// `/var/lib/cpn/sogo/db.json` or mint a new one (reused across reinstalls so data survives).
/// The password never appears in any returned message; only `db_name` / `db_user` do.
pub fn load_or_create_db_access() -> Result<SogoDbSecret, String> {
    let path = db_access_path();
    if path.is_file()
        && let Ok(raw) = fs::read_to_string(&path)
        && let Ok(existing) = serde_json::from_str::<SogoDbSecret>(&raw)
        && !existing.db_password.is_empty()
        && !existing.db_name.is_empty()
    {
        return Ok(existing);
    }
    let db = SogoDbSecret {
        schema_version: SCHEMA_VERSION,
        db_name: "cpn_sogo".into(),
        db_user: "cpn_sogo".into(),
        db_password: random_password(),
    };
    let json = serde_json::to_string_pretty(&db).map_err(|e| e.to_string())?;
    write_private(&path, &json)?;
    Ok(db)
}

/// Backward-compatible alias for `load_or_create_db_access`.
pub fn load_or_create_secret() -> Result<SogoDbSecret, String> {
    load_or_create_db_access()
}

pub fn db_access_exists() -> bool {
    db_access_path().is_file()
}

/// Backward-compatible alias for `db_access_exists`.
pub fn secret_exists() -> bool {
    db_access_exists()
}

/// Create the SOGo database + user on local MariaDB (TCP + socket grants).
pub fn ensure_database(db: &SogoDbSecret) -> Result<String, String> {
    create_database_with_user_tcp(&db.db_name, &db.db_user, &db.db_password)
}

fn run_sql(db: &str, sql: &str) -> Result<(), String> {
    let bin = mariadb_client_bin().ok_or_else(|| {
        "MariaDB client not found (`mariadb` or `mysql`). Install MariaDB from Host packages."
            .to_string()
    })?;
    let out = Command::new(bin)
        .args(["--database", db, "-e", sql])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("Failed to run {bin}: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "SOGo SQL failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

fn sql_quote(raw: &str) -> String {
    format!(
        "'{}'",
        raw.replace('\\', "\\\\")
            .replace('\'', "''")
            .replace('\0', "")
    )
}

/// Password hash for a local system mailbox user (`/etc/shadow`, root-only read).
fn shadow_hash(user: &str) -> Option<String> {
    let raw = fs::read_to_string("/etc/shadow").ok()?;
    for line in raw.lines() {
        let mut parts = line.split(':');
        if parts.next()? != user {
            continue;
        }
        let hash = parts.next()?.trim();
        if hash.is_empty() || hash.starts_with('!') || hash.starts_with('*') {
            return None;
        }
        return Some(hash.to_string());
    }
    None
}

/// One row of the SOGo SQL user source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SogoUserRow {
    pub uid: String,
    pub display: String,
    pub mail: String,
    pub password_hash: String,
}

pub fn render_users_sql(rows: &[SogoUserRow]) -> String {
    let mut sql = String::from(
        "CREATE TABLE IF NOT EXISTS sogo_users (c_uid VARCHAR(255) NOT NULL PRIMARY KEY, c_name VARCHAR(255) NOT NULL, c_password VARCHAR(255) NOT NULL, c_cn VARCHAR(255) NOT NULL, mail VARCHAR(255) NOT NULL) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4; START TRANSACTION; DELETE FROM sogo_users; ",
    );
    for row in rows {
        sql.push_str(&format!(
            "INSERT INTO sogo_users (c_uid, c_name, c_password, c_cn, mail) VALUES ({uid}, {uid}, {pw}, {cn}, {mail}); ",
            uid = sql_quote(&row.uid),
            pw = sql_quote(&row.password_hash),
            cn = sql_quote(&row.display),
            mail = sql_quote(&row.mail),
        ));
    }
    sql.push_str("COMMIT;");
    sql
}

fn collect_user_rows() -> Vec<SogoUserRow> {
    let mut rows = Vec::new();
    for account in crate::mail_accounts::list_accounts() {
        if !account.enabled {
            continue;
        }
        let Ok(local) = crate::panel_ops_mailbox_provision::local_part(&account.address) else {
            continue;
        };
        let Some(hash) = shadow_hash(&local) else {
            continue;
        };
        rows.push(SogoUserRow {
            uid: account.address.clone(),
            display: local,
            mail: account.address.clone(),
            password_hash: hash,
        });
    }
    rows
}

/// Rebuild `sogo_users` from enabled CPN mailboxes. Returns the number of synced logins.
pub fn sync_users(db: &SogoDbSecret) -> Result<usize, String> {
    let rows = collect_user_rows();
    run_sql(&db.db_name, &render_users_sql(&rows))?;
    Ok(rows.len())
}

/// Best-effort resync hook for mailbox create / password change paths.
pub fn sync_users_if_installed() {
    if !crate::apps_sogo_repo::sogod_binary_present() || !db_access_exists() {
        return;
    }
    if let Ok(db) = load_or_create_db_access() {
        let _ = sync_users(&db);
    }
}

fn host_timezone() -> String {
    if let Ok(target) = fs::read_link("/etc/localtime") {
        let s = target.to_string_lossy();
        if let Some(idx) = s.find("zoneinfo/") {
            let tz = &s[idx + "zoneinfo/".len()..];
            if !tz.is_empty() && tz.len() < 64 {
                return tz.to_string();
            }
        }
    }
    "UTC".into()
}

fn host_mail_domain() -> String {
    let out = Command::new("hostname")
        .arg("-f")
        .stdin(Stdio::null())
        .output()
        .ok();
    let fqdn = out
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    match fqdn.split_once('.') {
        Some((_, domain)) if !domain.is_empty() && domain.contains('.') => domain.to_string(),
        _ => fqdn,
    }
}

fn plist_string(raw: &str) -> String {
    format!("\"{}\"", raw.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Render the CPN-managed `sogo.conf` (OpenStep plist). Secrets are embedded; file is 0640 root:sogo.
pub fn render_sogo_conf(db: &SogoDbSecret, timezone: &str, mail_domain: &str) -> String {
    let dsn = format!(
        "mysql://{}:{}@127.0.0.1:3306/{}",
        db.db_user, db.db_password, db.db_name
    );
    let table = |name: &str| plist_string(&format!("{dsn}/{name}"));
    let domain_line = if mail_domain.is_empty() {
        String::new()
    } else {
        format!("  SOGoMailDomain = {};\n", plist_string(mail_domain))
    };
    format!(
        "{{\n  /* Managed by CPN (SOGo host package). Reinstall or Start from Plugins > Host regenerates this file. */\n\
  WOPort = {port};\n  WOWorkersCount = 3;\n  SxVMemLimit = 512;\n  WOUseRelativeURLs = NO;\n\n\
  SOGoProfileURL = {profile};\n  OCSFolderInfoURL = {folder};\n  OCSSessionsFolderURL = {sessions};\n  OCSEMailAlarmsFolderURL = {alarms};\n  OCSStoreURL = {store};\n  OCSAclURL = {acl};\n  OCSCacheFolderURL = {cache};\n  OCSAdminURL = {admin};\n\n\
  SOGoUserSources = (\n    {{\n      type = sql;\n      id = cpn_mailboxes;\n      displayName = \"CPN mailboxes\";\n      viewURL = {users};\n      canAuthenticate = YES;\n      isAddressBook = YES;\n      userPasswordAlgorithm = crypt;\n      LoginFieldNames = (c_uid, mail);\n    }}\n  );\n\n\
  SOGoIMAPServer = \"imap://127.0.0.1:143\";\n  SOGoSieveServer = \"sieve://127.0.0.1:4190\";\n  SOGoSieveScriptsEnabled = YES;\n  SOGoVacationEnabled = YES;\n  SOGoForwardEnabled = YES;\n  SOGoSMTPServer = \"smtp://127.0.0.1:587\";\n  SOGoMailingMechanism = smtp;\n  SOGoSMTPAuthenticationType = PLAIN;\n  SOGoMailAuxiliaryUserAccountsEnabled = YES;\n{domain_line}\n\
  SOGoMemcachedHost = \"127.0.0.1\";\n  SOGoTimeZone = {tz};\n  SOGoLanguage = English;\n  SOGoPageTitle = \"SOGo\";\n  SOGoLoginModule = Mail;\n  SOGoPasswordChangeEnabled = NO;\n  SOGoEnableEMailAlarms = YES;\n  SOGoAppointmentSendEMailNotifications = YES;\n  SOGoFoldersSendEMailNotifications = YES;\n  SOGoACLsSendEMailNotifications = YES;\n  SOGoXSRFValidationEnabled = YES;\n  SOGoTrustProxyAuthentication = NO;\n  SOGoSuperUsernames = ();\n}}\n",
        port = plist_string(SOGO_LOOPBACK),
        profile = table("sogo_user_profile"),
        folder = table("sogo_folder_info"),
        sessions = table("sogo_sessions_folder"),
        alarms = table("sogo_alarms_folder"),
        store = table("sogo_store"),
        acl = table("sogo_acl"),
        cache = table("sogo_cache_folder"),
        admin = table("sogo_admin"),
        users = table("sogo_users"),
        tz = plist_string(timezone),
    )
}

/// Write `/etc/sogo/sogo.conf` (0640 root:sogo) from the managed template.
pub fn write_sogo_conf(db: &SogoDbSecret) -> Result<(), String> {
    let body = render_sogo_conf(db, &host_timezone(), &host_mail_domain());
    let path = Path::new(SOGO_CONF);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create /etc/sogo: {e}"))?;
    }
    if path.is_file() {
        let existing = fs::read_to_string(path).unwrap_or_default();
        if !existing.contains("Managed by CPN") {
            let backup = format!("{SOGO_CONF}.cpn-backup");
            let _ = fs::copy(path, &backup);
        }
    }
    fs::write(path, body).map_err(|e| format!("Could not write {SOGO_CONF}: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o640));
    }
    let _ = Command::new("chown")
        .args(["root:sogo", SOGO_CONF])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    Ok(())
}

pub fn conf_is_cpn_managed() -> bool {
    fs::read_to_string(SOGO_CONF)
        .map(|raw| raw.contains("Managed by CPN"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret() -> SogoDbSecret {
        SogoDbSecret {
            schema_version: 1,
            db_name: "cpn_sogo".into(),
            db_user: "cpn_sogo".into(),
            db_password: "p4ss\"word".into(),
        }
    }

    #[test]
    fn conf_embeds_dsn_and_mail_backends() {
        let conf = render_sogo_conf(&secret(), "Europe/Oslo", "example.com");
        assert!(conf.contains("Managed by CPN"));
        assert!(conf.contains("mysql://cpn_sogo:p4ss\\\"word@127.0.0.1:3306/cpn_sogo/sogo_users"));
        assert!(conf.contains("SOGoIMAPServer = \"imap://127.0.0.1:143\""));
        assert!(conf.contains("SOGoSMTPServer = \"smtp://127.0.0.1:587\""));
        assert!(conf.contains("SOGoTimeZone = \"Europe/Oslo\""));
        assert!(conf.contains("SOGoMailDomain = \"example.com\""));
        assert!(conf.contains("WOPort = \"127.0.0.1:20000\""));
        assert!(conf.contains("userPasswordAlgorithm = crypt"));
    }

    #[test]
    fn conf_without_domain_omits_mail_domain() {
        let conf = render_sogo_conf(&secret(), "UTC", "");
        assert!(!conf.contains("SOGoMailDomain"));
    }

    #[test]
    fn users_sql_escapes_quotes_and_is_transactional() {
        let rows = vec![SogoUserRow {
            uid: "o'neil@example.com".into(),
            display: "o'neil".into(),
            mail: "o'neil@example.com".into(),
            password_hash: "$6$ab\\cd$hash".into(),
        }];
        let sql = render_users_sql(&rows);
        assert!(sql.starts_with("CREATE TABLE IF NOT EXISTS sogo_users"));
        assert!(sql.contains("START TRANSACTION; DELETE FROM sogo_users;"));
        assert!(sql.contains("'o''neil@example.com'"));
        assert!(sql.contains("'$6$ab\\\\cd$hash'"));
        assert!(sql.ends_with("COMMIT;"));
    }

    #[test]
    fn empty_users_still_creates_table() {
        let sql = render_users_sql(&[]);
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS sogo_users"));
        assert!(!sql.contains("INSERT INTO"));
    }
}
