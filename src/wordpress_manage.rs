//! WordPress site scan, refresh, plugins/themes, and maintenance helpers.

use crate::account::now_unix;
use crate::panel_ops_php::detect_php;
use crate::sites::{list_sites, normalize_domain};
use crate::wordpress::{
    WordpressSite, delete_wordpress_site, get_wordpress_site, list_wordpress_sites,
    upsert_wordpress_site,
};
use crate::wordpress_wpcli::{
    WpCliStatus, detect_wp_cli, is_wordpress_docroot, wp_option_get, wp_option_update, wp_run,
};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Default)]
pub struct PluginRow {
    pub name: String,
    pub status: String,
    pub version: String,
    pub update: String,
}

#[derive(Debug, Clone, Default)]
pub struct ThemeRow {
    pub name: String,
    pub status: String,
    pub version: String,
}

#[derive(Debug, Clone)]
pub struct WordpressRefreshResult {
    pub site: WordpressSite,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct WordpressSiteSnapshot {
    pub site: WordpressSite,
    pub plugins: Vec<PluginRow>,
    pub themes: Vec<ThemeRow>,
    pub wp_cli: WpCliStatus,
}

#[derive(Debug, Deserialize)]
struct WpPluginJson {
    #[serde(default)]
    name: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    update: String,
}

#[derive(Debug, Deserialize)]
struct WpThemeJson {
    #[serde(default)]
    name: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    version: String,
}

fn docroot_for_domain(domain: &str) -> Result<PathBuf, String> {
    let site =
        get_wordpress_site(domain).ok_or_else(|| format!("WordPress `{domain}` not registered"))?;
    Ok(PathBuf::from(&site.docroot))
}

fn read_docroot_path(path: &Path) -> Result<(), String> {
    if !path.is_dir() {
        return Err(format!("Document root not found: {}", path.display()));
    }
    if !is_wordpress_docroot(path) {
        return Err(format!("No WordPress installation at {}", path.display()));
    }
    Ok(())
}

pub fn list_plugins(domain_raw: &str) -> Result<Vec<PluginRow>, String> {
    let domain = normalize_domain(domain_raw)?;
    let docroot = docroot_for_domain(&domain)?;
    read_docroot_path(&docroot)?;
    let raw = wp_run(&docroot, &["plugin", "list", "--format=json"])?;
    let parsed: Vec<WpPluginJson> =
        serde_json::from_str(&raw).map_err(|e| format!("Could not parse plugin list: {e}"))?;
    Ok(parsed
        .into_iter()
        .map(|row| PluginRow {
            name: row.name,
            status: row.status,
            version: row.version,
            update: row.update,
        })
        .collect())
}

pub fn list_themes(domain_raw: &str) -> Result<Vec<ThemeRow>, String> {
    let domain = normalize_domain(domain_raw)?;
    let docroot = docroot_for_domain(&domain)?;
    read_docroot_path(&docroot)?;
    let raw = wp_run(&docroot, &["theme", "list", "--format=json"])?;
    let parsed: Vec<WpThemeJson> =
        serde_json::from_str(&raw).map_err(|e| format!("Could not parse theme list: {e}"))?;
    Ok(parsed
        .into_iter()
        .map(|row| ThemeRow {
            name: row.name,
            status: row.status,
            version: row.version,
        })
        .collect())
}

pub fn refresh_wordpress_site(domain_raw: &str) -> Result<WordpressRefreshResult, String> {
    let domain = normalize_domain(domain_raw)?;
    let mut site = get_wordpress_site(&domain)
        .ok_or_else(|| format!("WordPress `{domain}` not registered"))?;
    let docroot = PathBuf::from(&site.docroot);
    read_docroot_path(&docroot)?;
    let mut notes = Vec::new();

    site.wp_version = wp_run(&docroot, &["core", "version"])
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|e| {
            notes.push(format!("Could not read WP version: {e}"));
            String::new()
        });

    site.php_version = detect_php()
        .version
        .unwrap_or_else(|| site.php_version.clone());

    site.theme = wp_run(
        &docroot,
        &["theme", "list", "--status=active", "--field=name"],
    )
    .map(|s| s.trim().to_string())
    .unwrap_or_else(|e| {
        notes.push(format!("Could not read active theme: {e}"));
        String::new()
    });

