//! Ownership policy resolution for Fix permissions.

use super::OwnershipFixPolicy;
use crate::mail_home_layout::EMAILS_DIR;
use crate::site_unix_owner::find_unix_user_by_home;
use crate::sites::{hosting_home_root, list_sites, site_home_from_record};
use std::path::{Path, PathBuf};

pub(super) fn path_is_under(path: &Path, root: &Path) -> bool {
    let path_s = path.to_string_lossy().replace('\\', "/");
    let root_s = root.to_string_lossy().replace('\\', "/");
    if path_s == root_s {
        return true;
    }
    let prefix = if root_s.ends_with('/') {
        root_s
    } else {
        format!("{root_s}/")
    };
    path_s.starts_with(&prefix)
}

/// Resolve the CPN ownership policy for a concrete absolute path.
pub fn resolve_ownership_policy(path: &Path) -> Result<OwnershipFixPolicy, String> {
    let home_root = hosting_home_root();
    if path_is_under(path, &home_root) {
        if let Some(user) = resolve_site_or_mailbox_user(path)? {
            if user.eq_ignore_ascii_case("root") {
                return Err("Resolved website Unix account cannot be root".into());
            }
            return Ok(OwnershipFixPolicy::SetUser { user });
        }
        return Err(format!(
            "not a registered CPN site home (no website Unix account for {})",
            path.display()
        ));
    }

    // Non-site path outside hosting homes: modes only when owner is not root.
    let (uid, _gid) = super::apply::path_owner_ids(path)?;
    if uid == 0 {
        return Err(format!(
            "refusing modes-only fix on root-owned non-site path {}",
            path.display()
        ));
    }
    Ok(OwnershipFixPolicy::ModesOnly)
}

fn resolve_site_or_mailbox_user(path: &Path) -> Result<Option<String>, String> {
    // Prefer mailbox Unix account for `/home/<domain>/e-mails/<local>/...`.
    if let Some(mailbox_home) = mailbox_home_for_path(path)
        && let Some(user) = find_unix_user_by_home(&mailbox_home)
    {
        return Ok(Some(user));
    }

    // Registered site whose home contains this path (longest home wins).
    if let Ok(sites) = list_sites() {
        let mut best: Option<(usize, String)> = None;
        for site in sites {
            let home = site_home_from_record(&site);
            if !path_is_under(path, &home) {
                continue;
            }
            let len = home.to_string_lossy().len();
            let user = site
                .unix_user
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .or_else(|| find_unix_user_by_home(&home));
            if let Some(user) = user
                && best.as_ref().is_none_or(|(best_len, _)| len > *best_len)
            {
                best = Some((len, user));
            }
        }
        if let Some((_, user)) = best {
            return Ok(Some(user));
        }
    }

    // Walk up looking for a passwd home match.
    let home_root = hosting_home_root();
    let mut cur = Some(path.to_path_buf());
    while let Some(dir) = cur {
        if dir == home_root {
            break;
        }
        if let Some(user) = find_unix_user_by_home(&dir) {
            return Ok(Some(user));
        }
        cur = dir.parent().map(|p| p.to_path_buf());
    }
    Ok(None)
}

pub(super) fn mailbox_home_for_path(path: &Path) -> Option<PathBuf> {
    let s = path.to_string_lossy().replace('\\', "/");
    let parts: Vec<&str> = s.split('/').filter(|p| !p.is_empty()).collect();
    // home / <domain> / e-mails / <local> [/ ...]
    if parts.len() >= 4 && parts[0] == "home" && parts[2] == EMAILS_DIR {
        return Some(PathBuf::from(format!(
            "/home/{}/{}/{}",
            parts[1], EMAILS_DIR, parts[3]
        )));
    }
    None
}
