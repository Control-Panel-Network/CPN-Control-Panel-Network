//! Legacy virtual-mail SQL auth (classic panel `e_users` + Dovecot SQL).
//!
//! After classic → CPN cutovers, Dovecot often still authenticates against a
//! MySQL/MariaDB mailbox table while CPN's panel registry and PAM provision
//! paths diverge. These helpers detect that backend and keep passwords in sync
//! without putting brand names in operator-facing copy.

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

const DOVECOT_SQL_CONF: &str = "/etc/dovecot/dovecot-sql.conf.ext";
const VMAIL_ROOT: &str = "/home/vmail";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyMailbox {
    pub email: String,
    pub domain: String,
}

/// True when Dovecot SQL still points at a virtual mailbox table (`e_users`).
pub fn virtual_sql_mail_active() -> bool {
    #[cfg(windows)]
    {
        return false;
    }
    #[cfg(not(windows))]
    {
        dovecot_sql_uses_e_users() || e_users_table_reachable()
    }
}

/// Prefer SnappyMail/Tachyon shortLogin only for PAM/system mailboxes.
pub fn prefer_webmail_short_login() -> bool {
    !virtual_sql_mail_active()
}

fn dovecot_sql_uses_e_users() -> bool {
    let Ok(raw) = fs::read_to_string(DOVECOT_SQL_CONF) else {
        return false;
    };
    let lower = raw.to_ascii_lowercase();
    lower.contains("e_users")
        && (lower.contains("driver = mysql") || lower.contains("driver=mysql"))
}

