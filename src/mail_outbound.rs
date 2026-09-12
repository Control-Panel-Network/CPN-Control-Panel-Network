//! Outbound mail helpers. Prefer configured SMTP; fall back to local Postfix.

use crate::postfix_fallback::{postfix_is_ready, postfix_local_smtp};
use crate::smtp_settings::{SmtpSettings, SmtpTlsMode, load_smtp};
use lettre::message::header::{ContentTransferEncoding, ContentType};
use lettre::message::{Body, Mailbox, Message, SinglePart};
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

    let email = build_plain_message(from, to, &message.subject, &message.body)?;

    let transport = build_transport(settings)?;
    transport
        .send(&email)
        .map_err(|error| format!("SMTP send failed: {error}"))?;
    Ok(())
}

/// Build a plain-text message without quoted-printable so `token=` URLs stay intact.
fn build_plain_message(
    from: Mailbox,
    to: Mailbox,
    subject: &str,
    body: &str,
) -> Result<Message, String> {
    let raw = body.as_bytes().to_vec();
    // Prefer 7bit for ASCII (readable in mail spools). Fall back to base64 for non-ASCII.
    let encoded = Body::new_with_encoding(raw.clone(), ContentTransferEncoding::SevenBit)
        .or_else(|_| Body::new_with_encoding(raw, ContentTransferEncoding::Base64))
        .map_err(|_| "Could not encode email body".to_string())?;

    Message::builder()
        .from(from)
        .to(to)
        .subject(sanitize_header(subject))
        .singlepart(
            SinglePart::builder()
                .header(ContentType::TEXT_PLAIN)
                .body(encoded),
        )
        .map_err(|error| format!("Could not build email: {error}"))
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
pub fn build_password_reset_email(
    reset_url: &str,
    login_url: &str,
    alternate_reset_urls: &[String],
) -> OutboundMessage {
    let mut body = format!(
        "A password reset was requested for your CPN panel account.\r\n\r\n\
Open this link to choose a new password (the link expires in about one hour and can be used only once):\r\n\
{reset_url}\r\n"
    );
    if !alternate_reset_urls.is_empty() {
        body.push_str(
            "\r\nIf that link does not open from your browser (for example a hostname without DNS, or VirtualBox NAT), try:\r\n",
        );
        for alt in alternate_reset_urls {
            body.push_str(alt);
            body.push_str("\r\n");
        }
    }
    body.push_str(
        "\r\nIf you did not request this, you can ignore this message. Your password will stay unchanged.\r\n\r\n\
Sign in page: ",
    );
    body.push_str(login_url);
    body.push_str(
        "\r\n\r\nIf the link does not work, ask a server operator to reset the account with the CPN CLI.\r\n",
    );
    OutboundMessage {
        to: String::new(),
        subject: "CPN panel password reset request".into(),
        body,
    }
}

/// Legacy notice without a token (kept for callers that only have a login URL).
pub fn build_password_reset_notice(login_url: &str) -> OutboundMessage {
    build_password_reset_email(login_url, login_url, &[])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_reset_email_contains_token_url() {
        let reset = "https://panel.example/reset-password?token=abc123deadbeef";
        let login = "https://panel.example/login";
        let msg = build_password_reset_email(reset, login, &[]);
        assert_eq!(msg.subject, "CPN panel password reset request");
        assert!(msg.body.contains(reset), "body must include reset URL");
        assert!(msg.body.contains(login), "body must include login URL");
        assert!(
            msg.body.contains("expires") || msg.body.contains("once"),
            "body should mention time/single-use"
        );
        assert!(
            !msg.body
                .contains("operator can reset the account when mail delivery"),
            "must not be the old operator-only dead-end body"
        );
    }

    #[test]
    fn password_reset_email_lists_alternate_urls() {
        let reset = "http://127.0.0.1:2089/reset-password?token=abcdef";
        let alt = "http://127.0.0.1:2087/reset-password?token=abcdef".to_string();
        let msg = build_password_reset_email(
            reset,
            "http://127.0.0.1:2089/login",
            std::slice::from_ref(&alt),
        );
        assert!(msg.body.contains(reset));
        assert!(msg.body.contains(&alt));
        assert!(msg.body.contains("VirtualBox NAT") || msg.body.contains("does not open"));
    }

    #[test]
    fn plain_message_does_not_qp_mangle_token_equals() {
        let from: Mailbox = "cpn@localhost".parse().unwrap();
        let to: Mailbox = "user@example.com".parse().unwrap();
        let body = "Open:\r\nhttp://127.0.0.1:2089/reset-password?token=9a09f70300845dd9bd32b\r\n";
        let email =
            build_plain_message(from, to, "CPN panel password reset request", body).unwrap();
        let formatted = email.formatted();
        let raw = String::from_utf8_lossy(&formatted);
        assert!(
            raw.contains("token=9a09f70300845dd9bd32b"),
            "raw message must keep literal token="
        );
        assert!(
            !raw.contains("token=3D"),
            "must not quoted-printable encode = in token query"
        );
        assert!(
            raw.to_ascii_lowercase()
                .contains("content-transfer-encoding: 7bit")
                || raw
                    .to_ascii_lowercase()
                    .contains("content-transfer-encoding: base64"),
            "expected 7bit or base64 CTE, got:\n{raw}"
        );
    }
}
