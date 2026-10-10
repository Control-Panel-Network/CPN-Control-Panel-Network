//! Reseller Center v1: hierarchy, store, and branding.
//!
//! Store: `$CPN_DATA_DIR/resellers.json` (mode 600). Account role / parent live on
//! `PanelBootstrap` (`role`, `parent_reseller`) with serde defaults for older files.
//! Quota helpers: `panel_reseller_quota`.

use crate::account::{now_unix, write_account_file};
use crate::account_mgmt::{find_account, list_accounts, usernames_equal};
use crate::packages::is_panel_admin;
use crate::panel_reseller_quota::{
    QuotaSlice, add_limited, package_slice_for, quotas_cover_committed,
};
use crate::paths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

pub use crate::panel_reseller_quota::{ResellerQuotas, format_quota_cell, quotas_from_fields};

pub const ROLE_USER: &str = "user";
pub const ROLE_RESELLER: &str = "reseller";
const STORE_SCHEMA: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResellerBranding {
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub tagline: String,
    #[serde(default)]
    pub logo_url: String,
    #[serde(default)]
    pub primary_color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResellerRecord {
    pub username: String,
    pub quotas: ResellerQuotas,
    #[serde(default)]
    pub branding: ResellerBranding,
    pub created_at_unix: u64,
    pub updated_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ResellersFile {
    #[serde(default)]
    schema_version: u32,
    #[serde(default)]
    resellers: Vec<ResellerRecord>,
}

pub fn resellers_path() -> PathBuf {
    paths::join_data("resellers.json")
}

pub fn ensure_reseller_store_migrated() -> Result<(), String> {
    let path = resellers_path();
    if path.is_file() {
        let _ = load_file()?;
        return Ok(());
    }
    save_file(&ResellersFile {
        schema_version: STORE_SCHEMA,
        resellers: Vec::new(),
    })
}

fn write_mode_600(path: &PathBuf, contents: &[u8]) -> Result<(), String> {
    let dir = paths::default_data_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
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
        .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn load_file() -> Result<ResellersFile, String> {
    let path = resellers_path();
    if !path.is_file() {
        return Ok(ResellersFile {
            schema_version: STORE_SCHEMA,
            resellers: Vec::new(),
        });
    }
    let raw =
        fs::read_to_string(&path).map_err(|e| format!("Could not read {}: {e}", path.display()))?;
    let mut file: ResellersFile =
        serde_json::from_str(&raw).map_err(|e| format!("Invalid resellers.json: {e}"))?;
    file.schema_version = STORE_SCHEMA;
    Ok(file)
}

fn save_file(file: &ResellersFile) -> Result<(), String> {
    let mut out = file.clone();
    out.schema_version = STORE_SCHEMA;
    let json = serde_json::to_string_pretty(&out)
        .map_err(|e| format!("Could not encode resellers.json: {e}"))?;
    write_mode_600(&resellers_path(), json.as_bytes())
}

pub fn list_resellers() -> Result<Vec<ResellerRecord>, String> {
    let mut rows = load_file()?.resellers;
    rows.sort_by(|a, b| {
        a.username
            .to_ascii_lowercase()
            .cmp(&b.username.to_ascii_lowercase())
    });
    Ok(rows)
}

pub fn get_reseller(username: &str) -> Result<Option<ResellerRecord>, String> {
    let file = load_file()?;
    Ok(file
        .resellers
        .into_iter()
        .find(|r| usernames_equal(&r.username, username)))
}

pub fn normalize_role(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        ROLE_RESELLER => ROLE_RESELLER.into(),
        _ => String::new(),
    }
}

pub fn account_is_reseller(username: &str) -> bool {
    if is_panel_admin(username) {
        return false;
    }
    if let Ok(Some(_)) = get_reseller(username) {
        return true;
    }
    find_account(username)
        .map(|(boot, _)| normalize_role(&boot.role) == ROLE_RESELLER)
        .unwrap_or(false)
}

pub fn can_access_reseller_center(username: &str) -> bool {
    is_panel_admin(username) || account_is_reseller(username)
}

/// Hub / nav: Reseller Center is for owner/admin and resellers only.
pub fn is_reseller_center_href(href: &str) -> bool {
    let path = href.split(['?', '#']).next().unwrap_or(href);
    path == "/account/users/reseller"
}

pub fn can_manage_account(actor: &str, target: &str) -> bool {
    if is_panel_admin(actor) || usernames_equal(actor, target) {
        return true;
    }
    if !account_is_reseller(actor) {
        return false;
    }
    find_account(target)
        .map(|(boot, _)| usernames_equal(&boot.parent_reseller, actor))
        .unwrap_or(false)
}

pub fn child_usernames(reseller: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for acct in list_accounts()? {
        if let Ok((boot, _)) = find_account(&acct.username)
            && usernames_equal(&boot.parent_reseller, reseller)
        {
            out.push(boot.username);
        }
    }
    out.sort_by_key(|u| u.to_ascii_lowercase());
    Ok(out)
}

/// Sum of assigned package caps for reseller children (not live usage).
pub fn pool_committed(reseller: &str) -> Result<QuotaSlice, String> {
    let mut out = QuotaSlice::default();
    for child in child_usernames(reseller)? {
        let slice = package_slice_for(&child);
        add_limited(&mut out.websites, slice.websites)?;
        add_limited(&mut out.mailboxes, slice.mailboxes)?;
        add_limited(&mut out.databases, slice.databases)?;
        add_limited(&mut out.ftp_accounts, slice.ftp_accounts)?;
        add_limited(&mut out.storage_mb, slice.storage_mb)?;
        add_limited(&mut out.bandwidth_mb, slice.bandwidth_mb)?;
    }
    Ok(out)
}

fn set_account_fields(username: &str, role: &str, parent_reseller: &str) -> Result<(), String> {
    let (mut boot, path) = find_account(username)?;
    if is_panel_admin(&boot.username) {
        return Err("The panel owner account cannot become a reseller or child user".into());
    }
    boot.role = normalize_role(role);
    boot.parent_reseller = parent_reseller.trim().to_string();
    write_account_file(&path, &boot)
}

pub fn promote_reseller(
    username_raw: &str,
    quotas: ResellerQuotas,
) -> Result<ResellerRecord, String> {
    let (boot, _) = find_account(username_raw)?;
    if is_panel_admin(&boot.username) {
        return Err("The panel owner cannot be promoted to reseller".into());
    }
    if !boot.parent_reseller.trim().is_empty() {
        return Err("Unassign this user from their parent reseller before promoting".into());
    }
    let committed = pool_committed(&boot.username).unwrap_or_default();
    quotas_cover_committed(&quotas, &committed)?;
    set_account_fields(&boot.username, ROLE_RESELLER, "")?;
    let mut file = load_file()?;
    let now = now_unix();
    if let Some(existing) = file
        .resellers
        .iter_mut()
        .find(|r| usernames_equal(&r.username, &boot.username))
    {
        existing.quotas = quotas;
        existing.updated_at_unix = now;
        let record = existing.clone();
        save_file(&file)?;
        return Ok(record);
    }
    let record = ResellerRecord {
        username: boot.username.clone(),
        quotas,
        branding: ResellerBranding::default(),
        created_at_unix: now,
        updated_at_unix: now,
    };
    file.resellers.push(record.clone());
    save_file(&file)?;
    Ok(record)
}

pub fn demote_reseller(username_raw: &str) -> Result<(), String> {
    let (boot, _) = find_account(username_raw)?;
    let children = child_usernames(&boot.username)?;
    if !children.is_empty() {
        return Err(format!(
            "Reseller still has {} child user(s); unassign them first",
            children.len()
        ));
    }
    set_account_fields(&boot.username, ROLE_USER, "")?;
    let mut file = load_file()?;
    file.resellers
        .retain(|r| !usernames_equal(&r.username, &boot.username));
    save_file(&file)
}

pub fn update_reseller_quotas(username_raw: &str, quotas: ResellerQuotas) -> Result<(), String> {
    let (boot, _) = find_account(username_raw)?;
    let committed = pool_committed(&boot.username)?;
    quotas_cover_committed(&quotas, &committed)?;
    let mut file = load_file()?;
    let Some(row) = file
        .resellers
        .iter_mut()
        .find(|r| usernames_equal(&r.username, &boot.username))
    else {
        return Err(format!("`{}` is not a reseller", boot.username));
    };
    row.quotas = quotas;
    row.updated_at_unix = now_unix();
    set_account_fields(&boot.username, ROLE_RESELLER, "")?;
    save_file(&file)
}

pub fn update_reseller_branding(
    username_raw: &str,
    branding: ResellerBranding,
) -> Result<(), String> {
    let (boot, _) = find_account(username_raw)?;
    let mut file = load_file()?;
    let Some(row) = file
        .resellers
        .iter_mut()
        .find(|r| usernames_equal(&r.username, &boot.username))
    else {
        return Err(format!("`{}` is not a reseller", boot.username));
    };
    row.branding = sanitize_branding(branding)?;
    row.updated_at_unix = now_unix();
    save_file(&file)
}

fn sanitize_branding(mut branding: ResellerBranding) -> Result<ResellerBranding, String> {
    branding.display_name = branding.display_name.trim().chars().take(80).collect();
    branding.tagline = branding.tagline.trim().chars().take(160).collect();
    branding.logo_url = branding.logo_url.trim().chars().take(512).collect();
    branding.primary_color = branding.primary_color.trim().chars().take(32).collect();
    if !branding.logo_url.is_empty() {
        let lower = branding.logo_url.to_ascii_lowercase();
        if !(lower.starts_with("https://") || lower.starts_with('/')) {
            return Err("Logo URL must be https:// or a site-relative path".into());
        }
    }
    if !branding.primary_color.is_empty() {
        let c = branding.primary_color.as_str();
        let ok = c.starts_with('#')
            && (c.len() == 4 || c.len() == 7)
            && c.chars().skip(1).all(|ch| ch.is_ascii_hexdigit());
        if !ok {
            return Err("Primary color must be a hex value such as #006CFA".into());
        }
    }
    Ok(branding)
}

pub fn assign_user_to_reseller(child_raw: &str, reseller_raw: &str) -> Result<(), String> {
    let (child, _) = find_account(child_raw)?;
    let (reseller, _) = find_account(reseller_raw)?;
    if is_panel_admin(&child.username) {
        return Err("The panel owner cannot be assigned under a reseller".into());
    }
    if usernames_equal(&child.username, &reseller.username) {
        return Err("A reseller cannot be assigned under themselves".into());
    }
    let Some(parent) = get_reseller(&reseller.username)? else {
        return Err(format!("`{}` is not a reseller", reseller.username));
    };
    if account_is_reseller(&child.username) {
        return Err(
            "Demote the child reseller before assigning them under another reseller".into(),
        );
    }
    let child_slice = package_slice_for(&child.username);
    let mut baseline = QuotaSlice::default();
    for name in child_usernames(&reseller.username)? {
        if usernames_equal(&name, &child.username) {
            continue;
        }
        let slice = package_slice_for(&name);
        add_limited(&mut baseline.websites, slice.websites)?;
        add_limited(&mut baseline.mailboxes, slice.mailboxes)?;
        add_limited(&mut baseline.databases, slice.databases)?;
        add_limited(&mut baseline.ftp_accounts, slice.ftp_accounts)?;
        add_limited(&mut baseline.storage_mb, slice.storage_mb)?;
        add_limited(&mut baseline.bandwidth_mb, slice.bandwidth_mb)?;
    }
    add_limited(&mut baseline.websites, child_slice.websites)?;
    add_limited(&mut baseline.mailboxes, child_slice.mailboxes)?;
    add_limited(&mut baseline.databases, child_slice.databases)?;
    add_limited(&mut baseline.ftp_accounts, child_slice.ftp_accounts)?;
    add_limited(&mut baseline.storage_mb, child_slice.storage_mb)?;
    add_limited(&mut baseline.bandwidth_mb, child_slice.bandwidth_mb)?;
    quotas_cover_committed(&parent.quotas, &baseline)?;
    set_account_fields(&child.username, ROLE_USER, &reseller.username)
}

pub fn unassign_user(child_raw: &str) -> Result<(), String> {
    let (child, _) = find_account(child_raw)?;
    set_account_fields(&child.username, &child.role, "")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn store_roundtrip_promote() {
        with_test_data_dir(|| {
            ensure_reseller_store_migrated().unwrap();
            let policy = crate::account::default_password_policy();
            crate::account_mgmt::create_account(
                "cpnowner",
                Some("CpN-Reseller-Owner-9x!"),
                false,
                "owner@example.com",
                policy.clone(),
                "en",
            )
            .unwrap();
            crate::account_mgmt::create_account(
                "resell1",
                Some("CpN-Reseller-Child-9x!"),
                false,
                "r@example.com",
                policy,
                "en",
            )
            .unwrap();
            let quotas = ResellerQuotas {
                websites: 5,
                mailboxes: 20,
                databases: 5,
                ftp_accounts: 5,
                storage_mb: 10240,
                bandwidth_mb: 102400,
            };
            let row = promote_reseller("resell1", quotas).unwrap();
            assert_eq!(row.username, "resell1");
            assert!(account_is_reseller("resell1"));
            assert!(can_access_reseller_center("resell1"));
            update_reseller_branding(
                "resell1",
                ResellerBranding {
                    display_name: "Acme Host".into(),
                    tagline: "Fast sites".into(),
                    logo_url: "https://example.com/logo.png".into(),
                    primary_color: "#006CFA".into(),
                },
            )
            .unwrap();
            let got = get_reseller("resell1").unwrap().unwrap();
            assert_eq!(got.branding.display_name, "Acme Host");
            demote_reseller("resell1").unwrap();
            assert!(!account_is_reseller("resell1"));
        });
    }
}
