use super::*;
use crate::account::{
    accounts_dir, default_password_policy, hash_password, new_password_salt, persist_bootstrap,
    setup_account, with_test_data_dir, write_account_file,
};
use crate::panel_session::{SESSION_COOKIE, create_session_token, session_cookie_name};

const SECRET: &str = "unit-test-session-secret-value-0123456789";

fn make_owner() {
    setup_account(
        "ntowner",
        Some(&["Owner", "Passw0rd", "xyz"].concat()),
        false,
        "owner@example.com",
        default_password_policy(),
        "en",
    )
    .unwrap();
}

fn add_extra(name: &str, role: &str, parent_owner: &str, parent_reseller: &str) {
    let password = ["Extra", "Passw0rd", "abc"].concat();
    let salt = new_password_salt();
    let boot = PanelBootstrap {
        schema_version: 1,
        username: name.into(),
        recovery_email: "x@example.com".into(),
        password_hash: hash_password(&password, &salt),
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
    write_account_file(&accounts_dir().join(format!("{name}.json")), &boot).unwrap();
}

fn jar(user: &str) -> String {
    format!("{SESSION_COOKIE}={}", create_session_token(user, SECRET))
}

#[test]
fn owner_session_is_admitted_even_with_totp_enrolled_in_the_past() {
    with_test_data_dir(|| {
        make_owner();
        let boot = load_bootstrap().unwrap();
        let mut boot = boot;
        boot.must_change_password = false;
        persist_bootstrap(&boot).unwrap();
        assert_eq!(
            owner_from_session_cookie(&jar("ntowner"), "panel.example", SECRET),
            Ok("ntowner".to_string())
        );
        // Extra unrelated cookies in the jar do not matter.
        let mixed = format!("a=b; {}; c=d", jar("NTOwner"));
        assert!(owner_from_session_cookie(&mixed, "panel.example", SECRET).is_ok());
    });
}

#[test]
fn normal_user_reseller_and_acl_sessions_are_denied() {
    with_test_data_dir(|| {
        make_owner();
        add_extra("plainuser", "", "", "");
        add_extra("resell", "reseller", "", "");
        add_extra("aclsub", "subuser", "ntowner", "");
        add_extra("jailed", "", "", "resell");
        for name in ["plainuser", "resell", "aclsub", "jailed"] {
            assert_eq!(
                owner_from_session_cookie(&jar(name), "panel.example", SECRET),
                Err(DenyReason::NotOwner),
                "{name} must not get webmail admin through SSO"
            );
        }
    });
}

#[test]
fn forged_expired_or_wrong_secret_sessions_are_anonymous() {
    with_test_data_dir(|| {
        make_owner();
        let wrong = format!(
            "{SESSION_COOKIE}={}",
            create_session_token("ntowner", "other")
        );
        assert_eq!(
            owner_from_session_cookie(&wrong, "panel.example", SECRET),
            Err(DenyReason::Invalid)
        );
        assert_eq!(
            owner_from_session_cookie(&format!("{SESSION_COOKIE}=abc"), "panel.example", SECRET),
            Err(DenyReason::Invalid)
        );
        assert_eq!(
            owner_from_session_cookie("", "panel.example", SECRET),
            Err(DenyReason::Invalid)
        );
        assert_eq!(
            owner_from_session_cookie("other=1", "panel.example", SECRET),
            Err(DenyReason::Invalid)
        );
        let huge = format!("{SESSION_COOKIE}={}", "a".repeat(MAX_COOKIE_CHARS));
        assert_eq!(
            owner_from_session_cookie(&huge, "panel.example", SECRET),
            Err(DenyReason::Invalid)
        );
    });
}

#[test]
fn disabled_or_must_change_owner_session_is_denied() {
    with_test_data_dir(|| {
        make_owner();
        let mut boot = load_bootstrap().unwrap();
        boot.must_change_password = true;
        persist_bootstrap(&boot).unwrap();
        assert_eq!(
            owner_from_session_cookie(&jar("ntowner"), "panel.example", SECRET),
            Err(DenyReason::MustChangePassword)
        );
        boot.must_change_password = false;
        boot.disabled = true;
        persist_bootstrap(&boot).unwrap();
        assert_eq!(
            owner_from_session_cookie(&jar("ntowner"), "panel.example", SECRET),
            Err(DenyReason::Disabled)
        );
    });
}

#[test]
fn owner_with_reseller_role_flags_is_denied() {
    with_test_data_dir(|| {
        make_owner();
        let mut boot = load_bootstrap().unwrap();
        boot.must_change_password = false;
        boot.role = "reseller".into();
        persist_bootstrap(&boot).unwrap();
        assert_eq!(
            owner_from_session_cookie(&jar("ntowner"), "panel.example", SECRET),
            Err(DenyReason::RoleNotAllowed)
        );
    });
}

#[test]
fn port_scoped_cookie_names_follow_the_host_header() {
    with_test_data_dir(|| {
        make_owner();
        let mut boot = load_bootstrap().unwrap();
        boot.must_change_password = false;
        persist_bootstrap(&boot).unwrap();
        let token = create_session_token("ntowner", SECRET);
        let scoped = format!("{}={token}", session_cookie_name(Some(2092)));
        assert!(owner_from_session_cookie(&scoped, "panel.example:2092", SECRET).is_ok());
        // A scoped cookie is not accepted for a different port, nor for the bare host.
        assert!(owner_from_session_cookie(&scoped, "panel.example:2087", SECRET).is_err());
        assert!(owner_from_session_cookie(&scoped, "panel.example", SECRET).is_err());
    });
}

#[test]
fn client_hints_never_expose_account_existence() {
    use crate::tachyon_admin_bridge::DenyReason as R;
    assert_eq!(R::MfaRequired.client_hint(), Some("mfa_required"));
    assert_eq!(R::MfaFailed.client_hint(), Some("mfa_failed"));
    for r in [
        R::BadPassword,
        R::NotOwner,
        R::RoleNotAllowed,
        R::Disabled,
        R::Invalid,
    ] {
        assert_eq!(r.client_hint(), None, "{r:?} must stay generic");
    }
}
