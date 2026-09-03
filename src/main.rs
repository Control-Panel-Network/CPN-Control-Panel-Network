use actix_web::{App, HttpRequest, HttpResponse, HttpServer, Responder, get, post, web};
use cpn_installer::account::{account_public_from_disk, default_password_policy, setup_account};
use cpn_installer::auth_pages::{
    forgot_password_ack_html, forgot_password_html, installer_token_required_html,
    login_post_ack_html, panel_login_html,
};
use cpn_installer::installer::AppState;
use cpn_installer::model::{
    AccountSetupRequest, InstallRequest, InstallerEvent, InstallerStatus, LanguageRequest,
    MailInstallRequest, OptionalTokenQuery, TokenQuery,
};
use cpn_installer::status_pages::status_html_page;
use futures_util::StreamExt;
use rand::{Rng, distr::Alphanumeric};
use rust_embed::Embed;
use std::{env, sync::Arc, time::Duration};
use tokio::sync::broadcast;

const PORT: u16 = 8787;
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Embed)]
#[folder = "installer-ui/dist"]
struct UiAssets;

fn authorized(state: &AppState, query: &TokenQuery) -> bool {
    state.token.as_bytes() == query.token.as_bytes()
}

fn token_matches(state: &AppState, token: Option<&str>) -> bool {
    match token {
        Some(value) if !value.is_empty() => state.token.as_bytes() == value.as_bytes(),
        _ => false,
    }
}

fn install_finished(status: &InstallerStatus) -> bool {
    status.phase == "completed"
        || status
            .account
            .as_ref()
            .map(|value| value.configured)
            .unwrap_or(false)
        || account_public_from_disk().is_some()
}

fn wants_html(request: &HttpRequest) -> bool {
    request
        .headers()
        .get(actix_web::http::header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.contains("text/html"))
        .unwrap_or(false)
}

fn normalize_language(raw: &str) -> Result<String, String> {
    let value = raw.trim().to_lowercase();
    match value.as_str() {
        "en" | "en-us" | "en-gb" => Ok("en".into()),
        "es" | "es-es" | "es-mx" => Ok("es".into()),
        "nb" | "nb-no" | "no" | "nn" => Ok("nb".into()),
        _ => Err("Idioma no soportado (usa en, es o nb)".into()),
    }
}

fn panel_login_url_for(status: &InstallerStatus, token: &str) -> String {
    if let Some(existing) = &status.panel_login_url {
        return existing.clone();
    }
    let host = status
        .environment
        .as_ref()
        .and_then(|env_info| env_info.addresses.first())
        .cloned()
        .unwrap_or_else(|| "127.0.0.1".into());
    format!("http://{host}:{PORT}/login?token={token}")
}

fn enrich_status(mut status: InstallerStatus, token: &str) -> InstallerStatus {
    status.version = VERSION.into();
    if status.account.is_none() {
        status.account = account_public_from_disk();
    }
    status.panel_login_url = Some(panel_login_url_for(&status, token));
    status
}

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

#[get("/login")]
async fn login_page(
    state: web::Data<Arc<AppState>>,
    query: web::Query<OptionalTokenQuery>,
) -> HttpResponse {
    let status = state.status.read().await.clone();
    let finished = install_finished(&status);
    let valid_token = token_matches(&state, query.token.as_deref());
    if !(finished || valid_token) {
        return HttpResponse::Unauthorized()
            .content_type("text/html; charset=utf-8")
            .body(installer_token_required_html());
    }
    let payload = enrich_status(status, &state.token);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(panel_login_html(&payload))
}

#[post("/login")]
async fn login_submit(
    state: web::Data<Arc<AppState>>,
    query: web::Query<OptionalTokenQuery>,
) -> HttpResponse {
    let status = state.status.read().await.clone();
    let finished = install_finished(&status);
    let valid_token = token_matches(&state, query.token.as_deref());
    if !(finished || valid_token) {
        return HttpResponse::Unauthorized()
            .content_type("text/html; charset=utf-8")
            .body(installer_token_required_html());
    }
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(login_post_ack_html(query.token.as_deref()))
}

#[derive(Debug, serde::Deserialize)]
struct ForgotPasswordForm {
    #[serde(default)]
    username: String,
    #[serde(default)]
    email: String,
}

#[get("/forgot-password")]
async fn forgot_password_page() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(forgot_password_html())
}

