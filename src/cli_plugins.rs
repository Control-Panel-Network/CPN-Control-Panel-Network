//! CLI handlers for `cpn plugin …`.

use crate::plugins::{
    catalog_repo_url, install_plugin, list_installed, list_installed_all, migrate_legacy_plugins,
    plugins_install_path_display, set_plugin_enabled, uninstall_plugin,
};

pub fn list_plugins(domain: Option<&str>) -> Result<(), String> {
    let plugins = match domain {
        Some(d) if !d.trim().is_empty() => list_installed(d)?,
        _ => list_installed_all()?,
    };
    if plugins.is_empty() {
        println!("(no plugins)");
        println!(
            "install_path={}",
            plugins_install_path_display(domain.filter(|d| !d.trim().is_empty()))
        );
        println!("catalog={}", catalog_repo_url());
        return Ok(());
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

pub fn install(domain: &str, id: &str) -> Result<(), String> {
    let manifest = install_plugin(domain, id)?;
    println!(
        "installed {} v{} under {}",
        manifest.id,
        manifest.version,
        plugins_install_path_display(Some(domain))
    );
    Ok(())
}

pub fn remove(domain: &str, id: &str) -> Result<(), String> {
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
