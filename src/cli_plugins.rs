//! CLI handlers for `cpn plugin …`.

use crate::plugins::{
    catalog_repo_url, install_plugin, list_installed, plugins_install_path_display,
    set_plugin_enabled, uninstall_plugin,
};

pub fn list_plugins() -> Result<(), String> {
    let plugins = list_installed()?;
    if plugins.is_empty() {
        println!("(no plugins)");
        println!("install_path={}", plugins_install_path_display());
        println!("catalog={}", catalog_repo_url());
        return Ok(());
    }
    for item in plugins {
        let m = item.manifest;
        println!(
            "{}\tname={}\tversion={}\tenabled={}\tcategory={}\tpath={}",
            m.id,
            m.name,
            m.version,
            m.enabled,
            m.category,
            item.path.display()
        );
    }
    Ok(())
}

pub fn install(id: &str) -> Result<(), String> {
    let manifest = install_plugin(id)?;
    println!(
        "installed {} v{} under {}",
        manifest.id,
        manifest.version,
        plugins_install_path_display()
    );
    Ok(())
}

pub fn remove(id: &str) -> Result<(), String> {
    uninstall_plugin(id)?;
    println!("removed plugin {id}");
    Ok(())
}

pub fn enable(id: &str) -> Result<(), String> {
    let manifest = set_plugin_enabled(id, true)?;
    println!("enabled {}", manifest.id);
    Ok(())
}

pub fn disable(id: &str) -> Result<(), String> {
    let manifest = set_plugin_enabled(id, false)?;
    println!("disabled {}", manifest.id);
    Ok(())
}
