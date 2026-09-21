//! Sidebar section visibility via per-user ACL grants and package entitlements.
//!
//! Store: `$CPN_DATA_DIR/sidebar-visibility.json` (mode 600).
//! Default: every section visible. Panel admin keeps full access unless a grant
//! sets `restrict_admin` for that member.

use crate::account::{data_dir, now_unix};
use crate::packages::package_for_account;
use crate::panel_admin::is_panel_admin;
use crate::panel_nav_catalog::{ACCOUNT, ADMINISTRATION, HOSTING, NavEntry};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy)]
pub struct NavVisibilityItem {
    pub id: &'static str,
    pub label: &'static str,
    pub href: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidebarVisibilityGrant {
    pub member: String,
    /// Top-level nav entry ids to hide for this member.
    #[serde(default)]
    pub hidden_nav_ids: Vec<String>,
    /// When true, also apply to the bootstrap panel admin account.
    #[serde(default)]
    pub restrict_admin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SidebarVisibilityFile {
    #[serde(default)]
    schema_version: u32,
    #[serde(default)]
    grants: Vec<SidebarVisibilityGrant>,
    #[serde(default)]
    updated_at_unix: u64,
}

fn path() -> PathBuf {
    data_dir().join("sidebar-visibility.json")
}

fn write_mode_600(path: &PathBuf, contents: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
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

fn load_file() -> SidebarVisibilityFile {
    let Ok(raw) = fs::read_to_string(path()) else {
        return SidebarVisibilityFile {
            schema_version: SCHEMA_VERSION,
            grants: Vec::new(),
            updated_at_unix: 0,
        };
    };
    let mut loaded: SidebarVisibilityFile = serde_json::from_str(&raw).unwrap_or_default();
    loaded.schema_version = SCHEMA_VERSION;
    loaded
}

fn save_file(file: &SidebarVisibilityFile) -> Result<(), String> {
    let mut out = file.clone();
    out.schema_version = SCHEMA_VERSION;
    out.updated_at_unix = now_unix();
    let json = serde_json::to_string_pretty(&out)
        .map_err(|e| format!("Could not serialize sidebar visibility: {e}"))?;
    write_mode_600(&path(), json.as_bytes())
}

fn names_equal(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// Controllable top-level sidebar sections (catalog entry ids).
pub fn controllable_nav_items() -> Vec<NavVisibilityItem> {
    let mut items = Vec::new();
    for section in [HOSTING, ACCOUNT, ADMINISTRATION] {
        for entry in section {
            match *entry {
                NavEntry::Link { id, href, label } => {
                    if id == "dashboard" {
                        continue;
                    }
                    items.push(NavVisibilityItem { id, label, href });
                }
                NavEntry::Group {
                    id, href, label, ..
                } => {
                    items.push(NavVisibilityItem { id, label, href });
                }
            }
        }
    }
    items
}

pub fn known_nav_ids() -> HashSet<&'static str> {
    controllable_nav_items().into_iter().map(|i| i.id).collect()
}

fn sanitize_hidden_ids(raw: &[String]) -> Result<Vec<String>, String> {
    let known = known_nav_ids();
    let mut out = Vec::new();
    for id in raw {
        let trimmed = id.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "dashboard" {
            return Err("Dashboard cannot be hidden".into());
        }
        if !known.contains(trimmed) {
            return Err(format!("Unknown sidebar section `{trimmed}`"));
        }
        if !out.iter().any(|x: &String| x == trimmed) {
            out.push(trimmed.to_string());
        }
    }
    Ok(out)
}

/// Validate package-level sidebar hidden ids (same rules as ACL grants).
pub fn sanitize_package_hidden_ids(raw: &[String]) -> Result<Vec<String>, String> {
    sanitize_hidden_ids(raw)
}

pub fn list_grants() -> Vec<SidebarVisibilityGrant> {
    load_file().grants
}

pub fn set_grant_for_member(
    member: &str,
    hidden_nav_ids: Vec<String>,
    restrict_admin: bool,
) -> Result<(), String> {
    let member = member.trim();
    if member.is_empty() {
        return Err("Member username is required".into());
    }
    let hidden = sanitize_hidden_ids(&hidden_nav_ids)?;
    let mut file = load_file();
    if let Some(existing) = file
        .grants
        .iter_mut()
        .find(|g| names_equal(&g.member, member))
    {
        existing.hidden_nav_ids = hidden;
        existing.restrict_admin = restrict_admin;
    } else {
        file.grants.push(SidebarVisibilityGrant {
            member: member.to_string(),
            hidden_nav_ids: hidden,
            restrict_admin,
        });
    }
    save_file(&file)
}

pub fn remove_grant_at(index: usize) -> Result<(), String> {
    let mut file = load_file();
    if index >= file.grants.len() {
        return Err("Sidebar visibility grant not found".into());
    }
    file.grants.remove(index);
    save_file(&file)
}

fn grant_for(username: &str) -> Option<SidebarVisibilityGrant> {
    load_file()
        .grants
        .into_iter()
        .find(|g| names_equal(&g.member, username))
}

fn package_hidden_ids(username: &str) -> Vec<String> {
    package_for_account(username)
        .map(|pkg| pkg.sidebar_hidden_nav_ids)
        .unwrap_or_default()
}

/// Whether this user may see the given top-level nav entry id in the sidebar.
pub fn can_see_nav_id(username: &str, nav_id: &str) -> bool {
    if nav_id == "dashboard" || nav_id.is_empty() {
        return true;
    }
    let admin = is_panel_admin(username);
    let grant = grant_for(username);
    if admin {
        match &grant {
            Some(g) if g.restrict_admin => {
                if g.hidden_nav_ids.iter().any(|id| id == nav_id) {
                    return false;
                }
            }
            _ => return true,
        }
    } else if let Some(g) = &grant
        && g.hidden_nav_ids.iter().any(|id| id == nav_id)
    {
        return false;
    }
    if package_hidden_ids(username).iter().any(|id| id == nav_id) {
        return false;
    }
    true
}

fn catalog_href_map() -> Vec<(&'static str, &'static str)> {
    let mut pairs = Vec::new();
    for section in [HOSTING, ACCOUNT, ADMINISTRATION] {
        for entry in section {
            match *entry {
                NavEntry::Link { id, href, .. } => pairs.push((href, id)),
                NavEntry::Group {
                    id, href, children, ..
                } => {
                    pairs.push((href, id));
                    for child in children {
                        pairs.push((child.href, id));
                    }
                }
            }
        }
    }
    pairs.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    pairs
}

/// Map a request path to a top-level nav id when it belongs to the catalog.
pub fn nav_id_for_path(path: &str) -> Option<&'static str> {
    let path = path.split('?').next().unwrap_or(path).trim_end_matches('/');
    if path.is_empty() || path == "/" {
        return Some("dashboard");
    }
    for (href, id) in catalog_href_map() {
        let href = href.split('?').next().unwrap_or(href).trim_end_matches('/');
        if path == href || path.starts_with(&format!("{href}/")) {
            return Some(id);
        }
    }
    // Common aliases outside exact catalog children.
    if path.starts_with("/websites") {
        return Some("websites");
    }
    if path.starts_with("/wordpress") {
        return Some("wordpress");
    }
    if path.starts_with("/email") {
        return Some("email");
    }
    if path.starts_with("/databases") || path.starts_with("/ftp") {
        return Some("databases");
    }
    if path.starts_with("/backups") {
        return Some("backups");
    }
    if path.starts_with("/plugins") || path.starts_with("/apps") {
        return Some("plugins");
    }
    if path.starts_with("/account") {
        return Some("users");
    }
    if path.starts_with("/packages") {
        return Some("packages");
    }
    if path.starts_with("/server") {
        if path.starts_with("/server/files") {
            return Some("root-files");
        }
        return Some("server");
    }
    if path.starts_with("/security") {
        return Some("security");
    }
    if path.starts_with("/settings") {
        return Some("settings");
    }
    None
}

/// Paths that must never be blocked by sidebar visibility.
pub fn path_is_exempt(path: &str) -> bool {
    let path = path.split('?').next().unwrap_or(path);
    matches!(
        path,
        "/" | "/login"
            | "/login/2fa"
            | "/logout"
            | "/status"
            | "/api/status"
            | "/favicon.ico"
            | "/favicon.svg"
            | "/apple-touch-icon.png"
            | "/cpn-logo.svg"
            | "/cpn-brand-mark.svg"
    ) || path.starts_with("/api/")
        || path.starts_with("/install")
        || path.starts_with("/phpmyadmin")
        || path.starts_with("/webmail")
        || path.starts_with("/assets/")
        || path.starts_with("/static/")
}

/// Whether the signed-in user may open this panel path.
pub fn path_allowed(username: &str, path: &str) -> bool {
    if path_is_exempt(path) {
        return true;
    }
    match nav_id_for_path(path) {
        Some(id) => can_see_nav_id(username, id),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::{
        PanelBootstrap, default_password_policy, new_password_salt, with_test_data_dir,
        write_account_file,
    };

    fn write_admin() {
        let salt = new_password_salt();
        let boot = PanelBootstrap {
            schema_version: 1,
            username: "owner".into(),
            recovery_email: "owner@example.com".into(),
            password_hash: "x".into(),
            password_salt: salt,
            password_policy: default_password_policy(),
            language: "en".into(),
            created_at_unix: 1,
            must_change_password: false,
            totp_required: false,
        };
        write_account_file(&crate::account::bootstrap_path(), &boot).expect("bootstrap");
    }

    #[test]
    fn admin_full_by_default_guest_can_be_restricted() {
        with_test_data_dir(|| {
            write_admin();
            assert!(can_see_nav_id("owner", "backups"));
            set_grant_for_member("ops", vec!["backups".into()], false).unwrap();
            assert!(!can_see_nav_id("ops", "backups"));
            assert!(can_see_nav_id("ops", "websites"));
            assert!(path_allowed("ops", "/dashboard"));
            assert!(!path_allowed("ops", "/backups/create"));
            assert_eq!(nav_id_for_path("/backups/create"), Some("backups"));
        });
    }

    #[test]
    fn dashboard_cannot_be_hidden() {
        with_test_data_dir(|| {
            let err = set_grant_for_member("ops", vec!["dashboard".into()], false).unwrap_err();
            assert!(err.contains("Dashboard"));
        });
    }
}
