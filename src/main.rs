use actix_web::{App, HttpRequest, HttpResponse, HttpServer, Responder, get, post, web};
use cpn_installer::account::{account_public_from_disk, default_password_policy};
use cpn_installer::auth_api::{
    account_setup, api_logout_get, api_logout_post, dashboard_page, forgot_password_page,
    forgot_password_submit, login_page, login_submit, logout_get, logout_post, panel_alias,
};
use cpn_installer::auth_pages::installer_token_required_html;
use cpn_installer::http_helpers::{
    VERSION, authorized_request, enrich_status, install_finished, install_session_cookie_header,
    normalize_language, panel_account_ready, remote_origin_ok, smtp_status_public, token_matches,
    wants_html, websocket_origin_ok,
};
use cpn_installer::installer::AppState;
use cpn_installer::installer_transitions::{can_start_mail, can_start_server};
use cpn_installer::listen_port::{resolve_listen_port, validate_listen_port};
use cpn_installer::manifest::detect_existing_install;
use cpn_installer::model::{
    InstallRequest, InstallerEvent, InstallerStatus, LanguageRequest, ListenPortRequest,
    MailInstallRequest, OptionalTokenQuery, SessionBootstrapRequest, TokenQuery,
};
use cpn_installer::panel_hub_routes::{
    backups_create_route, backups_destinations_route, backups_destinations_save,
    backups_gdrive_route, backups_remote_route, backups_restore_route, backups_schedule_route,
    backups_schedule_save, databases_all_route, databases_create_get, databases_create_post,
    databases_delete_get, databases_delete_post, databases_manager_route,
    databases_phpmyadmin_route, email_accounts_route, email_catchall_route, email_catchall_save,
    email_create_route, email_debugger, email_delivery_route, email_dkim_ensure, email_dkim_route,
    email_forwarding_route, email_forwarding_save, email_limits, email_mailscanner,
    email_marketing, email_password, email_pattern_fwd, email_plus, email_queue, email_rspamd,
    email_spamassassin, email_webmail_route, ftp_accounts_route, ftp_create, ftp_delete, ftp_reset,
    security_page, server_dns_defaults, server_dns_nameservers, server_dns_nameservers_save,
    server_dns_zones, server_dns_zones_delete, server_dns_zones_save, server_docker_apps,
    server_docker_containers, server_docker_images, server_files_page, server_packages_page,
    server_page, server_php_configs, server_php_extensions, server_php_tuning,
    server_processes_page, server_services_control, server_services_page, settings_page,
    settings_port_page, users_plans_page,
};
use cpn_installer::panel_network::{
    OldPortPolicy, active_redirect_migration, apply_network_change, network_public,
    purge_expired_migration, save_panel_hostname,
};
use cpn_installer::panel_routes::{
    apps_install, apps_page, apps_reinstall, apps_uninstall, backups_page, backups_run,
    databases_install_mariadb, databases_page, email_account_create, email_account_disable,
    email_account_enable, email_page, plugins_dashboard_page, plugins_disable, plugins_enable,
    plugins_install, plugins_page, plugins_settings_page, plugins_settings_save, plugins_uninstall,
    preview_content, preview_mode_page, websites_create, websites_delete, websites_manage,
    websites_page, websites_prefs, websites_pretty_manage, websites_preview_redirect,
    websites_resume, websites_suspend,
};
use cpn_installer::panel_theme_routes::{
    panel_color_mode_get, panel_color_mode_set, panel_design_get, panel_design_preset,
    panel_design_restore, panel_design_save,
};
use cpn_installer::status_pages::status_html_page;
use futures_util::StreamExt;
use rand::{Rng, distr::Alphanumeric};
use rust_embed::Embed;
use std::{
    env,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};
use tokio::sync::broadcast;

#[derive(Embed)]
#[folder = "installer-ui/dist"]
struct UiAssets;

fn status_response(request: &HttpRequest, payload: &InstallerStatus) -> HttpResponse {
    if wants_html(request) {
        HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(status_html_page(payload))
    } else {
        HttpResponse::Ok().json(payload)
    }
}

fn serve_index_html() -> HttpResponse {
    match UiAssets::get("index.html") {
        Some(asset) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(asset.data),
        None => HttpResponse::ServiceUnavailable()
            .body("La interfaz web aún no está incluida en este binario"),
    }
}

