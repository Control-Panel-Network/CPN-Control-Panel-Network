//! HTTP coverage for Docker panel routes (HEAD must match GET auth redirects).

use crate::account::{
    PanelBootstrap, default_password_policy, generate_password, hash_password, new_password_salt,
    with_test_data_dir, write_account_file,
};
use crate::http_helpers::build_allowed_hosts;
use crate::installer::AppState;
use crate::model::{AccountPublic, InstallerStatus};
use crate::panel_hub_routes::{
    docker_home, docker_images_route, docker_stacks_route, docker_view_route,
};
use actix_web::{App, http::StatusCode, web};
use rand::Rng;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
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
        disabled: false,
    });
    web::Data::new(Arc::new(AppState {
        status: std::sync::RwLock::new(status),
        events,
        token: format!("install-token-{}", rand::rng().random::<u64>()),
        session_id: format!("session-id-{}", rand::rng().random::<u64>()),
        bind_port: 2087,
        allow_remote: false,
        allowed_hosts: build_allowed_hosts(2087, &[]),
        cancel_requested: AtomicBool::new(false),
        active_child_pids: std::sync::Mutex::new(Vec::new()),
        install_log_detail: std::sync::Mutex::new(crate::installer::InstallLogDetail::Full),
    }))
}

fn write_admin_account(password: &str) {
    let salt = new_password_salt();
    let boot = PanelBootstrap {
        schema_version: 1,
        username: "Admin".into(),
        recovery_email: "admin@example.com".into(),
        password_hash: hash_password(password, &salt),
        password_salt: salt,
        password_policy: default_password_policy(),
        language: "en".into(),
        created_at_unix: 1,
        must_change_password: false,
        totp_required: false,
        disabled: false,
    };
    let path = crate::account::bootstrap_path();
    write_account_file(&path, &boot).expect("write bootstrap");
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
}

async fn head_status(
    app: &impl actix_web::dev::Service<actix_web::dev::ServiceRequest>,
    uri: &str,
) -> StatusCode {
    let req = actix_web::test::TestRequest::default()
        .method(actix_web::http::Method::HEAD)
        .uri(uri)
        .to_request();
    actix_web::test::call_service(app, req).await.status()
}

async fn get_status(
    app: &impl actix_web::dev::Service<actix_web::dev::ServiceRequest>,
    uri: &str,
) -> StatusCode {
    let req = actix_web::test::TestRequest::default()
        .method(actix_web::http::Method::GET)
        .uri(uri)
        .to_request();
    actix_web::test::call_service(app, req).await.status()
}

#[test]
fn docker_routes_head_matches_get_when_unauthenticated() {
    with_test_data_dir(|| {
        write_admin_account(&generate_password(&default_password_policy()));
        runtime().block_on(async {
            let app = actix_web::test::init_service(
                App::new()
                    .app_data(test_state("completed"))
                    .service(docker_home)
                    .service(docker_images_route)
                    .service(docker_stacks_route)
                    .service(docker_view_route),
            )
            .await;

            for uri in [
                "/docker",
                "/docker/images",
                "/docker/stacks",
                "/docker/view/example",
            ] {
                let get_st = get_status(&app, uri).await;
                let head_st = head_status(&app, uri).await;
                assert_ne!(
                    head_st,
                    StatusCode::NOT_FOUND,
                    "HEAD {uri} must not 404 (GET was {get_st})"
                );
                assert_eq!(
                    head_st, get_st,
                    "HEAD and GET status should match for {uri}"
                );
                assert_eq!(
                    head_st,
                    StatusCode::SEE_OTHER,
                    "unauthenticated {uri} should redirect to login"
                );
            }
        });
    });
}
