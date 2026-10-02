//! Discover WordPress installs under registered site docroots and nested
//! sub-domain homes (for example `/home/parent.tld/sub.parent.tld/public_html`).

use crate::account::now_unix;
use crate::sites::{
    create_site, list_sites, load_site, normalize_domain, resolve_parent_domain,
    site_home_from_record, SiteRecord,
};
use crate::wordpress::{
    WordpressSite, get_wordpress_site, list_wordpress_sites, new_site_id, upsert_wordpress_site,
};
use crate::wordpress_manage::{WordpressRefreshResult, refresh_wordpress_site};
use crate::wordpress_wpcli::is_wordpress_docroot;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
struct WpCandidate {
    domain: String,
    docroot: String,
    owner: String,
}

fn skipped_home_child(name: &str) -> bool {
    matches!(
        name,
        "public_html"
            | "backups"
            | "logs"
            | "plugins"
            | "private"
            | "tmp"
            | "mail"
            | "etc"
            | "ssl"
            | "cgi-bin"
            | "home"
            | "users"
    ) || name.starts_with('.')
}

fn looks_like_domain_dir(name: &str) -> bool {
    if skipped_home_child(name) || !name.contains('.') {
        return false;
    }
    normalize_domain(name).is_ok()
}

/// Prefer `public_html`, then the home directory itself (restored layouts).
fn wordpress_docroot_under(home: &Path) -> Option<PathBuf> {
    let public_html = home.join("public_html");
    if is_wordpress_docroot(&public_html) {
        return Some(public_html);
    }
    if is_wordpress_docroot(home) {
        return Some(home.to_path_buf());
    }
    None
}

fn push_candidate(map: &mut BTreeMap<String, WpCandidate>, candidate: WpCandidate) {
    let key = candidate.domain.to_ascii_lowercase();
    map.entry(key).or_insert(candidate);
}

fn candidates_from_registered(sites: &[SiteRecord]) -> BTreeMap<String, WpCandidate> {
    let mut map = BTreeMap::new();
    for site in sites {
        let docroot = PathBuf::from(&site.docroot);
        if !is_wordpress_docroot(&docroot) {
            continue;
        }
        push_candidate(
            &mut map,
            WpCandidate {
                domain: site.domain.clone(),
                docroot: site.docroot.clone(),
                owner: site.owner.clone(),
            },
        );
    }
    map
}

/// Walk primary site homes for nested `*.domain` folders with WordPress.
fn discover_nested_under_primaries(
    sites: &[SiteRecord],
    map: &mut BTreeMap<String, WpCandidate>,
) {
    for site in sites {
        let is_primary = resolve_parent_domain(&site.domain)
            .ok()
            .flatten()
            .is_none();
        if !is_primary {
            continue;
        }
        let home = site_home_from_record(site);
        if !home.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(&home) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if !looks_like_domain_dir(&name) {
                continue;
            }
            let Ok(domain) = normalize_domain(&name) else {
                continue;
            };
            let parent_ok = resolve_parent_domain(&domain)
                .ok()
                .flatten()
                .as_deref()
                == Some(site.domain.as_str());
            if !parent_ok {
                continue;
            }
            let child_home = entry.path();
            let Some(docroot) = wordpress_docroot_under(&child_home) else {
                continue;
            };
            push_candidate(
                map,
                WpCandidate {
                    domain,
                    docroot: docroot.to_string_lossy().into_owned(),
                    owner: site.owner.clone(),
                },
            );
        }
    }
}

fn ensure_cpn_site(candidate: &WpCandidate) {
    if load_site(&candidate.domain).is_ok() {
        return;
    }
    let _ = create_site(
        &candidate.domain,
        &candidate.owner,
        Some(&candidate.docroot),
        None,
        Some("Discovered by WordPress scan"),
    );
}

