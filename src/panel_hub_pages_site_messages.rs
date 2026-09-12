//! Settings UI: global suspend message and site-ready placeholder template.

use crate::panel_hubs::feature_shell;
use crate::panel_website_manage_ui::html_escape;
use crate::site_messages::{SiteMessageDefaults, load_defaults};

pub fn site_messages_settings_page(notice: Option<&str>, error: Option<&str>) -> String {
    let defaults = load_defaults();
    let ok = notice
        .map(|n| {
            format!(
                r#"<p class="notice ok">{}</p>"#,
                html_escape(n)
            )
        })
        .unwrap_or_default();
    let err = error
        .map(|e| {
            format!(
                r#"<p class="notice error">{}</p>"#,
                html_escape(e)
            )
        })
        .unwrap_or_default();
    let body = format!(
        r#"{ok}{err}
<p class="muted">These defaults apply panel-wide. When a CPN admin suspends a site, visitors see the global suspend message (not the site owner's custom text). Website owners can set their own suspend message on each site's Manage page; that copy is used only when they suspend the site themselves.</p>
<form method="post" action="/settings/site-messages" class="stack-form" style="margin-top:16px;max-width:48rem;">
  <label for="suspend_message_html">Default suspend message (plain text)</label>
  <textarea id="suspend_message_html" name="suspend_message_html" rows="4" style="width:100%;font:inherit;">{suspend}</textarea>
  <p class="muted">Shown for admin suspends and as the fallback when an owner has not set a custom message. HTML tags are stripped.</p>
  <label for="site_ready_html" style="margin-top:1rem;display:block;">Default site-ready template (<code>index.html</code> for new docroots)</label>
  <textarea id="site_ready_html" name="site_ready_html" rows="14" style="width:100%;font-family:ui-monospace,monospace;font-size:13px;">{ready}</textarea>
  <p class="muted">Used when CPN creates a new document root (and when an owner resets the placeholder). Existing sites keep their files until reset. Scripts and event handlers are rejected.</p>
  <button type="submit" class="btn-primary" style="margin-top:12px;">Save site messages</button>
</form>
<form method="post" action="/settings/site-messages/reset-builtins" style="margin-top:20px;" onsubmit="return confirm('Restore built-in CPN defaults for both fields?');">
  <button type="submit" class="btn-warn">Restore built-in defaults</button>
</form>"#,
        ok = ok,
        err = err,
        suspend = html_escape(&defaults.suspend_message_html),
        ready = html_escape(&defaults.site_ready_html),
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
    format!(
        r#"<div id="suspend-message" class="manage-section">
  <h3>Suspend message</h3>
  <p class="manage-muted">Shown when <strong>you</strong> suspend this site. If a CPN admin suspends it, visitors see the panel default instead. Leave empty to use the panel default.</p>
  <form method="post" action="/websites/suspend-message" class="stack-form">
    <input type="hidden" name="domain" value="{domain_q}">
    <label for="owner_suspend_message">Your suspend message (plain text)</label>
    <textarea id="owner_suspend_message" name="owner_suspend_message" rows="4" style="width:100%;max-width:40rem;font:inherit;">{msg}</textarea>
    <button type="submit" class="btn-primary" style="margin-top:8px;">Save suspend message</button>
  </form>
  <form method="post" action="/websites/reset-placeholder" style="margin-top:16px;" onsubmit="return confirm('Overwrite index.html in this document root with the current CPN site-ready template?');">
    <input type="hidden" name="domain" value="{domain_q}">
    <button type="submit" class="btn-warn">Reset placeholder index.html</button>
  </form>
</div>"#,
        domain_q = domain_q,
        msg = html_escape(owner_message),
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
        assert!(!html.to_lowercase().contains("cyberpanel"));
        assert!(!html.contains('\u{2014}'));
        assert!(!html.contains('\u{2013}'));
    }

    #[test]
    fn manage_form_has_owner_fields() {
        let html = site_suspend_message_form("demo.example", "Back soon");
        assert!(html.contains("owner_suspend_message"));
        assert!(html.contains("Back soon"));
        assert!(html.contains("reset-placeholder"));
    }
}
