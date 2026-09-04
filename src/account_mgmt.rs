//! Account create / list / passwd / delete for the operator CLI.

use std::{fs, path::PathBuf};

use crate::account::{
    AccountSetupResult, PanelBootstrap, accounts_dir, bootstrap_path, generate_password,
    hash_password, load_bootstrap, new_password_salt, now_unix, password_meets_policy,
    validate_policy, validate_recovery_email, write_account_file,
};
use crate::model::{AccountPublic, PasswordPolicy};

/// Require a non-empty username (CLI mutations; empty does not default to admin).
pub fn require_username(raw: &str) -> Result<String, String> {
    let username = raw.trim();
    if username.is_empty() {
        return Err("Username is required".into());
    }
    if username.chars().count() > 128 {
        return Err("Username cannot exceed 128 characters".into());
    }
    if username.chars().any(|ch| ch.is_control()) {
        return Err("Username cannot include control characters".into());
    }
    Ok(username.to_string())
}

fn account_file_key(username: &str) -> String {
    username
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn extra_account_path(username: &str) -> PathBuf {
    accounts_dir().join(format!("{}.json", account_file_key(username)))
}

fn load_account_file(path: &std::path::Path) -> Result<PanelBootstrap, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    serde_json::from_str(&raw)
        .map_err(|error| format!("Invalid account JSON in {}: {error}", path.display()))
}

