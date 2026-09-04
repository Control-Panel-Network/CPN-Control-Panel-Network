//! Package quota usage and create-time enforcement.

use crate::mail_accounts;
use crate::packages::{
    PackageUsage, QuotaResource, UNLIMITED, format_limit_display, package_for_account,
};
use crate::resource_accounts::{list_databases, list_ftp_accounts};
use crate::sites::{list_sites, parent_domain_candidates, site_home_from_record};
use std::fs;
use std::path::Path;

fn names_equal(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

fn owned_site_domains(username: &str) -> Result<Vec<String>, String> {
    let sites = list_sites()?;
    Ok(sites
        .into_iter()
        .filter(|s| names_equal(&s.owner, username))
        .map(|s| s.domain)
        .collect())
}

fn dir_size_mb(path: &Path, budget_bytes: &mut u64) -> u64 {
    if *budget_bytes == 0 {
        return 0;
    }
    let Ok(meta) = fs::symlink_metadata(path) else {
        return 0;
    };
    if meta.file_type().is_symlink() {
        return 0;
    }
    if meta.is_file() {
        let len = meta.len().min(*budget_bytes);
        *budget_bytes = budget_bytes.saturating_sub(len);
        return len.div_ceil(1024 * 1024);
    }
    if !meta.is_dir() {
        return 0;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    let mut total = 0u64;
    for entry in entries.flatten() {
        if *budget_bytes == 0 {
            break;
        }
        total = total.saturating_add(dir_size_mb(&entry.path(), budget_bytes));
    }
    total
}

fn disk_used_mb(username: &str) -> Result<u64, String> {
    let sites = list_sites()?;
    let mut total = 0u64;
    // Cap walk at ~50 GiB equivalent so quota checks stay bounded.
    let mut budget = 50u64 * 1024 * 1024 * 1024;
    for site in sites
        .into_iter()
        .filter(|s| names_equal(&s.owner, username))
    {
        let home = site_home_from_record(&site);
        total = total.saturating_add(dir_size_mb(&home, &mut budget));
    }
    Ok(total)
}

pub fn usage_for_account(username: &str) -> Result<PackageUsage, String> {
    let package = package_for_account(username)?;
    let owned = owned_site_domains(username)?;
    let domains_used = owned.len() as u64;
    let emails_used = mail_accounts::list_accounts()
        .into_iter()
        .filter(|m| {
            if owned.iter().any(|d| names_equal(d, &m.domain)) {
                return true;
            }
            m.address
                .rsplit_once('@')
                .map(|(_, domain)| owned.iter().any(|d| names_equal(d, domain)))
                .unwrap_or(false)
        })
        .count() as u64;
    let databases_used = list_databases()
        .into_iter()
        .filter(|d| names_equal(&d.owner, username))
        .count() as u64;
    let ftp_used = list_ftp_accounts()
        .into_iter()
        .filter(|f| names_equal(&f.owner, username))
        .count() as u64;
    let disk_mb_used = disk_used_mb(username)?;
    Ok(PackageUsage {
        package_id: package.id,
        package_name: package.name,
        domains_used,
        domains_limit: package.domains,
        emails_used,
        emails_limit: package.emails,
        databases_used,
        databases_limit: package.databases,
        ftp_used,
        ftp_limit: package.ftp_accounts,
        disk_mb_used,
        disk_mb_limit: package.disk_mb,
        bandwidth_mb_limit: package.bandwidth_mb,
        fqdn_enabled: package.fqdn_enabled,
    })
}

fn limit_reached(used: u64, limit: i64) -> bool {
    if limit == UNLIMITED {
        return false;
    }
    if limit < 0 {
        return true;
    }
    used >= limit as u64
}

/// Reject when the account would exceed its package quota for `resource`.
pub fn require_quota(username: &str, resource: QuotaResource) -> Result<(), String> {
    let usage = usage_for_account(username)?;
    let (label, used, limit) = match resource {
        QuotaResource::Domains => ("domains", usage.domains_used, usage.domains_limit),
        QuotaResource::Emails => ("emails", usage.emails_used, usage.emails_limit),
        QuotaResource::Databases => ("databases", usage.databases_used, usage.databases_limit),
        QuotaResource::FtpAccounts => ("FTP accounts", usage.ftp_used, usage.ftp_limit),
        QuotaResource::DiskMb => ("disk (MB)", usage.disk_mb_used, usage.disk_mb_limit),
    };
    if limit_reached(used, limit) {
        return Err(format!(
            "Package `{}` quota exceeded for {label}: used {used}, limit {}",
            usage.package_name,
            format_limit_display(limit, "")
        ));
    }
    Ok(())
}

/// Enforce domain + FQDN + soft disk checks before creating a website or subdomain.
pub fn require_site_create_allowed(owner: &str, domain_raw: &str) -> Result<(), String> {
    require_quota(owner, QuotaResource::Domains)?;
    require_quota(owner, QuotaResource::DiskMb)?;
    let domain = domain_raw.trim().to_ascii_lowercase();
    let is_subdomain = !parent_domain_candidates(&domain).is_empty();
    if is_subdomain {
        let package = package_for_account(owner)?;
        if !package.fqdn_enabled {
            return Err(format!(
                "Package `{}` does not allow FQDN / subdomain creation",
                package.name
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::{now_unix, with_test_data_dir};
    use crate::account_mgmt::create_account;
    use crate::model::PasswordPolicy;
    use crate::packages::{
        DEFAULT_PACKAGE_ID, PackageInput, UNLIMITED, assign_package, create_package,
        ensure_default_package, update_package,
    };
    use crate::sites::create_site;
    use std::fs;

    fn policy() -> PasswordPolicy {
        PasswordPolicy {
            min_length: 8,
            require_special: true,
            require_uppercase: true,
            require_number: true,
        }
    }

    #[test]
    fn default_package_and_quota_math() {
        with_test_data_dir(|| {
            let home = std::env::temp_dir().join(format!(
                "cpn-pkg-home-{}-{}",
                std::process::id(),
                now_unix()
            ));
            let _ = fs::remove_dir_all(&home);
            fs::create_dir_all(&home).unwrap();
            unsafe {
                std::env::set_var("CPN_SITES_HOME", &home);
            }

            let pkg = ensure_default_package().unwrap();
            assert_eq!(pkg.name, "Default");
            assert_eq!(pkg.domains, 20);

            create_account(
                "ops",
                Some("OpsPass1!"),
                false,
                "ops@example.com",
                policy(),
                "en",
            )
            .unwrap();
            assign_package("ops", DEFAULT_PACKAGE_ID).unwrap();

            let tight = create_package(PackageInput {
                name: "Tiny".into(),
                disk_mb: UNLIMITED,
                bandwidth_mb: UNLIMITED,
                domains: 1,
                emails: 0,
                databases: 0,
                ftp_accounts: 0,
                fqdn_enabled: false,
                notes: String::new(),
            })
            .unwrap();
            assign_package("ops", &tight.id).unwrap();

            create_site("example.com", "ops", None, None, None).unwrap();
            let err = require_site_create_allowed("ops", "blog.example.com").unwrap_err();
            assert!(err.contains("quota exceeded") || err.contains("does not allow FQDN"));

            update_package(
                &tight.id,
                PackageInput {
                    name: "Tiny".into(),
                    disk_mb: UNLIMITED,
                    bandwidth_mb: UNLIMITED,
                    domains: 5,
                    emails: 0,
                    databases: 0,
                    ftp_accounts: 0,
                    fqdn_enabled: false,
                    notes: String::new(),
                },
            )
            .unwrap();
            let err = require_site_create_allowed("ops", "blog.example.com").unwrap_err();
            assert!(err.contains("FQDN"));

            unsafe {
                std::env::remove_var("CPN_SITES_HOME");
            }
            let _ = fs::remove_dir_all(&home);
        });
    }
}
