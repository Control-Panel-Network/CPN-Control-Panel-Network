//! `cpn totp` subcommands: status / disable / clear (no secrets printed).

use crate::account_mfa::{clear_totp_force, totp_status_for};
use crate::account_mgmt::find_account;
use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum TotpCommands {
    /// Show whether TOTP is enabled (and pending enroll) for an account
    Status {
        #[arg(long)]
        username: String,
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
