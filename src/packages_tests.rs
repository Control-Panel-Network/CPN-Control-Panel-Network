use super::*;
use crate::account::with_test_data_dir;
use crate::account_mgmt::create_account;
use crate::model::PasswordPolicy;

fn policy() -> PasswordPolicy {
    PasswordPolicy {
        min_length: 8,
        require_special: false,
        require_uppercase: true,
        require_number: true,
    }
}

#[test]
fn delete_blocked_when_assigned() {
    with_test_data_dir(|| {
        create_account(
            "ops",
            Some("OpsPass1!"),
            false,
            "ops@example.com",
            policy(),
            "en",
        )
        .unwrap();
        let pkg = create_package_for(
            "ops",
            PackageInput {
                name: "Reseller".into(),
                disk_mb: 500,
                bandwidth_mb: 500,
                domains: 2,
                emails: 10,
                databases: 2,
                ftp_accounts: 2,
                fqdn_enabled: true,
                notes: String::new(),
                sidebar_hidden_nav_ids: Vec::new(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(pkg.name, "ops_Reseller");
        assign_package("ops", &pkg.id).unwrap();
        assert!(delete_package(&pkg.id).is_err());
        assign_package("ops", DEFAULT_PACKAGE_ID).unwrap();
        delete_package(&pkg.id).unwrap();
    });
}

#[test]
fn default_update_keeps_default_name() {
    with_test_data_dir(|| {
        ensure_default_package().unwrap();
        let updated = update_package_for(
            DEFAULT_PACKAGE_ID,
            "cpnowner",
            PackageInput {
                name: "Anything".into(),
                disk_mb: 2000,
                bandwidth_mb: 2000,
                domains: 50,
                emails: 100,
                databases: 100,
                ftp_accounts: 100,
                fqdn_enabled: true,
                notes: String::new(),
                sidebar_hidden_nav_ids: Vec::new(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(updated.name, "Default");
        assert_eq!(updated.disk_mb, 2000);
    });
}

#[test]
fn edit_keeps_owner_prefix() {
    with_test_data_dir(|| {
        let pkg = create_package_for(
            "cpnowner",
            PackageInput {
                name: "test".into(),
                disk_mb: 1000,
                bandwidth_mb: 1000,
                domains: 20,
                emails: 100,
                databases: 100,
                ftp_accounts: 100,
                fqdn_enabled: true,
                notes: String::new(),
                sidebar_hidden_nav_ids: Vec::new(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(pkg.name, "cpnowner_test");
        let updated = update_package_for(
            &pkg.id,
            "other",
            PackageInput {
                name: "admin".into(),
                disk_mb: 1000,
                bandwidth_mb: 1000,
                domains: 20,
                emails: 100,
                databases: 100,
                ftp_accounts: 100,
                fqdn_enabled: true,
                notes: String::new(),
                sidebar_hidden_nav_ids: Vec::new(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(updated.name, "cpnowner_admin");
        assert_eq!(package_custom_name_for_edit(&updated), "admin");
    });
}

#[test]
fn zero_limit_means_none_allowed() {
    with_test_data_dir(|| {
        let pkg = create_package_for(
            "ops",
            PackageInput {
                name: "Locked".into(),
                disk_mb: 0,
                bandwidth_mb: 0,
                domains: 0,
                emails: 0,
                databases: 0,
                ftp_accounts: 0,
                fqdn_enabled: true,
                notes: String::new(),
                sidebar_hidden_nav_ids: Vec::new(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(pkg.disk_mb, 0);
        assert_eq!(pkg.bandwidth_mb, 0);
        assert_eq!(pkg.domains, 0);
        assert_eq!(pkg.database_disk_mb, UNLIMITED);
        assert!(!is_unlimited(pkg.emails));
        assert_eq!(format_limit_display(pkg.disk_mb, "MB"), "0 MB");
        assert_eq!(format_limit_display(0, ""), "0");
        assert_eq!(format_limit_display(UNLIMITED, ""), "∞");
    });
}
