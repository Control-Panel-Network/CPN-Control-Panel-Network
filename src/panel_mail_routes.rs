//! Email mailbox create/enable/disable panel routes.

use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use crate::mail_accounts::{MailAccountInput, MailSmtpMode, create_account, set_account_enabled};
use crate::smtp_settings::SmtpTlsMode;
use actix_web::{HttpRequest, HttpResponse, post, web};
use std::sync::Arc;

fn login_redirect() -> HttpResponse {
    HttpResponse::SeeOther()
        .append_header(("Location", "/login"))
        .finish()
}

fn urlencoding_simple(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 3);
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn email_redirect(notice: Option<&str>, error: Option<&str>) -> String {
    let mut url = "/email/accounts".to_string();
    if let Some(notice) = notice {
        url.push_str(&format!("?notice={}", urlencoding_simple(notice)));
    } else if let Some(error) = error {
        url.push_str(&format!("?error={}", urlencoding_simple(error)));
    }
    url
}

#[derive(Debug, serde::Deserialize)]
pub struct MailAccountCreateForm {
    #[serde(default)]
    address: String,
    #[serde(default)]
    domain: String,
    #[serde(default)]
    enabled: String,
    #[serde(default)]
    smtp_mode: String,
    #[serde(default)]
    smtp_host: String,
    #[serde(default)]
    smtp_port: String,
    #[serde(default)]
    smtp_tls: String,
    #[serde(default)]
    smtp_username: String,
    #[serde(default)]
    smtp_password: String,
}

impl MailAccountCreateForm {
    fn to_input(&self) -> Result<MailAccountInput, String> {
        let smtp_mode = match self.smtp_mode.trim().to_ascii_lowercase().as_str() {
            "external" | "smtp" => MailSmtpMode::External,
            "postfix" | "postfix_local" | "local" | "" => MailSmtpMode::PostfixLocal,
            other => {
                return Err(format!(
                    "Unknown SMTP mode `{other}`. Use external or postfix_local."
                ));
            }
        };
        let enabled = matches!(
            self.enabled.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        );
        let smtp_port = if self.smtp_port.trim().is_empty() {
            None
        } else {
            Some(
                self.smtp_port
                    .trim()
                    .parse::<u16>()
                    .map_err(|_| "SMTP port must be a number".to_string())?,
            )
        };
        let smtp_tls = match self.smtp_tls.trim().to_ascii_lowercase().as_str() {
            "" | "starttls" => Some(SmtpTlsMode::Starttls),
            "tls" => Some(SmtpTlsMode::Tls),
            "none" => Some(SmtpTlsMode::None),
            other => return Err(format!("Unknown TLS mode `{other}`")),
        };
        Ok(MailAccountInput {
            address: self.address.clone(),
            domain: self.domain.clone(),
            enabled,
            smtp_mode,
            smtp_host: self.smtp_host.clone(),
            smtp_port,
            smtp_tls,
            smtp_username: self.smtp_username.clone(),
            smtp_password: self.smtp_password.clone(),
        })
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct MailAccountIdForm {
    #[serde(default)]
    id: String,
}

#[post("/email/accounts/create")]
pub async fn email_account_create(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<MailAccountCreateForm>,
) -> HttpResponse {
    if panel_user_from_request(&state, &http).is_none() {
        return login_redirect();
    }
    match form.to_input().and_then(create_account) {
        Ok(account) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                email_redirect(Some(&format!("Created mailbox {}", account.address)), None),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header(("Location", email_redirect(None, Some(&error))))
            .finish(),
    }
}

#[post("/email/accounts/enable")]
pub async fn email_account_enable(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<MailAccountIdForm>,
) -> HttpResponse {
    if panel_user_from_request(&state, &http).is_none() {
        return login_redirect();
    }
    match set_account_enabled(&form.id, true) {
        Ok(account) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                email_redirect(Some(&format!("Enabled {}", account.address)), None),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header(("Location", email_redirect(None, Some(&error))))
            .finish(),
    }
}

#[post("/email/accounts/disable")]
pub async fn email_account_disable(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<MailAccountIdForm>,
) -> HttpResponse {
    if panel_user_from_request(&state, &http).is_none() {
        return login_redirect();
    }
    match set_account_enabled(&form.id, false) {
        Ok(account) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                email_redirect(Some(&format!("Disabled {}", account.address)), None),
            ))
            .finish(),
        Err(error) => HttpResponse::SeeOther()
            .append_header(("Location", email_redirect(None, Some(&error))))
            .finish(),
    }
}
