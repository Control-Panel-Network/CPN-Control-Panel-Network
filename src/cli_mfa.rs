//! `cpn mfa` subcommands: clear TOTP + passkeys (+ pending ceremonies) for an account.

use crate::account_mfa::clear_totp_force;
use crate::account_mgmt::find_account;
use crate::account_passkeys::clear_all_passkeys;
use crate::panel_webauthn::clear_all_ceremonies;
use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum MfaCommands {
    /// Clear TOTP, all passkeys, and pending WebAuthn ceremonies (requires --yes)
    Clear {
        #[arg(long)]
        username: String,
        #[arg(long)]
        yes: bool,
    },
}

fn resolve_username(raw: &str) -> Result<String, String> {
    let (boot, _) = find_account(raw)?;
    Ok(boot.username)
}

pub fn run(
    command: MfaCommands,
    require_root: fn() -> Result<(), String>,
    confirm_delete: fn(&str, bool) -> Result<(), String>,
) -> Result<(), String> {
    match command {
        MfaCommands::Clear { username, yes } => {
            require_root()?;
            let username = resolve_username(&username)?;
            confirm_delete(
                "Clear all MFA for this account (TOTP + passkeys)? This cannot be undone.",
                yes,
            )?;
            let totp_was = clear_totp_force(&username)?;
            let passkeys_removed = clear_all_passkeys(&username)?;
            let ceremonies_removed = clear_all_ceremonies()?;
            println!(
                "cleared mfa ok\ttotp_was_enabled={totp_was}\tpasskeys_removed={passkeys_removed}\tceremonies_removed={ceremonies_removed}"
            );
            Ok(())
        }
    }
}
