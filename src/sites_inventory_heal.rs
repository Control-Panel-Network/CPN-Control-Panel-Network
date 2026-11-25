//! Heal migrated website inventory: legacy owners and unregistered hostname vhosts.
//!
//! Cockpit / Contabo web console on `:9090` is never treated as a website. Only real
//! LiteSpeed / OpenLiteSpeed vhost directories with a matching home docroot are
//! considered for registration.

use crate::account::load_bootstrap;
use crate::package_usage_scope::is_known_panel_username;
use crate::sites::{SiteModify, create_site, list_sites, load_site, modify_site};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const LSWS_VHOSTS: &str = "/usr/local/lsws/conf/vhosts";

/// Vhost directory names that are panel/tooling, not customer websites.
fn is_skipped_vhost_name(name: &str) -> bool {
    let n = name.trim().to_ascii_lowercase();
    matches!(
        n.as_str(),
        "cpnpanel"
            | "example"
            | "example.com"
            | "phpmyadmin"
            | "pma"
            | "snappymail"
            | "rainloop"
            | "tachyon"
            | "webmail"
            | "default"
            | "_"
            | "lsphp"
    ) || n.starts_with("cpn")
        || !n.contains('.')
}

fn panel_admin_username() -> Option<String> {
    load_bootstrap().map(|b| b.username)
}

fn hostname_candidates() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(raw) = fs::read_to_string("/etc/hostname") {
        let h = raw.trim().to_ascii_lowercase();
        if !h.is_empty() {
            out.push(h);
        }
    }
    if let Ok(outp) = Command::new("hostname").arg("-f").output()
        && outp.status.success()
    {
        let h = String::from_utf8_lossy(&outp.stdout)
            .trim()
            .to_ascii_lowercase();
        if !h.is_empty() && !out.iter().any(|x| x == &h) {
            out.push(h);
        }
    }
    if let Some(panel) = crate::panel_network::load_panel_hostname() {
        let h = panel.trim().to_ascii_lowercase();
        if !h.is_empty() && !out.iter().any(|x| x == &h) {
            out.push(h);
        }
    }
    out
}

fn guess_docroot(domain: &str) -> Option<PathBuf> {
    let home = PathBuf::from(format!("/home/{domain}"));
    let candidates = [
        home.join("public_html"),
        home.join(domain),
        home.join("public_html").join(domain),
    ];
    candidates.into_iter().find(|path| path.is_dir())
}

fn list_lsws_vhost_names() -> Vec<String> {
    let dir = Path::new(LSWS_VHOSTS);
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if is_skipped_vhost_name(&name) {
            continue;
        }
        names.push(name.to_ascii_lowercase());
    }
    names.sort();
    names.dedup();
    names
}

/// Remap site.owner values that are not panel accounts onto the bootstrap admin.
pub fn heal_legacy_site_owners() -> Result<String, String> {
    let Some(admin) = panel_admin_username() else {
        return Err("Panel bootstrap account is missing".into());
    };
    let sites = list_sites()?;
    let mut changed = 0usize;
    let mut samples = Vec::new();
    for site in sites {
        if is_known_panel_username(&site.owner) {
            continue;
        }
        let previous = site.owner.clone();
        modify_site(
            &site.domain,
            SiteModify {
                owner: Some(admin.clone()),
                ..Default::default()
            },
        )?;
        changed = changed.saturating_add(1);
        if samples.len() < 5 {
            samples.push(format!("{} (was `{previous}`)", site.domain));
        }
    }
    if changed == 0 {
        return Ok("No legacy site owners needed remapping".into());
    }
    Ok(format!(
        "Remapped {changed} site owner(s) to `{admin}`: {}",
        samples.join(", ")
    ))
}

/// Register LiteSpeed vhosts that have a `/home/<fqdn>` docroot but no CPN site row.
///
/// Prefers the server FQDN / hostname candidates, then other unregistered vhosts with
/// a real home. Does not invent Cockpit `:9090` as a website.
pub fn heal_unregistered_hostname_vhosts() -> Result<String, String> {
    let Some(admin) = panel_admin_username() else {
        return Err("Panel bootstrap account is missing".into());
    };
    let registered: std::collections::BTreeSet<String> = list_sites()?
        .into_iter()
        .map(|s| s.domain.to_ascii_lowercase())
        .collect();
    let mut vhosts = list_lsws_vhost_names();
    // Prefer detecting the server hostname first when present as a vhost.
    for host in hostname_candidates().into_iter().rev() {
        if vhosts.iter().any(|v| v == &host) {
            vhosts.retain(|v| v != &host);
            vhosts.insert(0, host);
        }
    }

    let mut registered_now = Vec::new();
    let mut skipped = 0usize;
    for name in vhosts {
        if registered.contains(&name) {
            continue;
        }
        let Some(docroot) = guess_docroot(&name) else {
            skipped = skipped.saturating_add(1);
            continue;
        };
        let doc = docroot.to_string_lossy().to_string();
        match create_site(
            &name,
            &admin,
            Some(&doc),
            None,
            Some("Registered from existing LiteSpeed vhost / home (inventory heal)"),
        ) {
            Ok(site) => {
                registered_now.push(site.domain);
            }
            Err(err) if err.contains("already exists") => {
                let _ = load_site(&name);
            }
            Err(err) => {
                return Err(format!("Could not register `{name}`: {err}"));
            }
        }
    }

    if registered_now.is_empty() {
        return Ok(format!(
            "No unregistered hostname vhosts with home docroots (skipped {skipped} without /home)"
        ));
    }
    Ok(format!(
        "Registered {} hostname/site vhost(s) under `{admin}`: {}",
        registered_now.len(),
        registered_now.join(", ")
    ))
}

/// Combined inventory heal used by System Repair / `cpn doctor`.
pub fn heal_sites_inventory() -> Result<String, String> {
    let owners = heal_legacy_site_owners()?;
    let hosts = heal_unregistered_hostname_vhosts()?;
    Ok(format!("{owners}; {hosts}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_tooling_vhost_names() {
        assert!(is_skipped_vhost_name("CPNPanel"));
        assert!(is_skipped_vhost_name("phpMyAdmin"));
        assert!(is_skipped_vhost_name("localhost"));
        assert!(!is_skipped_vhost_name("vmi123.example.net"));
        assert!(!is_skipped_vhost_name("shop.example.com"));
    }
}
