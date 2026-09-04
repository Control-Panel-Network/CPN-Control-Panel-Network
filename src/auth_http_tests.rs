//! Actix HTTP coverage for panel login and dashboard ACL (issue #8).

use crate::account::{
    default_password_policy, hash_password, with_test_data_dir, write_account_file, PanelBootstrap,
};
use crate::auth_api::{dashboard_page, login_submit};
use crate::http_helpers::build_allowed_hosts;
use crate::installer::AppState;
use crate::model::{AccountPublic, InstallerStatus};
use crate::panel_session::SESSION_COOKIE;
use actix_web::{http::StatusCode, web, App};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::broadcast;

fn test_state(phase: &'static str) -> web::Data<Arc<AppState>> {
    let (events, _) = broadcast::channel(8);
    let mut status = InstallerStatus {
        phase,
        ..Default::default()
    };
    status.account = Some(AccountPublic {
        username: "Admin".into(),
        recovery_email: "admin@example.com".into(),
        configured: true,
    });
    web::Data::new(Arc::new(AppState {
        status: std::sync::RwLock::new(status),
        events,
        token: "install-token-for-tests-0123456789".into(),
        session_id: "session-id-for-tests-0123456789ab".into(),
        bind_port: 2087,
        allow_remote: false,
        allowed_hosts: build_allowed_hosts(2087, &[]),
        cancel_requested: AtomicBool::new(false),
        active_child_pids: std::sync::Mutex::new(Vec::new()),
    }))
}

fn write_admin_account(password: &str) {
    let salt = "aabbccddeeff00112233445566778899";
    let boot = PanelBootstrap {
        schema_version: 1,
        username: "Admin".into(),
        recovery_email: "admin@example.com".into(),
        password_hash: hash_password(password, salt),
        password_salt: salt.into(),
        password_policy: default_password_policy(),
        language: "en".into(),
        created_at_unix: 1,
    };
    let path = crate::account::bootstrap_path();
    write_account_file(&path, &boot).expect("write bootstrap");
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime")
}

#[test]
fn login_valid_sets_session_and_redirects() {
    with_test_data_dir(|| {
        unsafe {
            std::env::set_var("CPN_PANEL_SESSION_SECRET", "unit-test-session-secret");
        }
        write_admin_account("Secret1!");
        runtime().block_on(async {
            let app = actix_web::test::init_service(
                App::new()
                    .app_data(test_state("completed"))
                    .service(login_submit),
            )
            .await;
            let req = actix_web::test::TestRequest::post()
                .uri("/login")
                .set_form(&[
                    ("username", "Admin"),
                    ("password", "Secret1!"),
                    ("remember_me", "0"),
                ])
                .to_request();
            let resp = actix_web::test::call_service(&app, req).await;
            assert_eq!(resp.status(), StatusCode::SEE_OTHER);
            let location = resp
                .headers()
                .get(actix_web::http::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("");
            assert_eq!(location, "/dashboard");
            let cookie = resp
                .headers()
                .get(actix_web::http::header::SET_COOKIE)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("");
            assert!(
                cookie.contains(SESSION_COOKIE),
                "missing session cookie: {cookie}"
            );
            assert!(cookie.contains("HttpOnly"));
        });
        unsafe {
            std::env::remove_var("CPN_PANEL_SESSION_SECRET");
        }
    });
}

#[test]
fn login_invalid_returns_401_without_session_cookie() {
    with_test_data_dir(|| {
        unsafe {
            std::env::set_var("CPN_PANEL_SESSION_SECRET", "unit-test-session-secret");
        }
        write_admin_account("Secret1!");
        runtime().block_on(async {
            let app = actix_web::test::init_service(
                App::new()
                    .app_data(test_state("completed"))
                    .service(login_submit),
            )
            .await;
            let req = actix_web::test::TestRequest::post()
                .uri("/login")
                .set_form(&[
                    ("username", "Admin"),
                    ("password", "WrongPass1!"),
                    ("remember_me", "0"),
                ])
                .to_request();
            let resp = actix_web::test::call_service(&app, req).await;
            assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
            let set_cookie = resp
                .headers()
                .get(actix_web::http::header::SET_COOKIE)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("");
            assert!(
                !set_cookie.contains(SESSION_COOKIE),
                "invalid login must not set session cookie: {set_cookie}"
            );
        });
        unsafe {
            std::env::remove_var("CPN_PANEL_SESSION_SECRET");
        }
    });
}

#[test]
fn dashboard_without_session_redirects_to_login() {
    runtime().block_on(async {
        let app = actix_web::test::init_service(
            App::new()
                .app_data(test_state("completed"))
                .service(dashboard_page),
        )
        .await;
        let req = actix_web::test::TestRequest::get()
            .uri("/dashboard")
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        let location = resp
            .headers()
            .get(actix_web::http::header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        assert_eq!(location, "/login");
    });
}
