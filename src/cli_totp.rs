//! `cpn totp` subcommands: status / disable / clear (no secrets printed).

use crate::account_mfa::{
    begin_totp_enroll, clear_totp_force, confirm_totp_enroll, totp_status_for,
};
use crate::account_mgmt::find_account;
use crate::cli_common::write_mode600_temp_file;
use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum TotpCommands {
    /// Show whether TOTP is enabled (and pending enroll) for an account
    Status {
        #[arg(long)]
        username: String,
    },
    /// Start TOTP enrollment; write secret material to a mode-600 temp file (never stdout)
    Begin {
        #[arg(long)]
        username: String,
    },
    /// Confirm pending TOTP enrollment with a 6-digit code
    Confirm {
        #[arg(long)]
        username: String,
        #[arg(long)]
        code: String,
    },
    /// Disable TOTP for an account without an authenticator code (prompt, or --yes to skip)
    Disable {
        #[arg(long)]
        username: String,
        /// Skip interactive confirmation
        #[arg(long)]
        yes: bool,
    },
    /// Alias of disable: remove TOTP secret and pending enroll (prompt, or --yes to skip)
    Clear {
        #[arg(long)]
        username: String,
        /// Skip interactive confirmation
        #[arg(long)]
        yes: bool,
    },
}

fn resolve_username(raw: &str) -> Result<String, String> {
    let (boot, _) = find_account(raw)?;
    Ok(boot.username)
}

fn clear_for(
    username: &str,
    yes: bool,
    confirm_delete: fn(&str, bool) -> Result<(), String>,
) -> Result<(), String> {
    let username = resolve_username(username)?;
    confirm_delete(
        "Clear TOTP for this account? Authenticator codes will stop working.",
        yes,
    )?;
    let was_enabled = clear_totp_force(&username)?;
    println!("cleared totp ok\twas_enabled={was_enabled}");
    Ok(())
}

pub fn run(
    command: TotpCommands,
    require_root: fn() -> Result<(), String>,
    confirm_delete: fn(&str, bool) -> Result<(), String>,
) -> Result<(), String> {
    match command {
        TotpCommands::Status { username } => {
            require_root()?;
            let username = resolve_username(&username)?;
            let (enabled, pending) = totp_status_for(&username);
            println!("totp_enabled={enabled}\tpending_enroll={pending}");
            Ok(())
        }
        TotpCommands::Begin { username } => {
            require_root()?;
            let username = resolve_username(&username)?;
            let (secret_b32, otpauth_uri, _svg) = begin_totp_enroll(&username)?;
            let body = format!(
                "# TOTP enrollment for {username}\n# base32_secret (manual entry)\n{secret_b32}\n# otpauth_uri\n{otpauth_uri}\n"
            );
            let path = write_mode600_temp_file("cpn-totp-enroll", &body)?;
            crate::panel_audit_cli::record_cli("cli.totp.begin", &username, true, "pending=1");
            println!("totp_enroll_pending=true");
            println!("totp_enroll_file={}", path.display());
            println!(
                "note=Add the secret to an authenticator app, then run: sudo cpn totp confirm --username {username} --code CODE"
            );
            Ok(())
        }
        TotpCommands::Confirm { username, code } => {
            require_root()?;
            let username = resolve_username(&username)?;
            let code = code.trim();
            if code.is_empty() {
                return Err("Authenticator code is required (pass --code)".into());
            }
            let backup_codes = confirm_totp_enroll(&username, code)?;
            let joined = backup_codes.join("\n");
            let path = write_mode600_temp_file("cpn-totp-backup-codes", &joined)?;
            crate::panel_audit_cli::record_cli(
                "cli.totp.confirm",
                &username,
                true,
                &format!("backup_codes_count={}", backup_codes.len()),
            );
            println!("totp_enabled=true");
            println!("backup_codes_file={}", path.display());
            println!("backup_codes_count={}", backup_codes.len());
            Ok(())
        }
        TotpCommands::Disable { username, yes } => {
            require_root()?;
            clear_for(&username, yes, confirm_delete)
        }
        TotpCommands::Clear { username, yes } => {
            require_root()?;
            clear_for(&username, yes, confirm_delete)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::{
        PanelBootstrap, default_password_policy, new_password_salt, now_unix, persist_bootstrap,
        with_test_data_dir,
    };
    use crate::account_mfa::{
        begin_totp_enroll, clear_totp_force, confirm_totp_enroll, totp_enabled_for,
    };
    use crate::account_totp::{decode_totp_secret, totp_code_at};

    fn seed_account(username: &str) {
        let salt = new_password_salt();
        let boot = PanelBootstrap {
            schema_version: 1,
            username: username.to_string(),
            password_hash: "x".into(),
            password_salt: salt,
            recovery_email: "admin@example.com".into(),
            language: "en".into(),
            password_policy: default_password_policy(),
            created_at_unix: 1,
            must_change_password: false,
            totp_required: true,
            disabled: false,
            role: String::new(),
            parent_reseller: String::new(),
            parent_owner: String::new(),
        };
        persist_bootstrap(&boot).expect("bootstrap");
    }

    #[test]
    fn clear_force_removes_enabled_totp() {
        with_test_data_dir(|| {
            seed_account("admin");
            let (secret_b32, _, _) = begin_totp_enroll("admin").unwrap();
            let secret = decode_totp_secret(&secret_b32).unwrap();
            let code = format!("{:06}", totp_code_at(&secret, now_unix()));
            confirm_totp_enroll("admin", &code).unwrap();
            assert!(totp_enabled_for("admin"));
            let was = clear_totp_force("admin").unwrap();
            assert!(was);
            assert!(!totp_enabled_for("admin"));
            let (enabled, pending) = totp_status_for("admin");
            assert!(!enabled);
            assert!(!pending);
            let username = resolve_username("Admin").unwrap();
            assert_eq!(username, "admin");
        });
    }
}
