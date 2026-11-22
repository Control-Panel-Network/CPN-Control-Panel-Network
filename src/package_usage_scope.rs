//! Which sites and related resources count toward a panel account's package usage.
//!
//! After CyberPanel (and similar) migrations, apex sites often keep a legacy owner
//! string equal to the domain name. That string is not a CPN panel account, so a
//! strict `site.owner == username` filter under-counts Websites, Mailboxes, and
//! anything else keyed off owned domains. Panel admin usage therefore also includes
//! sites whose owner is not a known panel username.

use crate::account_mgmt::find_account;
use crate::panel_admin::is_panel_admin;
use crate::sites::{SiteRecord, list_sites};

fn names_equal(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// True when `username` resolves to a real panel account (bootstrap or accounts/).
pub fn is_known_panel_username(username: &str) -> bool {
    let username = username.trim();
    if username.is_empty() {
        return false;
    }
    find_account(username).is_ok()
}

/// True when this site should count toward `username`'s package / dashboard usage.
pub fn site_counts_for_package_usage(username: &str, site: &SiteRecord) -> bool {
    if names_equal(&site.owner, username) {
        return true;
    }
    // Legacy migrate owners (domain-named Unix/CyberPanel users) are not panel
    // accounts. Attribute those sites to the panel bootstrap admin only.
    is_panel_admin(username) && !is_known_panel_username(&site.owner)
}

/// Sites that count toward package usage for `username`.
pub fn sites_for_package_usage(username: &str) -> Vec<SiteRecord> {
    list_sites()
        .unwrap_or_default()
        .into_iter()
        .filter(|site| site_counts_for_package_usage(username, site))
        .collect()
}

/// Domain names (and aliases) from [`sites_for_package_usage`].
pub fn owned_site_domains_for_usage(username: &str) -> Vec<String> {
    let mut out = Vec::new();
    for site in sites_for_package_usage(username) {
        out.push(site.domain.clone());
        for alias in site.aliases {
            if !alias.trim().is_empty() {
                out.push(alias);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::{now_unix, with_test_data_dir};
    use crate::account_mgmt::create_account;
    use crate::model::PasswordPolicy;
    use crate::sites::create_site;
    use std::fs;

    fn policy() -> PasswordPolicy {
        PasswordPolicy {
            min_length: 8,
            require_special: false,
            require_uppercase: true,
            require_number: true,
        }
    }

    #[test]
    fn admin_counts_legacy_domain_owner_apex() {
        with_test_data_dir(|| {
            let home = std::env::temp_dir().join(format!(
                "cpn-usage-scope-{}-{}",
                std::process::id(),
                now_unix()
            ));
            let _ = fs::remove_dir_all(&home);
            fs::create_dir_all(&home).unwrap();
            unsafe {
                std::env::set_var("CPN_SITES_HOME", &home);
            }

            // First account becomes bootstrap admin.
            create_account(
                "ntowner",
                Some("OwnerPass1!"),
                false,
                "owner@example.com",
                policy(),
                "en",
            )
            .unwrap();
            // CyberPanel-style owner: domain string, not a panel account.
            create_site("example.com", "example.com", None, None, None).unwrap();
            create_site("blog.example.com", "ntowner", None, None, None).unwrap();

            let for_owner = sites_for_package_usage("ntowner");
            assert_eq!(for_owner.len(), 2, "{for_owner:?}");
            assert!(for_owner.iter().any(|s| s.domain == "example.com"));
            assert!(for_owner.iter().any(|s| s.domain == "blog.example.com"));

            create_account(
                "alice",
                Some("AlicePass1!"),
                false,
                "alice@example.com",
                policy(),
                "en",
            )
            .unwrap();
            let for_alice = sites_for_package_usage("alice");
            assert!(
                for_alice.is_empty(),
                "non-admin must not inherit legacy owners: {for_alice:?}"
            );

            unsafe {
                std::env::remove_var("CPN_SITES_HOME");
            }
            let _ = fs::remove_dir_all(&home);
        });
    }
}
