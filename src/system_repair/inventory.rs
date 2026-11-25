//! Website inventory checks: legacy owners and unregistered hostname vhosts.

use super::{CheckStatus, HealResult, push};
use crate::package_usage_scope::is_known_panel_username;
use crate::sites::list_sites;
use crate::sites_inventory_heal;
use std::fs;
use std::path::Path;

pub const INVENTORY_HEAL_IDS: &[&str] = &["sites.inventory"];

fn legacy_owner_count() -> usize {
    list_sites()
        .unwrap_or_default()
        .into_iter()
        .filter(|s| !is_known_panel_username(&s.owner))
        .count()
}

fn unregistered_hostname_hint() -> usize {
    let registered: std::collections::BTreeSet<String> = list_sites()
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.domain.to_ascii_lowercase())
        .collect();
    let vhosts = Path::new("/usr/local/lsws/conf/vhosts");
    let Ok(entries) = fs::read_dir(vhosts) else {
        return 0;
    };
    let mut n = 0usize;
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(|s| s.to_ascii_lowercase()) else {
            continue;
        };
        if !name.contains('.') || registered.contains(&name) {
            continue;
        }
        let home = Path::new("/home").join(&name);
        if home.join("public_html").is_dir() || home.join(&name).is_dir() {
            n = n.saturating_add(1);
        }
    }
    n
}

pub fn collect(checks: &mut Vec<super::RepairCheck>) {
    let legacy = legacy_owner_count();
    if legacy == 0 {
        push(
            checks,
            "sites.legacy_owners",
            "websites",
            "Site owners",
            CheckStatus::Pass,
            "All registered sites use known panel account owners",
            false,
            Some("sites.inventory"),
        );
    } else {
        push(
            checks,
            "sites.legacy_owners",
            "websites",
            "Site owners",
            CheckStatus::Warn,
            format!(
                "{legacy} site(s) still use a legacy/non-panel owner string (common after CyberPanel migrate). Dashboard package usage attributes these to the panel admin; heal remaps owners."
            ),
            false,
            Some("sites.inventory"),
        );
    }

    let orphans = unregistered_hostname_hint();
    if orphans == 0 {
        push(
            checks,
            "sites.hostname_vhosts",
            "websites",
            "Hostname vhosts",
            CheckStatus::Pass,
            "No unregistered LiteSpeed vhosts with /home docroots (Cockpit :9090 is not a website)",
            false,
            Some("sites.inventory"),
        );
    } else {
        push(
            checks,
            "sites.hostname_vhosts",
            "websites",
            "Hostname vhosts",
            CheckStatus::Warn,
            format!(
                "{orphans} LiteSpeed vhost(s) have a /home docroot but are missing from CPN websites (often the server FQDN). Heal registers them. Contabo Cockpit on :9090 is not a CPN site."
            ),
            false,
            Some("sites.inventory"),
        );
    }
}

pub fn heal_sites_inventory() -> HealResult {
    match sites_inventory_heal::heal_sites_inventory() {
        Ok(message) => HealResult {
            heal_id: "sites.inventory".into(),
            ok: true,
            message,
        },
        Err(message) => HealResult {
            heal_id: "sites.inventory".into(),
            ok: false,
            message,
        },
    }
}
