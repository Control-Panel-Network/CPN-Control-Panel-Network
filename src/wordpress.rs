//! WordPress site registry JSON under `$CPN_DATA_DIR/wordpress-sites.json`.
//! Secrets (DB/admin passwords) are never stored here after install.

use crate::account::{data_dir, now_unix};
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::PathBuf};

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordpressSite {
    pub id: String,
    pub domain: String,
    pub title: String,
    pub docroot: String,
    pub site_url: String,
    pub admin_user: String,
    pub db_name: String,
    pub db_user: String,
    pub owner: String,
    #[serde(default)]
    pub wp_version: String,
    #[serde(default)]
    pub php_version: String,
    #[serde(default)]
    pub theme: String,
    #[serde(default)]
    pub plugin_count: u32,
    #[serde(default = "default_true")]
    pub search_indexing: bool,
    #[serde(default)]
    pub debugging: bool,
    #[serde(default)]
    pub maintenance: bool,
    #[serde(default)]
    pub password_protection: bool,
    pub created_at_unix: u64,
    pub updated_at_unix: u64,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct WordpressStore {
    #[serde(default = "default_schema")]
    schema_version: u32,
    #[serde(default)]
    sites: Vec<WordpressSite>,
}

fn default_schema() -> u32 {
    SCHEMA_VERSION
}

pub fn wordpress_store_path() -> PathBuf {
    data_dir().join("wordpress-sites.json")
}

fn write_mode_600(path: &PathBuf, contents: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create data dir: {e}"))?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    file.write_all(contents)
        .map_err(|e| format!("Could not save {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn load_store() -> WordpressStore {
    let Ok(raw) = fs::read_to_string(wordpress_store_path()) else {
        return WordpressStore {
            schema_version: SCHEMA_VERSION,
            sites: Vec::new(),
        };
    };
    serde_json::from_str(&raw).unwrap_or(WordpressStore {
        schema_version: SCHEMA_VERSION,
        sites: Vec::new(),
    })
}

fn persist_store(store: &WordpressStore) -> Result<(), String> {
    let json = serde_json::to_string_pretty(store)
        .map_err(|e| format!("Could not serialize WordPress registry: {e}"))?;
    write_mode_600(&wordpress_store_path(), json.as_bytes())
}

/// Ensure the WordPress JSON registry exists (migration hook).
pub fn ensure_wordpress_store_migrated() -> Result<(), String> {
    let path = wordpress_store_path();
    if path.is_file() {
        let _ = load_store();
        return Ok(());
    }
    persist_store(&WordpressStore {
        schema_version: SCHEMA_VERSION,
        sites: Vec::new(),
    })
}

pub fn list_wordpress_sites() -> Vec<WordpressSite> {
    let mut sites = load_store().sites;
    sites.sort_by(|a, b| a.domain.cmp(&b.domain));
    sites
}

pub fn get_wordpress_site(domain_raw: &str) -> Option<WordpressSite> {
    let domain = domain_raw.trim().to_ascii_lowercase();
    load_store()
        .sites
        .into_iter()
        .find(|s| s.domain.eq_ignore_ascii_case(&domain))
}

pub fn upsert_wordpress_site(site: WordpressSite) -> Result<WordpressSite, String> {
    let mut store = load_store();
    store.schema_version = SCHEMA_VERSION;
    if let Some(existing) = store
        .sites
        .iter_mut()
        .find(|s| s.domain.eq_ignore_ascii_case(&site.domain))
    {
        *existing = site.clone();
    } else {
        store.sites.push(site.clone());
    }
    persist_store(&store)?;
    Ok(site)
}

pub fn delete_wordpress_site(domain_raw: &str) -> Result<(), String> {
    let domain = domain_raw.trim().to_ascii_lowercase();
    if domain.is_empty() {
        return Err("Domain is required".into());
    }
    let mut store = load_store();
    let before = store.sites.len();
    store
        .sites
        .retain(|s| !s.domain.eq_ignore_ascii_case(&domain));
    if store.sites.len() == before {
        return Err(format!("WordPress site `{domain}` not found"));
    }
    store.schema_version = SCHEMA_VERSION;
    persist_store(&store)
}

pub fn new_site_id() -> String {
    format!("wp-{}", now_unix())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn registry_roundtrip() {
        with_test_data_dir(|| {
            ensure_wordpress_store_migrated().unwrap();
            let site = WordpressSite {
                id: new_site_id(),
                domain: "blog.example.com".into(),
                title: "Blog".into(),
                docroot: "/home/example.com/blog.example.com/public_html".into(),
                site_url: "https://blog.example.com".into(),
                admin_user: "admin".into(),
                db_name: "wp_blog".into(),
                db_user: "wp_blog".into(),
                owner: "admin".into(),
                wp_version: "6.7".into(),
                php_version: "8.3".into(),
                theme: "twentytwentyfive".into(),
                plugin_count: 2,
                search_indexing: true,
                debugging: false,
                maintenance: false,
                password_protection: false,
                created_at_unix: now_unix(),
                updated_at_unix: now_unix(),
            };
            upsert_wordpress_site(site.clone()).unwrap();
            let loaded = get_wordpress_site("blog.example.com").unwrap();
            assert_eq!(loaded.title, "Blog");
            assert_eq!(list_wordpress_sites().len(), 1);
            delete_wordpress_site("blog.example.com").unwrap();
            assert!(get_wordpress_site("blog.example.com").is_none());
        });
    }
}
