mod domain;
mod environment;
mod installer;
mod model;
mod oauth;
mod panel;
mod secrets;

use actix_web::{
    App, HttpRequest, HttpResponse, HttpServer, Responder,
    cookie::{Cookie, SameSite},
    get, post, web,
};
use futures_util::StreamExt;
use installer::AppState;
use model::{
    BootstrapQuery, CloudflareCallbackQuery, DnsProvider, DnsRequest, DomainRequest,
    InstallRequest, InstallerEvent, InstallerPhase, InstallerStatus, MailInstallRequest,
    SetupStage,
};
use rand::{Rng, distr::Alphanumeric};
use rust_embed::Embed;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use subtle::ConstantTimeEq;
use tokio::sync::{RwLock, broadcast};

const PORT: u16 = 8787;
const SESSION_COOKIE: &str = "cpn_session";

#[derive(Embed)]
#[folder = "installer-ui/dist"]
struct UiAssets;

fn same_secret(left: &str, right: &str) -> bool {
    left.as_bytes().ct_eq(right.as_bytes()).into()
}

fn authorized(state: &AppState, request: &HttpRequest) -> bool {
    request
        .cookie(SESSION_COOKIE)
        .is_some_and(|cookie| same_secret(cookie.value(), &state.token))
}

fn request_host(request: &HttpRequest) -> String {
    let host = request.connection_info().host().split(':').next().unwrap_or("");
    if host.len() > 253 || !host.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'[' | b']')) {
        return String::new();
    }
    host.to_owned()
}

fn safe_error(value: &str) -> String {
    value
        .chars()
        .filter(|character| !matches!(character, '\r' | '\n' | '\0'))
        .take(240)
        .collect()
}

fn origin_is_same_host(request: &HttpRequest) -> bool {
    let Some(origin) = request.headers().get("origin") else {
        return true;
    };
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    url::Url::parse(origin)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .is_some_and(|origin_host| origin_host.eq_ignore_ascii_case(&request_host(request)))
}

fn require_auth(state: &AppState, request: &HttpRequest) -> Result<(), HttpResponse> {
    if !authorized(state, request) {
        return Err(
            HttpResponse::Unauthorized().json(serde_json::json!({"error":"Sesión no válida"}))
        );
    }
    if !origin_is_same_host(request) {
        return Err(
            HttpResponse::Forbidden().json(serde_json::json!({"error":"Origen no permitido"}))
        );
    }
    Ok(())
}

#[get("/api/bootstrap")]
async fn bootstrap(
    state: web::Data<Arc<AppState>>,
    request: HttpRequest,
    query: web::Query<BootstrapQuery>,
) -> impl Responder {
    if !same_secret(&query.token, &state.token)
        || state
            .bootstrap_used
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
    {
        return HttpResponse::Unauthorized()
            .body("El enlace de inicio no es válido o ya fue utilizado");
    }
    let cookie = Cookie::build(SESSION_COOKIE, state.token.clone())
        .path("/")
        .http_only(true)
        .same_site(SameSite::Strict)
        .secure(request.connection_info().scheme() == "https")
        .max_age(actix_web::cookie::time::Duration::hours(8))
        .finish();
    HttpResponse::SeeOther()
        .append_header(("Location", "/"))
        .cookie(cookie)
        .finish()
}

#[get("/api/status")]
async fn get_status(state: web::Data<Arc<AppState>>, request: HttpRequest) -> impl Responder {
    if let Err(response) = require_auth(&state, &request) {
        return response;
    }
    HttpResponse::Ok().json(state.status.read().await.clone())
}

