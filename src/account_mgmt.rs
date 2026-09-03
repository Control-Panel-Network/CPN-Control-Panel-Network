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
    if let Some(boot) = load_bootstrap() {
        if usernames_equal(&boot.username, &username) {
            return Ok((boot, bootstrap_path()));
        }
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
    accounts.sort_by(|a, b| a.username.to_lowercase().cmp(&b.username.to_lowercase()));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::default_password_policy;
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn create_reset_delete_account_roundtrip() {
        let _guard = LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join(format!("cpn-account-cli-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        unsafe {
            std::env::set_var("CPN_DATA_DIR", &dir);
        }
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
        assert!(bootstrap_path().is_file());
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

        unsafe {
            std::env::remove_var("CPN_DATA_DIR");
        }
        let _ = fs::remove_dir_all(&dir);
    }
}
