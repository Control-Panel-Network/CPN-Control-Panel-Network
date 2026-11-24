use super::*;
use crate::account::{
    accounts_dir, default_password_policy, new_password_salt, setup_account, with_test_data_dir,
    write_account_file,
};

fn pw(parts: &[&str]) -> String {
    parts.concat()
}

fn owner_pw() -> String {
    pw(&["Owner", "Passw0rd", "xyz"])
}

fn make_owner() {
    setup_account(
        "ntowner",
        Some(&owner_pw()),
        false,
        "owner@example.com",
        default_password_policy(),
        "en",
    )
    .unwrap();
    // Setup marks MFA required; tests below enroll TOTP or clear the flag explicitly.
}

fn set_flags(f: impl FnOnce(&mut PanelBootstrap)) {
    let mut boot = load_bootstrap().unwrap();
    f(&mut boot);
    crate::account::persist_bootstrap(&boot).unwrap();
}

fn add_extra(name: &str, role: &str, parent_owner: &str, parent_reseller: &str) {
    let password = pw(&["Extra", "Passw0rd", "abc"]);
    let salt = new_password_salt();
    let boot = PanelBootstrap {
        schema_version: 1,
        username: name.into(),
        recovery_email: "x@example.com".into(),
        password_hash: crate::account::hash_password(&password, &salt),
        password_salt: salt,
        password_policy: default_password_policy(),
        language: "en".into(),
        created_at_unix: 1,
        must_change_password: false,
        totp_required: false,
        disabled: false,
        role: role.into(),
        parent_reseller: parent_reseller.into(),
        parent_owner: parent_owner.into(),
    };
    let path = accounts_dir().join(format!("{name}.json"));
    write_account_file(&path, &boot).unwrap();
}

#[test]
fn owner_with_password_and_no_required_mfa_is_allowed() {
    with_test_data_dir(|| {
        make_owner();
        set_flags(|b| b.totp_required = false);
        assert_eq!(verify_owner_admin_login("ntowner", &owner_pw(), ""), Ok(()));
        assert_eq!(verify_owner_admin_login("NTOwner", &owner_pw(), ""), Ok(()));
    });
}

#[test]
fn wrong_password_is_denied() {
    with_test_data_dir(|| {
        make_owner();
        set_flags(|b| b.totp_required = false);
        let bad = pw(&["Wrong", "Passw0rd", "123"]);
        assert_eq!(
            verify_owner_admin_login("ntowner", &bad, ""),
            Err(DenyReason::BadPassword)
        );
    });
}

#[test]
fn normal_user_reseller_and_acl_accounts_are_denied() {
    with_test_data_dir(|| {
        make_owner();
        set_flags(|b| b.totp_required = false);
        add_extra("plainuser", "", "", "");
        add_extra("resell", "reseller", "", "");
        add_extra("aclsub", "subuser", "ntowner", "");
        add_extra("jailed", "", "", "resell");
        let extra = pw(&["Extra", "Passw0rd", "abc"]);
        for name in ["plainuser", "resell", "aclsub", "jailed"] {
            assert_eq!(
                verify_owner_admin_login(name, &extra, ""),
                Err(DenyReason::NotOwner),
                "{name} must not reach webmail admin"
            );
        }
        // Owner password must not unlock an extra username either.
        assert_eq!(
            verify_owner_admin_login("aclsub", &owner_pw(), ""),
            Err(DenyReason::NotOwner)
        );
    });
}

#[test]
fn owner_role_guard_rejects_reseller_and_acl_flags() {
    with_test_data_dir(|| {
        make_owner();
        set_flags(|b| {
            b.totp_required = false;
            b.role = "reseller".into();
        });
        assert_eq!(
            verify_owner_admin_login("ntowner", &owner_pw(), ""),
            Err(DenyReason::RoleNotAllowed)
        );
        set_flags(|b| {
            b.role = String::new();
            b.parent_owner = "someone".into();
        });
        assert_eq!(
            verify_owner_admin_login("ntowner", &owner_pw(), ""),
            Err(DenyReason::RoleNotAllowed)
        );
    });
}

