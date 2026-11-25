//! Walk mailbox trees and ensure IMAP system folders exist.

use crate::panel_ops_mailbox_folders::{
    ensure_imap_system_folders, ensure_imap_system_folders_for_email, ensure_maildir_system_folders,
};
use std::fs;
use std::path::Path;

/// Heal system folders for domain e-mails homes, legacy PAM homes, and classic vmail trees.
pub fn heal_all_maildir_system_folders() -> Result<usize, String> {
    let mut count = 0usize;
    count += heal_domain_emails_trees()?;
    count += heal_pam_homes()?;
    count += heal_classic_vmail_tree()?;
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

fn heal_domain_emails_trees() -> Result<usize, String> {
    let home = Path::new("/home");
    if !home.is_dir() {
        return Ok(0);
    }
    let mut count = 0usize;
    for domain_ent in fs::read_dir(home).map_err(|e| e.to_string())?.flatten() {
        let domain_path = domain_ent.path();
        if !domain_path.is_dir() {
            continue;
        }
        let emails = domain_path.join(crate::mail_home_layout::EMAILS_DIR);
        if !emails.is_dir() {
            continue;
        }
        let domain = domain_ent
            .file_name()
            .to_string_lossy()
            .to_ascii_lowercase();
        if domain.starts_with('.') || domain == "vmail" || domain == "lost+found" {
            continue;
        }
        let Ok(users) = fs::read_dir(&emails) else {
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
            if ensure_maildir_system_folders(&maildir.to_string_lossy(), &email, Some(&local))
                .is_ok()
            {
                count += 1;
            }
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
        if user == "lost+found" || user == "vmail" || user.starts_with('.') || user.contains('.') {
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

fn heal_classic_vmail_tree() -> Result<usize, String> {
    let root = Path::new(crate::mail_home_layout::CLASSIC_VMAIL_ROOT);
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
