//! Outbound mail helpers. Prefer configured SMTP; fall back to local Postfix.

use crate::postfix_fallback::{postfix_is_ready, postfix_local_smtp};
use crate::smtp_settings::{SmtpSettings, SmtpTlsMode, load_smtp};
use lettre::message::{Mailbox, Message};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::{Tls, TlsParameters};
use lettre::{SmtpTransport, Transport};

#[derive(Debug, Clone)]
pub struct OutboundMessage {
    pub to: String,
    pub subject: String,
    pub body: String,
}

pub fn smtp_is_ready() -> bool {
    if load_smtp().is_some_and(|settings| {
        !settings.host.trim().is_empty() && !settings.from_address.trim().is_empty()
    }) {
        return true;
    }
    postfix_is_ready()
}

/// Resolve outbound settings: disk SMTP first, else Postfix localhost when ready.
pub fn resolve_outbound_settings(from_hint: Option<&str>) -> Result<SmtpSettings, String> {
    if let Some(settings) = load_smtp()
        && !settings.host.trim().is_empty()
        && !settings.from_address.trim().is_empty()
    {
        return Ok(settings);
    }
    if postfix_is_ready() {
        return Ok(postfix_local_smtp(from_hint.unwrap_or("")));
    }
    Err("No outbound mail path: configure SMTP or install/enable local Postfix.".into())
}

/// Best-effort send via configured SMTP or Postfix localhost.
pub fn send_mail(message: &OutboundMessage) -> Result<(), String> {
    let settings = resolve_outbound_settings(Some(message.to.as_str()))?;
    send_mail_with_settings(&settings, message)
}

pub fn send_mail_with_settings(
    settings: &SmtpSettings,
    message: &OutboundMessage,
) -> Result<(), String> {
    if settings.host.trim().is_empty() || settings.from_address.trim().is_empty() {
        return Err("SMTP is not configured".into());
    }
    if message.to.trim().is_empty() {
        return Err("Recipient address is empty".into());
    }

    let from: Mailbox = settings
        .from_address
        .trim()
        .parse()
        .map_err(|error| format!("Invalid SMTP from address: {error}"))?;
    let to: Mailbox = message
        .to
        .trim()
        .parse()
        .map_err(|error| format!("Invalid recipient address: {error}"))?;

    let email = Message::builder()
        .from(from)
        .to(to)
        .subject(sanitize_header(&message.subject))
        .body(message.body.clone())
        .map_err(|error| format!("Could not build email: {error}"))?;

    let transport = build_transport(settings)?;
    transport
        .send(&email)
        .map_err(|error| format!("SMTP send failed: {error}"))?;
    Ok(())
}

fn build_transport(settings: &SmtpSettings) -> Result<SmtpTransport, String> {
    let host = settings.host.trim();
    let mut builder = match settings.tls_mode {
        SmtpTlsMode::Tls => {
            let tls = TlsParameters::new(host.to_string())
                .map_err(|error| format!("TLS setup failed: {error}"))?;
            SmtpTransport::relay(host)
                .map_err(|error| format!("SMTP relay setup failed: {error}"))?
                .port(settings.port)
                .tls(Tls::Wrapper(tls))
        }
        SmtpTlsMode::Starttls => {
            let tls = TlsParameters::new(host.to_string())
                .map_err(|error| format!("STARTTLS setup failed: {error}"))?;
            SmtpTransport::starttls_relay(host)
                .map_err(|error| format!("SMTP STARTTLS setup failed: {error}"))?
                .port(settings.port)
                .tls(Tls::Required(tls))
        }
        SmtpTlsMode::None => SmtpTransport::builder_dangerous(host).port(settings.port),
    };

    if !settings.username.trim().is_empty() {
        builder = builder.credentials(Credentials::new(
            settings.username.clone(),
            settings.password.clone(),
        ));
    }

    Ok(builder.build())
}

fn sanitize_header(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch == '\r' || ch == '\n' { ' ' } else { ch })
        .collect()
}

pub fn build_setup_confirmation(
    username: &str,
    login_url: &str,
    include_password: bool,
    password: Option<&str>,
) -> OutboundMessage {
    let mut body = format!(
        "Your CPN panel account was created.\r\n\r\nUsername: {username}\r\nLogin: {login_url}\r\n"
    );
    if include_password {
        if let Some(value) = password {
            body.push_str("\r\nPassword: ");
            body.push_str(value);
            body.push_str(
                "\r\n\r\nStore this password securely. Prefer changing it after first login.\r\n",
            );
        }
    } else {
        body.push_str(
            "\r\nThe password was not included in this message. Use the password you set during setup.\r\n",
        );
    }
    OutboundMessage {
        to: String::new(),
        subject: "CPN panel account ready".into(),
        body,
    }
}

/// Password reset email with a one-time reset URL (preferred path).
pub fn build_password_reset_email(reset_url: &str, login_url: &str) -> OutboundMessage {
    OutboundMessage {
        to: String::new(),
        subject: "CPN panel password reset request".into(),
        body: format!(
            "A password reset was requested for your CPN panel account.\r\n\r\n\
Open this link to choose a new password (the link expires in about one hour and can be used only once):\r\n\
{reset_url}\r\n\r\n\
If you did not request this, you can ignore this message. Your password will stay unchanged.\r\n\r\n\
Sign in page: {login_url}\r\n\r\n\
If the link does not work, ask a server operator to reset the account with the CPN CLI.\r\n"
        ),
    }
}

/// Legacy notice without a token (kept for callers that only have a login URL).
pub fn build_password_reset_notice(login_url: &str) -> OutboundMessage {
    build_password_reset_email(login_url, login_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_reset_email_contains_token_url() {
        let reset = "https://panel.example/reset-password?token=abc123deadbeef";
        let login = "https://panel.example/login";
        let msg = build_password_reset_email(reset, login);
        assert_eq!(msg.subject, "CPN panel password reset request");
        assert!(msg.body.contains(reset), "body must include reset URL");
        assert!(msg.body.contains(login), "body must include login URL");
        assert!(
            msg.body.contains("expires") || msg.body.contains("once"),
            "body should mention time/single-use"
        );
        assert!(
            !msg.body.contains("operator can reset the account when mail delivery"),
            "must not be the old operator-only dead-end body"
        );
    }
}