fn ensure_wordpress_registry(candidate: &WpCandidate) -> Result<(), String> {
    if get_wordpress_site(&candidate.domain).is_some() {
        return Ok(());
    }
    let now = now_unix();
    let stub = WordpressSite {
        id: new_site_id(),
        domain: candidate.domain.clone(),
        title: candidate.domain.clone(),
        docroot: candidate.docroot.clone(),
        site_url: format!("https://{}", candidate.domain),
        admin_user: "-".into(),
        db_name: "-".into(),
        db_user: "-".into(),
        owner: candidate.owner.clone(),
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
    Ok(())
}

/// Scan registered sites plus nested sub-domain homes; register and refresh.
pub fn scan_wordpress_sites() -> Result<Vec<WordpressRefreshResult>, String> {
    let sites = list_sites().unwrap_or_default();
    let mut map = candidates_from_registered(&sites);
    discover_nested_under_primaries(&sites, &mut map);

    for existing in list_wordpress_sites() {
        if map.contains_key(&existing.domain.to_ascii_lowercase()) {
            continue;
        }
        push_candidate(
            &mut map,
            WpCandidate {
                domain: existing.domain.clone(),
                docroot: existing.docroot.clone(),
                owner: existing.owner.clone(),
            },
        );
    }

    let mut results = Vec::new();
    for candidate in map.values() {
        ensure_cpn_site(candidate);
        ensure_wordpress_registry(candidate)?;
        match refresh_wordpress_site(&candidate.domain) {
            Ok(refreshed) => results.push(refreshed),
            Err(note) => {
                if let Some(site) = get_wordpress_site(&candidate.domain) {
                    results.push(WordpressRefreshResult {
                        site,
                        notes: vec![note],
                    });
                }
            }
        }
    }
    results.sort_by(|a, b| a.site.domain.cmp(&b.site.domain));
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::sites::create_site;

    fn touch_wp(docroot: &Path) {
        fs::create_dir_all(docroot).unwrap();
        fs::write(docroot.join("wp-config.php"), b"<?php\n").unwrap();
        fs::create_dir_all(docroot.join("wp-includes")).unwrap();
    }

    #[test]
    fn scan_finds_nested_subdomain_wordpress() {
        with_test_data_dir(|| {
            let parent = create_site("example.com", "owner", None, None, None).unwrap();
            let nested_home = PathBuf::from(std::env::var("CPN_SITES_HOME").unwrap())
                .join("example.com")
                .join("blog.example.com");
            let docroot = nested_home.join("public_html");
            touch_wp(&docroot);

            assert!(load_site("blog.example.com").is_err());
            assert!(is_wordpress_docroot(&docroot));
            assert_eq!(parent.domain, "example.com");

            let results = scan_wordpress_sites().expect("scan");
            assert!(
                results
                    .iter()
                    .any(|r| r.site.domain == "blog.example.com"),
                "expected blog.example.com in scan results: {:?}",
                results
                    .iter()
                    .map(|r| r.site.domain.clone())
                    .collect::<Vec<_>>()
            );
            assert!(load_site("blog.example.com").is_ok());
            assert!(get_wordpress_site("blog.example.com").is_some());
            let listed = list_wordpress_sites();
            assert!(listed.iter().any(|s| s.domain == "blog.example.com"));
        });
    }

    #[test]
    fn scan_finds_wordpress_at_nested_home_without_public_html() {
        with_test_data_dir(|| {
            let _ = create_site("example.com", "owner", None, None, None).unwrap();
            let nested_home = PathBuf::from(std::env::var("CPN_SITES_HOME").unwrap())
                .join("example.com")
                .join("wp-test.example.com");
            touch_wp(&nested_home);

            let results = scan_wordpress_sites().expect("scan");
            assert!(results
                .iter()
                .any(|r| r.site.domain == "wp-test.example.com"));
            let site = get_wordpress_site("wp-test.example.com").unwrap();
            assert!(site.docroot.ends_with("wp-test.example.com"));
        });
    }
}
