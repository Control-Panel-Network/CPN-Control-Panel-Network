//! Default mail client configuration panel (POP3/IMAP/SMTP/Sieve).

use crate::panel_ops_mail_onboarding::{
    MAIL_CLIENT_PLACEHOLDER_DOMAIN, MailMode, load_mail_onboarding, mail_client_display_domains,
    mail_client_server_host,
};

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// One or more client-settings cards from mailbox addresses (scoped per domain).
pub fn mail_client_configs_for_mailboxes(mailbox_addresses: &[String]) -> String {
    let domains = mail_client_display_domains(mailbox_addresses);
    if domains.is_empty() {
        return mail_client_config_html(MAIL_CLIENT_PLACEHOLDER_DOMAIN);
    }
    if domains.len() == 1 {
        return mail_client_config_html(&domains[0]);
    }
    domains
        .iter()
        .map(|domain| {
            format!(
                r#"<div class="mail-client-domain-group" style="margin-top:18px;">
  <h3 style="margin:0 0 8px;font-size:1rem;">{domain}</h3>
  {body}
</div>"#,
                domain = esc(domain),
                body = mail_client_config_html(domain),
            )
        })
        .collect()
}

/// CPN-branded mail client settings card for a mailbox domain.
pub fn mail_client_config_html(domain: &str) -> String {
    let server = mail_client_server_host(domain);
    let cfg = load_mail_onboarding();
    let mode_note = match cfg.mail_mode {
        MailMode::Local => {
            "Local hosting mail (defaults below). Optional external hosts can be set in Server onboarding."
        }
        MailMode::External => {
            "External mail mode is active. Hosts below follow onboarding external IMAP/SMTP when set."
        }
    };
    let smtp_host =
        if cfg.mail_mode == MailMode::External && !cfg.external_smtp_host.trim().is_empty() {
            let ext = cfg.external_smtp_host.trim().to_ascii_lowercase();
            let domain = domain.trim().to_ascii_lowercase();
            let host_matches =
                domain.is_empty() || ext == domain || ext.ends_with(&format!(".{domain}"));
            if host_matches { ext } else { server.clone() }
        } else {
            server.clone()
        };
    format!(
        r#"<article class="section-card mail-client-config" style="margin-top:18px;">
  <h2 style="display:flex;align-items:center;gap:8px;margin:0 0 8px;">
    <span aria-hidden="true">&#9881;</span> Mail Client Configuration
  </h2>
  <p class="muted" style="margin:0 0 14px;">{mode}</p>
  <div class="mail-proto-block" style="margin-bottom:12px;">
    <div style="background:var(--cpn-accent,#3b82f6);color:#fff;padding:8px 12px;font-weight:600;">POP3 Settings</div>
    <table class="data-table" style="width:100%;margin:0;"><tbody>
      <tr><td style="width:28%;">Server</td><td><code>{server}</code></td></tr>
      <tr><td>Port</td><td>110 / 995 (SSL)</td></tr>
      <tr><td>Security</td><td>STARTTLS</td></tr>
    </tbody></table>
  </div>
  <div class="mail-proto-block" style="margin-bottom:12px;">
    <div style="background:var(--cpn-accent,#3b82f6);color:#fff;padding:8px 12px;font-weight:600;">IMAP Settings</div>
    <table class="data-table" style="width:100%;margin:0;"><tbody>
      <tr><td style="width:28%;">Server</td><td><code>{server}</code></td></tr>
      <tr><td>Port</td><td>143 / 993 (SSL)</td></tr>
      <tr><td>Security</td><td>STARTTLS</td></tr>
    </tbody></table>
  </div>
  <div class="mail-proto-block" style="margin-bottom:12px;">
    <div style="background:var(--cpn-accent,#3b82f6);color:#fff;padding:8px 12px;font-weight:600;">SMTP Settings</div>
    <table class="data-table" style="width:100%;margin:0;"><tbody>
      <tr><td style="width:28%;">Server</td><td><code>{smtp}</code></td></tr>
      <tr><td>Port</td><td>25 / 587 / 465 (SSL)</td></tr>
      <tr><td>Security</td><td>STARTTLS</td></tr>
    </tbody></table>
  </div>
  <div class="mail-proto-block">
    <div style="background:var(--cpn-accent,#3b82f6);color:#fff;padding:8px 12px;font-weight:600;">Sieve Settings</div>
    <table class="data-table" style="width:100%;margin:0;"><tbody>
      <tr><td style="width:28%;">Server</td><td><code>{server}</code></td></tr>
      <tr><td>Port</td><td>4190</td></tr>
      <tr><td>Security</td><td>None (Plain)</td></tr>
      <tr><td>Purpose</td><td>Email Filtering</td></tr>
    </tbody></table>
  </div>
</article>"#,
        mode = esc(mode_note),
        server = esc(&server),
        smtp = esc(&smtp_host),
    )
}