#[post("/api/domain/validate")]
async fn validate_domain(
    state: web::Data<Arc<AppState>>,
    request: HttpRequest,
    input: web::Json<DomainRequest>,
) -> impl Responder {
    if let Err(response) = require_auth(&state, &request) {
        return response;
    }
    if state.status.read().await.stage != SetupStage::Domain {
        return HttpResponse::Conflict()
            .json(serde_json::json!({"error":"Primero instala el servidor web y el webmail"}));
    }
    let validation = domain::validate_domain(&input.domain).await;
    if validation.valid {
        let mut current = state.status.write().await;
        current.domain = validation.normalized.clone();
        current.domain_is_cloudflare = validation.cloudflare;
        current.dns_provider = None;
        current.cloudflare_connected = false;
        current.stage = SetupStage::Dns;
        current.phase = InstallerPhase::Ready;
        current.message = "Dominio validado. Selecciona cómo administrar el DNS".into();
        let _ = state.events.send(InstallerEvent::Progress {
            status: current.clone(),
        });
    }
    HttpResponse::Ok().json(validation)
}

#[post("/api/dns/configure")]
async fn configure_dns(
    state: web::Data<Arc<AppState>>,
    request: HttpRequest,
    input: web::Json<DnsRequest>,
) -> impl Responder {
    if let Err(response) = require_auth(&state, &request) {
        return response;
    }
    if input.provider != DnsProvider::Local {
        return HttpResponse::BadRequest()
            .json(serde_json::json!({"error":"Cloudflare requiere autorización OAuth"}));
    }
    let mut current = state.status.write().await;
    if current.stage != SetupStage::Dns || current.domain.is_none() {
        return HttpResponse::Conflict()
            .json(serde_json::json!({"error":"Primero valida el dominio"}));
    }
    current.dns_provider = Some(DnsProvider::Local);
    current.message = "Preparando el Panel permanente".into();
    let snapshot = current.clone();
    drop(current);
    let installed = match panel::provision(&snapshot, None).await {
        Ok(installed) => installed,
        Err(error) => {
            let mut current = state.status.write().await;
            current.error = Some(error.clone());
            current.message = "No se pudo activar el Panel permanente".into();
            return HttpResponse::InternalServerError().json(serde_json::json!({"error":error}));
        }
    };
    let mut current = state.status.write().await;
    current.stage = SetupStage::Complete;
    current.phase = InstallerPhase::Completed;
    current.progress = 100;
    current.panel_url = Some(installed.url);
    current.panel_admin_email = Some(installed.email);
    current.panel_admin_password = Some(installed.password);
    current.message = "DNS local configurado. El Panel ya está operativo".into();
    let snapshot = current.clone();
    let _ = state.events.send(InstallerEvent::Progress {
        status: snapshot.clone(),
    });
    HttpResponse::Ok().json(snapshot)
}

#[post("/api/dns/cloudflare/start")]
async fn start_cloudflare(state: web::Data<Arc<AppState>>, request: HttpRequest) -> impl Responder {
    if let Err(response) = require_auth(&state, &request) {
        return response;
    }
    let (domain, eligible) = {
        let current = state.status.read().await;
        (
            current.domain.clone(),
            current.stage == SetupStage::Dns && current.domain_is_cloudflare,
        )
    };
    if !eligible {
        return HttpResponse::Conflict()
            .json(serde_json::json!({"error":"El dominio no usa nameservers de Cloudflare"}));
    }
    let scheme = request
        .headers()
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .filter(|scheme| matches!(*scheme, "http" | "https"))
        .unwrap_or("http");
    let host = request_host(&request);
    if host.is_empty() {
        return HttpResponse::BadRequest()
            .json(serde_json::json!({"error":"La dirección del instalador no es válida"}));
    }
    let callback = format!(
        "{scheme}://{host}/api/dns/cloudflare/callback"
    );
    match oauth::start(&callback, domain.as_deref().unwrap_or_default()).await {
        Ok((pending, authorization_url)) => {
            *state.pending_oauth.write().await = Some(pending);
            state.status.write().await.error = None;
            HttpResponse::Ok().json(serde_json::json!({"authorization_url":authorization_url}))
        }
        Err(error) => HttpResponse::BadGateway().json(serde_json::json!({"error":error})),
    }
}