fn usernames_equal(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Locate bootstrap or extra account by username.
pub fn find_account(username_raw: &str) -> Result<(PanelBootstrap, PathBuf), String> {
    let username = require_username(username_raw)?;
    if let Some(boot) = load_bootstrap()
        && usernames_equal(&boot.username, &username)
    {
        return Ok((boot, bootstrap_path()));
    }
    let path = extra_account_path(&username);
    if path.is_file() {
        let boot = load_account_file(&path)?;
        if usernames_equal(&boot.username, &username) {
            return Ok((boot, path));
        }
    }
    Err(format!("Account `{username}` not found"))
}

pub fn list_accounts() -> Result<Vec<AccountPublic>, String> {
    let mut accounts = Vec::new();
    if let Some(boot) = load_bootstrap() {
        accounts.push(AccountPublic {
            username: boot.username,
            recovery_email: boot.recovery_email,
            configured: true,
        });
    }
    let dir = accounts_dir();
    if dir.is_dir() {
        let entries = fs::read_dir(&dir)
            .map_err(|error| format!("Could not read {}: {error}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|error| format!("Could not read account entry: {error}"))?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let boot = load_account_file(&path)?;
            if accounts
                .iter()
                .any(|existing| usernames_equal(&existing.username, &boot.username))
            {
                continue;
            }
            accounts.push(AccountPublic {
                username: boot.username,
                recovery_email: boot.recovery_email,
                configured: true,
            });
        }
    }
    accounts.sort_by_key(|a| a.username.to_lowercase());
    Ok(accounts)
}

fn account_exists(username: &str) -> bool {
    find_account(username).is_ok()
}

fn build_bootstrap(
    username: String,
    recovery_email: String,
    password: &str,
    policy: PasswordPolicy,
    language: &str,
) -> PanelBootstrap {
    let salt = new_password_salt();
    let password_hash = hash_password(password, &salt);
    PanelBootstrap {
        schema_version: 1,
        username,
        recovery_email,
        password_hash,
        password_salt: salt,
        password_policy: policy,
        language: language.to_string(),
        created_at_unix: now_unix(),
    }
}

fn resolve_password(
    password_raw: Option<&str>,
    generate: bool,
    policy: &PasswordPolicy,
) -> Result<(String, Option<String>), String> {
    if generate {
        let value = generate_password(policy);
        return Ok((value.clone(), Some(value)));
    }
    let Some(password) = password_raw
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Err("Provide a password via --password-stdin or pass --generate".into());
    };
    password_meets_policy(password, policy)?;
    Ok((password.to_string(), None))
}

/// Create the panel bootstrap account or an extra account under `accounts/`.
pub fn create_account(
    username_raw: &str,
    password_raw: Option<&str>,
    generate: bool,
    recovery_email_raw: &str,
    policy: PasswordPolicy,
    language: &str,
) -> Result<AccountSetupResult, String> {
    validate_policy(&policy)?;
    let username = require_username(username_raw)?;
    if account_exists(&username) {
        return Err(format!("Account `{username}` already exists"));
    }
    let recovery_email = validate_recovery_email(recovery_email_raw).map_err(|err| {
        if err.contains("correo") || err.contains("Correo") || err.contains("Indica") {
            "Recovery email is required and must look like user@example.com".into()
        } else {
            err
        }
    })?;
    let (password, generated_password) = resolve_password(password_raw, generate, &policy)?;
    let boot = build_bootstrap(
        username.clone(),
        recovery_email.clone(),
        &password,
        policy,
        language,
    );
    let path = if load_bootstrap().is_none() {
        bootstrap_path()
    } else {
        extra_account_path(&username)
    };
    write_account_file(&path, &boot)?;
    Ok(AccountSetupResult {
        public: AccountPublic {
            username,
            recovery_email,
            configured: true,
        },
        generated_password,
    })
}

/// Reset / recover password for an existing account.
pub fn reset_account_password(
    username_raw: &str,
    password_raw: Option<&str>,
    generate: bool,
) -> Result<AccountSetupResult, String> {
    let (mut boot, path) = find_account(username_raw)?;
    validate_policy(&boot.password_policy)?;
    let (password, generated_password) =
        resolve_password(password_raw, generate, &boot.password_policy)?;
    let salt = new_password_salt();
    boot.password_salt = salt.clone();
    boot.password_hash = hash_password(&password, &salt);
    write_account_file(&path, &boot)?;
    Ok(AccountSetupResult {
        public: AccountPublic {
            username: boot.username,
            recovery_email: boot.recovery_email,
            configured: true,
        },
        generated_password,
    })
}

pub fn delete_account(username_raw: &str) -> Result<(), String> {
    let (_boot, path) = find_account(username_raw)?;
    fs::remove_file(&path)
        .map_err(|error| format!("Could not delete {}: {error}", path.display()))?;
    Ok(())
}

/// Change password for the signed-in account (requires current password).
pub fn change_own_password(
    username_raw: &str,
    current_password: &str,
    new_password_raw: Option<&str>,
    generate: bool,
) -> Result<AccountSetupResult, String> {
    use crate::account::verify_password;
    let (mut boot, path) = find_account(username_raw)?;
    if !verify_password(current_password, &boot.password_salt, &boot.password_hash) {
        return Err("Current password is incorrect".into());
    }
    validate_policy(&boot.password_policy)?;
    let (password, generated_password) = if generate {
        let value = generate_password(&boot.password_policy);
        (value.clone(), Some(value))
    } else {
        let Some(password) = new_password_raw
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Err("Enter a new password or enable generate".into());
        };
        password_meets_policy(password, &boot.password_policy)?;
        (password.to_string(), None)
    };
    let salt = new_password_salt();
    boot.password_salt = salt.clone();
    boot.password_hash = hash_password(&password, &salt);
    write_account_file(&path, &boot)?;
    Ok(AccountSetupResult {
        public: AccountPublic {
            username: boot.username,
            recovery_email: boot.recovery_email,
            configured: true,
        },
        generated_password,
    })
}

/// Update recovery email and/or language for the signed-in account.
pub fn update_own_profile(
    username_raw: &str,
    recovery_email_raw: Option<&str>,
    language_raw: Option<&str>,
) -> Result<AccountPublic, String> {
    let (mut boot, path) = find_account(username_raw)?;
    if let Some(email_raw) = recovery_email_raw {
        boot.recovery_email = validate_recovery_email(email_raw).map_err(|err| {
            if err.contains("correo") || err.contains("Correo") || err.contains("Indica") {
                "Recovery email is required and must look like user@example.com".into()
            } else {
                err
            }
        })?;
    }
    if let Some(lang_raw) = language_raw {
        boot.language = crate::http_helpers::normalize_language(lang_raw)?;
    }
    write_account_file(&path, &boot)?;
    Ok(AccountPublic {
        username: boot.username,
        recovery_email: boot.recovery_email,
        configured: true,
    })
}

