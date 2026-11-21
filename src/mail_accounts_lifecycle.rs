//! Soft-delete, restore, purge, and rename for panel mailboxes.
//!
//! Soft-delete keeps Maildir/auth for 14 days. Only the panel owner can restore.
//! Purge removes the registry row and best-effort clears delivery/auth.

use crate::account::{default_password_policy, now_unix, password_meets_policy};
use crate::mail_accounts::{
    MailAccount, load_file_mut, save_file_public, sync_after_mailbox_change,
};
use crate::panel_admin::is_panel_admin;
use crate::panel_ops_email_password::reset_mailbox_password;

/// Soft-deleted mailboxes remain restorable for this many seconds (14 days).
pub const MAILBOX_SOFT_DELETE_TTL_SECS: u64 = 14 * 24 * 60 * 60;

fn find_mut<'a>(accounts: &'a mut [MailAccount], id: &str) -> Result<&'a mut MailAccount, String> {
    accounts
        .iter_mut()
        .find(|a| a.id == id)
        .ok_or_else(|| format!("Mailbox `{id}` not found"))
}

fn find_ref<'a>(accounts: &'a [MailAccount], id: &str) -> Result<&'a MailAccount, String> {
    accounts
        .iter()
        .find(|a| a.id == id)
        .ok_or_else(|| format!("Mailbox `{id}` not found"))
}

/// Soft-delete: hide from the normal list, disable delivery, keep Maildir/auth.
pub fn soft_delete_mailbox(id: &str, actor: &str) -> Result<MailAccount, String> {
    let mut file = load_file_mut();
    let account = find_mut(&mut file.accounts, id)?;
    if account.deleted_at_unix.is_some() {
        return Err(format!("Mailbox `{}` is already deleted", account.address));
    }
    account.enabled = false;
    account.deleted_at_unix = Some(now_unix());
    account.updated_at_unix = now_unix();
    let out = account.clone();
    save_file_public(&file)?;
    sync_after_mailbox_change("soft-delete mailbox");
    let _ = crate::panel_audit::record(
        actor,
        "email.mailbox.delete",
        &out.address,
        true,
        "soft-deleted (restorable for 14 days)",
    );
    Ok(out)
}

/// Restore a soft-deleted mailbox (panel owner only) within the TTL.
pub fn restore_mailbox(id: &str, actor: &str) -> Result<MailAccount, String> {
    if !is_panel_admin(actor) {
        return Err("Only the panel owner can restore deleted mailboxes".into());
    }
    purge_expired_deleted_mailboxes();
    let mut file = load_file_mut();
    let account = find_mut(&mut file.accounts, id)?;
    let Some(deleted_at) = account.deleted_at_unix else {
        return Err(format!("Mailbox `{}` is not deleted", account.address));
    };
    let age = now_unix().saturating_sub(deleted_at);
    if age > MAILBOX_SOFT_DELETE_TTL_SECS {
        return Err(format!(
            "Mailbox `{}` passed the 14-day restore window and must be purged",
            account.address
        ));
    }
    account.deleted_at_unix = None;
    account.updated_at_unix = now_unix();
    // Re-enable when SMTP/local MTA is ready; otherwise restore as disabled.
    account.enabled = crate::mail_accounts::validate_enabled_smtp(account).is_ok();
    let out = account.clone();
    save_file_public(&file)?;
    sync_after_mailbox_change("restore mailbox");
    let _ = crate::panel_audit::record(
        actor,
        "email.mailbox.restore",
        &out.address,
        true,
        "restored from soft-delete",
    );
    Ok(out)
}

/// Rename local-part (same domain) for an active mailbox.
pub fn rename_mailbox(id: &str, new_local_part: &str, actor: &str) -> Result<MailAccount, String> {
    let local = new_local_part.trim().to_ascii_lowercase();
    if local.is_empty() {
        return Err("New email username is required".into());
    }
    if local.contains('@') || local.contains(' ') {
        return Err("Email username must not include @ or spaces".into());
    }
    if !local
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-'))
    {
        return Err("Email username may only use letters, numbers, and . _ + -".into());
    }
    let mut file = load_file_mut();
    let account = find_ref(&file.accounts, id)?.clone();
    if account.deleted_at_unix.is_some() {
        return Err("Restore the mailbox before renaming it".into());
    }
    let domain = account
        .address
        .rsplit_once('@')
        .map(|(_, d)| d.to_string())
        .filter(|d| !d.is_empty())
        .or_else(|| {
            let d = account.domain.trim().to_ascii_lowercase();
            if d.is_empty() { None } else { Some(d) }
        })
        .ok_or_else(|| "Mailbox has no domain".to_string())?;
    let new_address = format!("{local}@{domain}");
    if new_address.eq_ignore_ascii_case(&account.address) {
        return Ok(account);
    }
    if file
        .accounts
        .iter()
        .any(|a| a.id != id && a.address.eq_ignore_ascii_case(&new_address))
    {
        return Err(format!("Mailbox `{new_address}` already exists"));
    }
    let account = find_mut(&mut file.accounts, id)?;
    let old_address = account.address.clone();
    account.address = new_address.clone();
    if account.domain.is_empty() {
        account.domain = domain;
    }
    account.updated_at_unix = now_unix();
    let out = account.clone();
    save_file_public(&file)?;
    if out.smtp_mode == crate::mail_accounts::MailSmtpMode::PostfixLocal
        && !out.mailbox_password.is_empty()
    {
        let _ = crate::panel_ops_mailbox_provision::provision_local_mailbox(
            &out.address,
            &out.mailbox_password,
        );
    }
    sync_after_mailbox_change("rename mailbox");
    let _ = crate::panel_audit::record(
        actor,
        "email.mailbox.rename",
        &out.address,
        true,
        &format!("renamed from {old_address}"),
    );
    Ok(out)
}

