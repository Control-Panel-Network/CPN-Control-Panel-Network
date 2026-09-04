//! Panel mailboxes: every enabled account must have valid SMTP (external or Postfix).

use crate::account::{data_dir, now_unix};
use crate::postfix_fallback::{postfix_is_ready, require_postfix_smtp_ready};
use crate::smtp_settings::{SmtpSetupInput, SmtpTlsMode, validate_smtp_input};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MailSmtpMode {
    /// External SMTP host/port/encryption (and auth when required).
    External,
    /// Default local Postfix (127.0.0.1:25 or :587).
    PostfixLocal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailAccount {
    pub id: String,
    pub address: String,
    /// Associated site FQDN when scoped (domain or nested subdomain).
    #[serde(default)]
    pub domain: String,
    pub enabled: bool,
    pub smtp_mode: MailSmtpMode,
    #[serde(default)]
    pub smtp_host: String,
    #[serde(default)]
    pub smtp_port: u16,
    #[serde(default)]
    pub smtp_tls: SmtpTlsMode,
    #[serde(default)]
    pub smtp_username: String,
    /// Stored on disk only; never shown in panel HTML.
    #[serde(default)]
    pub smtp_password: String,
    pub created_at_unix: u64,
    pub updated_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct MailAccountsFile {
    #[serde(default)]
    schema_version: u32,
    #[serde(default)]
    accounts: Vec<MailAccount>,
}

#[derive(Debug, Clone)]
pub struct MailAccountInput {
    pub address: String,
    pub domain: String,
    pub enabled: bool,
    pub smtp_mode: MailSmtpMode,
    pub smtp_host: String,
    pub smtp_port: Option<u16>,
    pub smtp_tls: Option<SmtpTlsMode>,
    pub smtp_username: String,
    pub smtp_password: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MailAccountPublic {
    pub id: String,
    pub address: String,
    pub domain: String,
    pub enabled: bool,
    pub smtp_mode: MailSmtpMode,
    pub smtp_summary: String,
    pub smtp_valid: bool,
    pub smtp_error: Option<String>,
}

fn accounts_path() -> PathBuf {
    data_dir().join("mail-accounts.json")
}

fn load_file() -> MailAccountsFile {
    let Ok(raw) = fs::read_to_string(accounts_path()) else {
        return MailAccountsFile {
            schema_version: SCHEMA_VERSION,
            accounts: Vec::new(),
        };
    };
    serde_json::from_str(&raw).unwrap_or(MailAccountsFile {
        schema_version: SCHEMA_VERSION,
        accounts: Vec::new(),
    })
}

fn save_file(file: &MailAccountsFile) -> Result<(), String> {
    let path = accounts_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create data dir: {e}"))?;
    }
    let mut out = file.clone();
    out.schema_version = SCHEMA_VERSION;
    let raw = serde_json::to_string_pretty(&out)
        .map_err(|e| format!("Could not serialize mail accounts: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("Could not write mail accounts: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn normalize_address(raw: &str) -> Result<String, String> {
    let address = raw.trim().to_ascii_lowercase();
    if address.is_empty() || !address.contains('@') || address.contains(' ') {
        return Err("Mailbox address must be a valid email".into());
    }
    Ok(address)
}

/// Validate SMTP for an account that is (or will be) enabled.
pub fn validate_enabled_smtp(account: &MailAccount) -> Result<(), String> {
    match account.smtp_mode {
        MailSmtpMode::PostfixLocal => require_postfix_smtp_ready(),
        MailSmtpMode::External => {
            let input = SmtpSetupInput {
                host: account.smtp_host.clone(),
                port: Some(if account.smtp_port == 0 {
                    587
                } else {
                    account.smtp_port
                }),
                tls_mode: Some(account.smtp_tls),
                from_address: account.address.clone(),
                username: Some(account.smtp_username.clone()),
                password: Some(account.smtp_password.clone()),
            };
            let settings = validate_smtp_input(&input)?;
            if !settings.username.is_empty() && settings.password.is_empty() {
                return Err(
                    "SMTP username is set but password is empty. Provide a password or clear the username."
                        .into(),
                );
            }
            Ok(())
        }
    }
}

pub fn smtp_validity(account: &MailAccount) -> (bool, Option<String>) {
    match validate_enabled_smtp(account) {
        Ok(()) => (true, None),
        Err(error) => (false, Some(error)),
    }
}

fn smtp_summary(account: &MailAccount) -> String {
    match account.smtp_mode {
        MailSmtpMode::PostfixLocal => {
            if postfix_is_ready() {
                "Postfix local (127.0.0.1)".into()
            } else {
                "Postfix local (not running)".into()
            }
        }
        MailSmtpMode::External => format!(
            "{}:{} ({:?})",
            account.smtp_host,
            if account.smtp_port == 0 {
                587
            } else {
                account.smtp_port
            },
            account.smtp_tls
        ),
    }
}

pub fn to_public(account: &MailAccount) -> MailAccountPublic {
    let (smtp_valid, smtp_error) = if account.enabled {
        smtp_validity(account)
    } else {
        // Disabled accounts still show whether they could enable cleanly.
        smtp_validity(account)
    };
    MailAccountPublic {
        id: account.id.clone(),
        address: account.address.clone(),
        domain: account.domain.clone(),
        enabled: account.enabled,
        smtp_mode: account.smtp_mode,
        smtp_summary: smtp_summary(account),
        smtp_valid,
        smtp_error,
    }
}

pub fn list_accounts() -> Vec<MailAccount> {
    load_file().accounts
}

pub fn list_accounts_public() -> Vec<MailAccountPublic> {
    list_accounts().iter().map(to_public).collect()
}

pub fn create_account(input: MailAccountInput) -> Result<MailAccount, String> {
    let address = normalize_address(&input.address)?;
    let account = MailAccount {
        id: format!("mbox-{}", now_unix()),
        address,
        domain: input.domain.trim().to_ascii_lowercase(),
        enabled: input.enabled,
        smtp_mode: input.smtp_mode,
        smtp_host: input.smtp_host.trim().to_string(),
        smtp_port: input.smtp_port.unwrap_or(587),
        smtp_tls: input.smtp_tls.unwrap_or_default(),
        smtp_username: input.smtp_username.trim().to_string(),
        smtp_password: input.smtp_password,
        created_at_unix: now_unix(),
        updated_at_unix: now_unix(),
    };
    if account.enabled {
        validate_enabled_smtp(&account)?;
    } else if account.smtp_mode == MailSmtpMode::External && !account.smtp_host.is_empty() {
        // Soft-check incomplete drafts only when host was provided.
        let _ = validate_enabled_smtp(&account);
    }
    let mut file = load_file();
    if file
        .accounts
        .iter()
        .any(|a| a.address.eq_ignore_ascii_case(&account.address))
    {
        return Err(format!("Mailbox `{}` already exists", account.address));
    }
    file.accounts.push(account.clone());
    save_file(&file)?;
    Ok(account)
}

pub fn set_account_enabled(id: &str, enabled: bool) -> Result<MailAccount, String> {
    let mut file = load_file();
    let Some(account) = file.accounts.iter_mut().find(|a| a.id == id) else {
        return Err(format!("Mailbox `{id}` not found"));
    };
    if enabled {
        validate_enabled_smtp(account)?;
    }
    account.enabled = enabled;
    account.updated_at_unix = now_unix();
    let out = account.clone();
    save_file(&file)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_enable_external_without_host() {
        let account = MailAccount {
            id: "x".into(),
            address: "a@example.com".into(),
            domain: String::new(),
            enabled: true,
            smtp_mode: MailSmtpMode::External,
            smtp_host: String::new(),
            smtp_port: 587,
            smtp_tls: SmtpTlsMode::Starttls,
            smtp_username: String::new(),
            smtp_password: String::new(),
            created_at_unix: 0,
            updated_at_unix: 0,
        };
        assert!(validate_enabled_smtp(&account).is_err());
    }
}
