//! Reseller pool quota parsing and committed-cap checks.

use crate::package_limits::{UNLIMITED, format_limit_display, is_unlimited, normalize_limit};
use crate::packages::{DEFAULT_PACKAGE_ID, Package, package_for_account};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResellerQuotas {
    /// Websites / domains.
    pub websites: i64,
    /// Mailboxes / emails.
    pub mailboxes: i64,
    pub databases: i64,
    pub ftp_accounts: i64,
    /// Storage in MB.
    pub storage_mb: i64,
    /// Bandwidth in MB.
    pub bandwidth_mb: i64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct QuotaSlice {
    pub websites: i64,
    pub mailboxes: i64,
    pub databases: i64,
    pub ftp_accounts: i64,
    pub storage_mb: i64,
    pub bandwidth_mb: i64,
}

pub fn parse_quota_field(raw: &str, label: &str) -> Result<i64, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(format!("{label} is required (-1 unlimited, 0 none)"));
    }
    let value: i64 = trimmed
        .parse()
        .map_err(|_| format!("{label} must be an integer (-1, 0, or positive)"))?;
    if value < UNLIMITED {
        return Err(format!("{label} cannot be less than -1"));
    }
    Ok(normalize_limit(value))
}

pub fn quotas_from_fields(
    websites: &str,
    mailboxes: &str,
    databases: &str,
    ftp_accounts: &str,
    storage_mb: &str,
    bandwidth_mb: &str,
) -> Result<ResellerQuotas, String> {
    Ok(ResellerQuotas {
        websites: parse_quota_field(websites, "Websites")?,
        mailboxes: parse_quota_field(mailboxes, "Mailboxes")?,
        databases: parse_quota_field(databases, "Databases")?,
        ftp_accounts: parse_quota_field(ftp_accounts, "FTP accounts")?,
        storage_mb: parse_quota_field(storage_mb, "Storage (MB)")?,
        bandwidth_mb: parse_quota_field(bandwidth_mb, "Bandwidth (MB)")?,
    })
}

pub fn package_slice_for(username: &str) -> QuotaSlice {
    let pkg = package_for_account(username).unwrap_or_else(|_| Package {
        id: DEFAULT_PACKAGE_ID.into(),
        name: "Default".into(),
        disk_mb: 0,
        bandwidth_mb: 0,
        domains: 0,
        emails: 0,
        databases: 0,
        database_disk_mb: 0,
        ftp_accounts: 0,
        mailing_lists: UNLIMITED,
        autoresponders: UNLIMITED,
        forwarders: UNLIMITED,
        email_filters: UNLIMITED,
        alias_domains: UNLIMITED,
        subdomains: UNLIMITED,
        fqdn_enabled: true,
        notes: String::new(),
        sidebar_hidden_nav_ids: Vec::new(),
        created_at_unix: 0,
        updated_at_unix: 0,
    });
    QuotaSlice {
        websites: pkg.domains,
        mailboxes: pkg.emails,
        databases: pkg.databases,
        ftp_accounts: pkg.ftp_accounts,
        storage_mb: pkg.disk_mb,
        bandwidth_mb: pkg.bandwidth_mb,
    }
}

pub fn add_limited(sum: &mut i64, value: i64) -> Result<(), String> {
    if is_unlimited(value) {
        return Err(
            "Child package has an unlimited limit; parent pool must also be unlimited".into(),
        );
    }
    *sum = sum.saturating_add(value.max(0));
    Ok(())
}

pub fn pool_allows(parent: i64, committed: i64) -> bool {
    if is_unlimited(parent) {
        return true;
    }
    if parent == 0 {
        return committed == 0;
    }
    committed <= parent
}

pub fn quotas_cover_committed(
    quotas: &ResellerQuotas,
    committed: &QuotaSlice,
) -> Result<(), String> {
    let checks = [
        ("Websites", quotas.websites, committed.websites),
        ("Mailboxes", quotas.mailboxes, committed.mailboxes),
        ("Databases", quotas.databases, committed.databases),
        ("FTP accounts", quotas.ftp_accounts, committed.ftp_accounts),
        ("Storage (MB)", quotas.storage_mb, committed.storage_mb),
        (
            "Bandwidth (MB)",
            quotas.bandwidth_mb,
            committed.bandwidth_mb,
        ),
    ];
    for (label, parent, used) in checks {
        if !pool_allows(parent, used) {
            return Err(format!(
                "{label} pool {parent} cannot cover committed child package caps ({used})"
            ));
        }
    }
    Ok(())
}

pub fn format_quota_cell(limit: i64, unit: &str) -> String {
    format_limit_display(limit, unit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_allows_none_and_unlimited() {
        assert!(pool_allows(0, 0));
        assert!(!pool_allows(0, 1));
        assert!(pool_allows(-1, 999));
        assert!(pool_allows(10, 10));
        assert!(!pool_allows(10, 11));
    }
}
