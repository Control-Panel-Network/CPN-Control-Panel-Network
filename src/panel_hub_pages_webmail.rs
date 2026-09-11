//! Email > Webmail HTML (open client, internal embed, path regen).

use crate::panel_hubs::{feature_shell, not_configured_body};
use crate::panel_webmail::{
    WebmailPanelConfig, load_webmail_config, regenerate_webmail_path, save_webmail_config,
    webmail_admin_path, webmail_health_hint, webmail_label, webmail_open_path, webmail_ready,
};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn email_webmail_page(notice: Option<&str>, error: Option<&str>) -> String {
    let notice_html = flash("ok", notice);
    let error_html = flash("error", error);
    let body = if webmail_ready() {
        let cfg = load_webmail_config();
        let label = webmail_label();
        let open = webmail_open_path().unwrap_or_else(|| format!("{}/", cfg.public_path));
        let admin = webmail_admin_path().unwrap_or_else(|| format!("{}/?admin", cfg.public_path));
        let embed_checked = if cfg.internal_embed { " checked" } else { "" };
        let iframe = if cfg.internal_embed {
            format!(
                r#"<div class="webmail-embed" style="margin-top:16px;">
          <p class="muted">Internal webmail (same origin). <a href="{open}" target="_blank" rel="noopener noreferrer">Open in new tab</a></p>
          <iframe title="{label}" src="{open}" style="width:100%;min-height:70vh;border:1px solid var(--border, #334);border-radius:8px;background:#111;"></iframe>
        </div>"#,
                open = html_escape(&open),
                label = html_escape(label),
            )
        } else {
            String::new()
        };
        format!(
            r#"{notice}{error}
        <p><strong>{label}</strong> is installed. Open the real webmail UI (not only the plugin dashboard).</p>
        <p style="display:flex;flex-wrap:wrap;gap:10px;margin:16px 0;">
          <a class="btn-primary" href="{open}" target="_blank" rel="noopener noreferrer">Open {label}</a>
          <a class="btn-secondary" href="{admin}" target="_blank" rel="noopener noreferrer">{label} Admin</a>
          <a class="btn-secondary" href="/email/webmail/app">Internal view</a>
        </p>
        <p class="muted">Public path: <code>{path}</code> (proxied through this panel to PHP-FPM on {health}). Mailbox data under <code>/var/lib/cpn-webmail</code> is preserved when you regenerate the path.</p>
        <form method="post" action="/email/webmail/settings" class="stack-form" style="max-width:520px;margin-top:16px;">
          <label for="auto_login_account">Auto-login account (email)</label>
          <input id="auto_login_account" name="auto_login_account" type="email" value="{auto}" autocomplete="off" placeholder="user@example.com">
          <p class="muted">Best-effort Email prefill on Open. True SSO into SnappyMail/Roundcube is not available without storing mailbox passwords; the password is still entered in webmail.</p>
          <label for="public_path">Webmail URL path</label>
          <input id="public_path" name="public_path" type="text" value="{path}" autocomplete="off">
          <label style="display:flex;align-items:center;gap:10px;font-weight:600;">
            <input type="checkbox" name="internal_embed" value="1"{embed}>
            Enable internal webmail embed on this page
          </label>
          <button type="submit" class="btn-primary">Save webmail settings</button>
        </form>
        <form method="post" action="/email/webmail/regenerate-path" style="margin-top:12px;" onsubmit="return confirm('Regenerate the public webmail path? Bookmarks to the old URL will stop working. Mail data is kept.');">
          <button type="submit" class="btn-secondary">Regenerate webmail URL path</button>
        </form>
        {iframe}"#,
            notice = notice_html,
            error = error_html,
            label = html_escape(label),
            open = html_escape(&open),
            admin = html_escape(&admin),
            path = html_escape(&cfg.public_path),
            health = html_escape(webmail_health_hint()),
            auto = html_escape(&cfg.auto_login_account),
            embed = embed_checked,
            iframe = iframe,
        )
    } else {
        format!(
            r#"{notice}{error}{}"#,
            not_configured_body(
                "Webmail client is not installed yet.",
                "Install SnappyMail or Roundcube from the installer mail stage, or add the SnappyMail/Roundcube plugin for a site. When installed, Open Webmail appears here."
            ),
            notice = notice_html,
            error = error_html,
        )
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("Webmail", None),
        ],
        "Webmail",
        "Open SnappyMail or Roundcube.",
        &body,
        None,
        None,
    )
}

pub fn email_webmail_app_page() -> String {
    if !webmail_ready() {
        return email_webmail_page(
            None,
            Some("Install SnappyMail or Roundcube before using internal webmail."),
        );
    }
    let cfg = load_webmail_config();
    if !cfg.internal_embed {
        return email_webmail_page(
            None,
            Some("Enable internal webmail embed under Email > Webmail settings first."),
        );
    }
    let open = webmail_open_path().unwrap_or_else(|| format!("{}/", cfg.public_path));
    let label = webmail_label();
    let body = format!(
        r#"<p style="display:flex;flex-wrap:wrap;gap:10px;">
        <a class="btn-primary" href="{open}" target="_blank" rel="noopener noreferrer">Open in new tab</a>
        <a class="btn-secondary" href="/email/webmail">Webmail settings</a>
      </p>
      <iframe title="{label}" src="{open}" style="width:100%;min-height:75vh;border:1px solid var(--border, #334);border-radius:8px;background:#111;"></iframe>"#,
        open = html_escape(&open),
        label = html_escape(label),
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("Webmail", Some("/email/webmail")),
            ("Internal", None),
        ],
        &format!("Internal {label}"),
        "Embedded webmail inside CPN.",
        &body,
        None,
        None,
    )
}

pub fn apply_webmail_settings_form(
    auto_login_account: &str,
    public_path: &str,
    internal_embed: bool,
) -> Result<String, String> {
    let mut cfg = load_webmail_config();
    cfg.auto_login_account = auto_login_account.trim().to_string();
    cfg.public_path = public_path.trim().to_string();
    cfg.internal_embed = internal_embed;
    save_webmail_config(&cfg)?;
    Ok("Webmail settings saved".into())
}

pub fn apply_regenerate_path() -> Result<String, String> {
    let cfg = regenerate_webmail_path()?;
    Ok(format!(
        "Webmail path moved to {}. Mail data was not deleted.",
        cfg.public_path
    ))
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

#[allow(dead_code)]
pub fn default_config_for_tests() -> WebmailPanelConfig {
    WebmailPanelConfig::default()
}
