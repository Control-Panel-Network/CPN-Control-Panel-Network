//! Detect and heal root-owned registered website homes.

use std::path::PathBuf;

use super::account::{ensure_primary_unix_user, resolve_unix_user_for_site, running_as_root};
use super::perms::{apply_site_ownership, path_owner_ids};
use crate::sites::{site_home_from_record, SiteRecord};

/// Heal ownership for one registered site (home inode + docroot).
pub fn heal_site_ownership(site: &SiteRecord) -> Result<String, String> {
    if !running_as_root() {
        return Err("Ownership heal requires root".into());
    }
    let home = site_home_from_record(site);
    let docroot = PathBuf::from(&site.docroot);
    let user = match resolve_unix_user_for_site(site) {
        Ok(u) => u,
        Err(_) => ensure_primary_unix_user(&site.domain, &home)?,
    };
    apply_site_ownership(&home, &docroot, &user)?;
    let _ = crate::sites::modify_site(
        &site.domain,
        crate::sites::SiteModify {
            unix_user: Some(Some(user.clone())),
            ..crate::sites::SiteModify::default()
        },
    );
    Ok(format!(
        "{}: home+docroot set to `{user}:{user}`",
        site.domain
    ))
}

/// Scan registered sites for root-owned homes. Returns domain list.
pub fn root_owned_site_homes() -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let Ok(sites) = crate::sites::list_sites() else {
        return out;
    };
    for site in sites {
        let home = site_home_from_record(&site);
        if !home.is_dir() {
            continue;
        }
        if let Ok((uid, _)) = path_owner_ids(&home)
            && uid == 0
        {
            out.push((site.domain, home));
        }
    }
    out
}

/// Heal all registered root-owned site homes. Returns summary (no secrets).
pub fn heal_all_root_owned_site_homes() -> Result<String, String> {
    let offenders = root_owned_site_homes();
    if offenders.is_empty() {
        return Ok("No registered website homes are owned by root".into());
    }
    let mut ok = 0usize;
    let mut fail = 0usize;
    let mut notes = Vec::new();
    for (domain, _) in &offenders {
        match crate::sites::load_site(domain).and_then(|s| heal_site_ownership(&s)) {
            Ok(msg) => {
                ok += 1;
                notes.push(msg);
            }
            Err(err) => {
                fail += 1;
                notes.push(format!("{domain}: {err}"));
            }
        }
    }
    if fail > 0 {
        return Err(format!(
            "Ownership heal: {ok} fixed, {fail} failed. {}",
            notes.join("; ")
        ));
    }
    Ok(format!(
        "Ownership heal: {ok} site home(s) set to CPN website Unix user:group. {}",
        notes.join("; ")
    ))
}
