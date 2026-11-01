//! Panel sub-users: secondary logins that manage the owner account with ACL-like caps.
//!
//! Distinct from Reseller Center (separate child hosting accounts with quotas).
//! Sub-users reuse sidebar visibility grants and site ACL (`all_owned_by` parent).

use crate::account::{default_password_policy, write_account_file};
use crate::account_mgmt::{create_account, delete_account, find_account, list_accounts, usernames_equal};
use crate::packages::is_panel_admin;
use crate::sidebar_visibility::{controllable_nav_items, set_grant_for_member};
use crate::site_acl::{SiteAclGrant, add_grant, list_grants, remove_grant_at};
use serde::{Deserialize, Serialize};

pub const ROLE_SUBUSER: &str = "subuser";

/// Nav section ids a sub-user may be granted (dashboard/docs stay available).
pub const GRANTABLE_NAV_IDS: &[&str] = &[
    "websites",
    "wordpress",
    "email",
    "databases",
    "ftp",
    "dns",
    "backups",
    "ssl",
    "plugins",
    "docker",
];

/// Always hidden for sub-users (owner management stays with the panel owner).
const FORCE_HIDDEN_NAV_IDS: &[&str] = &[
    "users",
    "packages",
    "server",
    "mariadb",
    "root-files",
    "php",
    "services",
    "panel-migrate",
    "system-repair",
    "processes",
    "logs",
    "package-manager",
    "litespeed",
    "security",
    "settings",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubUserPublic {
    pub username: String,
    pub recovery_email: String,
    pub parent_owner: String,
    pub disabled: bool,
    pub allowed_nav_ids: Vec<String>,
}

pub fn normalize_subuser_role(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        ROLE_SUBUSER => ROLE_SUBUSER.into(),
        _ => String::new(),
    }
}

pub fn account_is_subuser(username: &str) -> bool {
    if is_panel_admin(username) {
        return false;
    }
    find_account(username)
        .map(|(boot, _)| {
            normalize_subuser_role(&boot.role) == ROLE_SUBUSER
                || !boot.parent_owner.trim().is_empty()
        })
        .unwrap_or(false)
}

pub fn parent_owner_of(username: &str) -> Option<String> {
    let (boot, _) = find_account(username).ok()?;
    let parent = boot.parent_owner.trim();
    if parent.is_empty() {
        return None;
    }
    Some(parent.to_string())
}

/// Resource owner for sites and package metering: parent for sub-users, else self.
pub fn resource_owner_for(username: &str) -> String {
    parent_owner_of(username).unwrap_or_else(|| username.to_string())
}

pub fn can_manage_subusers(actor: &str) -> bool {
    is_panel_admin(actor)
}

pub fn grantable_nav_items() -> Vec<crate::sidebar_visibility::NavVisibilityItem> {
    controllable_nav_items()
        .into_iter()
        .filter(|item| GRANTABLE_NAV_IDS.contains(&item.id))
        .collect()
}

fn sanitize_allowed_nav_ids(raw: &[String]) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for id in raw {
        let trimmed = id.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !GRANTABLE_NAV_IDS.contains(&trimmed) {
            return Err(format!("Capability `{trimmed}` is not grantable to sub-users"));
        }
        if !out.iter().any(|x: &String| x == trimmed) {
            out.push(trimmed.to_string());
        }
    }
    Ok(out)
}

fn hidden_ids_for_allowed(allowed: &[String]) -> Vec<String> {
    let mut hidden = Vec::new();
    for item in controllable_nav_items() {
        let id = item.id;
        if id == "dashboard" || id == "docs" {
            continue;
        }
        let force = FORCE_HIDDEN_NAV_IDS.contains(&id);
        let allowed_here = allowed.iter().any(|a| a == id);
        if force || !allowed_here {
            hidden.push(id.to_string());
        }
    }
    hidden
}

fn allowed_from_hidden(hidden: &[String]) -> Vec<String> {
    GRANTABLE_NAV_IDS
        .iter()
        .filter(|id| !hidden.iter().any(|h| h == *id))
        .map(|id| (*id).to_string())
        .collect()
}

fn sync_sidebar_grant(member: &str, allowed: &[String]) -> Result<(), String> {
    let hidden = hidden_ids_for_allowed(allowed);
    set_grant_for_member(member, hidden, false)
}

