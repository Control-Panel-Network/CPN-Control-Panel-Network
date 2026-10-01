//! Best-effort Users / ACL / Packages import from classic source-panel backups.
//!
//! Classic `meta.xml` website archives often embed the website owner account,
//! an ACL role name, and a websites-limit hint. They do not ship a full panel
//! user directory or hosting-package catalog. Optional `panel-config/*.json`
//! payloads (when present) are merged as well.
//!
//! Never writes under `/var/lib/cpn/mfa/` (or `$CPN_DATA_DIR/mfa/`).

use crate::backup_restore_accounts_apply::{
    apply_acl_for_user, apply_package_hint, ensure_imported_account, merge_optional_packages_json,
    merge_optional_site_acl_json, merge_optional_users_json, remap_reserved_username,
};
use std::fs;
use std::path::{Path, PathBuf};

/// Parsed classic meta.xml owner + limit hints.
#[derive(Debug, Clone, Default)]
pub struct ClassicMetaAccounts {
    pub master_domain: String,
    pub username: String,
    pub email: String,
    pub acl_name: String,
    pub websites_limit: Option<i64>,
    pub state_active: bool,
    /// Source password hash (often bcrypt). Not portable to CPN PBKDF2.
    pub source_password_hash: String,
}

#[derive(Debug, Clone, Default)]
pub struct AccountsRestoreOpts {
    pub restore_users: bool,
    pub restore_acl: bool,
    pub restore_packages: bool,
}