#[get("/api/dns/cloudflare/callback")]
async fn cloudflare_callback(
    state: web::Data<Arc<AppState>>,
    query: web::Query<CloudflareCallbackQuery>,
) -> impl Responder {
    let pending = state.pending_oauth.write().await.take();
    let Some(pending) = pending else {
        return HttpResponse::BadRequest()
            .body("La autorización no corresponde a una sesión activa");
    };
    if !same_secret(&pending.session_id, &query.session) {
        return HttpResponse::BadRequest().body("La sesión OAuth no coincide");
    }
    if let Some(error) = query.oauth_error.as_deref() {
        let mut current = state.status.write().await;
        current.phase = InstallerPhase::Ready;
        current.error = Some(format!("Cloudflare rechazó la autorización: {}", safe_error(error)));
        current.message =
            "No se pudo autorizar Cloudflare. Revisa los permisos e inténtalo otra vez".into();
        let _ = state.events.send(InstallerEvent::Error {
            status: current.clone(),
        });
        return HttpResponse::SeeOther()
            .append_header(("Location", "/"))
            .finish();
    }
    let Some(claim) = query.claim.as_deref() else {
        return HttpResponse::BadRequest()
            .body("Cloudflare no devolvió las credenciales esperadas");
    };
    let domain = state.status.read().await.domain.clone().unwrap_or_default();
    match oauth::claim(&pending, claim, &domain).await {
        Ok(authorization) => {
            let snapshot = state.status.read().await.clone();
            let installed = match panel::provision(&snapshot, Some(&authorization)).await {
                Ok(installed) => installed,
                Err(error) => return HttpResponse::InternalServerError().body(error),
            };
            *state.cloudflare.write().await = Some(authorization);
            let mut current = state.status.write().await;
            current.dns_provider = Some(DnsProvider::Cloudflare);
            current.cloudflare_connected = true;
            current.stage = SetupStage::Complete;
            current.phase = InstallerPhase::Completed;
            current.progress = 100;
            current.panel_url = Some(installed.url);
            current.panel_admin_email = Some(installed.email);
            current.panel_admin_password = Some(installed.password);
            current.message = "Cloudflare fue autorizado, cifrado y el Panel está operativo".into();
            current.error = None;
            let _ = state.events.send(InstallerEvent::Progress {
                status: current.clone(),
            });
            HttpResponse::SeeOther()
                .append_header(("Location", "/"))
                .finish()
        }
        Err(error) => HttpResponse::BadGateway().body(error),
    }
}

#[post("/api/install/server")]
async fn start_install(
    state: web::Data<Arc<AppState>>,
    request: HttpRequest,
    input: web::Json<InstallRequest>,
) -> impl Responder {
    if let Err(response) = require_auth(&state, &request) {
        return response;
    }
    if unsafe { libc::geteuid() } != 0 {
        return HttpResponse::BadRequest()
            .json(serde_json::json!({"error":"Ejecuta el instalador como root"}));
    }
    let mut current = state.status.write().await;
    if current.stage != SetupStage::Server || current.phase != InstallerPhase::Ready {
        return HttpResponse::Conflict().json(
            serde_json::json!({"error":"El instalador no está listo para instalar el servidor"}),
        );
    }
    current.selected_server = Some(input.server);
    current.phase = InstallerPhase::Configuring;
    current.progress = 0;
    current.error = None;
    drop(current);
    tokio::spawn(installer::install(state.get_ref().clone(), input.server));
    HttpResponse::Accepted().finish()
}

#[post("/api/install/mail")]
async fn start_mail_install(
    state: web::Data<Arc<AppState>>,
    request: HttpRequest,
    input: web::Json<MailInstallRequest>,
) -> impl Responder {
    if let Err(response) = require_auth(&state, &request) {
        return response;
    }
    if unsafe { libc::geteuid() } != 0 {
        return HttpResponse::BadRequest()
            .json(serde_json::json!({"error":"Ejecuta el instalador como root"}));
    }
    let mut current = state.status.write().await;
    if current.stage != SetupStage::Mail
        || !matches!(
            current.phase,
            InstallerPhase::Ready | InstallerPhase::Completed
        )
    {
        return HttpResponse::Conflict().json(
            serde_json::json!({"error":"El servidor web debe terminar correctamente primero"}),
        );
    }
    current.selected_mail = Some(input.mail);
    current.phase = InstallerPhase::Configuring;
    current.progress = 0;
    current.error = None;
    drop(current);
    tokio::spawn(installer::install_mail(state.get_ref().clone(), input.mail));
    HttpResponse::Accepted().finish()
}

