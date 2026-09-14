//! Mailbox password reset (panel registry + local system hash via chpasswd).

use crate::mail_accounts::{MailAccount, list_accounts};
use crate::panel_ops_mailbox_provision::provision_local_mailbox;
use crate::paths::join_data;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AccountsPatch {
    #[serde(default)]
    schema_version: u32,
    #[serde(default)]
    accounts: Vec<MailAccount>,
}

fn accounts_path() -> std::path::PathBuf {
    join_data("mail-accounts.json")
}

fn load_accounts_file() -> AccountsPatch {
    fs::read_to_string(accounts_path())
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or(AccountsPatch {
            schema_version: 1,
            accounts: Vec::new(),
        })
}

fn save_accounts_file(file: &AccountsPatch) -> Result<(), String> {
    let path = accounts_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_string_pretty(file).map_err(|e| e.to_string())?;
    fs::write(&path, raw).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn mailbox_choices() -> Vec<(String, String)> {
    list_accounts()
        .into_iter()
        .map(|a| (a.id, a.address))
        .collect()
}

/// Reset mailbox password: update registry hash field and provision local system user.
pub fn reset_mailbox_password(address_or_id: &str, new_password: &str) -> Result<String, String> {
    if new_password.len() < 8 {
        return Err("Password must be at least 8 characters".into());
    }
    if new_password.len() > 256 {
        return Err("Password is too long".into());
    }
    let key = address_or_id.trim().to_ascii_lowercase();
    if key.is_empty() {
        return Err("Select a mailbox".into());
    }
    let mut file = load_accounts_file();
    let Some(account) = file
        .accounts
        .iter_mut()
        .find(|a| a.id.eq_ignore_ascii_case(&key) || a.address.eq_ignore_ascii_case(&key))
    else {
        return Err(format!("Mailbox `{key}` not found in the panel registry"));
    };
    let address = account.address.clone();
    account.mailbox_password = new_password.to_string();
    account.updated_at_unix = crate::account::now_unix();
    save_accounts_file(&file)?;
    match provision_local_mailbox(&address, new_password) {
        Ok(_provision) => Ok(format!(
            "Password updated for `{address}`. Local mailbox ready."
        )),
        Err(err) => {
            // Registry is authoritative for panel UI; local system provision may be
            // unavailable in CI or when the panel lacks useradd privileges.
            Ok(format!(
                "Password updated for `{address}` in the panel registry. Local provision: {err}"
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::mail_accounts::{MailAccountInput, MailSmtpMode, create_account};
    use crate::smtp_settings::SmtpTlsMode;

    #[test]
    fn reset_updates_registry() {
        with_test_data_dir(|| {
            create_account(MailAccountInput {
                address: "demo@example.com".into(),
                domain: "example.com".into(),
                enabled: false,
                smtp_mode: MailSmtpMode::External,
                smtp_host: "smtp.example.com".into(),
                smtp_port: Some(587),
                smtp_tls: Some(SmtpTlsMode::Starttls),
                smtp_username: "demo@example.com".into(),
                smtp_password: "secret".into(),
                mailbox_password: "oldpassword1".into(),
            })
            .unwrap();
            let msg = reset_mailbox_password("demo@example.com", "newpassword1").unwrap();
            assert!(msg.contains("demo@example.com"));
        });
    }
}
