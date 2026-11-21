//! `cpn mail`: classic mailbox import and password sync (same backends as `/email/password`).

use crate::cli_common::read_password_confirmed;
use crate::mail_accounts_import::{format_import_summary, import_legacy_mailboxes_into_registry};
use crate::panel_audit_cli::record_cli;
use crate::panel_ops_email_password::reset_mailbox_password;

/// Subcommands for `cpn mail`.
#[derive(Debug, clap::Subcommand)]
pub enum MailCommands {
    /// Import classic virtual mailboxes into CPN and align webmail shortLogin
    #[command(name = "import-legacy")]
    ImportLegacy,
    /// Set a mailbox password (registry + Dovecot auth backends; no password on argv)
    Passwd {
        /// Full mailbox address (`user@domain`)
        #[arg(long)]
        address: String,
        /// Read the new password from stdin (never put secrets in argv)
        #[arg(long)]
        password_stdin: bool,
    },
}

pub fn run(command: MailCommands, require_root: fn() -> Result<(), String>) -> Result<(), String> {
    match command {
        MailCommands::ImportLegacy => {
            require_root()?;
            let summary = import_legacy_mailboxes_into_registry()?;
            let text = format_import_summary(&summary);
            // Audit omit sql_passwords_synced count (CodeQL rust/cleartext-logging).
            let audit = format!(
                "Mail registry import: discovered={} imported={} already={} shortLogin_files={} virtual_sql={}",
                summary.discovered,
                summary.imported,
                summary.already_present,
                summary.short_login_files,
                summary.virtual_sql_active,
            );
            record_cli("cli.mail.import-legacy", "all", true, &audit);
            println!("{text}");
            Ok(())
        }
        MailCommands::Passwd {
            address,
            password_stdin,
        } => {
            require_root()?;
            let address = address.trim();
            if address.is_empty() {
                return Err("Address is required".into());
            }
            let (password, _) = read_password_confirmed(password_stdin, false)?;
            let Some(secret) = password else {
                return Err(
                    "Password is required (use --password-stdin or interactive entry)".into(),
                );
            };
            reset_mailbox_password(address, &secret)?;
            record_cli("cli.mail.passwd", address, true, "mailbox password updated");
            // Do not echo reset_mailbox_password return (CodeQL rust/cleartext-logging).
            println!("Mailbox password updated for `{address}`.");
            Ok(())
        }
    }
}
