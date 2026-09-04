//! Site-scoped app pieces under `/home/<domain>/...` (nested for subdomains).
//!
//! Host engines (MariaDB, MySQL, RabbitMQ) stay system packages. This module
//! drops markers/links under the selected domain home and records ACL associations.

use crate::account::{data_dir, now_unix};
use crate::apps::AppId;
use crate::sites::{load_site, site_home_from_record};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppBinding {
    pub app: String,
    pub domain: String,
    #[serde(default)]
    pub path: String,
    pub updated_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AppBindingsFile {
    #[serde(default)]
    schema_version: u32,
    #[serde(default)]
    bindings: Vec<AppBinding>,
}

fn bindings_path() -> PathBuf {
    data_dir().join("app-bindings.json")
}

fn load_bindings() -> AppBindingsFile {
    let Ok(raw) = fs::read_to_string(bindings_path()) else {
        return AppBindingsFile {
            schema_version: SCHEMA_VERSION,
            bindings: Vec::new(),
        };
    };
    serde_json::from_str(&raw).unwrap_or(AppBindingsFile {
        schema_version: SCHEMA_VERSION,
        bindings: Vec::new(),
    })
}

fn save_bindings(file: &AppBindingsFile) -> Result<(), String> {
    let path = bindings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create data dir: {e}"))?;
    }
    let mut out = file.clone();
    out.schema_version = SCHEMA_VERSION;
    let raw = serde_json::to_string_pretty(&out)
        .map_err(|e| format!("Could not serialize app bindings: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("Could not write app bindings: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn bindings_for_domain(domain: &str) -> Vec<AppBinding> {
    load_bindings()
        .bindings
        .into_iter()
        .filter(|b| b.domain.eq_ignore_ascii_case(domain.trim()))
        .collect()
}

pub fn list_bindings() -> Vec<AppBinding> {
    load_bindings().bindings
}

fn upsert_binding(app: AppId, domain: &str, path: &str) -> Result<(), String> {
    let mut file = load_bindings();
    file.bindings
        .retain(|b| !(b.app == app.as_str() && b.domain.eq_ignore_ascii_case(domain)));
    file.bindings.push(AppBinding {
        app: app.as_str().into(),
        domain: domain.to_string(),
        path: path.to_string(),
        updated_at_unix: now_unix(),
    });
    save_bindings(&file)
}

fn remove_binding(app: AppId, domain: &str) -> Result<(), String> {
    let mut file = load_bindings();
    file.bindings
        .retain(|b| !(b.app == app.as_str() && b.domain.eq_ignore_ascii_case(domain)));
    save_bindings(&file)
}

fn site_apps_dir(domain: &str) -> Result<PathBuf, String> {
    let site = load_site(domain)?;
    Ok(site_home_from_record(&site).join("apps"))
}

fn write_note(path: &Path, body: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
    }
    fs::write(path, body).map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o644));
    }
    Ok(())
}

fn try_symlink(target: &Path, link: &Path) -> Result<(), String> {
    if link.exists() {
        return Ok(());
    }
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
            .map_err(|e| format!("Could not link {}: {e}", link.display()))?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = (target, link);
        Ok(())
    }
}

/// Whether this app drops content under the selected domain home.
pub fn is_site_scoped(app: AppId) -> bool {
    matches!(app, AppId::Phpmyadmin | AppId::Email)
}

/// Host engines may still be associated with a domain for ACL/display only.
pub fn is_associable(app: AppId) -> bool {
    matches!(
        app,
        AppId::Mariadb | AppId::Mysql | AppId::Rabbitmq | AppId::Phpmyadmin | AppId::Email
    )
}

pub fn apply_site_scope(app: AppId, domain: &str) -> Result<String, String> {
    let site = load_site(domain)?;
    let home = site_home_from_record(&site);
    let apps = home.join("apps");
    fs::create_dir_all(&apps).map_err(|e| format!("Could not create {}: {e}", apps.display()))?;

    match app {
        AppId::Phpmyadmin => {
            let dest = apps.join("phpmyadmin");
            let candidates = [
                PathBuf::from("/usr/share/phpMyAdmin"),
                PathBuf::from("/usr/share/phpmyadmin"),
            ];
            let mut linked = false;
            for cand in &candidates {
                if cand.is_dir() {
                    let _ = fs::remove_file(&dest);
                    let _ = fs::remove_dir_all(&dest);
                    try_symlink(cand, &dest)?;
                    linked = true;
                    break;
                }
            }
            if !linked {
                write_note(
                    &dest.join("README.txt"),
                    "phpMyAdmin packages are installed on the host. Point a vhost document root at the distro phpMyAdmin share, or re-run after packages are present.\n",
                )?;
            }
            upsert_binding(app, &site.domain, &dest.display().to_string())?;
            Ok(format!(
                "Associated phpMyAdmin with `{}` under {}",
                site.domain,
                dest.display()
            ))
        }
        AppId::Email => {
            let dest = apps.join("webmail");
            write_note(
                &dest.join("README.txt"),
                "Email stack is host-level (Postfix + Dovecot). Use this domain/subdomain home for mailbox layout and webmail vhost wiring. Local webmail health URL is provisioned by the installer mail stage when enabled.\n",
            )?;
            upsert_binding(app, &site.domain, &dest.display().to_string())?;
            Ok(format!(
                "Associated Email/webmail path with `{}` under {}",
                site.domain,
                dest.display()
            ))
        }
        AppId::Mariadb | AppId::Mysql | AppId::Rabbitmq => {
            upsert_binding(app, &site.domain, "")?;
            Ok(format!(
                "Associated host app `{}` with `{}` for ACL/display (engine stays system-wide)",
                app.as_str(),
                site.domain
            ))
        }
    }
}

pub fn clear_site_scope(app: AppId, domain: &str) -> Result<String, String> {
    let site = load_site(domain)?;
    let apps = site_apps_dir(&site.domain)?;
    match app {
        AppId::Phpmyadmin => {
            let dest = apps.join("phpmyadmin");
            let _ = fs::remove_file(&dest);
            let _ = fs::remove_dir_all(&dest);
        }
        AppId::Email => {
            let dest = apps.join("webmail");
            let _ = fs::remove_dir_all(&dest);
        }
        _ => {}
    }
    remove_binding(app, &site.domain)?;
    Ok(format!(
        "Cleared `{}` association for `{}`",
        app.as_str(),
        site.domain
    ))
}
