//! Apply helpers for Users / ACL / Packages restore (classic source-panel).
//!
//! Called from `backup_restore_accounts`. Never writes MFA material.

use crate::account::{
    PanelBootstrap, accounts_dir, bootstrap_path, default_password_policy, generate_password,
    hash_password, load_bootstrap, new_password_salt, now_unix, write_account_file,
};
use crate::account_mgmt::{find_account, require_new_username, require_username};
use crate::packages::{
    PackageInput, UNLIMITED, assign_package, create_package, create_package_for, list_packages,
};
use crate::reserved_usernames::is_reserved_username;
use crate::site_acl::{SiteAclGrant, add_grant, list_grants};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn remap_reserved_username(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if is_reserved_username(trimmed) {
        let safe: String = trimmed
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect();
        return format!("imported-{safe}");
    }
    trimmed.to_string()
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

pub(crate) fn ensure_imported_account(
    desired_username: &str,
    email: &str,
    disabled: bool,
    warnings: &mut Vec<String>,
) -> Result<String, String> {
    let remapped = remap_reserved_username(desired_username);
    if remapped.is_empty() {
        return Err("Source archive has no importable username in meta.xml.".into());
    }
    if remapped != desired_username.trim() {
        warnings.push(format!(
            "Source username `{desired}` is reserved in CPN; importing as `{remapped}` (password reset required).",
            desired = desired_username.trim()
        ));
    }
    if find_account(&remapped).is_ok() {
        warnings.push(format!(
            "Account `{remapped}` already exists; skipped recreate (password and MFA unchanged)."
        ));
        return Ok(remapped);
    }
    let username = match require_new_username(&remapped) {
        Ok(u) => u,
        Err(e) => {
            warnings.push(format!(
                "Could not create `{remapped}` as new username: {e}"
            ));
            require_username(&remapped)?
        }
    };
    if find_account(&username).is_ok() {
        warnings.push(format!(
            "Account `{username}` already exists; skipped recreate."
        ));
        return Ok(username);
    }
    let policy = default_password_policy();
    let password = generate_password(&policy);
    let salt = new_password_salt();
    let password_hash = hash_password(&password, &salt);
    let recovery = if email.trim().contains('@') {
        email.trim().to_string()
    } else {
        format!("{username}@localhost.localdomain")
    };
    let boot = PanelBootstrap {
        schema_version: 1,
        username: username.clone(),
        recovery_email: recovery,
        password_hash,
        password_salt: salt,
        password_policy: policy,
        language: "en".into(),
        created_at_unix: now_unix(),
        must_change_password: true,
        totp_required: true,
        disabled,
    };
    let path = if load_bootstrap().is_none() {
        bootstrap_path()
    } else {
        accounts_dir().join(format!("{}.json", account_file_key(&username)))
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("accounts dir: {e}"))?;
    }
    write_account_file(&path, &boot)?;
    warnings.push(format!(
        "Created account `{username}` with a generated temporary password (must change on next login). Source bcrypt/panel hashes are not portable to CPN PBKDF2."
    ));
    let _ = password;
    Ok(username)
}

pub(crate) fn apply_acl_for_user(
    username: &str,
    acl_name: &str,
    master_domain: &str,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let role = acl_name.trim().to_ascii_lowercase();
    let existing = list_grants();
    let is_adminish = role.is_empty() || role == "admin" || role == "administrator";
    let grant = if is_adminish {
        SiteAclGrant {
            member: username.to_string(),
            domain: String::new(),
            all_owned_by: username.to_string(),
            can_install: true,
            can_uninstall: true,
            can_enable: true,
        }
    } else if !master_domain.trim().is_empty() {
        SiteAclGrant {
            member: username.to_string(),
            domain: master_domain.trim().to_ascii_lowercase(),
            all_owned_by: String::new(),
            can_install: true,
            can_uninstall: false,
            can_enable: true,
        }
    } else {
        warnings.push(format!(
            "ACL role `{acl_name}` had no masterDomain; skipped site ACL grant."
        ));
        return Ok(());
    };
    let dup = existing.iter().any(|g| {
        g.member.eq_ignore_ascii_case(&grant.member)
            && g.domain.eq_ignore_ascii_case(&grant.domain)
            && g.all_owned_by.eq_ignore_ascii_case(&grant.all_owned_by)
    });
    if dup {
        warnings.push(format!(
            "Site ACL grant for `{username}` already present; skipped duplicate."
        ));
        return Ok(());
    }
    add_grant(grant)?;
    warnings.push(format!(
        "Mapped source ACL `{acl_name}` to a CPN site ACL grant for `{username}` (not 1:1 source-panel ACL parity)."
    ));
    Ok(())
}

