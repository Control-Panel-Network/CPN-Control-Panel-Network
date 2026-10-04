//! Outbound mail helpers. Prefer configured SMTP; fall back to local Postfix.

use crate::postfix_fallback::{postfix_is_ready, postfix_local_smtp};
use crate::smtp_settings::{SmtpSettings, SmtpTlsMode, load_smtp};
use lettre::message::header::{ContentTransferEncoding, ContentType};
use lettre::message::{Body, Mailbox, Message, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::{Tls, TlsParameters};
use lettre::{SmtpTransport, Transport};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct OutboundMessage {
    pub to: String,
    pub subject: String,
    pub body: String,
    pub html_body: Option<String>,
    pub from_name: Option<String>,
}

impl OutboundMessage {
    pub fn new(to: impl Into<String>, subject: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            to: to.into(),
            subject: subject.into(),
            body: body.into(),
            html_body: None,
            from_name: None,
        }
    }
}

fn smtp_configured(settings: &SmtpSettings) -> bool {
    !settings.host.trim().is_empty() && !settings.from_address.trim().is_empty()
}

/// Owner-configured outbound provider from `smtp.json` (installer SMTP or a mail plugin).
/// Localhost rows without credentials are treated as Postfix, not a remote provider.
pub fn configured_provider_smtp() -> Option<SmtpSettings> {
    let settings = load_smtp().filter(smtp_configured)?;
    let host = settings.host.trim();
    let local = host == "127.0.0.1" || host.eq_ignore_ascii_case("localhost");
    if local && settings.username.trim().is_empty() {
        return None;
    }
    Some(settings)
}

pub fn smtp_is_ready() -> bool {
    configured_provider_smtp().is_some() || postfix_is_ready()
}

/// Resolve outbound settings: configured provider first, else Postfix localhost.
pub fn resolve_outbound_settings(from_hint: Option<&str>) -> Result<SmtpSettings, String> {
    if let Some(settings) = configured_provider_smtp() {
        return Ok(settings);
    }
    if postfix_is_ready() {
        return Ok(postfix_local_smtp(from_hint.unwrap_or("")));
    }
    Err(
        "No outbound mail path: install and configure an outbound mail provider plugin, or enable local Postfix."
            .into(),
    )
}

/// Operator-facing mail error. Never include passwords, tokens, or AUTH secrets.
pub fn public_mail_error(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("5.1.1") || lower.contains("virtual alias") || lower.contains("user unknown")
    {
        return "Local Postfix treated the support inbox as a hosted mailbox. Feedback uses a panel outbound listener on port 2525 that relays off-box. Configure an outbound mail provider plugin if this host cannot reach the internet.".into();
    }
    if lower.contains("relay")
        || lower.contains("5.7.1")
        || lower.contains("authentication required")
        || lower.contains("sasl")
    {
        return "Local mail rejected the message (relay or authentication). Feedback uses Postfix on port 25 without AUTH. Configure an outbound mail provider plugin if this host cannot send to the internet.".into();
    }
    if lower.contains("connection refused")
        || (lower.contains("connect") && lower.contains("os error"))
    {
        return "Could not connect to the mail service. Enable Postfix or the installed outbound mail provider.".into();
    }
    if lower.contains("timed out") || lower.contains("timeout") {
        return "Mail server timed out. Check Postfix or the outbound provider host and TLS settings.".into();
    }
    if lower.contains("starttls") || (lower.contains("tls") && lower.contains("failed")) {
        return "Mail TLS handshake failed. Set the correct TLS mode on the outbound mail provider, or use local Postfix.".into();
    }
    if lower.contains("invalid smtp from") || lower.contains("invalid recipient") {
        return "Mail From or recipient address is not valid.".into();
    }
    let mut cleaned = raw.replace(['\r', '\n'], " ");
    for needle in ["password=", "password:", "passwd=", "auth "] {
        if let Some(idx) = cleaned.to_ascii_lowercase().find(needle) {
            cleaned.truncate(idx);
            cleaned.push_str("[redacted]");
            break;
        }
    }
    cleaned = cleaned.chars().take(180).collect();
    if cleaned.trim().is_empty() {
        return "Feedback could not be delivered. Configure an outbound mail provider or local Postfix.".into();
    }
    format!("Feedback could not be delivered: {cleaned}")
}

/// Best-effort send via configured SMTP provider, then Postfix localhost.
pub fn send_mail(message: &OutboundMessage) -> Result<(), String> {
    send_mail_with_fallback(message)
}

pub fn send_mail_with_fallback(message: &OutboundMessage) -> Result<(), String> {
    if let Some(settings) = configured_provider_smtp() {
        match send_mail_with_settings(&settings, message) {
            Ok(()) => return Ok(()),
            Err(error) => {
                if postfix_is_ready() {
                    let _ = crate::postfix_fallback::ensure_postfix_panel_outbound();
                    let local = postfix_local_smtp("");
                    match send_mail_with_settings(&local, message) {
                        Ok(()) => return Ok(()),
                        Err(local_error) => return Err(public_mail_error(&local_error)),
                    }
                }
                return Err(public_mail_error(&error));
            }
        }
    }
    if postfix_is_ready() {
        let _ = crate::postfix_fallback::ensure_postfix_panel_outbound();
        return send_mail_with_settings(&postfix_local_smtp(""), message)
            .map_err(|error| public_mail_error(&error));
    }
    Err(
        "No outbound mail path: install and configure an outbound mail provider plugin, or enable local Postfix."
            .into(),
    )
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

    let email = build_outbound_message(
        apply_from_name(from, message.from_name.as_deref()),
        to,
        &message.subject,
        &message.body,
        message.html_body.as_deref(),
    )?;

    let transport = build_transport(settings)?;
    transport
        .send(&email)
        .map_err(|error| format!("SMTP send failed: {error}"))?;
    Ok(())
}

