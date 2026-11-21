//! Create and subscribe IMAP system folders for local Maildir mailboxes.
//!
//! SnappyMail/Tachyon need Sent / Drafts / Junk (Spam role) / Trash / Archive so compose
//! and send are not blocked on an incomplete "Select system folders" modal.
//!
//! Classic CyberPanel Dovecot often uses `prefix = INBOX.` so IMAP full names are
//! `INBOX.Sent` (not bare `Sent`). Fresh CPN hosts usually use an empty prefix.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Folder name + optional RFC 6154 SPECIAL-USE flag (Dovecot `15-mailboxes` / doveadm).
///
/// `Junk` carries `\Junk` (SnappyMail junk role / UI label "Spam").
const SYSTEM_FOLDERS: &[(&str, Option<&str>)] = &[
    ("Sent", Some(r"\Sent")),
    ("Drafts", Some(r"\Drafts")),
    ("Junk", Some(r"\Junk")),
    ("Trash", Some(r"\Trash")),
    ("Archive", Some(r"\Archive")),
];

const VMAIL_ROOT: &str = "/home/vmail";

/// IMAP LIST / settings_local name for a system folder (honours Dovecot namespace prefix).
pub fn imap_system_folder_name(bare: &str) -> String {
    format!("{}{bare}", imap_namespace_prefix())
}

/// Detect Dovecot private inbox namespace prefix (for example `INBOX.` or empty).
pub fn imap_namespace_prefix() -> String {
    detect_imap_namespace_prefix()
}

/// Ensure standard IMAP folders exist for a local system user (PAM mailbox).
pub fn ensure_imap_system_folders(user: &str) -> Result<(), String> {
    if !valid_local_user(user) {
        return Err("Invalid mailbox user for system folders".into());
    }
    let maildir = format!("/home/{user}/Maildir");
    ensure_maildir_system_folders(&maildir, user, Some(user))
}

/// Ensure system folders for a full email address (virtual SQL / vmail or PAM).
pub fn ensure_imap_system_folders_for_email(email: &str) -> Result<(), String> {
    let email = email.trim().to_ascii_lowercase();
    if !valid_email(&email) {
        return Err("Invalid mailbox address for system folders".into());
    }
    if let Some(maildir) = resolve_maildir_for_email(&email) {
        let owner = maildir_owner_hint(&maildir);
        return ensure_maildir_system_folders(&maildir, &email, owner.as_deref());
    }
    let local = email.split_once('@').map(|(l, _)| l).unwrap_or("");
    if valid_local_user(local) && Path::new(&format!("/home/{local}/Maildir")).is_dir() {
        return ensure_imap_system_folders(local);
    }
    doveadm_ensure_system_folders(&email)
}

