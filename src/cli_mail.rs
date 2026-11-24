//! `cpn mail`: classic mailbox import, password sync, home migrate, and mail-auth DNS.

use crate::cli_common::read_password_confirmed;
use crate::mail_accounts_import::{format_import_summary, import_legacy_mailboxes_into_registry};
use crate::mail_home_layout::migrate_legacy_top_level_homes;
use crate::panel_audit_cli::record_cli;
use crate::panel_ops_email_password::reset_mailbox_password;
use crate::panel_ops_mail_dns::{provision_mail_dns, provision_mail_dns_for_known_domains};

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
    /// Reset webmail client crypto key after a password change (fixes "login passphrase has changed")
    #[command(name = "webmail-rekey")]
    WebmailRekey {
        /// Full mailbox address (`user@domain`)
        #[arg(long)]
        address: String,
    },
    /// Move legacy `/home/<user>` Maildirs into `/home/<domain>/e-mails/<user>/`
    #[command(name = "migrate-homes")]
    MigrateHomes {
        /// Plan moves only (no filesystem changes)
        #[arg(long)]
        dry_run: bool,
    },
    /// Publish SPF / DKIM / DMARC (local zone + Cloudflare when configured)
    #[command(name = "publish-dns")]
    PublishDns {
        /// Apex domain (omit with `--all` to publish every known local-mail apex)
        #[arg(long)]
        domain: Option<String>,
        /// Publish for every known site/panel/onboarding apex
        #[arg(long)]
        all: bool,
    },
}

pub fn run(command: MailCommands, require_root: fn() -> Result<(), String>) -> Result<(), String> {
    match command {
        MailCommands::ImportLegacy => {
            require_root()?;
            let summary = import_legacy_mailboxes_into_registry()?;
            // Omit sql_passwords_synced from stdout/audit (CodeQL rust/cleartext-logging #179).
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
        MailCommands::WebmailRekey { address } => {
            require_root()?;
            let address = address.trim();
            let text = crate::webmail_cryptkey::reset_client_cryptkey(address)?;
            record_cli(
                "cli.mail.webmail-rekey",
                address,
                true,
                "webmail client key reset",
            );
            println!("{text}");
            Ok(())
        }
        MailCommands::MigrateHomes { dry_run } => {
            require_root()?;
            let report = migrate_legacy_top_level_homes(dry_run)?;
            let summary = format!(
                "mailbox home migrate: scanned={} migrated={} skipped={} dry_run={}",
                report.scanned, report.migrated, report.skipped, report.dry_run
            );
            record_cli("cli.mail.migrate-homes", "all", true, &summary);
            println!("{summary}");
            for note in report.notes {
                println!("{note}");
            }
            if !dry_run && report.migrated > 0 {
                crate::mail_hosted_domains::sync_hosted_mail_delivery_logged(
                    "migrate mailbox homes",
                );
                let _ = crate::panel_ops_mailbox_folders::heal_all_maildir_system_folders();
            }
            Ok(())
        }
        MailCommands::PublishDns { domain, all } => {
            require_root()?;
            if all {
                let text = provision_mail_dns_for_known_domains();
                record_cli("cli.mail.publish-dns", "all", true, "mail auth DNS publish");
                println!("{text}");
                return Ok(());
            }
            let Some(domain) = domain else {
                return Err("Provide --domain <apex> or --all".into());
            };
            let domain = domain.trim();
            if domain.is_empty() {
                return Err("Domain is required".into());
            }
            let text = provision_mail_dns(domain)?;
            record_cli(
                "cli.mail.publish-dns",
                domain,
                true,
                "mail auth DNS publish",
            );
            println!("{text}");
            Ok(())
        }
    }
}
