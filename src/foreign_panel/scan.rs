//! Read-only site inventory for a detected panel under a filesystem root.

use std::fs;
use std::path::{Path, PathBuf};

use super::catalog::{PanelSpec, ScanKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteHit {
    pub domain: String,
    pub docroot: String,
    pub owner: String,
    pub databases: Vec<String>,
    pub sql_files: Vec<String>,
    pub mail_dirs: Vec<String>,
    pub zone_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inventory {
    pub sites: Vec<SiteHit>,
    /// True when this panel has a parser. Empty sites can still be a real empty host.
    pub mapped: bool,
    pub note: String,
}

pub fn inventory_for(root: &Path, spec: &PanelSpec) -> Inventory {
    let mut sites = match spec.scan {
        ScanKind::UserDomains => scan_userdomains(root),
        ScanKind::DomainHomes => scan_domain_homes(root),
        ScanKind::VhostHttpdocs => scan_vhost_httpdocs(root),
        ScanKind::DirectAdmin => scan_directadmin(root),
        ScanKind::WebConf => scan_web_conf(root, spec.web_conf_users),
        ScanKind::Virtualmin => scan_virtualmin(root),
        ScanKind::WwwRoot => scan_wwwroot(root),
        ScanKind::Ispconfig => scan_ispconfig(root),
        ScanKind::Ispmanager => super::scan_tail::scan_ispmanager(root),
        ScanKind::ApisCp => super::scan_tail::scan_apiscp(root),
        ScanKind::Sentora => super::scan_tail::scan_sentora(root),
        ScanKind::BlueOnyx => super::scan_tail::scan_blueonyx(root),
        ScanKind::Froxlor => super::scan_tail::scan_froxlor(root),
        ScanKind::Ehcp => super::scan_tail::scan_ehcp(root),
        ScanKind::Easypanel => super::scan_tail::scan_easypanel(root),
        ScanKind::Zesle => super::scan_tail::scan_zesle(root),
        ScanKind::MarkerOnly => Vec::new(),
    };
    for site in &mut sites {
        enrich(root, site);
    }
    sites.sort_by(|a, b| a.domain.cmp(&b.domain));
    sites.dedup_by(|a, b| a.domain == b.domain);
    let mapped = !matches!(spec.scan, ScanKind::MarkerOnly);
    let note = if mapped {
        String::new()
    } else {
        "detected, inventory not mapped yet".into()
    };
    Inventory {
        sites,
        mapped,
        note,
    }
}

pub fn rel(root: &Path, rel_path: &str) -> PathBuf {
    rel_path
        .split('/')
        .filter(|part| !part.is_empty())
        .fold(root.to_path_buf(), |acc, part| acc.join(part))
}

pub fn looks_like_domain(name: &str) -> bool {
    let name = name.trim().trim_end_matches('.').to_ascii_lowercase();
    if name.len() < 3 || name.len() > 253 || !name.contains('.') {
        return false;
    }
    if name.starts_with('.') || name.ends_with('.') || name.contains("..") {
        return false;
    }
    name.chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '.')
}

pub(super) fn push_site(out: &mut Vec<SiteHit>, domain: &str, docroot: PathBuf, owner: &str) {
    if !looks_like_domain(domain) {
        return;
    }
    out.push(SiteHit {
        domain: domain.trim().trim_end_matches('.').to_ascii_lowercase(),
        docroot: docroot.to_string_lossy().into_owned(),
        owner: owner.trim().to_string(),
        databases: Vec::new(),
        sql_files: Vec::new(),
        mail_dirs: Vec::new(),
        zone_files: Vec::new(),
    });
}

fn scan_userdomains(root: &Path) -> Vec<SiteHit> {
    let mut out = Vec::new();
    for rel_path in ["etc/userdomains", "etc/userdatadomains"] {
        let path = rel(root, rel_path);
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((domain, user)) = line.split_once(':') else {
                continue;
            };
            let user = user.trim();
            let docroot = rel(root, &format!("home/{user}/public_html"));
            push_site(&mut out, domain.trim(), docroot, user);
            if let Some(site) = out.last_mut() {
                site.databases.extend(db_keys_in_file(&rel(
                    root,
                    &format!("var/cpanel/users/{user}"),
                )));
            }
        }
    }
    out
}