    site.plugin_count = list_plugins(&domain)
        .map(|rows| rows.len() as u32)
        .unwrap_or(0);

    site.search_indexing = wp_option_get(&docroot, "blog_public")
        .map(|v| v != "0")
        .unwrap_or(site.search_indexing);

    site.maintenance = docroot.join(".maintenance").is_file();
    site.password_protection = docroot.join(".htpasswd").is_file();
    site.debugging = wp_config_has_debug(&docroot);
    site.updated_at_unix = now_unix();

    let saved = upsert_wordpress_site(site)?;
    Ok(WordpressRefreshResult { site: saved, notes })
}

pub fn scan_wordpress_sites() -> Result<Vec<WordpressRefreshResult>, String> {
    let mut results = Vec::new();
    let registered: Vec<String> = list_wordpress_sites()
        .into_iter()
        .map(|s| s.domain)
        .collect();

    for site in list_sites().unwrap_or_default() {
        let docroot = PathBuf::from(&site.docroot);
        if !is_wordpress_docroot(&docroot) {
            continue;
        }
        let domain = site.domain.clone();
        if get_wordpress_site(&domain).is_none() {
            let now = now_unix();
            let stub = WordpressSite {
                id: crate::wordpress::new_site_id(),
                domain: domain.clone(),
                title: domain.clone(),
                docroot: site.docroot.clone(),
                site_url: format!("https://{domain}"),
                admin_user: "-".into(),
                db_name: "-".into(),
                db_user: "-".into(),
                owner: site.owner.clone(),
                wp_version: String::new(),
                php_version: String::new(),
                theme: String::new(),
                plugin_count: 0,
                search_indexing: true,
                debugging: false,
                maintenance: false,
                password_protection: false,
                created_at_unix: now,
                updated_at_unix: now,
            };
            upsert_wordpress_site(stub)?;
        }
        if let Ok(refreshed) = refresh_wordpress_site(&domain) {
            results.push(refreshed);
        }
    }

    for domain in registered {
        if results.iter().any(|r| r.site.domain == domain) {
            continue;
        }
        if let Ok(refreshed) = refresh_wordpress_site(&domain) {
            results.push(refreshed);
        }
    }
    Ok(results)
}

pub fn install_plugin(domain_raw: &str, source: &str) -> Result<String, String> {
    let domain = normalize_domain(domain_raw)?;
    let source = source.trim();
    if source.is_empty() {
        return Err("Plugin source is required".into());
    }
    let docroot = docroot_for_domain(&domain)?;
    read_docroot_path(&docroot)?;
    wp_run(
        &docroot,
        &["plugin", "install", source, "--activate", "--force"],
    )?;
    let _ = refresh_wordpress_site(&domain)?;
    Ok(format!("Installed and activated `{source}` on `{domain}`"))
}

pub fn activate_theme(domain_raw: &str, slug: &str) -> Result<String, String> {
    let domain = normalize_domain(domain_raw)?;
    let slug = slug.trim();
    if slug.is_empty() {
        return Err("Theme slug is required".into());
    }
    let docroot = docroot_for_domain(&domain)?;
    read_docroot_path(&docroot)?;
    wp_run(&docroot, &["theme", "activate", slug])?;
    let _ = refresh_wordpress_site(&domain)?;
    Ok(format!("Activated theme `{slug}` on `{domain}`"))
}

fn save_site_patch(domain: &str, patch: impl FnOnce(&mut WordpressSite)) -> Result<(), String> {
    let mut site =
        get_wordpress_site(domain).ok_or_else(|| format!("WordPress `{domain}` not registered"))?;
    patch(&mut site);
    site.updated_at_unix = now_unix();
    upsert_wordpress_site(site).map(|_| ())
}

pub fn set_search_indexing(domain_raw: &str, enabled: bool) -> Result<String, String> {
    let domain = normalize_domain(domain_raw)?;
    let docroot = docroot_for_domain(&domain)?;
    read_docroot_path(&docroot)?;
    wp_option_update(&docroot, "blog_public", if enabled { "1" } else { "0" })?;
    save_site_patch(&domain, |site| site.search_indexing = enabled)?;
    Ok(if enabled {
        format!("Search indexing enabled for `{domain}`")
    } else {
        format!("Search indexing disabled for `{domain}` (discourage search engines)")
    })
}