async fn websocket(
    request: HttpRequest,
    body: web::Payload,
    state: web::Data<Arc<AppState>>,
) -> actix_web::Result<HttpResponse> {
    if require_auth(&state, &request).is_err() {
        return Ok(HttpResponse::Unauthorized().finish());
    }
    let (response, mut session, mut messages) = actix_ws::handle(&request, body)?;
    let mut events = state.events.subscribe();
    let snapshot = InstallerEvent::Snapshot {
        status: state.status.read().await.clone(),
    };
    actix_web::rt::spawn(async move {
        let _ = session
            .text(serde_json::to_string(&snapshot).unwrap_or_default())
            .await;
        loop {
            tokio::select! {
                event = events.recv() => match event {
                    Ok(event) => if session.text(serde_json::to_string(&event).unwrap_or_default()).await.is_err() { break; },
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                },
                message = messages.next() => match message {
                    Some(Ok(actix_ws::Message::Ping(value))) => { let _ = session.pong(&value).await; }
                    Some(Ok(actix_ws::Message::Close(reason))) => { let _ = session.close(reason).await; break; }
                    None | Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    });
    Ok(response)
}

async fn static_asset(path: web::Path<String>) -> impl Responder {
    let requested = path.into_inner();
    let name = if requested.is_empty() {
        "index.html"
    } else {
        requested.as_str()
    };
    let asset = UiAssets::get(name).or_else(|| UiAssets::get("index.html"));
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

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    if std::env::args().any(|argument| argument == "--version") {
        println!("cpn-installer {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let remote_access = std::env::args().any(|argument| argument == "--allow-remote");
    if std::env::var_os("CPN_ALLOW_UNSUPPORTED_DEV").is_none() {
        installer::verify_almalinux().map_err(std::io::Error::other)?;
    }
    let bind_address = if remote_access {
        "0.0.0.0"
    } else {
        "127.0.0.1"
    };
    let token: String = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();
    let environment = environment::inspect(PORT, remote_access).await;
    let firewall_guard = if remote_access {
        match environment::open_installer_port(&environment).await {
            Ok(guard) => Some(guard),
            Err(error) => {
                eprintln!("Aviso: {error}");
                None
            }
        }
    } else {
        None
    };
    let (events, _) = broadcast::channel(256);
    let state = Arc::new(AppState {
        status: RwLock::new(InstallerStatus {
            phase: InstallerPhase::Ready,
            stage: SetupStage::Server,
            message: "El sistema está listo para continuar".into(),
            environment: Some(environment.clone()),
            ..InstallerStatus::default()
        }),
        events,
        token: token.clone(),
        bootstrap_used: AtomicBool::new(false),
        pending_oauth: RwLock::new(None),
        cloudflare: RwLock::new(None),
    });
    println!("\nCPN Control Panel Network · Instalador");
    println!("✓ El instalador web está listo para empezar:");
    if remote_access {
        for address in &environment.addresses {
            println!("  http://{address}:{PORT}/api/bootstrap?token={token}");
        }
    } else {
        println!("  http://127.0.0.1:{PORT}/api/bootstrap?token={token}");
        println!("  Para acceso remoto explícito: cpn-installer --allow-remote");
    }
    println!("\nMantén esta ventana abierta hasta finalizar. Pulsa Ctrl+C para detener.\n");
    let result = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .service(bootstrap)
            .service(get_status)
            .service(validate_domain)
            .service(configure_dns)
            .service(start_cloudflare)
            .service(cloudflare_callback)
            .service(start_install)
            .service(start_mail_install)
            .route("/api/events", web::get().to(websocket))
            .route("/{path:.*}", web::get().to(static_asset))
    })
    .keep_alive(Duration::from_secs(30))
    .bind((bind_address, PORT))?
    .run()
    .await;
    if let Some(guard) = firewall_guard {
        guard.cleanup().await;
    }
    result
}