#[get("/")]
async fn root_page(
    request: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<OptionalTokenQuery>,
) -> HttpResponse {
    let status = state.status.read().await.clone();
    if install_finished(&status) {
        return HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish();
    }
    let from_query = query.token.as_deref();
    // Query token is bootstrap-only. SPA exchanges it for HttpOnly cookie + Bearer storage
    // and strips ?token= from the address bar (issue #1).
    if token_matches(&state, from_query) {
        return serve_index_html();
    }
    // Cookie / header may already authorize without putting the token in the URL.
    let fake = TokenQuery {
        token: String::new(),
    };
    if authorized_request(&state, &fake, &request) {
        return serve_index_html();
    }
    // Already-installed systems (incl. maintenance phase): send token-less visitors
    // to panel login instead of the "installation is not finished" blocker.
    // Installer SPA remains available with ?token= above.
    if panel_account_ready(&status) {
        return HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish();
    }
    HttpResponse::Unauthorized()
        .content_type("text/html; charset=utf-8")
        .body(installer_token_required_html())
}

#[post("/api/session")]
async fn bootstrap_session(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    body: web::Json<SessionBootstrapRequest>,
) -> HttpResponse {
    // Token must arrive in JSON body (not query) so Set-Cookie is not tied to ?token= (CodeQL / #1).
    if !token_matches(&state, Some(body.token.as_str())) {
        return HttpResponse::Unauthorized().json(serde_json::json!({"error": "invalid token"}));
    }
    if !remote_origin_ok(&http, state.allow_remote, state.bind_port) {
        return HttpResponse::Forbidden().json(serde_json::json!({"error": "Origin no permitido"}));
    }
    let secure = http.connection_info().scheme() == "https";
    HttpResponse::Ok()
        .append_header((
            actix_web::http::header::SET_COOKIE,
            install_session_cookie_header(&state.session_id, secure),
        ))
        .append_header((
            actix_web::http::header::CACHE_CONTROL,
            "no-store, no-cache, must-revalidate",
        ))
        .json(serde_json::json!({"ok": true}))
}

#[get("/api/status")]
async fn api_status(
    request: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<TokenQuery>,
) -> HttpResponse {
    if !authorized_request(&state, &query, &request) {
        return HttpResponse::Unauthorized().finish();
    }
    let payload = enrich_status(state.status.read().await.clone(), &state.token);
    status_response(&request, &payload)
}

#[get("/status")]
async fn status_page(
    request: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<TokenQuery>,
) -> HttpResponse {
    if !authorized_request(&state, &query, &request) {
        return HttpResponse::Unauthorized()
            .content_type("text/html; charset=utf-8")
            .body("<!DOCTYPE html><html lang=\"en\"><body><h1>Access denied</h1><p>Invalid token.</p></body></html>");
    }
    let payload = enrich_status(state.status.read().await.clone(), &state.token);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(status_html_page(&payload))
}

#[post("/api/language")]
async fn set_language(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<TokenQuery>,
    request: web::Json<LanguageRequest>,
) -> HttpResponse {
    if !authorized_request(&state, &query, &http) {
        return HttpResponse::Unauthorized().finish();
    }
    if !remote_origin_ok(&http, state.allow_remote, state.bind_port) {
        return HttpResponse::Forbidden().json(serde_json::json!({"error": "Origin no permitido"}));
    }
    let language = match normalize_language(&request.language) {
        Ok(value) => value,
        Err(error) => {
            return HttpResponse::BadRequest().json(serde_json::json!({"error": error}));
        }
    };
    let mut current = state.status.write().await;
    current.language = language;
    let payload = enrich_status(current.clone(), &state.token);
    HttpResponse::Ok().json(payload)
}

