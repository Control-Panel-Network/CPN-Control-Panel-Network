//! Email → Proton Mail hub page (external / Bridge).

use crate::panel_hubs::feature_shell;
use crate::panel_ops_proton_mail::{
    ProtonMailSettings, load_proton_mail_settings, open_button_label, parse_port,
    sanitize_open_url, save_proton_mail_settings,
};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn flash(kind: &str, message: Option<&str>) -> String {
    let Some(message) = message.filter(|v| !v.is_empty()) else {
        return String::new();
    };
    let class = if kind == "error" {
        "panel-notice error"
    } else {
        "panel-notice ok"
    };
    format!(
        r#"<p class="{class}" role="status">{}</p>"#,
        html_escape(message)
    )
}

fn page_styles() -> &'static str {
    r#"
.proton-layout { display:grid; gap:16px; width:100%; max-width:100%; min-width:0; }
.proton-layout .stack-form { max-width:560px; width:100%; }
.proton-cards { display:grid; gap:14px; width:100%; max-width:100%; }
.proton-card {
  display:grid; gap:10px; padding:14px 16px; min-width:0; width:100%; box-sizing:border-box;
  border:1px solid var(--hairline, #2a2f3a); border-radius:14px;
  background:var(--panel, #1a1d26);
}
.proton-card h3 { margin:0; font-size:16px; }
.proton-actions { display:flex; flex-wrap:wrap; gap:10px; align-items:center; margin-top:4px; }
.proton-actions .btn-primary,
.proton-actions .btn-secondary {
  min-height:40px; padding:0 16px; border-radius:999px; font-weight:700;
}
.proton-meta {
  display:grid; grid-template-columns:repeat(auto-fit,minmax(160px,1fr)); gap:8px; min-width:0;
}
.proton-meta > div {
  border:1px solid var(--hairline, #2a2f3a); border-radius:10px; padding:8px 10px; min-width:0;
}
.proton-meta span { display:block; font-size:11px; color:#98a2b3; font-weight:600; }
.proton-meta code, .proton-meta strong {
  display:block; margin-top:4px; font-size:13px; font-weight:600; overflow-wrap:anywhere;
}
@media (max-width:720px) {
  .proton-layout .stack-form { max-width:100%; }
  .proton-actions a, .proton-actions button { width:100%; justify-content:center; text-align:center; }
  .proton-meta { grid-template-columns:1fr; }
}
"#
}

pub fn email_proton_page(notice: Option<&str>, error: Option<&str>) -> String {
    let settings = load_proton_mail_settings();
    let open_enabled = if settings.open_button_enabled {
        " checked"
    } else {
        ""
    };
    let label = open_button_label(&settings);
    let open_btn = if settings.open_button_enabled {
        format!(
            r#"<a class="btn-primary" href="{url}" target="_blank" rel="noopener noreferrer">{label}</a>"#,
            url = html_escape(&settings.open_url),
            label = html_escape(&label),
        )
    } else {
        r#"<span class="muted">Open Proton Mail button is disabled in settings.</span>"#.into()
    };
    let account_line = if settings.account_email.trim().is_empty() {
        "Not set".to_string()
    } else {
        html_escape(settings.account_email.trim())
    };
    let display_line = if settings.display_name.trim().is_empty() {
        "-".to_string()
    } else {
        html_escape(settings.display_name.trim())
    };

    let body = format!(
        r#"<style>{css}</style>
      <div class="proton-layout">
      {notice}{error}
      <p class="muted">Proton Mail is an <strong>external</strong> mailbox (https://mail.proton.me). CPN does not run a Proton server and does not host Proton end-to-end encryption on this panel. Tachyon remains the default active webmail when installed. Use this page to open Proton Mail and to document Bridge ports for local clients.</p>
      <div class="proton-cards">
        <article class="proton-card">
          <h3>Open Proton Mail</h3>
          <p class="muted" style="margin:0;">Opens Proton in a new tab. Optional display account: <code>{account}</code> ({display}).</p>
          <div class="proton-actions">{open_btn}
            <a class="btn-secondary" href="/email/webmail">Open active webmail</a>
          </div>
        </article>
        <article class="proton-card">
          <h3>Proton Mail Bridge (guidance)</h3>
          <p class="muted" style="margin:0;">Install Bridge on a workstation (not required on this CPN server). Bridge exposes local IMAP/SMTP so desktop clients can sync a Proton mailbox. Defaults below match common Bridge installs; confirm in the Bridge app if yours differ. Never store Proton passwords in git.</p>
          <div class="proton-meta">
            <div><span>Bridge host</span><code>{bridge_host}</code></div>
            <div><span>IMAP port</span><code>{imap}</code></div>
            <div><span>SMTP port</span><code>{smtp}</code></div>
            <div><span>TLS</span><strong>STARTTLS / Bridge certificate (per Bridge docs)</strong></div>
          </div>
          <p class="muted" style="margin:0;">Bridge automation (detect/heal on the server) is not LIVE in v1.0.0.</p>
        </article>
      </div>
      <form method="post" action="/email/proton/save" class="stack-form">
        <h3 style="margin:8px 0 0;">Settings</h3>
        <label for="account_email">Proton account email (display only)</label>
        <input id="account_email" name="account_email" type="email" value="{account_val}" autocomplete="off">
        <label for="display_name">Display name (optional)</label>
        <input id="display_name" name="display_name" type="text" value="{display_val}" autocomplete="off">
        <label style="display:flex;align-items:center;gap:10px;font-weight:600;">
          <input type="checkbox" name="open_button_enabled" value="1"{open_enabled}> Enable Open Proton Mail button
        </label>
        <label for="open_button_label">Custom Open button label (optional)</label>
        <input id="open_button_label" name="open_button_label" type="text" value="{label_val}" placeholder="Open Proton Mail" autocomplete="off">
        <label for="open_url">Open URL</label>
        <input id="open_url" name="open_url" type="url" value="{open_url}" placeholder="https://mail.proton.me">
        <label for="bridge_host">Bridge host</label>
        <input id="bridge_host" name="bridge_host" type="text" value="{bridge_host_val}" autocomplete="off">
        <label for="bridge_imap_port">Bridge IMAP port</label>
        <input id="bridge_imap_port" name="bridge_imap_port" type="number" min="1" max="65535" value="{imap}">
        <label for="bridge_smtp_port">Bridge SMTP port</label>
        <input id="bridge_smtp_port" name="bridge_smtp_port" type="number" min="1" max="65535" value="{smtp}">
        <label for="notes">Operator notes (optional)</label>
        <textarea id="notes" name="notes" rows="3">{notes}</textarea>
        <div class="proton-actions">
          <button type="submit" class="btn-primary">Save settings</button>
        </div>
      </form>
      </div>"#,
        css = page_styles(),
        notice = flash("ok", notice),
        error = flash("error", error),
        account = account_line,
        display = display_line,
        open_btn = open_btn,
        bridge_host = html_escape(&settings.bridge_host),
        imap = settings.bridge_imap_port,
        smtp = settings.bridge_smtp_port,
        account_val = html_escape(&settings.account_email),
        display_val = html_escape(&settings.display_name),
        open_enabled = open_enabled,
        label_val = html_escape(&settings.open_button_label),
        open_url = html_escape(&settings.open_url),
        bridge_host_val = html_escape(&settings.bridge_host),
        notes = html_escape(&settings.notes),
    );

    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("Proton Mail", None),
        ],
        "Proton Mail",
        "External Proton Mail and Bridge guidance (not a self-hosted Proton stack).",
        &body,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn save_proton_mail_form(
    account_email: &str,
    display_name: &str,
    open_button_enabled: bool,
    open_button_label: &str,
    open_url: &str,
    bridge_host: &str,
    bridge_imap_port: &str,
    bridge_smtp_port: &str,
    notes: &str,
) -> Result<(), String> {
    let settings = ProtonMailSettings {
        account_email: account_email.trim().to_string(),
        display_name: display_name.trim().to_string(),
        open_button_enabled,
        open_button_label: open_button_label.trim().to_string(),
        open_url: sanitize_open_url(open_url),
        bridge_host: {
            let h = bridge_host.trim();
            if h.is_empty() {
                "127.0.0.1".into()
            } else {
                h.to_string()
            }
        },
        bridge_imap_port: parse_port(bridge_imap_port, 1143),
        bridge_smtp_port: parse_port(bridge_smtp_port, 1025),
        notes: notes.trim().to_string(),
    };
    save_proton_mail_settings(&settings)
}