#[post("/forgot-password")]
async fn forgot_password_submit(form: web::Form<ForgotPasswordForm>) -> HttpResponse {
    // Always return the same ack page: no account enumeration, SMTP still stubbed.
    let _ = (form.username.trim(), form.email.trim());
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(forgot_password_ack_html())
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

#[post("/api/account/setup")]
async fn account_setup(
    state: web::Data<Arc<AppState>>,
    query: web::Query<TokenQuery>,
    request: web::Json<AccountSetupRequest>,
) -> HttpResponse {
    if !authorized(&state, &query) {
        return HttpResponse::Unauthorized().finish();
    }
    let mut current = state.status.write().await;
    if ["downloading", "installing", "testing"].contains(&current.phase) {
        return HttpResponse::Conflict()
            .json(serde_json::json!({"error": "Hay una instalación en curso"}));
    }
    let language = request
        .language
        .as_deref()
        .map(normalize_language)
        .transpose();
    let language = match language {
        Ok(Some(value)) => value,
        Ok(None) => current.language.clone(),
        Err(error) => {
            return HttpResponse::BadRequest().json(serde_json::json!({"error": error}));
        }
    };
    let policy = request
        .password_policy
        .clone()
        .unwrap_or_else(|| current.password_policy.clone());
    let result = setup_account(
        request.username.as_deref().unwrap_or(""),
        request.password.as_deref(),
        request.generate_password,
        &request.recovery_email,
        policy.clone(),
        &language,
    );
    match result {
        Ok(setup) => {
            current.account = Some(setup.public.clone());
            current.password_policy = policy;
            current.language = language;
            current.phase = "completed";
            current.message = "Cuenta inicial guardada".into();
            let login_url = panel_login_url_for(&current, &state.token);
            current.panel_login_url = Some(login_url.clone());
            HttpResponse::Ok().json(serde_json::json!({
                "account": setup.public,
                "generated_password": setup.generated_password,
                "panel_login_url": login_url,
            }))
        }
        Err(error) => HttpResponse::BadRequest().json(serde_json::json!({"error": error})),
    }
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
            .json(serde_json::json!({"error": "Ya hay una instalación en curso"}));
    }
    if current.server_ready && current.selected_mail.is_some() && current.phase == "completed" {
        return HttpResponse::Conflict().json(serde_json::json!({
            "error": "La instalación ya finalizó. Usa una operación de reinstalación explícita."
        }));
    }
    if !matches!(current.phase, "ready" | "completed" | "failed") {
        return HttpResponse::Conflict().json(serde_json::json!({
            "error": "Transición no válida para instalar el servidor"
        }));
    }
    current.selected_server = Some(request.server);
    current.selected_mail = None;
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
            .json(serde_json::json!({"error": "Ya hay una instalación en curso"}));
    }
    if !current.server_ready {
        return HttpResponse::Conflict().json(serde_json::json!({
            "error": "Instala y verifica el servidor web antes del correo"
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
    if env::args().any(|arg| arg == "--version" || arg == "-V") {
        println!("cpn-installer {VERSION}");
        return Ok(());
    }
    println!("\nCPN Server Panel · Instalador {VERSION}");
    println!("Iniciando el instalador web...\n");
    let token: String = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(28)
        .map(char::from)
        .collect();
    let environment = cpn_installer::environment::inspect(PORT).await;
    let remote = allow_remote_listen();
    if remote {
        match cpn_installer::environment::open_installer_port(&environment).await {
            Ok(()) => {}
            Err(error) => eprintln!("Aviso: {error}"),
        }
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
        message: "El sistema está listo para continuar".into(),
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
        mail_client_ready: false,
        mail_backend_ready: false,
        external_ports_configured: false,
        access_note: None,
        mail_releases,
    };
    initial.panel_login_url = Some(panel_login_url_for(&initial, &token));
    let state = Arc::new(AppState {
        status: tokio::sync::RwLock::new(initial),
        events,
        token: token.clone(),
    });
    println!("✓ El instalador web está listo para empezar:");
    if remote {
        println!("  Modo --allow-remote: escucha en 0.0.0.0:{PORT} (HTTP sin TLS).");
        if environment.addresses.is_empty() {
            println!("  http://127.0.0.1:{PORT}/?token={token}");
        } else {
            for address in &environment.addresses {
                println!("  http://{address}:{PORT}/?token={token}");
            }
        }
    } else {
        println!("  http://127.0.0.1:{PORT}/?token={token}");
        println!(
            "  Acceso remoto recomendado vía túnel SSH, por ejemplo:\n  ssh -L {PORT}:127.0.0.1:{PORT} user@host"
        );
        println!("  Para escuchar en todas las interfaces: --allow-remote o CPN_ALLOW_REMOTE=1");
    }
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
    if remote {
        let _ = cpn_installer::environment::close_installer_port(&environment).await;
    }
    result
}
