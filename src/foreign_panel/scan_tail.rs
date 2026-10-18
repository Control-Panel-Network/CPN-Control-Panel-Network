//! Long-tail panel layouts. Primary layouts stay in `scan.rs`.

use std::fs;
use std::path::Path;

use super::scan::{
    SiteHit, collect_domain_dirs, looks_like_domain, push_site, rel, scan_domain_homes,
};

pub(super) fn scan_ispmanager(root: &Path) -> Vec<SiteHit> {
    let mut out = Vec::new();
    let www = rel(root, "var/www");
    let Ok(users) = fs::read_dir(&www) else {
        return out;
    };
    for user in users.flatten() {
        let sites = user.path().join("data").join("www");
        let Ok(doms) = fs::read_dir(&sites) else {
            continue;
        };
        let owner = user.file_name().to_string_lossy().into_owned();
        for dom in doms.flatten() {
            let name = dom.file_name().to_string_lossy().into_owned();
            if dom.path().is_dir() {
                push_site(&mut out, &name, dom.path(), &owner);
            }
        }
    }
    out
}

pub(super) fn scan_apiscp(root: &Path) -> Vec<SiteHit> {
    let mut out = scan_domain_homes(root);
    let home = rel(root, "home");
    let Ok(entries) = fs::read_dir(&home) else {
        return out;
    };
    for entry in entries.flatten() {
        let html = entry.path().join("var").join("www").join("html");
        let domain_file = entry.path().join("domain");
        if html.is_dir()
            && let Ok(domain) = fs::read_to_string(&domain_file)
        {
            let owner = entry.file_name().to_string_lossy().into_owned();
            push_site(&mut out, domain.trim(), html, &owner);
        }
    }
    out
}

pub(super) fn scan_sentora(root: &Path) -> Vec<SiteHit> {
    let mut out = Vec::new();
    let base = rel(root, "var/sentora/hostdata");
    let Ok(entries) = fs::read_dir(&base) else {
        return out;
    };
    for entry in entries.flatten() {
        let docroot = entry.path().join("public_html");
        if !docroot.is_dir() {
            continue;
        }
        let owner = entry.file_name().to_string_lossy().into_owned();
        let domain = fs::read_to_string(entry.path().join("domain")).unwrap_or_default();
        let domain = if looks_like_domain(domain.trim()) {
            domain.trim().to_string()
        } else {
            owner.clone()
        };
        push_site(&mut out, &domain, docroot, &owner);
    }
    out
}

pub(super) fn scan_blueonyx(root: &Path) -> Vec<SiteHit> {
    let mut out = Vec::new();
    let dir = rel(root, "home/sites");
    let Ok(entries) = fs::read_dir(&dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let docroot = entry.path().join("web");
        if docroot.is_dir() {
            push_site(&mut out, &name, docroot, "");
        }
    }
    out
}

pub(super) fn scan_froxlor(root: &Path) -> Vec<SiteHit> {
    let mut out = Vec::new();
    let webs = rel(root, "var/customers/webs");
    if let Ok(users) = fs::read_dir(&webs) {
        for user in users.flatten() {
            let owner = user.file_name().to_string_lossy().into_owned();
            let Ok(doms) = fs::read_dir(user.path()) else {
                continue;
            };
            for dom in doms.flatten() {
                let name = dom.file_name().to_string_lossy().into_owned();
                if dom.path().is_dir() {
                    push_site(&mut out, &name, dom.path(), &owner);
                }
            }
        }
    }
    collect_domain_dirs(&rel(root, "var/www"), &mut out, "");
    out
}

pub(super) fn scan_ehcp(root: &Path) -> Vec<SiteHit> {
    let mut out = Vec::new();
    let base = rel(root, "var/www/vhosts");
    let Ok(entries) = fs::read_dir(&base) else {
        return out;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let httpdocs = entry.path().join("httpdocs");
        let public = entry.path().join("public_html");
        let docroot = if httpdocs.is_dir() { httpdocs } else { public };
        if docroot.is_dir() {
            push_site(&mut out, &name, docroot, "");
        }
    }
    out
}

pub(super) fn scan_easypanel(root: &Path) -> Vec<SiteHit> {
    let mut out = Vec::new();
    let projects = rel(root, "etc/easypanel/projects");
    let Ok(entries) = fs::read_dir(&projects) else {
        return out;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let domain = fs::read_to_string(entry.path().join("domain")).unwrap_or(name);
        let public = entry.path().join("public");
        let docroot = if public.is_dir() {
            public
        } else {
            entry.path()
        };
        push_site(&mut out, domain.trim(), docroot, "");
    }
    out
}

pub(super) fn scan_zesle(root: &Path) -> Vec<SiteHit> {
    let mut out = scan_domain_homes(root);
    collect_domain_dirs(&rel(root, "usr/local/zesle/domains"), &mut out, "");
    out
}