pub(crate) fn apply_package_hint(
    username: &str,
    master_domain: &str,
    websites_limit: Option<i64>,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let pkg_name = if master_domain.trim().is_empty() {
        format!("imported-{username}")
    } else {
        format!("imported-{}", master_domain.trim().to_ascii_lowercase())
    };
    let domains = match websites_limit {
        Some(n) if n > 0 => n,
        Some(0) | None => UNLIMITED,
        Some(n) => n,
    };
    let input = PackageInput {
        name: pkg_name.clone(),
        disk_mb: UNLIMITED,
        bandwidth_mb: UNLIMITED,
        domains,
        emails: UNLIMITED,
        databases: UNLIMITED,
        ftp_accounts: UNLIMITED,
        fqdn_enabled: true,
        notes: "Best-effort import from classic source-panel meta.xml (websites limit only)."
            .into(),
        sidebar_hidden_nav_ids: Vec::new(),
                ..Default::default()
            };
    let pkg = match create_package_for(username, input) {
        Ok(p) => p,
        Err(e) if e.contains("already exists") => {
            let existing = list_packages()?
                .into_iter()
                .find(|p| {
                    p.name.eq_ignore_ascii_case(&pkg_name)
                        || p.name
                            .eq_ignore_ascii_case(&format!("{username}_{pkg_name}"))
                })
                .ok_or(e)?;
            warnings.push(format!(
                "Package `{}` already exists; reusing id `{}`.",
                existing.name, existing.id
            ));
            existing
        }
        Err(e) => return Err(e),
    };
    assign_package(username, &pkg.id)?;
    let limit_label = if crate::packages::is_unlimited(pkg.domains) {
        "∞".to_string()
    } else {
        pkg.domains.to_string()
    };
    warnings.push(format!(
        "Assigned package `{}` (domains limit {}) to `{username}`. Full source hosting-package catalogs are not present in typical website archives.",
        pkg.name, limit_label
    ));
    Ok(())
}

#[derive(Debug, Deserialize)]
struct ImportedUserRow {
    username: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    disabled: bool,
}

#[derive(Debug, Deserialize)]
struct ImportedUsersFile {
    #[serde(default)]
    users: Vec<ImportedUserRow>,
    #[serde(default)]
    accounts: Vec<ImportedUserRow>,
}

fn find_named_json(staging: &Path, names: &[&str]) -> Option<PathBuf> {
    let mut stack = vec![staging.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else {
            continue;
        };
        for ent in rd.flatten() {
            let path = ent.path();
            if path.is_dir() {
                if path.components().count() < staging.components().count() + 5 {
                    stack.push(path);
                }
                continue;
            }
            let fname = ent.file_name().to_string_lossy().to_ascii_lowercase();
            if names.iter().any(|n| fname == *n) {
                return Some(path);
            }
        }
    }
    None
}

pub(crate) fn merge_optional_users_json(
    staging: &Path,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let Some(path) = find_named_json(staging, &["users.json", "accounts.json"]) else {
        return Ok(());
    };
    let raw = fs::read_to_string(&path).map_err(|e| format!("read users json: {e}"))?;
    let parsed: ImportedUsersFile =
        serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", path.display()))?;
    let rows = if !parsed.users.is_empty() {
        parsed.users
    } else {
        parsed.accounts
    };
    for row in rows {
        match ensure_imported_account(&row.username, &row.email, row.disabled, warnings) {
            Ok(_) => {}
            Err(e) => warnings.push(format!("Skipped user `{}`: {e}", row.username)),
        }
    }
    warnings.push(format!(
        "Merged optional users file `{}` (best-effort).",
        path.display()
    ));
    Ok(())
}

pub(crate) fn merge_optional_packages_json(
    staging: &Path,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let Some(path) = find_named_json(staging, &["packages.json"]) else {
        return Ok(());
    };
    #[derive(Deserialize)]
    struct PkgFile {
        #[serde(default)]
        packages: Vec<PackageInputRow>,
    }
    #[derive(Deserialize)]
    struct PackageInputRow {
        name: String,
        #[serde(default = "unlimited_default")]
        disk_mb: i64,
        #[serde(default = "unlimited_default")]
        bandwidth_mb: i64,
        #[serde(default = "unlimited_default")]
        domains: i64,
        #[serde(default = "unlimited_default")]
        emails: i64,
        #[serde(default = "unlimited_default")]
        databases: i64,
        #[serde(default = "unlimited_default")]
        ftp_accounts: i64,
        #[serde(default)]
        fqdn_enabled: bool,
        #[serde(default)]
        notes: String,
    }
    fn unlimited_default() -> i64 {
        UNLIMITED
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("read packages json: {e}"))?;
    let parsed: PkgFile =
        serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", path.display()))?;
    for row in parsed.packages {
        let input = PackageInput {
            name: row.name.clone(),
            disk_mb: row.disk_mb,
            bandwidth_mb: row.bandwidth_mb,
            domains: row.domains,
            emails: row.emails,
            databases: row.databases,
            ftp_accounts: row.ftp_accounts,
            fqdn_enabled: row.fqdn_enabled,
            notes: row.notes,
            sidebar_hidden_nav_ids: Vec::new(),
                ..Default::default()
            };
        match create_package(input) {
            Ok(p) => warnings.push(format!("Imported package `{}` from archive.", p.name)),
            Err(e) if e.contains("already exists") => {
                warnings.push(format!("Package `{}` already exists; skipped.", row.name))
            }
            Err(e) => warnings.push(format!("Package `{}`: {e}", row.name)),
        }
    }
    Ok(())
}

pub(crate) fn merge_optional_site_acl_json(
    staging: &Path,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let Some(path) = find_named_json(staging, &["site-acl.json", "acl.json"]) else {
        return Ok(());
    };
    #[derive(Deserialize)]
    struct AclFile {
        #[serde(default)]
        grants: Vec<SiteAclGrant>,
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("read acl json: {e}"))?;
    let parsed: AclFile =
        serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", path.display()))?;
    for grant in parsed.grants {
        match add_grant(grant) {
            Ok(()) => {}
            Err(e) => warnings.push(format!("ACL grant skipped: {e}")),
        }
    }
    warnings.push(format!(
        "Merged optional ACL file `{}` (best-effort).",
        path.display()
    ));
    Ok(())
}
