//! Shared Postfix hash/lmdb map apply helpers for CPN mail features.

use crate::postfix_fallback::postfix_is_ready;
use std::path::Path;
use std::process::Command;

/// Write `body` to `map_path`, run `postmap`, merge into `virtual_alias_maps`, reload Postfix.
pub fn apply_virtual_alias_map(map_path: &Path, body: &str) -> Result<String, String> {
    if let Some(parent) = map_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Cannot create mail map dir: {e}"))?;
    }
    std::fs::write(map_path, body).map_err(|e| format!("Cannot write virtual map: {e}"))?;

    if !postfix_is_ready() {
        return Ok(format!(
            "Map written to {} (Postfix not ready; apply again when the MTA is up).",
            map_path.display()
        ));
    }

    let map_str = map_path.to_string_lossy().to_string();
    let hash = Command::new("postmap").arg(&map_str).output();
    match hash {
        Ok(out) if out.status.success() => {
            let map_type = postconf_map_type();
            let map_arg = format!("{map_type}:{map_str}");
            let existing = Command::new("postconf")
                .args(["-h", "virtual_alias_maps"])
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();
            let new_maps = crate::mail_hosted_domains::merge_list_value(&existing, &map_arg);
            let _ = Command::new("postconf")
                .args(["-e", &format!("virtual_alias_maps={new_maps}")])
                .status();
            let _ = Command::new("systemctl")
                .args(["reload", "postfix"])
                .status();
            Ok(format!("Applied Postfix map at {}.", map_path.display()))
        }
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr);
            Ok(format!(
                "Map saved at {}; postmap note: {}",
                map_path.display(),
                err.trim()
            ))
        }
        Err(e) => Ok(format!(
            "Map saved at {}; postmap unavailable: {e}",
            map_path.display()
        )),
    }
}

fn postconf_map_type() -> String {
    Command::new("postconf")
        .args(["-h", "default_database_type"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "hash".into())
}

/// Normalize and validate a single email address.
pub fn normalize_email(addr: &str) -> Result<String, String> {
    let dest = addr.trim().to_ascii_lowercase();
    if dest.is_empty() || !dest.contains('@') || dest.contains(' ') || dest.len() > 254 {
        return Err("Must be a valid email address".into());
    }
    if dest.contains('\n') || dest.contains('\0') {
        return Err("Email must be a single line".into());
    }
    Ok(dest)
}

/// Resolve the panel account that owns a mail domain (site owner).
pub fn owner_for_mail_domain(domain: &str) -> Option<String> {
    let domain = domain.trim().to_ascii_lowercase();
    crate::sites::list_sites().ok()?.into_iter().find_map(|s| {
        if s.domain.eq_ignore_ascii_case(&domain) {
            return Some(s.owner);
        }
        if s.aliases
            .iter()
            .any(|a| a.eq_ignore_ascii_case(&domain))
        {
            return Some(s.owner);
        }
        None
    })
}

/// True when `username` may manage mail for `domain` (admin or site owner).
pub fn can_manage_mail_domain(username: &str, domain: &str) -> bool {
    if crate::packages::is_panel_admin(username) {
        return true;
    }
    owner_for_mail_domain(domain)
        .map(|o| o.trim().eq_ignore_ascii_case(username.trim()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_email_ok() {
        assert_eq!(
            normalize_email("  A@Example.COM ").unwrap(),
            "a@example.com"
        );
        assert!(normalize_email("nope").is_err());
    }
}