fn wp_config_has_debug(docroot: &Path) -> bool {
    let path = docroot.join("wp-config.php");
    let Ok(raw) = fs::read_to_string(&path) else {
        return false;
    };
    raw.contains("define( 'WP_DEBUG', true )")
        || raw.contains("define('WP_DEBUG', true)")
        || raw.contains("define( \"WP_DEBUG\", true )")
}

pub fn set_debugging(domain_raw: &str, enabled: bool) -> Result<String, String> {
    let domain = normalize_domain(domain_raw)?;
    let docroot = docroot_for_domain(&domain)?;
    read_docroot_path(&docroot)?;
    let config = docroot.join("wp-config.php");
    let raw =
        fs::read_to_string(&config).map_err(|e| format!("Could not read wp-config.php: {e}"))?;
    let updated = replace_wp_debug_constant(&raw, enabled);
    fs::write(&config, updated.as_bytes())
        .map_err(|e| format!("Could not write wp-config.php: {e}"))?;
    save_site_patch(&domain, |site| site.debugging = enabled)?;
    Ok(if enabled {
        format!("WP_DEBUG enabled for `{domain}`")
    } else {
        format!("WP_DEBUG disabled for `{domain}`")
    })
}

fn replace_wp_debug_constant(raw: &str, enabled: bool) -> String {
    let value = if enabled { "true" } else { "false" };
    let patterns = [
        (
            "define( 'WP_DEBUG', true )",
            format!("define( 'WP_DEBUG', {value} )"),
        ),
        (
            "define( 'WP_DEBUG', false )",
            format!("define( 'WP_DEBUG', {value} )"),
        ),
        (
            "define('WP_DEBUG', true)",
            format!("define('WP_DEBUG', {value})"),
        ),
        (
            "define('WP_DEBUG', false)",
            format!("define('WP_DEBUG', {value})"),
        ),
    ];
    for (from, to) in &patterns {
        if raw.contains(from) {
            return raw.replace(from, to);
        }
    }
    if raw.contains("/* That's all, stop editing") {
        let insert = format!("\ndefine( 'WP_DEBUG', {value} );\n");
        return raw.replace(
            "/* That's all, stop editing",
            &format!("{insert}/* That's all, stop editing"),
        );
    }
    format!("{raw}\ndefine( 'WP_DEBUG', {value} );\n")
}

pub fn set_maintenance(domain_raw: &str, enabled: bool) -> Result<String, String> {
    let domain = normalize_domain(domain_raw)?;
    let docroot = docroot_for_domain(&domain)?;
    read_docroot_path(&docroot)?;
    let file = docroot.join(".maintenance");
    if enabled {
        let payload = format!("<?php $upgrading = {};\n", now_unix());
        fs::write(&file, payload.as_bytes())
            .map_err(|e| format!("Could not write .maintenance: {e}"))?;
    } else if file.is_file() {
        fs::remove_file(&file).map_err(|e| format!("Could not remove .maintenance: {e}"))?;
    }
    save_site_patch(&domain, |site| site.maintenance = enabled)?;
    Ok(if enabled {
        format!("Maintenance mode enabled for `{domain}`")
    } else {
        format!("Maintenance mode cleared for `{domain}`")
    })
}

pub fn set_password_protection(
    domain_raw: &str,
    enabled: bool,
    username: &str,
    password: &str,
) -> Result<String, String> {
    let domain = normalize_domain(domain_raw)?;
    let docroot = docroot_for_domain(&domain)?;
    read_docroot_path(&docroot)?;
    let htpasswd = docroot.join(".htpasswd");
    let htaccess = docroot.join(".htaccess");

    if enabled {
        let user = username.trim();
        let pass = password.trim();
        if user.is_empty() || pass.is_empty() {
            return Err("Username and password are required for protection".into());
        }
        let hash = openssl_apr1_hash(pass)?;
        let line = format!("{user}:{hash}\n");
        fs::write(&htpasswd, line.as_bytes())
            .map_err(|e| format!("Could not write .htpasswd: {e}"))?;
        merge_htaccess_auth(&htaccess, &htpasswd)?;
    } else {
        if htpasswd.is_file() {
            fs::remove_file(&htpasswd).map_err(|e| format!("Could not remove .htpasswd: {e}"))?;
        }
        strip_htaccess_auth(&htaccess)?;
    }

    save_site_patch(&domain, |site| site.password_protection = enabled)?;
    Ok(if enabled {
        format!("Password protection enabled for `{domain}`")
    } else {
        format!("Password protection removed for `{domain}`")
    })
}