#[post("/api/listen-port")]
async fn set_listen_port(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<TokenQuery>,
    request: web::Json<ListenPortRequest>,
) -> HttpResponse {
    if !authorized_request(&state, &query, &http) {
        return HttpResponse::Unauthorized().finish();
    }
    if !remote_origin_ok(&http, state.allow_remote, state.bind_port) {
        return HttpResponse::Forbidden().json(serde_json::json!({"error": "Origin no permitido"}));
    }
    let port = match validate_listen_port(request.port) {
        Ok(value) => value,
        Err(error) => {
            return HttpResponse::BadRequest().json(serde_json::json!({"error": error}));
        }
    };
    let policy = match request.old_port_policy.as_deref() {
        None => None,
        Some(raw) => match OldPortPolicy::parse(raw) {
            Ok(value) => Some(value),
            Err(error) => {
                return HttpResponse::BadRequest().json(serde_json::json!({"error": error}));
            }
        },
    };
    let hostname_update = request.panel_hostname.as_ref().map(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
    let (preferred, migration) =
        match apply_network_change(port, state.bind_port, policy, hostname_update) {
            Ok(value) => value,
            Err(error) => {
                return HttpResponse::BadRequest().json(serde_json::json!({"error": error}));
            }
        };

    let restart_required = preferred != state.bind_port;
    let mut current = state.status.write().await;
    if !restart_required {
        current.listen_port = preferred;
        if let Some(env_info) = current.environment.as_mut() {
            env_info.port = preferred;
        }
        current.panel_login_url = None;
    }
    let payload = enrich_status(current.clone(), &state.token);
    let note = if restart_required {
        let policy_note = migration
            .as_ref()
            .map(|value| {
                format!(
                    " Old-port policy: {} (expires unix {}). Restart to bind the new port; redirect helper starts automatically when policy is redirect_*.",
                    value.mode.as_str(),
                    value.expires_at
                )
            })
            .unwrap_or_default();
        format!(
            "Preferred listen port {preferred} saved. Restart with: cpn-installer --port {preferred} (current session stays on {}).{policy_note}",
            state.bind_port
        )
    } else {
        format!("Listen port {preferred} confirmed for this session")
    };
    let network = network_public(state.bind_port, None);
    HttpResponse::Ok().json(serde_json::json!({
        "status": payload,
        "listen_port": state.bind_port,
        "preferred_listen_port": preferred,
        "restart_required": restart_required,
        "message": note,
        "network": network,
        "port_migration": migration,
    }))
}

#[post("/api/install/server")]
async fn start_install(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<TokenQuery>,
    request: web::Json<InstallRequest>,
) -> impl Responder {
    if !authorized_request(&state, &query, &http) {
        return HttpResponse::Unauthorized().finish();
    }
    if !remote_origin_ok(&http, state.allow_remote, state.bind_port) {
        return HttpResponse::Forbidden().json(serde_json::json!({"error": "Origin no permitido"}));
    }
    #[cfg(unix)]
    if unsafe { libc::geteuid() } != 0 {
        return HttpResponse::BadRequest().json(
            serde_json::json!({"error": "Ejecuta el instalador como root (sudo cpn-installer)"}),
        );
    }
    let mut current = state.status.write().await;
    if let Err(denied) = can_start_server(&current, request.force_reinstall) {
        return HttpResponse::Conflict().json(serde_json::json!({"error": denied.message}));
    }
    current.selected_server = Some(request.server);
    if request.force_reinstall {
        current.selected_mail = None;
        current.server_ready = false;
        current.external_ports_configured = false;
    } else {
        current.selected_mail = None;
    }
    current.phase = "downloading";
    current.progress = 1;
    current.error = None;
    drop(current);
    // Dedicated OS thread + runtime so sync/package work cannot starve Actix HTTP.
    let install_state = state.get_ref().clone();
    let server = request.server;
    let database = request.database;
    let install_phpmyadmin = request.install_phpmyadmin;
    let _ = std::thread::Builder::new()
        .name("cpn-install-server".into())
        .spawn(move || {
            let Ok(rt) = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
            else {
                eprintln!("cpn-installer: failed to build install runtime");
                return;
            };
            rt.block_on(cpn_installer::installer::install_with_database(
                install_state,
                server,
                database,
                install_phpmyadmin,
            ));
        });
    HttpResponse::Accepted().finish()
}

#[post("/api/install/mail")]
async fn start_mail_install(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<TokenQuery>,
    request: web::Json<MailInstallRequest>,
) -> impl Responder {
    if !authorized_request(&state, &query, &http) {
        return HttpResponse::Unauthorized().finish();
    }
    if !remote_origin_ok(&http, state.allow_remote, state.bind_port) {
        return HttpResponse::Forbidden().json(serde_json::json!({"error": "Origin no permitido"}));
    }
    #[cfg(unix)]
    if unsafe { libc::geteuid() } != 0 {
        return HttpResponse::BadRequest().json(
            serde_json::json!({"error": "Ejecuta el instalador como root (sudo cpn-installer)"}),
        );
    }
    let mut current = state.status.write().await;
    if let Err(denied) = can_start_mail(&current, request.force_reinstall) {
        return HttpResponse::Conflict().json(serde_json::json!({"error": denied.message}));
    }
    current.selected_mail = Some(request.mail);
    current.phase = "downloading";
    current.progress = 0;
    current.error = None;
    drop(current);
    let install_state = state.get_ref().clone();
    let mail = request.mail;
    let _ = std::thread::Builder::new()
        .name("cpn-install-mail".into())
        .spawn(move || {
            let Ok(rt) = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
            else {
                eprintln!("cpn-installer: failed to build mail install runtime");
                return;
            };
            rt.block_on(cpn_installer::installer::install_mail(install_state, mail));
        });
    HttpResponse::Accepted().finish()
}

async fn websocket(
    request: HttpRequest,
    body: web::Payload,
    state: web::Data<Arc<AppState>>,
    query: web::Query<TokenQuery>,
) -> actix_web::Result<HttpResponse> {
    if !authorized_request(&state, &query, &request) {
        return Ok(HttpResponse::Unauthorized().finish());
    }
    if !websocket_origin_ok(&request, state.allow_remote, state.bind_port) {
        return Ok(HttpResponse::Forbidden().finish());
    }
    let (response, mut session, mut messages) = actix_ws::handle(&request, body)?;
    let mut events = state.events.subscribe();
    let snapshot = InstallerEvent::Snapshot {
        status: enrich_status(state.status.read().await.clone(), &state.token),
    };
    actix_web::rt::spawn(async move {
        let _ = session
            .text(serde_json::to_string(&snapshot).unwrap_or_default())
            .await;
        loop {
            tokio::select! {
                event = events.recv() => match event {
                    Ok(event) => if session.text(serde_json::to_string(&event).unwrap_or_default()).await.is_err() { break; },
                    Err(broadcast::error::RecvError::Lagged(_)) => continue, Err(_) => break,
                },
                message = messages.next() => match message {
                    Some(Ok(actix_ws::Message::Ping(value))) => { let _ = session.pong(&value).await; }
                    Some(Ok(actix_ws::Message::Close(reason))) => { let _ = session.close(reason).await; break; }
                    None | Some(Err(_)) => break, _ => {}
                }
            }
        }
    });
    Ok(response)
}

async fn static_asset(path: web::Path<String>) -> impl Responder {
    let requested = path.into_inner();
    // Never treat empty path as the installer SPA here; `root_page` owns `/`.
    if requested.is_empty() {
        return HttpResponse::NotFound().finish();
    }
    let name = requested.as_str();
    let asset = UiAssets::get(name).or_else(|| {
        if name.contains('.') {
            None
        } else {
            UiAssets::get("index.html")
        }
    });
    match asset {
        Some(asset) => {
            let content_type = match name.rsplit('.').next() {
                Some("js") => "text/javascript; charset=utf-8",
                Some("css") => "text/css; charset=utf-8",
                Some("svg") => "image/svg+xml",
                Some("png") => "image/png",
                _ => "text/html; charset=utf-8",
            };
            HttpResponse::Ok()
                .content_type(content_type)
                .body(asset.data)
        }
        None => HttpResponse::ServiceUnavailable()
            .body("La interfaz web aún no está incluida en este binario"),
    }
}

fn allow_remote_listen() -> bool {
    env::args().any(|arg| arg == "--allow-remote" || arg == "--listen-all")
        || env::var("CPN_ALLOW_REMOTE").ok().as_deref() == Some("1")
}

fn apply_startup_network_flags(args: &[String], listen_port: u16) {
    let mut hostname: Option<String> = None;
    let mut policy: Option<OldPortPolicy> = None;
    let mut iter = args.iter().skip(1);
    while let Some(arg) = iter.next() {
        if arg == "--panel-hostname" {
            if let Some(value) = iter.next() {
                hostname = Some(value.clone());
            }
        } else if let Some(value) = arg.strip_prefix("--panel-hostname=") {
            hostname = Some(value.to_string());
        } else if arg == "--old-port-policy" {
            if let Some(value) = iter.next() {
                match OldPortPolicy::parse(value) {
                    Ok(parsed) => policy = Some(parsed),
                    Err(error) => eprintln!("cpn-installer: {error}"),
                }
            }
        } else if let Some(value) = arg.strip_prefix("--old-port-policy=") {
            match OldPortPolicy::parse(value) {
                Ok(parsed) => policy = Some(parsed),
                Err(error) => eprintln!("cpn-installer: {error}"),
            }
        }
    }
    // Capture previous preferred port before --port overwrites preferences on apply.
    let previous_preferred = cpn_installer::listen_port::load_preferred_listen_port();
    if let Some(host) = hostname {
        match save_panel_hostname(&host) {
            Ok(()) => println!("  Panel hostname saved: {host}"),
            Err(error) => eprintln!("cpn-installer: could not save panel hostname: {error}"),
        }
    }
    if let Some(policy) = policy {
        let old = previous_preferred.unwrap_or(cpn_installer::listen_port::DEFAULT_PORT);
        if old != listen_port {
            match cpn_installer::panel_network::build_port_migration(old, listen_port, policy) {
                Ok(Some(migration)) => {
                    if let Err(error) =
                        cpn_installer::panel_network::save_port_migration(&migration)
                    {
                        eprintln!("cpn-installer: could not save port migration: {error}");
                    } else {
                        println!(
                            "  Old-port policy {}: {} -> {} (expires unix {})",
                            policy.as_str(),
                            migration.old_port,
                            migration.new_port,
                            migration.expires_at
                        );
                    }
                }
                Ok(None) => {}
                Err(error) => eprintln!("cpn-installer: {error}"),
            }
        }
        let _ = cpn_installer::listen_port::save_preferred_listen_port(listen_port);
    }
}

fn listen_hosts() -> Vec<String> {
    if allow_remote_listen() {
        vec!["0.0.0.0".into()]
    } else {
        vec!["127.0.0.1".into()]
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if let Some(mode) = cpn_installer::cli_maintenance::parse_cli(&args) {
        let code = cpn_installer::cli_maintenance::run_cli(mode).await;
        if code != 0 {
            std::process::exit(code);
        }
        return Ok(());
    }

    let listen_port = match resolve_listen_port(&args) {
        Ok(port) => port,
        Err(error) => {
            eprintln!("cpn-installer: {error}");
            std::process::exit(2);
        }
    };
    apply_startup_network_flags(&args, listen_port);
    purge_expired_migration();
    #[cfg(unix)]
    if listen_port < 1024 && unsafe { libc::geteuid() } != 0 {
        eprintln!(
            "Aviso: el puerto {listen_port} es privilegiado (<1024). Suele requerir root, o elige un puerto >1024 (por defecto {}).",
            cpn_installer::listen_port::DEFAULT_PORT
        );
    }

    println!("\nCPN Server Panel · Instalador {VERSION}");
    println!("Iniciando el instalador web...\n");
    let token: String = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(28)
        .map(char::from)
        .collect();
    let session_id: String = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(28)
        .map(char::from)
        .collect();
    let environment = cpn_installer::environment::inspect(listen_port).await;
    let remote = allow_remote_listen();
    if remote {
        match cpn_installer::environment::open_installer_port(&environment).await {
            Ok(()) => {}
            Err(error) => eprintln!("Aviso: {error}"),
        }
    }
    let mail_releases = cpn_installer::mail_releases::load_mail_releases().await;
    let maintenance = cpn_installer::maintenance_api::load_maintenance_info().await;
    let existing = detect_existing_install(VERSION);
    let (events, _) = broadcast::channel(256);
    let bootstrap_account = account_public_from_disk();
    let phase = if maintenance.existing_install {
        "maintenance"
    } else if bootstrap_account.is_some() {
        "completed"
    } else {
        "ready"
    };
    let message = if maintenance.existing_install {
        format!(
            "CPN {} is already installed. Choose upgrade, repair, or continue config.",
            existing.package_version
        )
    } else {
        "El sistema está listo para continuar".into()
    };
    let mut initial = InstallerStatus {
        phase,
        progress: 0,
        message,
        selected_server: existing.selected_server,
        selected_mail: existing.selected_mail,
        environment: Some(environment.clone()),
        error: None,
        language: "en".into(),
        listen_port,
        panel_hostname: None,
        port_migration: None,
        public_base_url: None,
        account: bootstrap_account,
        password_policy: default_password_policy(),
        panel_login_path: "/login".into(),
        panel_login_url: None,
        version: VERSION.into(),
        server_ready: existing.selected_server.is_some() || existing.has_manifest,
        mail_client_ready: false,
        mail_backend_ready: false,
        external_ports_configured: false,
        access_note: None,
        mail_releases,
        smtp: Some(smtp_status_public()),
        maintenance: Some(maintenance),
    };
    initial = enrich_status(initial, &token);
    let startup_hostname = initial.panel_hostname.clone();
    let startup_migration = initial.port_migration.clone();
    let state = Arc::new(AppState {
        status: tokio::sync::RwLock::new(initial),
        events,
        token: token.clone(),
        session_id,
        bind_port: listen_port,
        allow_remote: remote,
        cancel_requested: AtomicBool::new(false),
    });
    println!("✓ El instalador web está listo para empezar:");
    if remote {
        println!("  Modo --allow-remote: escucha en 0.0.0.0:{listen_port} (HTTP sin TLS).");
        println!("  Prefer SSH tunnel or set the install cookie via first local visit.");
        println!(
            "  Bootstrap once: http://127.0.0.1:{listen_port}/?token=<full-token-from-secure-channel>"
        );
        println!(
            "  Token fingerprint (last 4): ...{}",
            &token[token.len().saturating_sub(4)..]
        );
        println!("  Full token also accepted via Authorization: Bearer or X-CPN-Token.");
    } else {
        println!("  http://127.0.0.1:{listen_port}/?token={token}");
        println!(
            "  Acceso remoto recomendado vía túnel SSH, por ejemplo:\n  ssh -L {listen_port}:127.0.0.1:{listen_port} user@host"
        );
        println!("  Para escuchar en todas las interfaces: --allow-remote o CPN_ALLOW_REMOTE=1");
    }
    println!("  Puerto de escucha: {listen_port} (cambia con --port, CPN_LISTEN_PORT, o la UI)");
    if let Some(hostname) = startup_hostname.as_ref() {
        println!(
            "  Panel hostname: https://{hostname}/login (DNS + reverse proxy on 443 -> {listen_port})"
        );
    }
    if let Some(migration) = startup_migration.as_ref() {
        println!(
            "  Port migration: {} -> {} ({}; expires unix {})",
            migration.old_port, migration.new_port, migration.mode, migration.expires_at
        );
    }
    println!("\nMantén esta ventana abierta hasta finalizar. Pulsa Ctrl+C para detener.\n");
    let hosts = listen_hosts();
    if let Some(migration) = active_redirect_migration(listen_port) {
        let redirect_hosts = hosts.clone();
        tokio::spawn(async move {
            cpn_installer::port_redirect::run_redirect_listeners(redirect_hosts, migration).await;
        });
    }
    let cancel_state = state.clone();
    let mut server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .service(root_page)
            .service(api_status)
            .service(status_page)
            .service(login_page)
            .service(login_submit)
            .service(dashboard_page)
            .service(websites_page)
            .service(websites_manage)
            .service(websites_preview_redirect)
            .service(websites_pretty_manage)
            .service(preview_mode_page)
            .service(preview_content)
            .service(websites_create)
            .service(websites_delete)
            .service(websites_suspend)
            .service(websites_resume)
            .service(websites_prefs)
            .service(panel_color_mode_get)
            .service(panel_color_mode_set)
            .service(panel_design_get)
            .service(panel_design_save)
            .service(panel_design_preset)
            .service(panel_design_restore)
            .service(email_page)
            .service(email_account_create)
            .service(email_account_enable)
            .service(email_account_disable)
            .service(databases_page)
            .service(databases_install_mariadb)
            .service(apps_page)
            .service(apps_install)
            .service(apps_reinstall)
            .service(apps_uninstall)
            .service(backups_page)
            .service(backups_run)
            .service(backups_create_route)
            .service(backups_restore_route)
            .service(backups_schedule_route)
            .service(backups_schedule_save)
            .service(backups_destinations_route)
            .service(backups_destinations_save)
            .service(backups_gdrive_route)
            .service(backups_remote_route)
            .service(server_page)
            .service(server_services_page)
            .service(server_services_control)
            .service(server_processes_page)
            .service(server_php_extensions)
            .service(server_php_configs)
            .service(server_php_tuning)
            .service(server_packages_page)
            .service(server_docker_apps)
            .service(server_docker_containers)
            .service(server_docker_images)
            .service(server_files_page)
            .service(server_dns_zones)
            .service(server_dns_zones_save)
            .service(server_dns_zones_delete)
            .service(server_dns_nameservers)
            .service(server_dns_defaults)
            .service(server_dns_nameservers_save)
            .service(settings_page)
            .service(settings_port_page)
            .service(security_page)
            .service(users_plans_page)
            .service(email_accounts_route)
            .service(email_create_route)
            .service(email_forwarding_route)
            .service(email_forwarding_save)
            .service(email_catchall_route)
            .service(email_catchall_save)
            .service(email_dkim_route)
            .service(email_dkim_ensure)
            .service(email_webmail_route)
            .service(email_delivery_route)
            .service(email_pattern_fwd)
            .service(email_limits)
            .service(email_password)
            .service(email_debugger)
            .service(email_queue)
            .service(email_spamassassin)
            .service(email_rspamd)
            .service(email_mailscanner)
            .service(email_marketing)
            .service(email_plus)
            .service(databases_all_route)
            .service(databases_create_get)
            .service(databases_create_post)
            .service(databases_delete_get)
            .service(databases_delete_post)
            .service(databases_manager_route)
            .service(databases_phpmyadmin_route)
            .service(ftp_accounts_route)
            .service(ftp_create)
            .service(ftp_delete)
            .service(ftp_reset)
            .service(plugins_page)
            .service(plugins_settings_page)
            .service(plugins_settings_save)
            .service(plugins_dashboard_page)
            .service(plugins_install)
            .service(plugins_uninstall)
            .service(plugins_enable)
            .service(plugins_disable)
            .service(panel_alias)
            .service(logout_get)
            .service(logout_post)
            .service(api_logout_get)
            .service(api_logout_post)
            .service(forgot_password_page)
            .service(forgot_password_submit)
            .service(set_language)
            .service(set_listen_port)
            .service(bootstrap_session)
            .service(account_setup)
            .service(start_install)
            .service(start_mail_install)
            .service(cpn_installer::maintenance_api::api_version_check)
            .service(cpn_installer::maintenance_api::api_releases)
            .service(cpn_installer::maintenance_api::start_maintenance)
            .route("/api/events", web::get().to(websocket))
            .route("/{path:.*}", web::get().to(static_asset))
    })
    .keep_alive(Duration::from_secs(30))
    // GHA matrix guests often expose 1 CPU. One Actix worker + sync install
    // work freezes /api/status for the whole smoke. Keep at least two workers.
    .workers(std::cmp::max(
        2,
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2),
    ));
    for host in hosts {
        server = server.bind((host.as_str(), listen_port))?;
    }
    println!(
        "Listening on port {listen_port} (hosts: {})",
        hosts.join(", ")
    );
    let _ = std::io::Write::flush(&mut std::io::stdout());
    let running = server.run();
    let handle = running.handle();
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{SignalKind, signal};
            let mut sigterm = signal(SignalKind::terminate()).ok();
            let mut sigint = signal(SignalKind::interrupt()).ok();
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = async {
                    if let Some(ref mut s) = sigterm { s.recv().await; }
                } => {}
                _ = async {
                    if let Some(ref mut s) = sigint { s.recv().await; }
                } => {}
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
        cancel_state.request_cancel();
        handle.stop(true).await;
    });
    let result = running.await;
    if remote {
        let _ = cpn_installer::environment::close_installer_port(&environment).await;
    }
    result
}
