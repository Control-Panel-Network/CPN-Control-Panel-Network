//! Import legacy virtual mailboxes into the CPN panel registry and align webmail login mode.

use crate::account::now_unix;
use crate::mail_accounts::{MailAccount, MailSmtpMode, list_accounts};
use crate::mail_legacy_sql::{
    LegacyMailbox, list_legacy_mailboxes, prefer_webmail_short_login, virtual_sql_mail_active,
};
use crate::smtp_settings::SmtpTlsMode;
use serde_json::Value;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MailImportSummary {
    pub discovered: usize,
    pub imported: usize,
    pub already_present: usize,
    pub short_login_files: usize,
    pub virtual_sql_active: bool,
}

/// Discover classic virtual mailboxes and register any missing addresses in `mail-accounts.json`.
/// Does not invent mailbox passwords; operators reset via Email → Change Password.
pub fn import_legacy_mailboxes_into_registry() -> Result<MailImportSummary, String> {
    let mut summary = MailImportSummary {
        virtual_sql_active: virtual_sql_mail_active(),
        ..MailImportSummary::default()
    };
    let discovered = list_legacy_mailboxes();
    summary.discovered = discovered.len();
    if discovered.is_empty() && !summary.virtual_sql_active {
        summary.short_login_files = ensure_webmail_short_login_for_backend()?;
        return Ok(summary);
    }
    let existing: std::collections::HashSet<String> = list_accounts()
        .into_iter()
        .map(|a| a.address.to_ascii_lowercase())
        .collect();
    let mut file = load_accounts_file()?;
    for row in discovered {
        if existing.contains(&row.email) {
            summary.already_present += 1;
            continue;
        }
        if file
            .accounts
            .iter()
            .any(|a| a.address.eq_ignore_ascii_case(&row.email))
        {
            summary.already_present += 1;
            continue;
        }
        file.accounts.push(account_from_legacy(&row));
        summary.imported += 1;
    }
    if summary.imported > 0 {
        save_accounts_file(&file)?;
    }
    summary.short_login_files = ensure_webmail_short_login_for_backend()?;
    Ok(summary)
}

fn account_from_legacy(row: &LegacyMailbox) -> MailAccount {
    let now = now_unix();
    MailAccount {
        id: format!("mbox-import-{:x}-{}", now, simple_hash(&row.email)),
        address: row.email.clone(),
        domain: row.domain.clone(),
        enabled: true,
        smtp_mode: MailSmtpMode::PostfixLocal,
        smtp_host: String::new(),
        smtp_port: 0,
        smtp_tls: SmtpTlsMode::Starttls,
        smtp_username: String::new(),
        smtp_password: String::new(),
        mailbox_password: String::new(),
        created_at_unix: now,
        updated_at_unix: now,
    }
}

fn simple_hash(input: &str) -> u32 {
    let mut h: u32 = 2166136261;
    for b in input.as_bytes() {
        h ^= u32::from(*b);
        h = h.wrapping_mul(16777619);
    }
    h
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct AccountsFile {
    #[serde(default)]
    schema_version: u32,
    #[serde(default)]
    accounts: Vec<MailAccount>,
}

fn accounts_path() -> std::path::PathBuf {
    crate::account::data_dir().join("mail-accounts.json")
}

fn load_accounts_file() -> Result<AccountsFile, String> {
    let path = accounts_path();
    if !path.is_file() {
        return Ok(AccountsFile {
            schema_version: 1,
            accounts: Vec::new(),
        });
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("Could not read mail accounts: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("mail-accounts.json is unreadable: {e}"))
}

fn save_accounts_file(file: &AccountsFile) -> Result<(), String> {
    let path = accounts_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create data dir: {e}"))?;
    }
    let mut out = file.clone();
    out.schema_version = 1;
    let raw = serde_json::to_string_pretty(&out)
        .map_err(|e| format!("Could not serialize mail accounts: {e}"))?;
    fs::write(&path, format!("{raw}\n")).map_err(|e| format!("Could not write mail accounts: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Set SnappyMail/Tachyon domain `shortLogin` to match the active mail auth mode.
pub fn ensure_webmail_short_login_for_backend() -> Result<usize, String> {
    let short = prefer_webmail_short_login();
    let mut touched = 0usize;
    for data_dir in crate::install_snappymail_lineage::lineage_data_dirs() {
        touched += patch_domains_short_login(Path::new(&data_dir), short)?;
    }
    Ok(touched)
}

fn patch_domains_short_login(data_dir: &Path, short_login: bool) -> Result<usize, String> {
    let domains = data_dir.join("_data_/_default_/domains");
    if !domains.is_dir() {
        return Ok(0);
    }
    let Ok(entries) = fs::read_dir(&domains) else {
        return Ok(0);
    };
    let mut touched = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(mut data) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let mut changed = false;
        for key in ["IMAP", "SMTP", "Sieve"] {
            if let Some(obj) = data.get_mut(key).and_then(|v| v.as_object_mut()) {
                let next = Value::Bool(short_login);
                if obj.get("shortLogin") != Some(&next) {
                    obj.insert("shortLogin".into(), next);
                    changed = true;
                }
            }
        }
        if changed {
            let pretty = serde_json::to_string_pretty(&data)
                .map_err(|e| format!("Could not encode {}: {e}", path.display()))?;
            fs::write(&path, format!("{pretty}\n"))
                .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
            touched += 1;
        }
    }
    Ok(touched)
}

pub fn format_import_summary(summary: &MailImportSummary) -> String {
    format!(
        "Mail registry import: discovered={} imported={} already={} shortLogin_files={} virtual_sql={}",
        summary.discovered,
        summary.imported,
        summary.already_present,
        summary.short_login_files,
        summary.virtual_sql_active
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_copy_avoids_classic_brand_token() {
        let text = format_import_summary(&MailImportSummary {
            discovered: 2,
            imported: 1,
            already_present: 1,
            short_login_files: 3,
            virtual_sql_active: true,
        });
        assert!(!text.to_ascii_lowercase().contains("cyberpanel"));
        assert!(text.contains("imported=1"));
    }

    #[test]
    fn account_from_legacy_uses_postfix_local() {
        let row = LegacyMailbox {
            email: "info@example.com".into(),
            domain: "example.com".into(),
        };
        let account = account_from_legacy(&row);
        assert_eq!(account.address, "info@example.com");
        assert_eq!(account.smtp_mode, MailSmtpMode::PostfixLocal);
        assert!(account.mailbox_password.is_empty());
        assert!(account.enabled);
    }
}