fn openssl_apr1_hash(password: &str) -> Result<String, String> {
    let out = Command::new("openssl")
        .args(["passwd", "-apr1", password])
        .output()
        .map_err(|e| format!("openssl not available for htpasswd hash: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "openssl passwd failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

const HTACCESS_MARKER: &str = "# CPN WordPress password protection";
const HTACCESS_MARKER_END: &str = "# End CPN WordPress password protection";

fn filter_htaccess_block(existing: &str, insert_block: Option<&str>) -> String {
    let mut out = Vec::new();
    let mut skip = false;
    for line in existing.lines() {
        if line.starts_with(HTACCESS_MARKER) {
            skip = true;
            continue;
        }
        if line.starts_with(HTACCESS_MARKER_END) {
            skip = false;
            continue;
        }
        if !skip {
            out.push(line);
        }
    }
    if let Some(block) = insert_block {
        if !out.is_empty() {
            out.push("");
        }
        out.push(block);
    }
    out.join("\n")
}

fn merge_htaccess_auth(htaccess: &Path, htpasswd: &Path) -> Result<(), String> {
    let block = format!(
        "{HTACCESS_MARKER}\nAuthType Basic\nAuthName \"Protected\"\nAuthUserFile {}\nRequire valid-user\n{HTACCESS_MARKER_END}",
        htpasswd.display()
    );
    let existing = fs::read_to_string(htaccess).unwrap_or_default();
    let merged = filter_htaccess_block(&existing, Some(&block));
    fs::write(htaccess, merged).map_err(|e| format!("Could not update .htaccess: {e}"))
}

fn strip_htaccess_auth(htaccess: &Path) -> Result<(), String> {
    if !htaccess.is_file() {
        return Ok(());
    }
    let existing =
        fs::read_to_string(htaccess).map_err(|e| format!("Could not read .htaccess: {e}"))?;
    fs::write(htaccess, filter_htaccess_block(&existing, None))
        .map_err(|e| format!("Could not update .htaccess: {e}"))
}

pub fn delete_wordpress(domain_raw: &str, remove_files: bool) -> Result<String, String> {
    let domain = normalize_domain(domain_raw)?;
    let site = get_wordpress_site(&domain)
        .ok_or_else(|| format!("WordPress `{domain}` not registered"))?;
    let docroot = PathBuf::from(&site.docroot);
    if remove_files && docroot.is_dir() {
        for name in [
            "wp-admin",
            "wp-includes",
            "wp-content",
            "wp-config.php",
            "wp-load.php",
            "wp-settings.php",
            "wp-blog-header.php",
            "wp-login.php",
            "index.php",
            "xmlrpc.php",
            ".maintenance",
            ".htpasswd",
        ] {
            let path = docroot.join(name);
            if path.is_dir() {
                let _ = fs::remove_dir_all(&path);
            } else if path.is_file() {
                let _ = fs::remove_file(&path);
            }
        }
        let _ = strip_htaccess_auth(&docroot.join(".htaccess"));
    }
    delete_wordpress_site(&domain)?;
    Ok(format!(
        "Removed WordPress registry entry for `{domain}`{}",
        if remove_files {
            " and deleted core files from docroot"
        } else {
            " (files left on disk)"
        }
    ))
}

pub fn all_sites_snapshot() -> Result<Vec<WordpressSiteSnapshot>, String> {
    let wp_cli = detect_wp_cli();
    let mut out = Vec::new();
    for site in list_wordpress_sites() {
        let domain = site.domain.clone();
        let plugins = list_plugins(&domain).unwrap_or_default();
        let themes = list_themes(&domain).unwrap_or_default();
        out.push(WordpressSiteSnapshot {
            site,
            plugins,
            themes,
            wp_cli: wp_cli.clone(),
        });
    }
    Ok(out)
}