fn apply_from_name(mut mailbox: Mailbox, name: Option<&str>) -> Mailbox {
    if let Some(raw) = name.map(str::trim).filter(|value| !value.is_empty()) {
        mailbox.name = Some(sanitize_header(raw));
    }
    mailbox
}

fn encode_body(raw: Vec<u8>, preferred: ContentTransferEncoding) -> Result<Body, String> {
    Body::new_with_encoding(raw, preferred)
        .or_else(|bytes| Body::new_with_encoding(bytes, ContentTransferEncoding::EightBit))
        .or_else(|bytes| Body::new_with_encoding(bytes, ContentTransferEncoding::Base64))
        .map_err(|_| "Could not encode email body".to_string())
}

fn text_part(
    body: &str,
    content_type: ContentType,
    encoding: ContentTransferEncoding,
) -> Result<SinglePart, String> {
    Ok(SinglePart::builder()
        .header(content_type)
        .body(encode_body(body.as_bytes().to_vec(), encoding)?))
}

/// Plain-text only: avoid quoted-printable so `token=` URLs stay intact.
fn build_plain_message(
    from: Mailbox,
    to: Mailbox,
    subject: &str,
    body: &str,
) -> Result<Message, String> {
    Message::builder()
        .from(from)
        .to(to)
        .subject(sanitize_header(subject))
        .singlepart(text_part(
            body,
            ContentType::TEXT_PLAIN,
            ContentTransferEncoding::SevenBit,
        )?)
        .map_err(|error| format!("Could not build email: {error}"))
}

fn build_outbound_message(
    from: Mailbox,
    to: Mailbox,
    subject: &str,
    body: &str,
    html_body: Option<&str>,
) -> Result<Message, String> {
    let Some(html) = html_body.filter(|value| !value.trim().is_empty()) else {
        return build_plain_message(from, to, subject, body);
    };
    Message::builder()
        .from(from)
        .to(to)
        .subject(sanitize_header(subject))
        .multipart(
            MultiPart::alternative()
                .singlepart(text_part(
                    body,
                    ContentType::TEXT_PLAIN,
                    ContentTransferEncoding::QuotedPrintable,
                )?)
                .singlepart(text_part(
                    html,
                    ContentType::TEXT_HTML,
                    ContentTransferEncoding::QuotedPrintable,
                )?),
        )
        .map_err(|error| format!("Could not build email: {error}"))
}

fn build_transport(settings: &SmtpSettings) -> Result<SmtpTransport, String> {
    let host = settings.host.trim();
    let timeout = Some(Duration::from_secs(20));
    let mut builder = match settings.tls_mode {
        SmtpTlsMode::Tls => {
            let tls = TlsParameters::new(host.to_string())
                .map_err(|error| format!("TLS setup failed: {error}"))?;
            SmtpTransport::relay(host)
                .map_err(|error| format!("SMTP relay setup failed: {error}"))?
                .port(settings.port)
                .timeout(timeout)
                .tls(Tls::Wrapper(tls))
        }
        SmtpTlsMode::Starttls => {
            let tls = TlsParameters::new(host.to_string())
                .map_err(|error| format!("STARTTLS setup failed: {error}"))?;
            SmtpTransport::starttls_relay(host)
                .map_err(|error| format!("SMTP STARTTLS setup failed: {error}"))?
                .port(settings.port)
                .timeout(timeout)
                .tls(Tls::Required(tls))
        }
        SmtpTlsMode::None => SmtpTransport::builder_dangerous(host)
            .port(settings.port)
            .timeout(timeout),
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
    OutboundMessage::new(String::new(), "CPN panel account ready", body)
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
    OutboundMessage::new(String::new(), "CPN panel password reset request", body)
}

/// Legacy notice without a token (kept for callers that only have a login URL).
pub fn build_password_reset_notice(login_url: &str) -> OutboundMessage {
    build_password_reset_email(login_url, login_url, &[])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_mail_error_hides_secrets_and_maps_relay() {
        let relay = public_mail_error(
            "SMTP send failed: 5.7.1 Relay access denied AUTH LOGIN password=supersecret",
        );
        assert!(!relay.to_ascii_lowercase().contains("supersecret"));
        assert!(relay.contains("port 25") || relay.contains("relay"));
        let auth = public_mail_error("failed password=hunter2");
        assert!(!auth.contains("hunter2"));
        assert!(auth.contains("[redacted]") || auth.contains("delivered"));
    }

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

    #[test]
    fn multipart_feedback_mail_has_html_fallback_and_from_name() {
        let from: Mailbox =
            apply_from_name("cpn-panel@localhost".parse().unwrap(), Some("CPN Panel"));
        let to: Mailbox = "info@example.com".parse().unwrap();
        let html = "<p>Hello &lt;script&gt;</p>";
        let email = build_outbound_message(
            from,
            to,
            "[CPN Feedback] test",
            "CPN Panel feedback\r\n\r\nMessage:\r\ntest\r\n",
            Some(html),
        )
        .unwrap();
        let raw = String::from_utf8_lossy(&email.formatted()).to_string();
        let lower = raw.to_ascii_lowercase();
        assert!(lower.contains("from:"));
        assert!(lower.contains("cpn panel"));
        assert!(lower.contains("cpn-panel@localhost"));
        assert!(lower.contains("multipart/alternative"));
        assert!(lower.contains("text/plain"));
        assert!(lower.contains("text/html"));
        assert!(raw.contains("CPN Panel feedback"));
        assert!(raw.contains("Hello") && raw.contains("script"));
        assert!(!raw.to_ascii_lowercase().contains("cyberpanel"));
    }
}
