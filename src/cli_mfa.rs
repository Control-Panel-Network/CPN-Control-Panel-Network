//! `cpn mfa` subcommands: status, enroll URL, and combined MFA clear (no secrets on stdout).

use crate::account_mfa::totp_status_for;
use crate::account_mfa_enroll_token::{
    MFA_ENROLL_TOKEN_TTL_SECS, create_mfa_enroll_token, has_pending_mfa_enroll_token,
};
use crate::account_mgmt::find_account;
use crate::account_passkeys::passkey_count;
use crate::account_security::needs_mfa_enrollment;
use crate::cli_panel::resolve_panel_url_info;
use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum MfaCommands {
    /// Show TOTP, passkey, and enrollment flags for an account (no secrets)
    Status {
        #[arg(long)]
        username: String,
    },
    /// Issue a short-lived HTTPS link for passkey/TOTP enrollment in a browser (root only)
    #[command(name = "enroll-url")]
    EnrollUrl {
        #[arg(long)]
        username: String,
    },
    /// Clear TOTP, all passkeys, and pending WebAuthn ceremonies (prompt, or --yes to skip)
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

pub fn run(
    command: MfaCommands,
    require_root: fn() -> Result<(), String>,
    confirm_delete: fn(&str, bool) -> Result<(), String>,
) -> Result<(), String> {
    match command {
        MfaCommands::Status { username } => {
            require_root()?;
            let username = resolve_username(&username)?;
            let (boot, _) = find_account(&username)?;
            let (totp_enabled, totp_pending) = totp_status_for(&username);
            let keys = passkey_count(&username);
            let needs_enroll = needs_mfa_enrollment(&username);
            let cli_token_pending = has_pending_mfa_enroll_token(&username);
            println!(
                "username={username}\ttotp_enabled={totp_enabled}\ttotp_pending_enroll={totp_pending}\tpasskey_count={keys}\ttotp_required={}\tneeds_mfa_enrollment={needs_enroll}\tcli_enroll_token_pending={cli_token_pending}",
                boot.totp_required
            );
            Ok(())
        }
        MfaCommands::EnrollUrl { username } => {
            require_root()?;
            let username = resolve_username(&username)?;
            if crate::account_lifecycle::account_is_disabled(&username) {
                return Err("Account is disabled; enable it before issuing an enroll link.".into());
            }
            let (raw, expires_at) = create_mfa_enroll_token(&username)?;
            let info = resolve_panel_url_info();
            let base = info.primary_base.trim_end_matches('/');
            let enroll_url = format!("{base}/account/security/enroll-cli?token={raw}");
            crate::panel_audit_cli::record_cli(
                "cli.mfa.enroll-url",
                &username,
                true,
                &format!("expires_at_unix={expires_at}; ttl_secs={MFA_ENROLL_TOKEN_TTL_SECS}"),
            );
            println!("username={username}");
            println!("enroll_url={enroll_url}");
            println!("expires_at_unix={expires_at}");
            println!("expires_in_secs={MFA_ENROLL_TOKEN_TTL_SECS}");
            println!(
                "note=Open enroll_url in a desktop browser (Windows Hello / passkeys need HTTPS and a matching RP ID hostname; loopback or http://IP may fail WebAuthn)"
            );
            if info.panel_public_url.is_none() && info.panel_hostname.is_none() {
                println!(
                    "warning=panel_public_url is unset; set `cpn network public-url` for reliable passkey RP ID outside local tests"
                );
            }
            Ok(())
        }
        MfaCommands::Clear { username, yes } => {
            require_root()?;
            let username = resolve_username(&username)?;
            confirm_delete(
                "Clear all MFA for this account (TOTP + passkeys)? This cannot be undone.",
                yes,
            )?;
            let totp_was = crate::account_mfa::clear_totp_force(&username)?;
            let passkeys_removed = crate::account_passkeys::clear_all_passkeys(&username)?;
            let ceremonies_removed = crate::panel_webauthn::clear_all_ceremonies()?;
            crate::panel_audit_cli::record_cli(
                "cli.mfa.clear",
                &username,
                true,
                &format!(
                    "totp_was_enabled={totp_was}; passkeys_removed={passkeys_removed}; ceremonies_removed={ceremonies_removed}"
                ),
            );
            println!(
                "cleared mfa ok\ttotp_was_enabled={totp_was}\tpasskeys_removed={passkeys_removed}\tceremonies_removed={ceremonies_removed}"
            );
            Ok(())
        }
    }
}