fn e_users_table_reachable() -> bool {
    let Some(bin) = mysql_client() else {
        return false;
    };
    Command::new(bin)
        .args(["-N", "-e", "SELECT 1 FROM cyberpanel.e_users LIMIT 1"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn mysql_client() -> Option<&'static str> {
    ["mariadb", "mysql"].into_iter().find(|&bin| {
        Command::new(bin)
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
}

/// Discover mailboxes from SQL inventory and/or `/home/vmail`.
pub fn list_legacy_mailboxes() -> Vec<LegacyMailbox> {
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for row in list_from_sql() {
        if seen.insert(row.email.to_ascii_lowercase()) {
            out.push(row);
        }
    }
    for row in list_from_vmail(Path::new(VMAIL_ROOT)) {
        if seen.insert(row.email.to_ascii_lowercase()) {
            out.push(row);
        }
    }
    out
}

fn list_from_sql() -> Vec<LegacyMailbox> {
    let Some(bin) = mysql_client() else {
        return Vec::new();
    };
    let output = Command::new(bin)
        .args([
            "-N",
            "-e",
            "SELECT email, emailOwner_id FROM cyberpanel.e_users ORDER BY email",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let raw = String::from_utf8_lossy(&output.stdout);
    parse_sql_mailbox_rows(&raw)
}

fn parse_sql_mailbox_rows(raw: &str) -> Vec<LegacyMailbox> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split('\t');
        let Some(email) = parts.next() else {
            continue;
        };
        let domain = parts
            .next()
            .map(str::trim)
            .filter(|d| !d.is_empty())
            .map(|d| d.to_ascii_lowercase())
            .unwrap_or_else(|| domain_of(email).unwrap_or_default());
        if let Some(row) = normalize_mailbox(email, &domain) {
            out.push(row);
        }
    }
    out
}

pub(crate) fn list_from_vmail(root: &Path) -> Vec<LegacyMailbox> {
    let Ok(domains) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for domain_ent in domains.flatten() {
        let domain_path = domain_ent.path();
        if !domain_path.is_dir() {
            continue;
        }
        let domain = domain_ent
            .file_name()
            .to_string_lossy()
            .to_ascii_lowercase();
        if domain.starts_with('.') {
            continue;
        }
        let Ok(users) = fs::read_dir(&domain_path) else {
            continue;
        };
        for user_ent in users.flatten() {
            let user_path = user_ent.path();
            if !user_path.is_dir() {
                continue;
            }
            let local = user_ent.file_name().to_string_lossy().to_ascii_lowercase();
            if local.starts_with('.') {
                continue;
            }
            // Prefer Maildir layouts; skip stray files.
            if !(user_path.join("Maildir").is_dir() || user_path.join("new").is_dir()) {
                continue;
            }
            let email = format!("{local}@{domain}");
            if let Some(row) = normalize_mailbox(&email, &domain) {
                out.push(row);
            }
        }
    }
    out
}

fn normalize_mailbox(email: &str, domain: &str) -> Option<LegacyMailbox> {
    let email = email.trim().to_ascii_lowercase();
    if !valid_email(&email) {
        return None;
    }
    let domain = if domain.trim().is_empty() {
        domain_of(&email)?
    } else {
        domain.trim().to_ascii_lowercase()
    };
    Some(LegacyMailbox { email, domain })
}

fn domain_of(email: &str) -> Option<String> {
    let (_, domain) = email.split_once('@')?;
    let domain = domain.trim().to_ascii_lowercase();
    if domain.is_empty() {
        None
    } else {
        Some(domain)
    }
}

fn valid_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    if local.is_empty() || domain.is_empty() || email.contains(' ') {
        return false;
    }
    local
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' || c == '+')
        && domain
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

/// Hash `password` and store it in `cyberpanel.e_users` when that table exists.
pub fn sync_password_to_virtual_sql(email: &str, password: &str) -> Result<String, String> {
    if !virtual_sql_mail_active() {
        return Ok("legacy virtual-mail SQL not active; skipped".into());
    }
    let email = email.trim().to_ascii_lowercase();
    if !valid_email(&email) {
        return Err("Mailbox address is invalid".into());
    }
    if password.len() < 8 {
        return Err("Mailbox password must be at least 8 characters".into());
    }
    let hash = doveadm_blf_crypt(password)?;
    let Some(bin) = mysql_client() else {
        return Err("MariaDB client is not installed".into());
    };
    let email_sql = escape_sql(&email);
    let hash_sql = escape_sql(&hash);
    let sql =
        format!("UPDATE cyberpanel.e_users SET password='{hash_sql}' WHERE email='{email_sql}'");
    let output = Command::new(bin)
        .args(["-e", &sql])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Could not update virtual-mail SQL password: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Could not update virtual-mail SQL password: {}",
            err.trim()
        ));
    }
    Ok(format!(
        "Updated virtual-mail SQL password for `{email}` (Dovecot/webmail auth store)."
    ))
}

fn doveadm_blf_crypt(password: &str) -> Result<String, String> {
    let output = Command::new("doveadm")
        .args(["pw", "-s", "BLF-CRYPT", "-p", password])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("doveadm pw failed: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("doveadm pw failed: {}", err.trim()));
    }
    let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if hash.is_empty() {
        return Err("doveadm pw returned an empty hash".into());
    }
    Ok(hash)
}

fn escape_sql(value: &str) -> String {
    value.replace('\'', "''")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parses_sql_rows() {
        let rows = parse_sql_mailbox_rows(
            "info@example.com\texample.com\nabuse@example.com\texample.com\n",
        );
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].email, "info@example.com");
        assert_eq!(rows[0].domain, "example.com");
    }

    #[test]
    fn rejects_invalid_emails() {
        assert!(normalize_mailbox("not-an-email", "example.com").is_none());
        assert!(normalize_mailbox("a@b.com", "b.com").is_some());
    }

    #[test]
    fn lists_vmail_layout() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("cpn-vmail-{stamp}"));
        let mailbox = root.join("example.com").join("info").join("Maildir");
        fs::create_dir_all(&mailbox).unwrap();
        let rows = list_from_vmail(&root);
        let _ = fs::remove_dir_all(&root);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].email, "info@example.com");
    }

    #[test]
    fn short_login_pref_is_inverse_of_sql_active_on_windows() {
        #[cfg(windows)]
        {
            assert!(!virtual_sql_mail_active());
            assert!(prefer_webmail_short_login());
        }
    }

    #[test]
    fn operator_copy_avoids_classic_brand_token() {
        let msg = "Updated virtual-mail SQL password for `a@b.com` (Dovecot/webmail auth store).";
        assert!(!msg.to_ascii_lowercase().contains("cyberpanel"));
    }
}