/// Rename the signed-in account. Caller must re-issue the session cookie.
pub fn rename_own_account(
    current_username_raw: &str,
    new_username_raw: &str,
) -> Result<AccountPublic, String> {
    let (mut boot, old_path) = find_account(current_username_raw)?;
    let new_username = require_username(new_username_raw)?;
    if usernames_equal(&boot.username, &new_username) {
        return Ok(AccountPublic {
            username: boot.username,
            recovery_email: boot.recovery_email,
            configured: true,
        });
    }
    if account_exists(&new_username) {
        return Err(format!("Account `{new_username}` already exists"));
    }
    boot.username = new_username.clone();
    let is_bootstrap = old_path == bootstrap_path();
    if is_bootstrap {
        write_account_file(&old_path, &boot)?;
    } else {
        let new_path = extra_account_path(&new_username);
        write_account_file(&new_path, &boot)?;
        if new_path != old_path {
            let _ = fs::remove_file(&old_path);
        }
    }
    // Best-effort MFA file rename (encrypted secrets stay valid under new name).
    let old_mfa = crate::account_mfa::load_mfa(current_username_raw);
    if old_mfa.totp_enabled || !old_mfa.totp_secret_enc.is_empty() {
        let mut moved = old_mfa;
        moved.username = new_username.clone();
        moved.updated_at_unix = now_unix();
        let _ = crate::account_mfa::save_mfa_for_rename(&moved, current_username_raw);
    }
    let _ = crate::account_passkeys::rename_passkey_store(current_username_raw, &new_username);
    Ok(AccountPublic {
        username: boot.username,
        recovery_email: boot.recovery_email,
        configured: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::{default_password_policy, with_test_data_dir};

    #[test]
    fn create_reset_delete_account_roundtrip() {
        with_test_data_dir(|| {
            let policy = default_password_policy();
            let created = create_account(
                "admin",
                None,
                true,
                "admin@example.com",
                policy.clone(),
                "en",
            )
            .expect("create bootstrap");
            assert!(
                bootstrap_path().is_file(),
                "bootstrap missing at {}",
                bootstrap_path().display()
            );
            assert!(created.generated_password.is_some());

            let second = create_account("ops", None, true, "ops@example.com", policy, "en")
                .expect("create extra");
            assert!(extra_account_path("ops").is_file());
            assert_eq!(list_accounts().unwrap().len(), 2);

            let reset = reset_account_password("ops", None, true).expect("reset");
            assert!(reset.generated_password.is_some());
            assert_ne!(reset.generated_password, second.generated_password);

            delete_account("ops").unwrap();
            assert_eq!(list_accounts().unwrap().len(), 1);
            delete_account("admin").unwrap();
            assert!(list_accounts().unwrap().is_empty());
        });
    }

    #[test]
    fn self_service_password_and_email() {
        with_test_data_dir(|| {
            let policy = default_password_policy();
            let created = create_account(
                "Admin",
                Some("AdminPass1!"),
                false,
                "admin@example.com",
                policy,
                "en",
            )
            .expect("create");
            assert!(created.generated_password.is_none());
            update_own_profile("Admin", Some("ops@example.com"), Some("nb")).unwrap();
            let (boot, _) = find_account("Admin").unwrap();
            assert_eq!(boot.recovery_email, "ops@example.com");
            assert_eq!(boot.language, "nb");
            let changed =
                change_own_password("Admin", "AdminPass1!", Some("NewPass2!"), false).unwrap();
            assert!(changed.generated_password.is_none());
            assert!(change_own_password("Admin", "wrong", Some("NewPass3!"), false).is_err());
        });
    }
}
