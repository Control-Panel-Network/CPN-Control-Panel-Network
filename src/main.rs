use actix_web::{App, HttpRequest, HttpResponse, HttpServer, Responder, get, post, web};
use cpn_installer::account::{account_public_from_disk, default_password_policy};
use cpn_installer::auth_api::{
    account_setup, forgot_password_page, forgot_password_submit, login_page, login_submit,
};
use cpn_installer::auth_pages::installer_token_required_html;
use cpn_installer::http_helpers::{
    VERSION, authorized_request, enrich_status, install_finished, install_token_cookie_header,
    normalize_language, panel_login_url_for, remote_origin_ok, smtp_status_public, token_matches,
    wants_html,
};
use cpn_installer::installer::AppState;
use cpn_installer::listen_port::{
    resolve_listen_port, save_preferred_listen_port, validate_listen_port,
};
use cpn_installer::manifest::detect_existing_install;
use cpn_installer::model::{
    InstallRequest, InstallerEvent, InstallerStatus, LanguageRequest, ListenPortRequest,
    MailInstallRequest, OptionalTokenQuery, TokenQuery,
};
use cpn_installer::status_pages::status_html_page;
use futures_util::StreamExt;
use rand::{Rng, distr::Alphanumeric};
use rust_embed::Embed;
use std::{env, sync::Arc, time::Duration};
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
    if token_matches(&state, from_query) {
        let mut response = serve_index_html();
        if from_query.is_some() {
            // Cookie carries a server-only session id (never the query token).
            if let Some(cookie) = install_token_cookie_header(
                &state.session_id,
                request.connection_info().scheme() == "https",
            ) {
                let _ = response.headers_mut().insert(
                    actix_web::http::header::SET_COOKIE,
                    actix_web::http::header::HeaderValue::try_from(cookie)
                        .unwrap_or_else(|_| actix_web::http::header::HeaderValue::from_static("")),
                );
            }
        }
        return response;
    }
    // Cookie / header may already authorize without putting the token in the URL.
    let fake = TokenQuery {
        token: String::new(),
    };
    if authorized_request(&state, &fake, &request) {
        return serve_index_html();
    }
    HttpResponse::Unauthorized()
        .content_type("text/html; charset=utf-8")
        .body(installer_token_required_html())
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
    if let Err(error) = save_preferred_listen_port(port) {
        return HttpResponse::BadRequest().json(serde_json::json!({"error": error}));
    }

    let restart_required = port != state.bind_port;
    let mut current = state.status.write().await;
    if !restart_required {
        current.listen_port = port;
        if let Some(env_info) = current.environment.as_mut() {
            env_info.port = port;
        }
        current.panel_login_url = None;
    }
    let payload = enrich_status(current.clone(), &state.token);
    let note = if restart_required {
        format!(
            "Preferred listen port {port} saved. Restart with: cpn-installer --port {port} (current session stays on {})",
            state.bind_port
        )
    } else {
        format!("Listen port {port} confirmed for this session")
    };
    HttpResponse::Ok().json(serde_json::json!({
        "status": payload,
        "listen_port": state.bind_port,
        "preferred_listen_port": port,
        "restart_required": restart_required,
        "message": note,
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
    if ["downloading", "installing", "testing"].contains(&current.phase) {
        return HttpResponse::Conflict()
            .json(serde_json::json!({"error": "Ya hay una instalación en curso"}));
    }
    if current.server_ready && !request.force_reinstall {
        return HttpResponse::Conflict().json(serde_json::json!({
            "error": "El servidor web ya está instalado. Envía force_reinstall=true o usa repair/upgrade."
        }));
    }
    if !matches!(current.phase, "ready" | "completed" | "failed") {
        return HttpResponse::Conflict().json(serde_json::json!({
            "error": "Transición no válida para instalar el servidor"
        }));
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
    tokio::spawn(cpn_installer::installer::install(
        state.get_ref().clone(),
        request.server,
    ));
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
    if ["downloading", "installing", "testing"].contains(&current.phase) {
        return HttpResponse::Conflict()
            .json(serde_json::json!({"error": "Ya hay una instalación en curso"}));
    }
    if !current.server_ready {
        return HttpResponse::Conflict().json(serde_json::json!({
            "error": "Instala y verifica el servidor web antes del correo"
        }));
    }
    if current.selected_mail.is_some() && current.phase == "completed" && !request.force_reinstall {
        return HttpResponse::Conflict().json(serde_json::json!({
            "error": "El correo ya está instalado. Envía force_reinstall=true para cambiar de receta."
        }));
    }
    if !matches!(current.phase, "completed" | "failed") {
        return HttpResponse::Conflict().json(serde_json::json!({
            "error": "Transición no válida para instalar el correo"
        }));
    }
    current.selected_mail = Some(request.mail);
    current.phase = "downloading";
    current.progress = 0;
    current.error = None;
    drop(current);
    tokio::spawn(cpn_installer::installer::install_mail(
        state.get_ref().clone(),
        request.mail,
    ));
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
    initial.panel_login_url = Some(panel_login_url_for(&initial, &token));
    let state = Arc::new(AppState {
        status: tokio::sync::RwLock::new(initial),
        events,
        token: token.clone(),
        session_id,
        bind_port: listen_port,
        allow_remote: remote,
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
    println!("\nMantén esta ventana abierta hasta finalizar. Pulsa Ctrl+C para detener.\n");
    let hosts = listen_hosts();
    let mut server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .service(root_page)
            .service(api_status)
            .service(status_page)
            .service(login_page)
            .service(login_submit)
            .service(forgot_password_page)
            .service(forgot_password_submit)
            .service(set_language)
            .service(set_listen_port)
            .service(account_setup)
            .service(start_install)
            .service(start_mail_install)
            .service(cpn_installer::maintenance_api::api_version_check)
            .service(cpn_installer::maintenance_api::api_releases)
            .service(cpn_installer::maintenance_api::start_maintenance)
            .route("/api/events", web::get().to(websocket))
            .route("/{path:.*}", web::get().to(static_asset))
    })
    .keep_alive(Duration::from_secs(30));
    for host in hosts {
        server = server.bind((host.as_str(), listen_port))?;
    }
    let running = server.run();
    let result = running.await;
    if remote {
        let _ = cpn_installer::environment::close_installer_port(&environment).await;
    }
    result
}
