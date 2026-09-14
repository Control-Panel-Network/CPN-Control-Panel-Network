//! Provision local Postfix/Dovecot system mailbox for a panel address.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

fn local_part(address: &str) -> Result<String, String> {
    let address = address.trim().to_ascii_lowercase();
    let Some((user, domain)) = address.split_once('@') else {
        return Err("Mailbox address must include @domain".into());
    };
    if user.is_empty() || domain.is_empty() {
        return Err("Mailbox address is incomplete".into());
    }
    if !user
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err("Local part may only contain letters, digits, ., _, -".into());
    }
    if user.len() > 32 {
        return Err("Local part is too long for a system mailbox user".into());
    }
    Ok(user.to_string())
}

fn user_exists(user: &str) -> bool {
    Command::new("id")
        .arg(user)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn set_password(user: &str, password: &str) -> Result<(), String> {
    let mut child = Command::new("chpasswd")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not run chpasswd: {e}"))?;
    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "chpasswd stdin unavailable".to_string())?;
        writeln!(stdin, "{user}:{password}").map_err(|e| format!("chpasswd write failed: {e}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("chpasswd failed: {e}"))?;
    if !output.status.success() {
        return Err("chpasswd rejected the mailbox password".into());
    }
    Ok(())
}

fn ensure_maildir(user: &str) -> Result<(), String> {
    let home = format!("/home/{user}");
    let maildir = format!("{home}/Maildir");
    for sub in ["new", "cur", "tmp"] {
        let p = format!("{maildir}/{sub}");
        fs::create_dir_all(&p).map_err(|e| format!("Cannot create Maildir: {e}"))?;
    }
    let _ = Command::new("chown")
        .args(["-R", &format!("{user}:{user}"), &maildir])
        .status();
    if Path::new("/etc/aliases").exists() {
        let _ = Command::new("newaliases").status();
    }
    let _ = home;
    Ok(())
}

/// Create or update a local system mailbox (Dovecot PAM / Postfix Maildir).
pub fn provision_local_mailbox(address: &str, password: &str) -> Result<String, String> {
    if password.len() < 8 {
        return Err("Mailbox password must be at least 8 characters".into());
    }
    let user = local_part(address)?;
    #[cfg(windows)]
    {
        let _ = user;
        return Ok(
            "Local mailbox provisioning is Linux-only; account was stored in the panel registry."
                .into(),
        );
    }
    #[cfg(not(windows))]
    {
        if !user_exists(&user) {
            let status = Command::new("useradd")
                .args(["-m", "-s", "/sbin/nologin", &user])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .status()
                .map_err(|e| format!("useradd failed: {e}"))?;
            if !status.success() {
                return Err(format!("useradd failed for `{user}`"));
            }
        }
        set_password(&user, password)?;
        ensure_maildir(&user)?;
        let _ = crate::panel_ops_mailbox_folders::ensure_dovecot_system_mailboxes_conf();
        let _ = crate::panel_ops_mailbox_folders::ensure_imap_system_folders(&user);
        let _ = crate::install_snappymail_folders::seed_settings_local_for_address(address);
        Ok(format!(
            "Local mailbox ready for `{address}` (system user `{user}`, Maildir + system folders)."
        ))
    }
}
