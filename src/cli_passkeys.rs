//! `cpn passkey` subcommands: list / rename / remove / clear WebAuthn credentials (no secrets).

use crate::account_mgmt::find_account;
use crate::account_passkeys::{
    clear_all_passkeys, delete_passkey, format_passkey_timestamp, list_passkey_summaries,
    rename_passkey,
};
use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum PasskeyCommands {
    /// List registered passkeys for an account (id, label, type, timestamps only)
    List {
        #[arg(long)]
        username: String,
    },
    /// Rename a passkey label by credential id
    Rename {
        #[arg(long)]
        username: String,
        /// Credential id from `cpn passkey list` (never a secret material dump)
        #[arg(long)]
        id: String,
        #[arg(long)]
        label: String,
    },
    /// Remove one passkey by credential id
    Remove {
        #[arg(long)]
        username: String,
        /// Credential id from `cpn passkey list` (never a secret material dump)
        #[arg(long)]
        id: String,
    },
    /// Remove all passkeys for an account (prompt, or --yes to skip)
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
    command: PasskeyCommands,
    require_root: fn() -> Result<(), String>,
    confirm_delete: fn(&str, bool) -> Result<(), String>,
) -> Result<(), String> {
    match command {
        PasskeyCommands::List { username } => {
            require_root()?;
            let username = resolve_username(&username)?;
            let rows = list_passkey_summaries(&username);
            if rows.is_empty() {
                println!("(no passkeys)");
                return Ok(());
            }
            for row in rows {
                println!(
                    "id={}\tlabel={}\ttype={}\tcreated={}\tlast_used={}",
                    row.id,
                    row.label,
                    row.type_label,
                    format_passkey_timestamp(row.created_at_unix),
                    format_passkey_timestamp(row.last_used_unix),
                );
            }
            Ok(())
        }
        PasskeyCommands::Rename {
            username,
            id,
            label,
        } => {
            require_root()?;
            let username = resolve_username(&username)?;
            let id = id.trim();
            if id.is_empty() {
                return Err("Passkey --id is required".into());
            }
            rename_passkey(&username, id, &label)?;
            println!("renamed passkey ok");
            Ok(())
        }
        PasskeyCommands::Remove { username, id } => {
            require_root()?;
            let username = resolve_username(&username)?;
            let id = id.trim();
            if id.is_empty() {
                return Err("Passkey --id is required".into());
            }
            delete_passkey(&username, id)?;
            println!("removed passkey ok");
            Ok(())
        }
        PasskeyCommands::Clear { username, yes } => {
            require_root()?;
            let username = resolve_username(&username)?;
            confirm_delete(
                "Remove all passkeys for this account? This cannot be undone.",
                yes,
            )?;
            let removed = clear_all_passkeys(&username)?;
            println!("cleared passkeys ok\tremoved={removed}");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::{
        PanelBootstrap, default_password_policy, new_password_salt, persist_bootstrap,
        with_test_data_dir,
    };
    use crate::account_passkeys::{PasskeyStore, save_passkeys};

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
        };
        persist_bootstrap(&boot).expect("bootstrap");
    }

    #[test]
    fn clear_all_removes_store_file() {
        with_test_data_dir(|| {
            seed_account("admin");
            let store = PasskeyStore {
                schema_version: 1,
                username: "admin".into(),
                credentials: Vec::new(),
            };
            // Empty credentials still writes a file via save; clear should remove it.
            save_passkeys(&store).expect("save");
            assert!(
                crate::account_passkeys::load_passkeys("admin")
                    .credentials
                    .is_empty()
            );
            let removed = clear_all_passkeys("admin").expect("clear");
            assert_eq!(removed, 0);
            let path = crate::account::data_dir()
                .join("passkeys")
                .join("admin.json");
            assert!(!path.exists());
        });
    }

    #[test]
    fn list_resolves_account_username() {
        with_test_data_dir(|| {
            seed_account("admin");
            let username = resolve_username("Admin").expect("resolve");
            assert_eq!(username, "admin");
            let rows = list_passkey_summaries(&username);
            assert!(rows.is_empty());
        });
    }

    #[test]
    fn rename_missing_id_errors() {
        with_test_data_dir(|| {
            seed_account("admin");
            let err = run(
                PasskeyCommands::Rename {
                    username: "admin".into(),
                    id: "missing".into(),
                    label: "New label".into(),
                },
                || Ok(()),
                |_, _| Ok(()),
            )
            .unwrap_err();
            assert!(err.contains("not found"));
        });
    }
}