pub(super) fn scan_domain_homes(root: &Path) -> Vec<SiteHit> {
    let mut out = Vec::new();
    let home = rel(root, "home");
    let Ok(entries) = fs::read_dir(&home) else {
        return out;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let docroot = entry.path().join("public_html");
        if docroot.is_dir() {
            push_site(&mut out, &name, docroot, &name);
        }
    }
    out
}

fn scan_vhost_httpdocs(root: &Path) -> Vec<SiteHit> {
    let mut out = Vec::new();
    let base = rel(root, "var/www/vhosts");
    let Ok(entries) = fs::read_dir(&base) else {
        return out;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let httpdocs = entry.path().join("httpdocs");
        if httpdocs.is_dir() {
            push_site(&mut out, &name, httpdocs, "");
        }
    }
    out
}

fn scan_directadmin(root: &Path) -> Vec<SiteHit> {
    let mut out = Vec::new();
    let home = rel(root, "home");
    let Ok(users) = fs::read_dir(&home) else {
        return out;
    };
    for user in users.flatten() {
        let user_name = user.file_name().to_string_lossy().into_owned();
        let domains = user.path().join("domains");
        let Ok(doms) = fs::read_dir(&domains) else {
            continue;
        };
        for dom in doms.flatten() {
            let name = dom.file_name().to_string_lossy().into_owned();
            let docroot = dom.path().join("public_html");
            if docroot.is_dir() {
                push_site(&mut out, &name, docroot, &user_name);
            }
        }
    }
    out
}

fn scan_web_conf(root: &Path, users_rel: &str) -> Vec<SiteHit> {
    let mut out = Vec::new();
    if users_rel.is_empty() {
        return out;
    }
    let users = rel(root, users_rel);
    let Ok(entries) = fs::read_dir(&users) else {
        return out;
    };
    for entry in entries.flatten() {
        let user = entry.file_name().to_string_lossy().into_owned();
        let web = entry.path().join("web.conf");
        let Ok(text) = fs::read_to_string(&web) else {
            continue;
        };
        let dbs = db_keys_in_file(&entry.path().join("db.conf"));
        for domain in quoted_or_plain(&text, "DOMAIN") {
            let docroot = rel(root, &format!("home/{user}/web/{domain}/public_html"));
            push_site(&mut out, &domain, docroot, &user);
            if let Some(site) = out.last_mut() {
                site.databases.extend(dbs.clone());
            }
        }
    }
    out
}

fn scan_virtualmin(root: &Path) -> Vec<SiteHit> {
    let mut out = Vec::new();
    let dir = rel(root, "etc/webmin/virtual-server/domains");
    let Ok(entries) = fs::read_dir(&dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let Ok(text) = fs::read_to_string(entry.path()) else {
            continue;
        };
        let domain = kv(&text, "dom").unwrap_or_default();
        let user = kv(&text, "user").unwrap_or_default();
        let listed = kv(&text, "public_html_path").unwrap_or_default();
        let docroot = if listed.is_empty() {
            rel(root, &format!("home/{domain}/public_html"))
        } else {
            map_path(root, &listed)
        };
        push_site(&mut out, &domain, docroot, &user);
        if let Some(site) = out.last_mut() {
            for key in ["mysql", "mysql_db", "db"] {
                if let Some(name) = kv(&text, key) {
                    if valid_db(&name) {
                        site.databases.push(name);
                    }
                }
            }
        }
    }
    out
}

