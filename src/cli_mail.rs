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

pub fn run(
    command: MailCommands,
    require_root: fn() -> Result<(), String>,
) -> Result<(), String> {
    match command {
        MailCommands::ImportLegacy => {
            require_root()?;
            let summary = import_legacy_mailboxes_into_registry()?;
            let text = format_import_summary(&summary);
            record_cli("cli.mail.import-legacy", "all", true, &text);
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
                return Err("Password is required (use --password-stdin or interactive entry)".into());
            };
            let msg = reset_mailbox_password(address, &secret)?;
            record_cli(
                "cli.mail.passwd",
                address,
                true,
                "mailbox password updated",
            );
            println!("{msg}");
            Ok(())
        }
    }
}