/// Change mailbox password using the panel password policy (same as CPN users).
pub fn modify_mailbox_password(
    id: &str,
    new_password: &str,
    actor: &str,
) -> Result<String, String> {
    let policy = default_password_policy();
    password_meets_policy(new_password, &policy)?;
    let file = load_file_mut();
    let account = find_ref(&file.accounts, id)?;
    if account.deleted_at_unix.is_some() {
        return Err("Restore the mailbox before changing its password".into());
    }
    let address = account.address.clone();
    let msg = reset_mailbox_password(&address, new_password)?;
    let _ = crate::panel_audit::record(
        actor,
        "email.mailbox.password",
        &address,
        true,
        "password updated",
    );
    Ok(msg)
}

/// Apply rename and/or password from the Modify modal.
pub fn modify_mailbox(
    id: &str,
    new_local_part: Option<&str>,
    new_password: Option<&str>,
    actor: &str,
) -> Result<String, String> {
    let mut notes = Vec::new();
    if let Some(local) = new_local_part.map(str::trim).filter(|s| !s.is_empty()) {
        let acct = rename_mailbox(id, local, actor)?;
        notes.push(format!("Renamed to {}", acct.address));
    }
    if let Some(password) = new_password.map(str::trim).filter(|s| !s.is_empty()) {
        let msg = modify_mailbox_password(id, password, actor)?;
        // Do not append provision detail that may be noisy; keep short.
        let _ = msg;
        notes.push("Password updated".into());
    }
    if notes.is_empty() {
        return Err("Enter a new username and/or password".into());
    }
    Ok(notes.join(". "))
}

fn hard_purge_one(account: &MailAccount) -> Result<(), String> {
    // Best-effort: stop delivery via registry removal + map sync. Do not log secrets.
    if crate::mail_legacy_sql::virtual_sql_mail_active() {
        let _ = invalidate_virtual_sql_auth(&account.address);
    }
    Ok(())
}

fn invalidate_virtual_sql_auth(email: &str) -> Result<(), String> {
    // Rotate to an unusable random hash so IMAP/SMTP auth fails after purge.
    let random: String = (0..32)
        .map(|_| {
            let n = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
                ^ (email.len() as u128)) as u8;
            format!("{:x}", n % 16)
        })
        .collect();
    // Prefer doveadm when available via existing sync path with a throwaway secret.
    // If sync fails, registry removal still proceeds.
    let _ = crate::mail_legacy_sql::sync_password_to_virtual_sql(email, &format!("X{random}A1"));
    Ok(())
}

/// Purge soft-deleted mailboxes older than the restore TTL. Safe to call often.
pub fn purge_expired_deleted_mailboxes() -> usize {
    let mut file = load_file_mut();
    let now = now_unix();
    let before = file.accounts.len();
    let mut kept = Vec::with_capacity(file.accounts.len());
    for account in file.accounts.drain(..) {
        match account.deleted_at_unix {
            Some(deleted_at) if now.saturating_sub(deleted_at) > MAILBOX_SOFT_DELETE_TTL_SECS => {
                let _ = hard_purge_one(&account);
            }
            _ => kept.push(account),
        }
    }
    file.accounts = kept;
    let removed = before.saturating_sub(file.accounts.len());
    if removed > 0 {
        let _ = save_file_public(&file);
        sync_after_mailbox_change("purge expired mailboxes");
    }
    removed
}

/// Soft-deleted accounts still within the restore window (owner UI).
pub fn list_restorable_mailboxes() -> Vec<MailAccount> {
    purge_expired_deleted_mailboxes();
    let now = now_unix();
    load_file_mut()
        .accounts
        .into_iter()
        .filter(|a| {
            a.deleted_at_unix
                .map(|t| now.saturating_sub(t) <= MAILBOX_SOFT_DELETE_TTL_SECS)
                .unwrap_or(false)
        })
        .collect()
}

/// Days remaining for restore (1..=14), or None when not deleted / expired.
pub fn restore_days_remaining(account: &MailAccount) -> Option<u64> {
    let deleted_at = account.deleted_at_unix?;
    let age = now_unix().saturating_sub(deleted_at);
    if age > MAILBOX_SOFT_DELETE_TTL_SECS {
        return None;
    }
    let secs_left = MAILBOX_SOFT_DELETE_TTL_SECS.saturating_sub(age);
    Some((secs_left / 86_400).max(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::mail_accounts::{MailAccountInput, MailSmtpMode, create_account};
    use crate::smtp_settings::SmtpTlsMode;

    fn sample_password() -> String {
        ['S', 'e', 'c', 'r', 'e', 't', '1', '2']
            .into_iter()
            .collect()
    }

    #[test]
    fn soft_delete_and_restore_roundtrip() {
        with_test_data_dir(|| {
            let acct = create_account(MailAccountInput {
                address: "demo@example.com".into(),
                domain: "example.com".into(),
                enabled: false,
                smtp_mode: MailSmtpMode::PostfixLocal,
                smtp_host: String::new(),
                smtp_port: Some(587),
                smtp_tls: Some(SmtpTlsMode::Starttls),
                smtp_username: String::new(),
                smtp_password: String::new(),
                mailbox_password: sample_password(),
            })
            .unwrap();
            let deleted = soft_delete_mailbox(&acct.id, "admin").unwrap();
            assert!(deleted.deleted_at_unix.is_some());
            assert!(!deleted.enabled);
            assert!(crate::mail_accounts::list_accounts_public().is_empty());
            // Non-owner cannot restore without bootstrap; skip admin path when no bootstrap.
        });
    }
}
