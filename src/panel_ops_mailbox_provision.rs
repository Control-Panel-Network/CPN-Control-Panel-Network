//! Provision local Postfix/Dovecot system mailbox for a panel address.
//!
//! Mailbox homes live under `/home/<domain>/e-mails/<user>/` (Maildir + sieve),
//! not as top-level `/home/<user>` next to website homes.

use std::io::Write;
use std::process::{Command, Stdio};

use crate::mail_home_layout::{preferred_mailbox_home, split_address};

pub(crate) fn local_part(address: &str) -> Result<String, String> {
    let (user, _domain) = split_address(address)?;
    if !user
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err("Local part may only contain letters, digits, ., _, -".into());
    }
    if user.len() > 32 {
        return Err("Local part is too long for a system mailbox user".into());
    }
    Ok(user)
}

pub(crate) fn user_exists(user: &str) -> bool {
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

/// Create or update a local system mailbox (Dovecot PAM / Postfix Maildir).
pub fn provision_local_mailbox(address: &str, password: &str) -> Result<String, String> {
    if password.len() < 8 {
        return Err("Mailbox password must be at least 8 characters".into());
    }
    let user = local_part(address)?;
    let (_, domain) = split_address(address)?;
    let home = preferred_mailbox_home(&domain, &user);
    #[cfg(windows)]
    {
        let _ = (user, home);
        Ok(
            "Local mailbox provisioning is Linux-only; account was stored in the panel registry."
                .into(),
        )
    }
    #[cfg(not(windows))]
    {
        use crate::mail_home_layout::{ensure_mailbox_unix_user, ensure_maildir_skeleton};
        // Classic virtual-mail stacks authenticate in SQL (`e_users`), not PAM on a short
        // Unix local part. Skip chpasswd/useradd so Change Password cannot fail before SQL.
        if crate::mail_legacy_sql::virtual_sql_mail_active() {
            return crate::mail_legacy_sql::sync_password_to_virtual_sql(address, password);
        }
        ensure_mailbox_unix_user(&user, &home)?;
        set_password(&user, password)?;
        ensure_maildir_skeleton(&home, &user)?;
        let _ = crate::panel_ops_mailbox_folders::ensure_dovecot_system_mailboxes_conf();
        let _ = crate::panel_ops_mailbox_folders::ensure_imap_system_folders_for_email(address);
        let _ = crate::install_snappymail_folders::seed_settings_local_for_address(address);
        Ok(format!(
            "Local mailbox ready for `{address}` (system user `{user}`, home `{}`).",
            home.display()
        ))
    }
}
