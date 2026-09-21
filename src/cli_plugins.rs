//! CLI handlers for `cpn plugin …`.

use crate::plugin_activation::{
    activate_host_plugin_for_domain, host_plugin_installed, install_host_plugin,
    is_host_scoped_plugin, list_host_installed_plugins, uninstall_host_plugin,
};
use crate::plugins::{
    catalog_repo_url, install_plugin, list_installed, list_installed_all, migrate_legacy_plugins,
    plugins_install_path_display, set_plugin_enabled, uninstall_plugin,
};

pub fn list_plugins(domain: Option<&str>) -> Result<(), String> {
    let host = list_host_installed_plugins();
    if domain.map(|d| d.trim().is_empty()).unwrap_or(true) && !host.is_empty() {
        println!("# Host-scoped plugins ($CPN_DATA_DIR/host-plugins/)");
        for item in &host {
            let m = &item.manifest;
            println!(
                "{}\thost=1\tname={}\tversion={}\tenabled={}\tcategory={}\tpath={}",
                m.id,
                m.name,
                m.version,
                m.enabled,
                m.category,
                item.path.display()
            );
        }
    }
    let plugins = match domain {
        Some(d) if !d.trim().is_empty() => list_installed(d)?,
        _ => list_installed_all()?,
    };
    if plugins.is_empty() && host.is_empty() {
        println!("(no plugins)");
        println!(
            "install_path={}",
            plugins_install_path_display(domain.filter(|d| !d.trim().is_empty()))
        );
        println!("catalog={}", catalog_repo_url());
        println!("hint=Host packages (MariaDB, phpMyAdmin, webmail): cpn app list");
        return Ok(());
    }
    if !plugins.is_empty() {
        println!("# Site plugins (/home/<domain>/plugins/)");
    }
    for item in plugins {
        let m = item.manifest;
        println!(
            "{}\tdomain={}\tname={}\tversion={}\tenabled={}\tcategory={}\tpath={}",
            m.id,
            item.domain,
            m.name,
            m.version,
            m.enabled,
            m.category,
            item.path.display()
        );
    }
    Ok(())
}

/// Install a catalog plugin.
/// Host-scoped ids install under host-plugins/ (optional site Activate when domain is set).
/// Community plugins require `--domain`.
pub fn install(domain: Option<&str>, id: &str, force_host: bool) -> Result<(), String> {
    let id = id.trim();
    if id.is_empty() {
        return Err("Plugin id is required".into());
    }
    let domain = domain.map(str::trim).filter(|d| !d.is_empty());
    if force_host || is_host_scoped_plugin(id) {
        if !host_plugin_installed(id) {
            let manifest = install_host_plugin(id)?;
            println!(
                "installed {} v{} on Host under host-plugins/",
                manifest.id, manifest.version
            );
        } else {
            println!("host plugin `{id}` already installed");
        }
        if let Some(domain) = domain {
            let manifest = activate_host_plugin_for_domain(domain, id)?;
            println!("activated {} for {}", manifest.id, domain);
        }
        return Ok(());
    }
    let domain = domain.ok_or_else(|| {
        "Domain is required for site plugins (or use --host for host-scoped Security packages)"
            .to_string()
    })?;
    let manifest = install_plugin(domain, id)?;
    println!(
        "installed {} v{} under {}",
        manifest.id,
        manifest.version,
        plugins_install_path_display(Some(domain))
    );
    Ok(())
}

pub fn remove(domain: Option<&str>, id: &str, from_host: bool) -> Result<(), String> {
    if from_host
        || (domain.map(|d| d.trim().is_empty()).unwrap_or(true) && is_host_scoped_plugin(id))
    {
        uninstall_host_plugin(id)?;
        println!("removed host plugin {id}");
        return Ok(());
    }
    let domain = domain
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .ok_or_else(|| {
            "Domain is required (or pass --host to remove a host-scoped plugin)".to_string()
        })?;
    uninstall_plugin(domain, id)?;
    println!("removed plugin {id} from {domain}");
    Ok(())
}

pub fn enable(domain: &str, id: &str) -> Result<(), String> {
    let manifest = set_plugin_enabled(domain, id, true)?;
    println!("enabled {} on {}", manifest.id, domain);
    Ok(())
}

pub fn disable(domain: &str, id: &str) -> Result<(), String> {
    let manifest = set_plugin_enabled(domain, id, false)?;
    println!("disabled {} on {}", manifest.id, domain);
    Ok(())
}

pub fn migrate(domain: &str) -> Result<(), String> {
    let moved = migrate_legacy_plugins(domain)?;
    println!(
        "migrated {moved} plugin(s) into {}",
        plugins_install_path_display(Some(domain))
    );
    Ok(())
}
