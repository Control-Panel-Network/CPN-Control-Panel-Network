//! Shared HTTP helpers for installer status and auth routes.

use crate::account::account_public_from_disk;
use crate::installer::AppState;
use crate::model::{InstallerStatus, SmtpStatusPublic, TokenQuery};
use crate::smtp_settings::{SmtpTlsMode, smtp_public_from_disk};
use actix_web::HttpRequest;

pub const PORT: u16 = 8787;
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn authorized(state: &AppState, query: &TokenQuery) -> bool {
    state.token.as_bytes() == query.token.as_bytes()
}

pub fn token_matches(state: &AppState, token: Option<&str>) -> bool {
    match token {
        Some(value) if !value.is_empty() => state.token.as_bytes() == value.as_bytes(),
        _ => false,
    }
}

pub fn install_finished(status: &InstallerStatus) -> bool {
    status.phase == "completed"
        || status
            .account
            .as_ref()
            .map(|value| value.configured)
            .unwrap_or(false)
        || account_public_from_disk().is_some()
}

pub fn wants_html(request: &HttpRequest) -> bool {
    request
        .headers()
        .get(actix_web::http::header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.contains("text/html"))
        .unwrap_or(false)
}

pub fn normalize_language(raw: &str) -> Result<String, String> {
    let value = raw.trim().to_lowercase();
    match value.as_str() {
        "en" | "en-us" | "en-gb" => Ok("en".into()),
        "es" | "es-es" | "es-mx" => Ok("es".into()),
        "nb" | "nb-no" | "no" | "nn" => Ok("nb".into()),
        _ => Err("Idioma no soportado (usa en, es o nb)".into()),
    }
}

pub fn panel_login_url_for(status: &InstallerStatus, token: &str) -> String {
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

pub fn smtp_status_public() -> SmtpStatusPublic {
    let value = smtp_public_from_disk();
    let tls_mode = value.tls_mode.map(|mode| match mode {
        SmtpTlsMode::Starttls => "starttls".into(),
        SmtpTlsMode::Tls => "tls".into(),
        SmtpTlsMode::None => "none".into(),
    });
    SmtpStatusPublic {
        configured: value.configured,
        host: value.host,
        port: value.port,
        tls_mode,
        from_address: value.from_address,
    }
}

pub fn enrich_status(mut status: InstallerStatus, token: &str) -> InstallerStatus {
    status.version = VERSION.into();
    if status.account.is_none() {
        status.account = account_public_from_disk();
    }
    status
        .panel_login_url
        .replace(panel_login_url_for(&status, token));
    // Never include SMTP passwords or SMTP auth usernames in public JSON.
    status.smtp = Some(smtp_status_public());
    status
}