fn remove_all_owned_by_grants(member: &str, parent: &str) -> Result<(), String> {
    loop {
        let grants = list_grants();
        let Some(index) = grants.iter().position(|g| {
            usernames_equal(&g.member, member) && usernames_equal(&g.all_owned_by, parent)
        }) else {
            return Ok(());
        };
        remove_grant_at(index)?;
    }
}

fn sync_site_acl_for_caps(member: &str, parent: &str, allowed: &[String]) -> Result<(), String> {
    remove_all_owned_by_grants(member, parent)?;
    let wants_sites = allowed.iter().any(|id| {
        matches!(
            id.as_str(),
            "websites" | "wordpress" | "plugins" | "ssl" | "backups"
        )
    });
    if !wants_sites {
        return Ok(());
    }
    add_grant(SiteAclGrant {
        member: member.to_string(),
        domain: String::new(),
        all_owned_by: parent.to_string(),
        can_install: true,
        can_uninstall: true,
        can_enable: true,
    })
}

fn set_subuser_fields(username: &str, parent_owner: &str) -> Result<(), String> {
    let (mut boot, path) = find_account(username)?;
    if is_panel_admin(&boot.username) {
        return Err("The panel owner cannot become a sub-user".into());
    }
    if !boot.parent_reseller.trim().is_empty() {
        return Err("Unassign this account from its reseller parent before making it a sub-user".into());
    }
    boot.role = ROLE_SUBUSER.to_string();
    boot.parent_owner = parent_owner.trim().to_string();
    boot.parent_reseller = String::new();
    write_account_file(&path, &boot)
}

fn current_allowed_nav_ids(username: &str) -> Vec<String> {
    let grants = crate::sidebar_visibility::list_grants();
    let Some(grant) = grants
        .into_iter()
        .find(|g| usernames_equal(&g.member, username))
    else {
        return Vec::new();
    };
    allowed_from_hidden(&grant.hidden_nav_ids)
}

pub fn list_subusers_for_owner(owner: &str) -> Result<Vec<SubUserPublic>, String> {
    let mut out = Vec::new();
    for acct in list_accounts()? {
        let Ok((boot, _)) = find_account(&acct.username) else {
            continue;
        };
        if !usernames_equal(&boot.parent_owner, owner) {
            continue;
        }
        if normalize_subuser_role(&boot.role) != ROLE_SUBUSER && boot.parent_owner.trim().is_empty()
        {
            continue;
        }
        out.push(SubUserPublic {
            username: boot.username.clone(),
            recovery_email: boot.recovery_email.clone(),
            parent_owner: boot.parent_owner.clone(),
            disabled: boot.disabled,
            allowed_nav_ids: current_allowed_nav_ids(&boot.username),
        });
    }
    out.sort_by_key(|u| u.username.to_ascii_lowercase());
    Ok(out)
}

pub fn create_subuser(
    owner: &str,
    username_raw: &str,
    recovery_email: &str,
    password_raw: Option<&str>,
    generate: bool,
    allowed_nav_ids: &[String],
) -> Result<(SubUserPublic, Option<String>), String> {
    if !can_manage_subusers(owner) {
        return Err("Only the panel owner can create sub-users".into());
    }
    let allowed = sanitize_allowed_nav_ids(allowed_nav_ids)?;
    if allowed.is_empty() {
        return Err("Select at least one capability for the sub-user".into());
    }
    let policy = default_password_policy();
    let created = create_account(
        username_raw,
        password_raw,
        generate || password_raw.map(str::trim).unwrap_or("").is_empty(),
        recovery_email,
        policy,
        "en",
    )?;
    let username = created.public.username.clone();
    if let Err(error) = set_subuser_fields(&username, owner) {
        let _ = delete_account(&username);
        return Err(error);
    }
    if let Err(error) = sync_sidebar_grant(&username, &allowed) {
        let _ = delete_account(&username);
        return Err(error);
    }
    if let Err(error) = sync_site_acl_for_caps(&username, owner, &allowed) {
        let _ = delete_account(&username);
        return Err(error);
    }
    Ok((
        SubUserPublic {
            username: username.clone(),
            recovery_email: created.public.recovery_email,
            parent_owner: owner.to_string(),
            disabled: false,
            allowed_nav_ids: allowed,
        },
        created.generated_password,
    ))
}

