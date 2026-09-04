//! Domain/subdomain management ACL for panel accounts and team members.
//!
//! Filesystem paths stay under `/home/<domain>/...` (nested for subdomains).
//! This module only answers who may manage a given site FQDN.

use crate::account::data_dir;
use crate::sites::{SiteRecord, list_sites, load_site};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SitePerm {
    Install,
    Uninstall,
    Enable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteAclGrant {
    /// Panel account username that receives the grant.
    pub member: String,
    /// Exact domain/subdomain FQDN. Empty when `all_owned_by` is set.
    #[serde(default)]
    pub domain: String,
    /// When set, member may manage every site whose `owner` matches this account.
    #[serde(default)]
    pub all_owned_by: String,
    #[serde(default = "default_true")]
    pub can_install: bool,
    #[serde(default = "default_true")]
    pub can_uninstall: bool,
    #[serde(default = "default_true")]
    pub can_enable: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SiteAclFile {
    #[serde(default)]
    schema_version: u32,
    #[serde(default)]
    grants: Vec<SiteAclGrant>,
}

fn acl_path() -> PathBuf {
    data_dir().join("site-acl.json")
}

fn load_acl() -> SiteAclFile {
    let path = acl_path();
    let Ok(raw) = fs::read_to_string(&path) else {
        return SiteAclFile {
            schema_version: SCHEMA_VERSION,
            grants: Vec::new(),
        };
    };
    serde_json::from_str(&raw).unwrap_or(SiteAclFile {
        schema_version: SCHEMA_VERSION,
        grants: Vec::new(),
    })
}

fn save_acl(file: &SiteAclFile) -> Result<(), String> {
    let path = acl_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create ACL dir: {e}"))?;
    }
    let mut out = file.clone();
    out.schema_version = SCHEMA_VERSION;
    let raw = serde_json::to_string_pretty(&out)
        .map_err(|e| format!("Could not serialize site ACL: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("Could not write site ACL: {e}"))
}

/// List all site ACL grants (team permission profiles).
pub fn list_grants() -> Vec<SiteAclGrant> {
    load_acl().grants
}

/// Add a site ACL grant for a panel account.
pub fn add_grant(grant: SiteAclGrant) -> Result<(), String> {
    let member = grant.member.trim();
    if member.is_empty() {
        return Err("Member username is required".into());
    }
    let domain = grant.domain.trim();
    let all_owned_by = grant.all_owned_by.trim();
    if domain.is_empty() && all_owned_by.is_empty() {
        return Err("Provide a domain FQDN or an all-owned-by account".into());
    }
    if !domain.is_empty() && !all_owned_by.is_empty() {
        return Err("Set either domain or all-owned-by, not both".into());
    }
    let mut file = load_acl();
    file.grants.push(SiteAclGrant {
        member: member.to_string(),
        domain: domain.to_string(),
        all_owned_by: all_owned_by.to_string(),
        can_install: grant.can_install,
        can_uninstall: grant.can_uninstall,
        can_enable: grant.can_enable,
    });
    save_acl(&file)
}

/// Remove a grant by zero-based index in [`list_grants`] order.
pub fn remove_grant_at(index: usize) -> Result<(), String> {
    let mut file = load_acl();
    if index >= file.grants.len() {
        return Err("ACL grant not found".into());
    }
    file.grants.remove(index);
    save_acl(&file)
}

fn names_equal(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

fn grant_allows(grant: &SiteAclGrant, perm: SitePerm) -> bool {
    match perm {
        SitePerm::Install => grant.can_install,
        SitePerm::Uninstall => grant.can_uninstall,
        SitePerm::Enable => grant.can_enable,
    }
}

/// True when the session user owns the site or holds a matching team grant.
pub fn can_manage_site(username: &str, domain_raw: &str, perm: SitePerm) -> Result<bool, String> {
    let site = load_site(domain_raw)?;
    if names_equal(&site.owner, username) {
        return Ok(true);
    }
    let acl = load_acl();
    for grant in &acl.grants {
        if !names_equal(&grant.member, username) {
            continue;
        }
        if !grant_allows(grant, perm) {
            continue;
        }
        if !grant.domain.trim().is_empty() && names_equal(&grant.domain, &site.domain) {
            return Ok(true);
        }
        if !grant.all_owned_by.trim().is_empty() && names_equal(&grant.all_owned_by, &site.owner) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn require_manage_site(
    username: &str,
    domain_raw: &str,
    perm: SitePerm,
) -> Result<SiteRecord, String> {
    let site = load_site(domain_raw)?;
    if can_manage_site(username, &site.domain, perm)? {
        return Ok(site);
    }
    Err(format!(
        "Account `{username}` is not allowed to manage plugins/apps for `{}`",
        site.domain
    ))
}

/// Sites the session user may manage (owner or grant).
pub fn sites_manageable_by(username: &str) -> Result<Vec<SiteRecord>, String> {
    let all = list_sites()?;
    let mut out = Vec::new();
    for site in all {
        if can_manage_site(username, &site.domain, SitePerm::Install)?
            || can_manage_site(username, &site.domain, SitePerm::Uninstall)?
            || can_manage_site(username, &site.domain, SitePerm::Enable)?
        {
            out.push(site);
        }
    }
    Ok(out)
}

/// Resolve `--domain` / `--subdomain` / full FQDN into a registered site domain.
pub fn resolve_target_domain(
    domain: Option<&str>,
    subdomain: Option<&str>,
) -> Result<String, String> {
    let domain = domain.map(str::trim).filter(|v| !v.is_empty());
    let subdomain = subdomain.map(str::trim).filter(|v| !v.is_empty());
    match (domain, subdomain) {
        (Some(d), None) => {
            let site = load_site(d)?;
            Ok(site.domain)
        }
        (Some(parent), Some(sub)) => {
            let fqdn = if sub.contains('.') {
                sub.to_ascii_lowercase()
            } else {
                format!(
                    "{}.{}",
                    sub.to_ascii_lowercase(),
                    parent.to_ascii_lowercase()
                )
            };
            let site = load_site(&fqdn)?;
            Ok(site.domain)
        }
        (None, Some(sub)) => {
            let site = load_site(sub)?;
            Ok(site.domain)
        }
        (None, None) => Err("Provide --domain and/or --subdomain (or a full FQDN)".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::{now_unix, with_test_data_dir};
    use crate::sites::create_site;

    #[test]
    fn owner_can_manage_without_grants() {
        with_test_data_dir(|| {
            let home =
                std::env::temp_dir().join(format!("cpn-acl-{}-{}", std::process::id(), now_unix()));
            let _ = fs::remove_dir_all(&home);
            fs::create_dir_all(&home).unwrap();
            unsafe {
                std::env::set_var("CPN_SITES_HOME", &home);
            }
            create_site("example.com", "Admin", None, None, None).unwrap();
            assert!(can_manage_site("admin", "example.com", SitePerm::Install).unwrap());
            assert!(!can_manage_site("ops", "example.com", SitePerm::Install).unwrap());
            unsafe {
                std::env::remove_var("CPN_SITES_HOME");
            }
            let _ = fs::remove_dir_all(&home);
        });
    }

    #[test]
    fn add_and_remove_grant_roundtrip() {
        with_test_data_dir(|| {
            assert!(list_grants().is_empty());
            add_grant(SiteAclGrant {
                member: "ops".into(),
                domain: "example.com".into(),
                all_owned_by: String::new(),
                can_install: true,
                can_uninstall: false,
                can_enable: true,
            })
            .unwrap();
            assert_eq!(list_grants().len(), 1);
            assert_eq!(list_grants()[0].member, "ops");
            remove_grant_at(0).unwrap();
            assert!(list_grants().is_empty());
            assert!(remove_grant_at(0).is_err());
        });
    }
}