#[test]
fn disabled_or_must_change_owner_is_denied() {
    with_test_data_dir(|| {
        make_owner();
        set_flags(|b| {
            b.totp_required = false;
            b.disabled = true;
        });
        assert_eq!(
            verify_owner_admin_login("ntowner", &owner_pw(), ""),
            Err(DenyReason::Disabled)
        );
        set_flags(|b| {
            b.disabled = false;
            b.must_change_password = true;
        });
        assert_eq!(
            verify_owner_admin_login("ntowner", &owner_pw(), ""),
            Err(DenyReason::MustChangePassword)
        );
    });
}

#[test]
fn owner_without_enrolled_mfa_is_denied_when_cpn_requires_it() {
    with_test_data_dir(|| {
        make_owner();
        set_flags(|b| b.totp_required = true);
        assert_eq!(
            verify_owner_admin_login("ntowner", &owner_pw(), ""),
            Err(DenyReason::MfaEnrollRequired)
        );
    });
}

#[test]
fn owner_with_totp_needs_the_same_code_as_the_panel() {
    with_test_data_dir(|| {
        use crate::account_mfa::{begin_totp_enroll, confirm_totp_enroll};
        use crate::account_totp::{decode_totp_secret, totp_code_at};
        make_owner();
        let (secret_b32, _uri, _svg) = begin_totp_enroll("ntowner").unwrap();
        let secret = decode_totp_secret(&secret_b32).unwrap();
        let code = format!("{:06}", totp_code_at(&secret, crate::account::now_unix()));
        confirm_totp_enroll("ntowner", &code).unwrap();

        assert_eq!(
            verify_owner_admin_login("ntowner", &owner_pw(), ""),
            Err(DenyReason::MfaRequired)
        );
        assert_eq!(
            verify_owner_admin_login("ntowner", &owner_pw(), "000000"),
            Err(DenyReason::MfaFailed)
        );
        let ok = format!("{:06}", totp_code_at(&secret, crate::account::now_unix()));
        assert_eq!(
            verify_owner_admin_login("ntowner", &owner_pw(), &ok),
            Ok(())
        );
    });
}

#[test]
fn repeated_failures_lock_the_bridge_without_locking_panel_login() {
    with_test_data_dir(|| {
        make_owner();
        set_flags(|b| b.totp_required = false);
        let bad = pw(&["Wrong", "Passw0rd", "123"]);
        for _ in 0..40 {
            let _ = verify_owner_admin_login("ntowner", &bad, "");
        }
        assert_eq!(
            verify_owner_admin_login("ntowner", &owner_pw(), ""),
            Err(DenyReason::RateLimited)
        );
        // Panel MFA/login rate namespace is separate.
        assert!(check_rate_limit("ntowner").is_ok());
    });
}

#[test]
fn invalid_input_is_rejected_early() {
    with_test_data_dir(|| {
        assert_eq!(
            verify_owner_admin_login("", &pw(&["a", "b"]), ""),
            Err(DenyReason::Invalid)
        );
        assert_eq!(
            verify_owner_admin_login("ntowner", &pw(&[]), ""),
            Err(DenyReason::Invalid)
        );
        let long: String = std::iter::repeat_n('a', MAX_PASSWORD_CHARS + 1).collect();
        assert_eq!(
            verify_owner_admin_login("ntowner", &long, ""),
            Err(DenyReason::Invalid)
        );
    });
}

#[test]
fn deny_codes_are_stable_and_secret_free() {
    for r in [
        DenyReason::Invalid,
        DenyReason::RateLimited,
        DenyReason::NotOwner,
        DenyReason::RoleNotAllowed,
        DenyReason::Disabled,
        DenyReason::MustChangePassword,
        DenyReason::BadPassword,
        DenyReason::MfaRequired,
        DenyReason::MfaEnrollRequired,
        DenyReason::PasskeyOnly,
        DenyReason::MfaFailed,
    ] {
        assert!(!r.code().is_empty());
        assert!(!r.code().contains(' '));
    }
}

#[test]
fn constant_time_eq_works() {
    assert!(constant_time_eq("abc", "abc"));
    assert!(!constant_time_eq("abc", "abd"));
    assert!(!constant_time_eq("abc", "abcd"));
}
