mod environment;
mod installer;
mod model;

use actix_web::{App, HttpRequest, HttpResponse, HttpServer, Responder, get, post, web};
use futures_util::StreamExt;
use installer::AppState;
use model::{InstallRequest, InstallerEvent, InstallerStatus, MailInstallRequest, TokenQuery};
use rand::{Rng, distr::Alphanumeric};
use rust_embed::Embed;
use std::{sync::Arc, time::Duration};
use tokio::sync::broadcast;

const PORT: u16 = 8787;

#[derive(Embed)]
#[folder = "installer-ui/dist"]
struct UiAssets;

fn authorized(state: &AppState, query: &TokenQuery) -> bool {
    state.token.as_bytes() == query.token.as_bytes()
}

#[get("/api/status")]
async fn status(state: web::Data<Arc<AppState>>, query: web::Query<TokenQuery>) -> impl Responder {
    if !authorized(&state, &query) {
        return HttpResponse::Unauthorized().finish();
    }
    HttpResponse::Ok().json(state.status.read().await.clone())
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
            .json(serde_json::json!({"error": "Ya hay una instalación en curso"}));
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
    println!("\nCPN Server Panel · Instalador");
    println!("Iniciando el instalador web...\n");
    let token: String = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(28)
        .map(char::from)
        .collect();
    let environment = environment::inspect(PORT).await;
    if let Err(error) = environment::open_installer_port(&environment).await {
        eprintln!("Aviso: {error}");
    }
    let (events, _) = broadcast::channel(256);
    let state = Arc::new(AppState {
        status: tokio::sync::RwLock::new(InstallerStatus {
            phase: "ready",
            progress: 0,
            message: "El sistema está listo para continuar".into(),
            selected_server: None,
            selected_mail: None,
            environment: Some(environment.clone()),
            error: None,
        }),
        events,
        token: token.clone(),
    });
    println!("✓ El instalador web está listo para empezar:");
    if environment.addresses.is_empty() {
        println!("  http://127.0.0.1:{PORT}/?token={token}");
    } else {
        for address in &environment.addresses {
            println!("  http://{address}:{PORT}/?token={token}");
        }
    }
    println!("\nMantén esta ventana abierta hasta finalizar. Pulsa Ctrl+C para detener.\n");
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .service(status)
            .service(start_install)
            .service(start_mail_install)
            .route("/api/events", web::get().to(websocket))
            .route("/{path:.*}", web::get().to(static_asset))
    })
    .keep_alive(Duration::from_secs(30))
    .bind(("0.0.0.0", PORT))?
    .run()
    .await
}
