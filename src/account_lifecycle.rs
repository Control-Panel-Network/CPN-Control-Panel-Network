//! Deactivate / re-enable panel accounts and admin rename entry points.

use crate::account::{to_account_public, write_account_file};
use crate::account_mgmt::{find_account, list_accounts, rename_own_account};
use crate::model::AccountPublic;
use crate::packages::is_panel_admin;

/// True when the account exists and is marked disabled.
pub fn account_is_disabled(username_raw: &str) -> bool {
    find_account(username_raw)
        .map(|(boot, _)| boot.disabled)
        .unwrap_or(false)
}

/// Count active (not disabled) bootstrap admin accounts.
pub fn count_active_admins() -> usize {
    match crate::account::load_bootstrap() {
        Some(boot) if !boot.disabled => 1,
        _ => 0,
    }
}

/// Rename any panel account (reserved-name checks via create/rename path).
pub fn rename_account(
    current_username_raw: &str,
    new_username_raw: &str,
) -> Result<AccountPublic, String> {
    rename_own_account(current_username_raw, new_username_raw)
}

fn set_disabled(username_raw: &str, disabled: bool, force: bool) -> Result<AccountPublic, String> {
    let (mut boot, path) = find_account(username_raw)?;
    if boot.disabled == disabled {
        return Ok(to_account_public(&boot));
    }
    if disabled && is_panel_admin(&boot.username) {
        if count_active_admins() <= 1 && !force {
            return Err(
                "Cannot deactivate the last active panel admin (would lock everyone out). \
                 Re-run with --force if you understand the risk, or create/enable another admin first."
                    .into(),
            );
        }
        if force && count_active_admins() <= 1 {
            eprintln!(
                "warning: deactivating the last active panel admin; panel admin UI will be locked until an admin is re-enabled via CLI"
            );
        }
    }
    boot.disabled = disabled;
    write_account_file(&path, &boot)?;
    Ok(to_account_public(&boot))
}

/// Deactivate an account so it cannot log in. Existing sessions are rejected.
pub fn deactivate_account(username_raw: &str, force: bool) -> Result<AccountPublic, String> {
    set_disabled(username_raw, true, force)
}

/// Re-enable a previously deactivated account.
pub fn enable_account(username_raw: &str) -> Result<AccountPublic, String> {
    set_disabled(username_raw, false, false)
}

pub fn status_label(disabled: bool) -> &'static str {
    if disabled { "disabled" } else { "active" }
}

pub fn role_label(username: &str) -> &'static str {
    if is_panel_admin(username) {
        "admin"
    } else {
        "user"
    }
}

pub fn list_accounts_detailed() -> Result<Vec<AccountPublic>, String> {
    list_accounts()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::{default_password_policy, with_test_data_dir};
    use crate::account_mgmt::create_account;

    #[test]
    fn rename_rejects_reserved_and_updates_extra_file() {
        with_test_data_dir(|| {
            let policy = default_password_policy();
            create_account(
                "panelowner",
                None,
                true,
                "owner@example.com",
                policy.clone(),
                "en",
            )
            .unwrap();
            create_account("siteuser", None, true, "site@example.com", policy, "en").unwrap();
            assert!(rename_account("siteuser", "admin").is_err());
            let renamed = rename_account("siteuser", "siteops").unwrap();
            assert_eq!(renamed.username, "siteops");
            assert!(find_account("siteuser").is_err());
            assert!(find_account("siteops").is_ok());
        });
    }

    #[test]
    fn deactivate_blocks_last_admin_without_force() {
        with_test_data_dir(|| {
            let policy = default_password_policy();
            create_account(
                "panelowner",
                None,
                true,
                "owner@example.com",
                policy.clone(),
                "en",
            )
            .unwrap();
            create_account("siteuser", None, true, "site@example.com", policy, "en").unwrap();
            assert!(deactivate_account("panelowner", false).is_err());
            let user = deactivate_account("siteuser", false).unwrap();
            assert!(user.disabled);
            assert!(account_is_disabled("siteuser"));
            let enabled = enable_account("siteuser").unwrap();
            assert!(!enabled.disabled);
            let forced = deactivate_account("panelowner", true).unwrap();
            assert!(forced.disabled);
            assert_eq!(count_active_admins(), 0);
        });
    }
}