fn scan_wwwroot(root: &Path) -> Vec<SiteHit> {
    let mut out = Vec::new();
    collect_domain_dirs(&rel(root, "www/wwwroot"), &mut out, "");
    let vhost = rel(root, "www/server/panel/vhost/nginx");
    if let Ok(entries) = fs::read_dir(&vhost) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(domain) = name.strip_suffix(".conf") else {
                continue;
            };
            let docroot = rel(root, &format!("www/wwwroot/{domain}"));
            if !out
                .iter()
                .any(|site| site.domain == domain.to_ascii_lowercase())
            {
                push_site(&mut out, domain, docroot, "");
            }
        }
    }
    out
}

fn scan_ispconfig(root: &Path) -> Vec<SiteHit> {
    let mut out = Vec::new();
    let www = rel(root, "var/www");
    if let Ok(entries) = fs::read_dir(&www) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let web = entry.path().join("web");
            if web.is_dir() {
                push_site(&mut out, &name, web, "");
            }
        }
    }
    let clients = rel(root, "var/www/clients");
    if let Ok(clients) = fs::read_dir(&clients) {
        for client in clients.flatten() {
            let Ok(webs) = fs::read_dir(client.path()) else {
                continue;
            };
            for web in webs.flatten() {
                let docroot = web.path().join("web");
                let domain_file = web.path().join("domain");
                if docroot.is_dir()
                    && let Ok(domain) = fs::read_to_string(&domain_file)
                {
                    push_site(&mut out, domain.trim(), docroot, "");
                }
            }
        }
    }
    out
}

pub(super) fn collect_domain_dirs(dir: &Path, out: &mut Vec<SiteHit>, owner: &str) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.path().is_dir() {
            push_site(out, &name, entry.path(), owner);
        }
    }
}

fn enrich(root: &Path, site: &mut SiteHit) {
    let docroot = PathBuf::from(&site.docroot);
    if let Some(parent) = docroot.parent()
        && let Ok(entries) = fs::read_dir(parent)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("sql") && path.is_file() {
                site.sql_files.push(path.to_string_lossy().into_owned());
            }
        }
    }
    let mail_rels = [
        format!("home/{}/mail/{}", site.owner, site.domain),
        format!("var/vmail/{}", site.domain),
        format!("home/{}/mail", site.domain),
    ];
    for rel_path in mail_rels {
        if site.owner.is_empty() && rel_path.contains("//") {
            continue;
        }
        let path = rel(root, &rel_path);
        if path.is_dir() {
            site.mail_dirs.push(path.to_string_lossy().into_owned());
        }
    }
    for rel_path in [
        format!("var/named/{}.db", site.domain),
        format!("etc/bind/zones/{}.zone", site.domain),
    ] {
        let path = rel(root, &rel_path);
        if path.is_file() {
            site.zone_files.push(path.to_string_lossy().into_owned());
        }
    }
}

fn db_keys_in_file(path: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let value = line
            .strip_prefix("DB=")
            .or_else(|| line.strip_prefix("DB:"))
            .map(str::trim);
        if let Some(value) = value {
            let value = value.trim_matches('\'').trim_matches('"');
            if valid_db(value) {
                out.push(value.to_string());
            }
        }
    }
    out.extend(quoted_or_plain(&text, "DB"));
    out.sort();
    out.dedup();
    out
}

fn quoted_or_plain(text: &str, key: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix(key) else {
            continue;
        };
        let rest = rest.trim().trim_start_matches('=').trim();
        let value = rest.trim_matches('\'').trim_matches('"').trim();
        if key == "DB" {
            if valid_db(value) {
                out.push(value.to_string());
            }
        } else if looks_like_domain(value) {
            out.push(value.to_ascii_lowercase());
        }
    }
    out
}

fn kv(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix(&format!("{key}=")) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn valid_db(name: &str) -> bool {
    let name = name.trim();
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn map_path(root: &Path, raw: &str) -> PathBuf {
    let mapped = rel(root, raw.trim_start_matches('/'));
    if mapped.exists() {
        return mapped;
    }
    let absolute = PathBuf::from(raw);
    if absolute.exists() { absolute } else { mapped }
}