fn tag_text(raw: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = raw.find(&open)? + open.len();
    let end = raw[start..].find(&close)? + start;
    let value = raw[start..end].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

/// Locate `meta.xml` under staging (top-level or one nested backup-* dir).
pub fn find_meta_xml(staging: &Path) -> Option<PathBuf> {
    let direct = staging.join("meta.xml");
    if direct.is_file() {
        return Some(direct);
    }
    let Ok(rd) = fs::read_dir(staging) else {
        return None;
    };
    for ent in rd.flatten() {
        let path = ent.path();
        if path.is_dir() {
            let cand = path.join("meta.xml");
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

/// Parse classic source-panel `meta.xml` account / ACL / limit fields.
pub fn parse_classic_meta_xml(path: &Path) -> Result<ClassicMetaAccounts, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("read meta.xml: {e}"))?;
    let username = tag_text(&raw, "userName").unwrap_or_default();
    let email = tag_text(&raw, "email").unwrap_or_default();
    let acl_name = tag_text(&raw, "aclName").unwrap_or_default();
    let master_domain = tag_text(&raw, "masterDomain").unwrap_or_default();
    let pw = tag_text(&raw, "userPassword").unwrap_or_default();
    let state = tag_text(&raw, "state")
        .unwrap_or_else(|| "ACTIVE".into())
        .eq_ignore_ascii_case("ACTIVE");
    let websites_limit = tag_text(&raw, "initWebsitesLimit").and_then(|s| s.parse::<i64>().ok());
    Ok(ClassicMetaAccounts {
        master_domain,
        username,
        email,
        acl_name,
        websites_limit,
        state_active: state,
        source_password_hash: pw,
    })
}

/// True when member paths suggest users / ACL / packages payloads exist.
pub fn members_suggest_accounts_payload(members: &[String]) -> (bool, bool, bool) {
    let mut has_meta = false;
    let mut has_users_json = false;
    let mut has_packages_json = false;
    let mut has_acl_json = false;
    for m in members {
        let n = m
            .trim()
            .trim_start_matches("./")
            .replace('\\', "/")
            .to_ascii_lowercase();
        if n == "meta.xml" || n.ends_with("/meta.xml") {
            has_meta = true;
        }
        if n.ends_with("users.json") || n.ends_with("accounts.json") {
            has_users_json = true;
        }
        if n.ends_with("packages.json") || n.ends_with("package-assignments.json") {
            has_packages_json = true;
        }
        if n.ends_with("site-acl.json") || n.ends_with("acl.json") {
            has_acl_json = true;
        }
    }
    let users = has_meta || has_users_json;
    let acl = has_meta || has_acl_json;
    let packages = has_meta || has_packages_json;
    (users, acl, packages)
}

/// Apply Users / ACL / Packages from staging when selected.
///
/// Does **not** touch MFA material under `mfa/`.
pub fn restore_accounts_from_staging(
    staging: &Path,
    opts: &AccountsRestoreOpts,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    if !opts.restore_users && !opts.restore_acl && !opts.restore_packages {
        return Ok(());
    }

    let meta = find_meta_xml(staging).and_then(|p| parse_classic_meta_xml(&p).ok());
    let mut imported_user = String::new();

    if opts.restore_users {
        if let Some(ref m) = meta
            && !m.username.trim().is_empty()
        {
            match ensure_imported_account(&m.username, &m.email, !m.state_active, warnings) {
                Ok(u) => imported_user = u,
                Err(e) => warnings.push(format!("Users import from meta.xml: {e}")),
            }
            if !m.source_password_hash.is_empty() {
                warnings.push(
                    "Source meta.xml password hash was detected but not applied (CPN uses PBKDF2; source panels often use bcrypt)."
                        .into(),
                );
            }
        } else {
            warnings.push(
                "No classic meta.xml owner user found; checking optional users.json only.".into(),
            );
        }
        merge_optional_users_json(staging, warnings)?;
    }

    if opts.restore_acl {
        if imported_user.is_empty()
            && let Some(ref m) = meta
            && !m.username.trim().is_empty()
        {
            imported_user = remap_reserved_username(&m.username);
        }
        if !imported_user.is_empty() {
            if let Some(ref m) = meta {
                apply_acl_for_user(&imported_user, &m.acl_name, &m.master_domain, warnings)?;
            }
        } else {
            warnings.push("ACL selected but no importable username; skipped meta ACL map.".into());
        }
        merge_optional_site_acl_json(staging, warnings)?;
    }

    if opts.restore_packages {
        if imported_user.is_empty()
            && let Some(ref m) = meta
            && !m.username.trim().is_empty()
        {
            imported_user = remap_reserved_username(&m.username);
        }
        if !imported_user.is_empty() {
            if let Some(ref m) = meta {
                apply_package_hint(&imported_user, &m.master_domain, m.websites_limit, warnings)?;
            }
        } else {
            warnings.push(
                "Packages selected but no importable username; skipped meta package assignment."
                    .into(),
            );
        }
        merge_optional_packages_json(staging, warnings)?;
    }

    warnings.push(
        "Users / ACL / Packages restore is best-effort and is not 1:1 with source control-panel parity. MFA keys under /var/lib/cpn/mfa were not modified."
            .into(),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::now_unix;
    use std::io::Write;

    #[test]
    fn parse_meta_extracts_owner_acl_limit() {
        let dir = std::env::temp_dir().join(format!(
            "cpn-meta-parse-{}-{}",
            std::process::id(),
            now_unix()
        ));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("meta.xml");
        let mut f = fs::File::create(&path).unwrap();
        writeln!(
            f,
            r#"<?xml version="1.0"?><metaFile>
            <masterDomain>example.com</masterDomain>
            <userName>Admin</userName>
            <userPassword>$2b$12$abc</userPassword>
            <email>ops@example.com</email>
            <aclName>admin</aclName>
            <initWebsitesLimit>5</initWebsitesLimit>
            <state>ACTIVE</state>
            </metaFile>"#
        )
        .unwrap();
        let parsed = parse_classic_meta_xml(&path).unwrap();
        assert_eq!(parsed.username, "Admin");
        assert_eq!(parsed.acl_name, "admin");
        assert_eq!(parsed.websites_limit, Some(5));
        assert_eq!(parsed.master_domain, "example.com");
        assert!(parsed.state_active);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn members_flag_meta_as_accounts_payload() {
        let (u, a, p) = members_suggest_accounts_payload(&[
            "./meta.xml".into(),
            "./public_html/index.html".into(),
        ]);
        assert!(u && a && p);
        let (u2, a2, p2) = members_suggest_accounts_payload(&["./public_html/index.html".into()]);
        assert!(!u2 && !a2 && !p2);
    }

    #[test]
    fn reserved_admin_remaps() {
        assert_eq!(remap_reserved_username("Admin"), "imported-admin");
        assert_eq!(remap_reserved_username("cpnowner"), "cpnowner");
    }
}