fn ensure_maildir_system_folders(
    maildir: &str,
    doveadm_user: &str,
    chown_user: Option<&str>,
) -> Result<(), String> {
    if !Path::new(maildir).is_dir() {
        return Err(format!("Maildir missing at `{maildir}`"));
    }
    for (name, _) in SYSTEM_FOLDERS {
        for sub in ["cur", "new", "tmp"] {
            let p = format!("{maildir}/.{name}/{sub}");
            fs::create_dir_all(&p).map_err(|e| format!("Cannot create {p}: {e}"))?;
        }
    }
    if let Some(owner) = chown_user {
        if valid_local_user(owner) || owner == "vmail" {
            let _ = Command::new("chown")
                .args(["-R", &format!("{owner}:{owner}"), maildir])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
    doveadm_ensure_system_folders(doveadm_user)
}

fn doveadm_ensure_system_folders(doveadm_user: &str) -> Result<(), String> {
    if !doveadm_available() {
        return Ok(());
    }
    let prefix = imap_namespace_prefix();
    for (name, special) in SYSTEM_FOLDERS {
        let imap_name = format!("{prefix}{name}");
        let _ = Command::new("doveadm")
            .args(["mailbox", "create", "-u", doveadm_user, &imap_name])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = Command::new("doveadm")
            .args(["mailbox", "subscribe", "-u", doveadm_user, &imap_name])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if let Some(flag) = special {
            let _ = Command::new("doveadm")
                .args([
                    "mailbox",
                    "update",
                    "-u",
                    doveadm_user,
                    "-s",
                    flag,
                    &imap_name,
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
    Ok(())
}

/// Heal system folders for PAM homes and classic `/home/vmail` trees.
pub fn heal_all_maildir_system_folders() -> Result<usize, String> {
    let mut count = 0usize;
    count += heal_pam_homes()?;
    count += heal_vmail_tree()?;
    for row in crate::mail_legacy_sql::list_legacy_mailboxes() {
        if ensure_imap_system_folders_for_email(&row.email).is_ok() {
            count += 1;
        }
    }
    for acct in crate::mail_accounts::list_accounts() {
        if ensure_imap_system_folders_for_email(&acct.address).is_ok() {
            count += 1;
        }
    }
    Ok(count)
}

fn heal_pam_homes() -> Result<usize, String> {
    let home = Path::new("/home");
    if !home.is_dir() {
        return Ok(0);
    }
    let mut count = 0usize;
    for entry in fs::read_dir(home).map_err(|e| e.to_string())?.flatten() {
        let name = entry.file_name();
        let Some(user) = name.to_str() else {
            continue;
        };
        if user == "lost+found" || user == "vmail" || user.starts_with('.') {
            continue;
        }
        if !entry.path().join("Maildir").is_dir() {
            continue;
        }
        if ensure_imap_system_folders(user).is_ok() {
            count += 1;
        }
    }
    Ok(count)
}

fn heal_vmail_tree() -> Result<usize, String> {
    let root = Path::new(VMAIL_ROOT);
    if !root.is_dir() {
        return Ok(0);
    }
    let mut count = 0usize;
    let Ok(domains) = fs::read_dir(root) else {
        return Ok(0);
    };
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
            let maildir = user_path.join("Maildir");
            if !maildir.is_dir() {
                continue;
            }
            let email = format!("{local}@{domain}");
            if ensure_maildir_system_folders(&maildir.to_string_lossy(), &email, Some("vmail"))
                .is_ok()
            {
                count += 1;
            }
        }
    }
    Ok(count)
}

fn resolve_maildir_for_email(email: &str) -> Option<String> {
    let (local, domain) = email.split_once('@')?;
    let candidates = [
        PathBuf::from(format!("{VMAIL_ROOT}/{domain}/{local}/Maildir")),
        PathBuf::from(format!("/home/{local}/Maildir")),
    ];
    for p in candidates {
        if p.is_dir() {
            return Some(p.to_string_lossy().into_owned());
        }
    }
    None
}

fn maildir_owner_hint(maildir: &str) -> Option<String> {
    if maildir.starts_with(VMAIL_ROOT) {
        return Some("vmail".into());
    }
    Path::new(maildir)
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
}

fn doveadm_available() -> bool {
    Command::new("doveadm")
        .args(["help"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn valid_local_user(user: &str) -> bool {
    !user.is_empty()
        && !user.contains('/')
        && !user.contains("..")
        && user
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

fn valid_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && !domain.is_empty()
        && !email.contains(' ')
        && local
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' || c == '+')
        && domain
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

/// Read Dovecot inbox namespace `prefix` (trimmed quotes). Empty when unset.
pub(crate) fn detect_imap_namespace_prefix() -> String {
    #[cfg(windows)]
    {
        return String::new();
    }
    #[cfg(not(windows))]
    {
        let mut files: Vec<PathBuf> = vec![PathBuf::from("/etc/dovecot/dovecot.conf")];
        if let Ok(rd) = fs::read_dir("/etc/dovecot/conf.d") {
            for ent in rd.flatten() {
                let p = ent.path();
                if p.extension().and_then(|e| e.to_str()) == Some("conf") {
                    files.push(p);
                }
            }
        }
        let mut best: Option<String> = None;
        for path in files {
            let Ok(raw) = fs::read_to_string(&path) else {
                continue;
            };
            if let Some(prefix) = parse_inbox_namespace_prefix(&raw) {
                if !prefix.is_empty() {
                    return prefix;
                }
                best = Some(prefix);
            }
        }
        best.unwrap_or_default()
    }
}

/// Parse `prefix = ...` from the first namespace block that sets `inbox = yes`.
pub(crate) fn parse_inbox_namespace_prefix(raw: &str) -> Option<String> {
    let mut in_namespace = false;
    let mut depth = 0i32;
    let mut prefix: Option<String> = None;
    let mut inbox = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if !in_namespace {
            let lower = trimmed.to_ascii_lowercase();
            if lower.starts_with("namespace") && trimmed.contains('{') {
                in_namespace = true;
                depth = 1;
                prefix = None;
                inbox = false;
                continue;
            }
            if lower.starts_with("namespace") {
                in_namespace = true;
                depth = 0;
                prefix = None;
                inbox = false;
                continue;
            }
            continue;
        }
        depth += trimmed.matches('{').count() as i32;
        depth -= trimmed.matches('}').count() as i32;
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("prefix") {
            if let Some(v) = ini_value(trimmed) {
                prefix = Some(v);
            }
        }
        if lower.starts_with("inbox") {
            if let Some(v) = ini_value(trimmed) {
                inbox = v.eq_ignore_ascii_case("yes");
            }
        }
        if depth <= 0 {
            in_namespace = false;
            if inbox {
                return Some(prefix.unwrap_or_default());
            }
        }
    }
    None
}

fn ini_value(line: &str) -> Option<String> {
    let (_, rest) = line.split_once('=')?;
    let v = rest.trim().trim_matches(|c| c == '"' || c == '\'').trim();
    Some(v.to_string())
}

/// CPN Dovecot drop-in for stock `namespace inbox` hosts.
///
/// Classic CyberPanel uses an unnamed namespace with `prefix = INBOX.`; we do not
/// declare a second namespace here (that can break LIST). Those hosts get folders via
/// `doveadm mailbox create` with the prefixed IMAP names instead.
pub fn ensure_dovecot_system_mailboxes_conf() -> Result<(), String> {
    let path = Path::new("/etc/dovecot/conf.d/99-cpn-mailboxes.conf");
    let body = r#"# Managed by CPN: auto-create/subscribe IMAP system folders for webmail.
# Junk carries \Junk (SnappyMail / Tachyon Spam role). Prefer Junk over a second Spam mailbox.
namespace inbox {
  mailbox Drafts {
    special_use = \Drafts
    auto = subscribe
  }
  mailbox Junk {
    special_use = \Junk
    auto = subscribe
  }
  mailbox Trash {
    special_use = \Trash
    auto = subscribe
  }
  mailbox Sent {
    special_use = \Sent
    auto = subscribe
  }
  mailbox Archive {
    special_use = \Archive
    auto = subscribe
  }
}
"#;
    if path.is_file()
        && let Ok(existing) = fs::read_to_string(path)
        && existing == body
    {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(path, body).map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
    let _ = Command::new("timeout")
        .args(["8", "systemctl", "reload", "dovecot"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{SYSTEM_FOLDERS, parse_inbox_namespace_prefix};

    #[test]
    fn includes_junk_sent_archive() {
        let names: Vec<&str> = SYSTEM_FOLDERS.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"Junk"));
        assert!(!names.contains(&"Spam"));
        assert!(names.contains(&"Sent"));
        assert!(names.contains(&"Archive"));
        let junk_flag = SYSTEM_FOLDERS
            .iter()
            .find(|(n, _)| *n == "Junk")
            .and_then(|(_, f)| *f);
        assert_eq!(junk_flag, Some(r"\Junk"));
    }

    #[test]
    fn parses_cyberpanel_inbox_prefix() {
        let raw = r#"
namespace {
    type = private
    separator = .
    prefix = INBOX.
    inbox = yes
}
"#;
        assert_eq!(parse_inbox_namespace_prefix(raw).as_deref(), Some("INBOX."));
    }

    #[test]
    fn parses_empty_prefix_namespace_inbox() {
        let raw = r#"
namespace inbox {
  inbox = yes
}
"#;
        assert_eq!(parse_inbox_namespace_prefix(raw).as_deref(), Some(""));
    }
}
