//! Create and subscribe IMAP system folders for local Maildir mailboxes.
//!
//! SnappyMail needs Sent / Drafts / Junk (Spam role) / Trash / Archive so compose
//! and send are not blocked on an incomplete "Select system folders" modal.

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

/// Folder name + optional RFC 6154 SPECIAL-USE flag (Dovecot `15-mailboxes` / doveadm).
///
/// `Junk` carries `\Junk` (SnappyMail junk role / UI label "Spam").
/// `Spam` is also created and subscribed so filters and clients that expect that
/// name have a mailbox; SPECIAL-USE stays on `Junk` only to avoid two junk roles.
const SYSTEM_FOLDERS: &[(&str, Option<&str>)] = &[
    ("Sent", Some(r"\Sent")),
    ("Drafts", Some(r"\Drafts")),
    ("Junk", Some(r"\Junk")),
    ("Spam", None),
    ("Trash", Some(r"\Trash")),
    ("Archive", Some(r"\Archive")),
];

/// Ensure standard IMAP folders exist for a local system user (PAM mailbox).
pub fn ensure_imap_system_folders(user: &str) -> Result<(), String> {
    if user.is_empty()
        || user.contains('/')
        || user.contains("..")
        || !user
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err("Invalid mailbox user for system folders".into());
    }
    let home = format!("/home/{user}");
    let maildir = format!("{home}/Maildir");
    if !Path::new(&maildir).is_dir() {
        return Err(format!("Maildir missing for `{user}`"));
    }

    // Always create Maildir++ layout so folders exist even if doveadm is down.
    for (name, _) in SYSTEM_FOLDERS {
        for sub in ["cur", "new", "tmp"] {
            let p = format!("{maildir}/.{name}/{sub}");
            fs::create_dir_all(&p).map_err(|e| format!("Cannot create {p}: {e}"))?;
        }
    }
    let _ = Command::new("chown")
        .args(["-R", &format!("{user}:{user}"), &maildir])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    // Prefer doveadm so IMAP LIST + subscriptions stay consistent with Dovecot.
    if Command::new("doveadm")
        .args(["help"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        for (name, special) in SYSTEM_FOLDERS {
            let _ = Command::new("doveadm")
                .args(["mailbox", "create", "-u", user, name])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = Command::new("doveadm")
                .args(["mailbox", "subscribe", "-u", user, name])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            if let Some(flag) = special {
                // Best-effort SPECIAL-USE (supported on Dovecot 2.3+).
                let _ = Command::new("doveadm")
                    .args(["mailbox", "update", "-u", user, "-s", flag, name])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
        }
    }
    Ok(())
}

/// Heal system folders for every local home with a Maildir (install / SnappyMail heal).
pub fn heal_all_maildir_system_folders() -> Result<usize, String> {
    let home = Path::new("/home");
    if !home.is_dir() {
        return Ok(0);
    }
    let mut count = 0usize;
    let entries = fs::read_dir(home).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(user) = name.to_str() else {
            continue;
        };
        if user == "lost+found" || user.starts_with('.') {
            continue;
        }
        let maildir = entry.path().join("Maildir");
        if !maildir.is_dir() {
            continue;
        }
        if ensure_imap_system_folders(user).is_ok() {
            count += 1;
        }
    }
    Ok(count)
}

/// CPN Dovecot drop-in: auto-create + subscribe system folders with SPECIAL-USE.
pub fn ensure_dovecot_system_mailboxes_conf() -> Result<(), String> {
    let path = Path::new("/etc/dovecot/conf.d/99-cpn-mailboxes.conf");
    let body = r#"# Managed by CPN: auto-create/subscribe IMAP system folders for webmail.
# Junk carries \Junk (SnappyMail Spam role). Spam is also subscribed without a second \Junk.
namespace inbox {
  mailbox Drafts {
    special_use = \Drafts
    auto = subscribe
  }
  mailbox Junk {
    special_use = \Junk
    auto = subscribe
  }
  mailbox Spam {
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
    if path.is_file() {
        if let Ok(existing) = fs::read_to_string(path) {
            if existing == body {
                return Ok(());
            }
        }
    }
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(path, body).map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
    let _ = Command::new("systemctl")
        .args(["reload", "dovecot"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::SYSTEM_FOLDERS;

    #[test]
    fn includes_junk_and_spam() {
        let names: Vec<&str> = SYSTEM_FOLDERS.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"Junk"));
        assert!(names.contains(&"Spam"));
        assert!(names.contains(&"Sent"));
        assert!(names.contains(&"Archive"));
        let junk_flag = SYSTEM_FOLDERS
            .iter()
            .find(|(n, _)| *n == "Junk")
            .and_then(|(_, f)| *f);
        assert_eq!(junk_flag, Some(r"\Junk"));
        let spam_flag = SYSTEM_FOLDERS
            .iter()
            .find(|(n, _)| *n == "Spam")
            .and_then(|(_, f)| *f);
        assert_eq!(spam_flag, None);
    }
}
