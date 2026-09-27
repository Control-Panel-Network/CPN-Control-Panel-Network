//! Settings UI: global suspend message and site-ready placeholder template.

use crate::panel_hubs::feature_shell;
use crate::panel_markdown::{
    html_template_editor_field, markdown_editor_field, markdown_toolbar_assets,
};
use crate::panel_website_manage_ui::html_escape;
use crate::site_messages::{SiteMessageDefaults, load_defaults};

const SITE_READY_PREVIEW_PATH: &str = "/settings/site-messages/preview-site-ready";

fn confirm_if_nonempty(field_id: &str, message: &str) -> String {
    format!(
        "var el=document.getElementById('{id}'); if(el && el.value.trim()) return confirm('{msg}'); return true;",
        id = field_id,
        msg = message.replace('\'', "\\'"),
    )
}

pub fn site_messages_settings_page(notice: Option<&str>, error: Option<&str>) -> String {
    let defaults = load_defaults();
    let ok = notice
        .map(|n| format!(r#"<p class="notice ok">{}</p>"#, html_escape(n)))
        .unwrap_or_default();
    let err = error
        .map(|e| format!(r#"<p class="notice error">{}</p>"#, html_escape(e)))
        .unwrap_or_default();
    let confirm_suspend = confirm_if_nonempty(
        "suspend_message_html",
        "Replace the current default suspend message with the built-in CPN factory text?",
    );
    let confirm_ready = confirm_if_nonempty(
        "site_ready_html",
        "Replace the current site-ready template with the built-in CPN factory HTML?",
    );
    let confirm_both = "var s=document.getElementById('suspend_message_html'); var r=document.getElementById('site_ready_html'); if((s&&s.value.trim())||(r&&r.value.trim())) return confirm('Restore built-in CPN factory defaults for both fields?'); return true;".to_string();
    let suspend_field = markdown_editor_field(
        "Default suspend message (Markdown)",
        "suspend_message_html",
        &defaults.suspend_message_html,
        "Explain why the site is unavailable...",
    );
    let ready_field = html_template_editor_field(
        "Default site-ready template (index.html for new docroots)",
        "site_ready_html",
        &defaults.site_ready_html,
        "<!DOCTYPE html>...",
        SITE_READY_PREVIEW_PATH,
    );
    let body = format!(
        r#"{assets}
{ok}{err}
<p class="muted">These defaults apply panel-wide. When a CPN admin suspends a site, visitors see the global suspend message (not the site owner's custom text). Website owners can set their own suspend message on each site's Manage page; that copy is used only when they suspend the site themselves.</p>
<p class="muted">Suspend copy uses Markdown (toolbar, Preview, and View HTML source). Rendered output is sanitized for visitors. Site-ready templates stay full HTML documents with Preview and View HTML source.</p>
<form method="post" action="/settings/site-messages" class="stack-form" style="margin-top:16px;max-width:48rem;">
  {suspend_field}
  {ready_field}
  <button type="submit" class="btn-primary" style="margin-top:12px;">Save site messages</button>
</form>
<div style="margin-top:16px;display:flex;flex-wrap:wrap;gap:8px;max-width:48rem;">
  <form method="post" action="/settings/site-messages/restore-suspend" onsubmit="{confirm_suspend}">
    <button type="submit" class="btn-warn">Restore factory default (suspend)</button>
  </form>
  <form method="post" action="/settings/site-messages/restore-site-ready" onsubmit="{confirm_ready}">
    <button type="submit" class="btn-warn">Restore factory default (site-ready)</button>
  </form>
  <form method="post" action="/settings/site-messages/reset-builtins" onsubmit="{confirm_both}">
    <button type="submit" class="btn-warn">Restore all built-in defaults</button>
  </form>
</div>
<p class="muted" style="max-width:48rem;margin-top:12px;"><a href="/settings/error-messages">Error messages</a> uses the same Markdown editor for panel 403, 404, and 500 pages.</p>"#,
        assets = markdown_toolbar_assets(),
        ok = ok,
        err = err,
        suspend_field = suspend_field,
        ready_field = ready_field,
        confirm_suspend = confirm_suspend,
        confirm_ready = confirm_ready,
        confirm_both = confirm_both,
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Settings", Some("/settings")),
            ("Site messages", None),
        ],
        "Site messages",
        "Suspend copy and new-site placeholder",
        &body,
        None,
        None,
    )
}

/// Manage-page block: owner suspend message + optional placeholder reset.
pub fn site_suspend_message_form(domain: &str, owner_message: &str) -> String {
    let domain_q = html_escape(domain);
    let confirm_restore = confirm_if_nonempty(
        "owner_suspend_message",
        "Clear your custom suspend message and use the CPN panel default instead?",
    );
    let editor = markdown_editor_field(
        "Your suspend message (Markdown)",
        "owner_suspend_message",
        owner_message,
        "Optional message for visitors when you suspend this site...",
    );
    format!(
        r#"<div id="suspend-message" class="manage-section">
  {assets}
  <h3>Suspend message</h3>
  <p class="manage-muted">Shown when <strong>you</strong> suspend this site. If a CPN admin suspends it, visitors see the panel default instead. Leave empty to use the panel default.</p>
  <form method="post" action="/websites/suspend-message" class="stack-form">
    <input type="hidden" name="domain" value="{domain_q}">
    {editor}
    <div style="margin-top:8px;display:flex;flex-wrap:wrap;gap:8px;">
      <button type="submit" class="btn-primary">Save suspend message</button>
    </div>
  </form>
  <form method="post" action="/websites/suspend-message/restore" style="margin-top:8px;" onsubmit="{confirm_restore}">
    <input type="hidden" name="domain" value="{domain_q}">
    <button type="submit" class="btn-warn">Restore default</button>
  </form>
  <form method="post" action="/websites/reset-placeholder" style="margin-top:16px;" onsubmit="return confirm('Overwrite index.html in this document root with the current CPN site-ready template?');">
    <input type="hidden" name="domain" value="{domain_q}">
    <button type="submit" class="btn-warn">Reset placeholder index.html</button>
  </form>
</div>"#,
        assets = markdown_toolbar_assets(),
        domain_q = domain_q,
        editor = editor,
        confirm_restore = confirm_restore,
    )
}

#[allow(dead_code)]
pub fn defaults_for_tests() -> SiteMessageDefaults {
    load_defaults()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_page_mentions_hierarchy() {
        let html = site_messages_settings_page(None, None);
        assert!(html.contains("Site messages"));
        assert!(html.contains("suspend_message_html"));
        assert!(html.contains("site_ready_html"));
        assert!(html.contains("admin suspends"));
        assert!(html.contains("Restore factory default (suspend)"));
        assert!(html.contains("restore-suspend"));
        assert!(html.contains("restore-site-ready"));
        assert!(html.contains("data-md-action=\"preview\""));
        assert!(html.contains("View HTML source"));
        assert!(html.contains("data-cpn-html-template"));
        assert!(!html.to_lowercase().contains("cyberpanel"));
        assert!(!html.contains('\u{2014}'));
        assert!(!html.contains('\u{2013}'));
    }

    #[test]
    fn manage_form_has_owner_fields_and_restore() {
        let html = site_suspend_message_form("demo.example", "Back soon");
        assert!(html.contains("owner_suspend_message"));
        assert!(html.contains("Back soon"));
        assert!(html.contains("reset-placeholder"));
        assert!(html.contains("suspend-message/restore"));
        assert!(html.contains("Restore default"));
        assert!(html.contains("confirm("));
        assert!(html.contains("View HTML source"));
    }
}
