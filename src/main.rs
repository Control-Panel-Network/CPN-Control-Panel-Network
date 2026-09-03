use actix_web::{App, HttpRequest, HttpResponse, HttpServer, Responder, get, post, web};
use cpn_installer::account::{account_public_from_disk, default_password_policy};
use cpn_installer::auth_api::{
    account_setup, forgot_password_page, forgot_password_submit, login_page, login_submit,
};
use cpn_installer::auth_pages::installer_token_required_html;
use cpn_installer::http_helpers::{
    PORT, VERSION, authorized, enrich_status, install_finished, normalize_language,
    panel_login_url_for, smtp_status_public, token_matches, wants_html,
};
use cpn_installer::installer::AppState;
use cpn_installer::model::{
    InstallRequest, InstallerEvent, InstallerStatus, LanguageRequest, MailInstallRequest,
    OptionalTokenQuery, TokenQuery,
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
            .body("La interfaz web a├║n no est├í incluida en este binario"),
    }
}

#[get("/")]
async fn root_page(
    state: web::Data<Arc<AppState>>,
    query: web::Query<OptionalTokenQuery>,
) -> HttpResponse {
    let status = state.status.read().await.clone();
    if install_finished(&status) {
        return HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish();
    }
    if token_matches(&state, query.token.as_deref()) {
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
    if !authorized(&state, &query) {
        return HttpResponse::Unauthorized().finish();
    }
    let payload = enrich_status(state.status.read().await.clone(), &state.token);
    status_response(&request, &payload)
}

#[get("/status")]
async fn status_page(
    state: web::Data<Arc<AppState>>,
    query: web::Query<TokenQuery>,
) -> HttpResponse {
    if !authorized(&state, &query) {
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
    state: web::Data<Arc<AppState>>,
    query: web::Query<TokenQuery>,
    request: web::Json<LanguageRequest>,
) -> HttpResponse {
    if !authorized(&state, &query) {
        return HttpResponse::Unauthorized().finish();
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

#[post("/api/install/server")]
async fn start_install(
    state: web::Data<Arc<AppState>>,
    query: web::Query<TokenQuery>,
    request: web::Json<InstallRequest>,
) -> impl Responder {
    if !authorized(&state, &query) {
        return HttpResponse::Unauthorized().finish();
    }
    if unsafe { libc::geteuid() } != 0 {
        return HttpResponse::BadRequest().json(
            serde_json::json!({"error": "Ejecuta el instalador como root (sudo cpn-installer)"}),
        );
    }
    let mut current = state.status.write().await;
    if ["downloading", "installing", "testing"].contains(&current.phase) {
        return HttpResponse::Conflict()
            .json(serde_json::json!({"error": "Ya hay una instalaci├│n en curso"}));
    }
    if current.server_ready && current.selected_mail.is_some() && current.phase == "completed" {
        return HttpResponse::Conflict().json(serde_json::json!({
            "error": "La instalaci├│n ya finaliz├│. Usa una operaci├│n de reinstalaci├│n expl├¡cita."
        }));
    }
    if !matches!(current.phase, "ready" | "completed" | "failed") {
        return HttpResponse::Conflict().json(serde_json::json!({
            "error": "Transici├│n no v├ílida para instalar el servidor"
        }));
    }
    current.selected_server = Some(request.server);
    current.selected_mail = None;
    current.phase = "downloading";
    current.progress = 1;
    current.error = None;
    drop(current);
    tokio::spawn(installer::install(state.get_ref().clone(), request.server));
    HttpResponse::Accepted().finish()
}

#[post("/api/install/mail")]
async fn start_mail_install(
    state: web::Data<Arc<AppState>>,
    query: web::Query<TokenQuery>,
    request: web::Json<MailInstallRequest>,
) -> impl Responder {
    if !authorized(&state, &query) {
        return HttpResponse::Unauthorized().finish();
    }
    if unsafe { libc::geteuid() } != 0 {
        return HttpResponse::BadRequest().json(
            serde_json::json!({"error": "Ejecuta el instalador como root (sudo cpn-installer)"}),
        );
    }
    let mut current = state.status.write().await;
    if ["downloading", "installing", "testing"].contains(&current.phase) {
        return HttpResponse::Conflict()
            .json(serde_json::json!({"error": "Ya hay una instalaci├│n en curso"}));
    }
    if !current.server_ready {
        return HttpResponse::Conflict().json(serde_json::json!({
            "error": "Instala y verifica el servidor web antes del correo"
        }));
    }
    if !matches!(current.phase, "completed" | "failed") {
        return HttpResponse::Conflict().json(serde_json::json!({
            "error": "Transici├│n no v├ílida para instalar el correo"
        }));
    }
    current.selected_mail = Some(request.mail);
    current.phase = "downloading";
    current.progress = 0;
    current.error = None;
    drop(current);
    tokio::spawn(installer::install_mail(
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
    if !authorized(&state, &query) {
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
            .body("La interfaz web a├║n no est├í incluida en este binario"),
    }
}

fn listen_hosts() -> Vec<String> {
    let _allow_remote = env::args().any(|arg| arg == "--allow-remote" || arg == "--listen-all")
        || env::var("CPN_ALLOW_REMOTE").ok().as_deref() == Some("1");
    let _ = _allow_remote;
    vec!["0.0.0.0".into()]
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    if env::args().any(|arg| arg == "--version" || arg == "-V") {
        println!("cpn-installer {VERSION}");
        return Ok(());
    }
    println!("\nCPN Server Panel ┬À Instalador {VERSION}");
    println!("Iniciando el instalador web...\n");
    let token: String = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(28)
        .map(char::from)
        .collect();
    let environment = cpn_installer::environment::inspect(PORT).await;
    if let Err(error) = cpn_installer::environment::open_installer_port(&environment).await {
        eprintln!("Aviso: {error}");
    }
    let mail_releases = cpn_installer::mail_releases::load_mail_releases().await;
    let (events, _) = broadcast::channel(256);
    let bootstrap_account = account_public_from_disk();
    let phase = if bootstrap_account.is_some() {
        "completed"
    } else {
        "ready"
    };
    let mut initial = InstallerStatus {
        phase,
        progress: 0,
        message: "El sistema est├í listo para continuar".into(),
        selected_server: None,
        selected_mail: None,
        environment: Some(environment.clone()),
        error: None,
        language: "en".into(),
        account: bootstrap_account,
        password_policy: default_password_policy(),
        panel_login_path: "/login".into(),
        panel_login_url: None,
        version: VERSION.into(),
        server_ready: false,
        mail_releases,
        smtp: Some(smtp_status_public()),
    };
    initial.panel_login_url = Some(panel_login_url_for(&initial, &token));
    let state = Arc::new(AppState {
        status: tokio::sync::RwLock::new(initial),
        events,
        token: token.clone(),
    });
    println!("Ô£ô El instalador web est├í listo para empezar:");
    if environment.addresses.is_empty() {
        println!("  http://127.0.0.1:{PORT}/?token={token}");
    } else {
        for address in &environment.addresses {
            println!("  http://{address}:{PORT}/?token={token}");
        }
    }
    println!("\nMant├®n esta ventana abierta hasta finalizar. Pulsa Ctrl+C para detener.\n");
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
            .service(account_setup)
            .service(start_install)
            .service(start_mail_install)
            .route("/api/events", web::get().to(websocket))
            .route("/{path:.*}", web::get().to(static_asset))
    })
    .keep_alive(Duration::from_secs(30));
    for host in hosts {
        server = server.bind((host.as_str(), PORT))?;
    }
    let running = server.run();
    let result = running.await;
    let _ = cpn_installer::environment::close_installer_port(&environment).await;
    result
}