pub fn update_subuser_capabilities(
    owner: &str,
    username_raw: &str,
    allowed_nav_ids: &[String],
) -> Result<SubUserPublic, String> {
    if !can_manage_subusers(owner) {
        return Err("Only the panel owner can update sub-users".into());
    }
    let (boot, _) = find_account(username_raw)?;
    if !usernames_equal(&boot.parent_owner, owner) {
        return Err("That account is not a sub-user of this owner".into());
    }
    let allowed = sanitize_allowed_nav_ids(allowed_nav_ids)?;
    if allowed.is_empty() {
        return Err("Select at least one capability for the sub-user".into());
    }
    sync_sidebar_grant(&boot.username, &allowed)?;
    sync_site_acl_for_caps(&boot.username, owner, &allowed)?;
    Ok(SubUserPublic {
        username: boot.username,
        recovery_email: boot.recovery_email,
        parent_owner: owner.to_string(),
        disabled: boot.disabled,
        allowed_nav_ids: allowed,
    })
}

pub fn delete_subuser(owner: &str, username_raw: &str) -> Result<(), String> {
    if !can_manage_subusers(owner) {
        return Err("Only the panel owner can delete sub-users".into());
    }
    let (boot, _) = find_account(username_raw)?;
    if !usernames_equal(&boot.parent_owner, owner) {
        return Err("That account is not a sub-user of this owner".into());
    }
    let username = boot.username.clone();
    let _ = remove_all_owned_by_grants(&username, owner);
    // Drop sidebar grant if present.
    let grants = crate::sidebar_visibility::list_grants();
    if let Some(index) = grants
        .iter()
        .position(|g| usernames_equal(&g.member, &username))
    {
        let _ = crate::sidebar_visibility::remove_grant_at(index);
    }
    delete_account(&username)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::sites::create_site;

    #[test]
    fn create_list_update_delete_subuser_roundtrip() {
        with_test_data_dir(|| {
            let policy = default_password_policy();
            create_account(
                "cpnowner",
                Some("CpN-Owner-Pass-9x!"),
                false,
                "owner@example.com",
                policy.clone(),
                "en",
            )
            .unwrap();
            let allowed = vec!["websites".into(), "email".into()];
            let (created, generated) = create_subuser(
                "cpnowner",
                "helper1",
                "helper@example.com",
                None,
                true,
                &allowed,
            )
            .unwrap();
            assert!(generated.is_some());
            assert_eq!(created.parent_owner, "cpnowner");
            assert!(account_is_subuser("helper1"));
            assert_eq!(resource_owner_for("helper1"), "cpnowner");
            assert_eq!(list_subusers_for_owner("cpnowner").unwrap().len(), 1);

            let home = std::env::temp_dir().join(format!(
                "cpn-subusers-{}-{}",
                std::process::id(),
                crate::account::now_unix()
            ));
            let _ = std::fs::remove_dir_all(&home);
            std::fs::create_dir_all(&home).unwrap();
            unsafe {
                std::env::set_var("CPN_SITES_HOME", &home);
            }
            create_site("example.com", "cpnowner", None, None, None).unwrap();
            assert!(
                crate::site_acl::can_manage_site("helper1", "example.com", crate::site_acl::SitePerm::Install)
                    .unwrap()
            );
            assert!(crate::sidebar_visibility::path_allowed("helper1", "/websites"));
            assert!(crate::sidebar_visibility::path_allowed("helper1", "/email"));
            assert!(!crate::sidebar_visibility::path_allowed("helper1", "/docker"));
            assert!(!crate::sidebar_visibility::path_allowed(
                "helper1",
                "/account/users/create"
            ));

            update_subuser_capabilities("cpnowner", "helper1", &["email".into()]).unwrap();
            assert!(!crate::sidebar_visibility::path_allowed("helper1", "/websites"));
            assert!(crate::sidebar_visibility::path_allowed("helper1", "/email"));

            delete_subuser("cpnowner", "helper1").unwrap();
            assert!(find_account("helper1").is_err());
            unsafe {
                std::env::remove_var("CPN_SITES_HOME");
            }
            let _ = std::fs::remove_dir_all(&home);
        });
    }

    #[test]
    fn reject_empty_capabilities() {
        with_test_data_dir(|| {
            let policy = default_password_policy();
            create_account(
                "cpnowner",
                Some("CpN-Owner-Pass-9x!"),
                false,
                "owner@example.com",
                policy,
                "en",
            )
            .unwrap();
            let err = create_subuser(
                "cpnowner",
                "helper2",
                "h2@example.com",
                Some("CpN-Helper-Pass-9x!"),
                false,
                &[],
            )
            .unwrap_err();
            assert!(err.to_ascii_lowercase().contains("capability") || err.contains("at least"));
        });
    }
}
