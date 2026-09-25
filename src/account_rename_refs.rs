//! Best-effort username reference updates after account rename.

use crate::account::data_dir;
use crate::sites::{SiteModify, list_sites, modify_site};
use std::fs;

fn names_equal(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Rewrite package assignments, site ACL members, and site owners.
pub fn rename_username_references(old_username: &str, new_username: &str) {
    if names_equal(old_username, new_username) {
        return;
    }
    let _ = rename_package_assignment(old_username, new_username);
    let _ = rename_site_acl_member(old_username, new_username);
    if let Ok(sites) = list_sites() {
        for site in sites {
            if names_equal(&site.owner, old_username) {
                let _ = modify_site(
                    &site.domain,
                    SiteModify {
                        owner: Some(new_username.to_string()),
                        ..SiteModify::default()
                    },
                );
            }
        }
    }
}

fn rename_package_assignment(old_username: &str, new_username: &str) -> Result<(), String> {
    let path = data_dir().join("package-assignments.json");
    if !path.is_file() {
        return Ok(());
    }
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("Could not read package assignments: {error}"))?;
    let mut value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|error| format!("Invalid package assignments JSON: {error}"))?;
    let Some(assignments) = value
        .get_mut("assignments")
        .and_then(|v| v.as_array_mut())
    else {
        return Ok(());
    };
    let mut changed = false;
    for entry in assignments.iter_mut() {
        let Some(obj) = entry.as_object_mut() else {
            continue;
        };
        let Some(name) = obj.get("username").and_then(|v| v.as_str()) else {
            continue;
        };
        if names_equal(name, old_username) {
            obj.insert(
                "username".into(),
                serde_json::Value::String(new_username.to_string()),
            );
            changed = true;
        }
    }
    if changed {
        let json = serde_json::to_string_pretty(&value)
            .map_err(|error| format!("Could not serialize package assignments: {error}"))?;
        fs::write(&path, json)
            .map_err(|error| format!("Could not write package assignments: {error}"))?;
    }
    Ok(())
}

fn rename_site_acl_member(old_username: &str, new_username: &str) -> Result<(), String> {
    let path = data_dir().join("site-acl.json");
    if !path.is_file() {
        return Ok(());
    }
    let raw =
        fs::read_to_string(&path).map_err(|error| format!("Could not read site ACL: {error}"))?;
    let mut value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|error| format!("Invalid site ACL JSON: {error}"))?;
    let Some(grants) = value.get_mut("grants").and_then(|v| v.as_array_mut()) else {
        return Ok(());
    };
    let mut changed = false;
    for grant in grants.iter_mut() {
        let Some(obj) = grant.as_object_mut() else {
            continue;
        };
        for key in ["member", "all_owned_by"] {
            let Some(name) = obj.get(key).and_then(|v| v.as_str()) else {
                continue;
            };
            if names_equal(name, old_username) {
                obj.insert(
                    key.into(),
                    serde_json::Value::String(new_username.to_string()),
                );
                changed = true;
            }
        }
    }
    if changed {
        let json = serde_json::to_string_pretty(&value)
            .map_err(|error| format!("Could not serialize site ACL: {error}"))?;
        fs::write(&path, json).map_err(|error| format!("Could not write site ACL: {error}"))?;
    }
    Ok(())
}
