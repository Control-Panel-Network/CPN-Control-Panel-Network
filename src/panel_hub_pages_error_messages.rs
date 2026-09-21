//! Settings UI: owner-editable 403 / 404 / 500 Markdown messages.

use crate::panel_error_messages::{
    BUILTIN_FORBIDDEN, BUILTIN_INTERNAL, BUILTIN_NOT_FOUND, load_messages,
};
use crate::panel_hubs::feature_shell;
use crate::panel_markdown::{markdown_editor_field, markdown_toolbar_assets};
use crate::panel_website_manage_ui::html_escape;
use crate::site_messages::{builtin_site_ready_html, builtin_suspend_message};

pub fn error_messages_settings_page(notice: Option<&str>, error: Option<&str>) -> String {
    let messages = load_messages();
    let ok = notice
        .map(|n| format!(r#"<p class="notice ok">{}</p>"#, html_escape(n)))
        .unwrap_or_default();
    let err = error
        .map(|e| format!(r#"<p class="notice error">{}</p>"#, html_escape(e)))
        .unwrap_or_default();
    let suspend_note = html_escape(builtin_suspend_message());
    let ready_note = html_escape(
        "Site-ready and suspend visitor copy live under Site messages (plain text / HTML templates).",
    );
    let body = format!(
        r#"{assets}
{ok}{err}
<p class="muted">CPN owner only. Each message supports Markdown (bold, italic, code, links, images, lists, headings, tables, horizontal rules). Scripts and unsafe URLs are stripped when rendered.</p>
<p class="muted">{ready_note} Current factory suspend preview: {suspend_note}</p>
<p class="muted"><a href="/settings/site-messages">Open Site messages</a> for suspend and site-ready templates.</p>
<form method="post" action="/settings/error-messages" class="stack-form" style="margin-top:16px;max-width:48rem;">
  {forbidden}
  {not_found}
  {internal}
  <button type="submit" class="btn-primary">Save error messages</button>
</form>
<div style="margin-top:16px;display:flex;flex-wrap:wrap;gap:8px;max-width:48rem;">
  <form method="post" action="/settings/error-messages/restore-forbidden"
        onsubmit="return confirm('Restore the built-in 403 message?');">
    <button type="submit" class="btn-warn">Restore 403 default</button>
  </form>
  <form method="post" action="/settings/error-messages/restore-not-found"
        onsubmit="return confirm('Restore the built-in 404 message?');">
    <button type="submit" class="btn-warn">Restore 404 default</button>
  </form>
  <form method="post" action="/settings/error-messages/restore-internal"
        onsubmit="return confirm('Restore the built-in 500 message?');">
    <button type="submit" class="btn-warn">Restore 500 default</button>
  </form>
  <form method="post" action="/settings/error-messages/restore-all"
        onsubmit="return confirm('Restore all built-in panel error messages?');">
    <button type="submit" class="btn-warn">Restore all defaults</button>
  </form>
</div>
<details style="margin-top:18px;max-width:48rem;">
  <summary class="muted">Built-in defaults (reference)</summary>
  <ul class="muted">
    <li>403: {d403}</li>
    <li>404: {d404}</li>
    <li>500: {d500}</li>
    <li>Site-ready uses an HTML template (see Site messages), not this page.</li>
  </ul>
</details>"#,
        assets = markdown_toolbar_assets(),
        ok = ok,
        err = err,
        ready_note = ready_note,
        suspend_note = suspend_note,
        forbidden = markdown_editor_field(
            "403 Forbidden (Markdown)",
            "forbidden_md",
            &messages.forbidden_md,
            "Describe what this access denial means for the user...",
        ),
        not_found = markdown_editor_field(
            "404 Not Found (Markdown)",
            "not_found_md",
            &messages.not_found_md,
            "Explain that the page was not found...",
        ),
        internal = markdown_editor_field(
            "500 Internal Error (Markdown)",
            "internal_md",
            &messages.internal_md,
            "Explain that something went wrong...",
        ),
        d403 = html_escape(BUILTIN_FORBIDDEN),
        d404 = html_escape(BUILTIN_NOT_FOUND),
        d500 = html_escape(BUILTIN_INTERNAL),
    );
    let _ = builtin_site_ready_html();
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Settings", Some("/settings")),
            ("Error messages", None),
        ],
        "Error messages",
        "Forbidden and panel error copy",
        &body,
        None,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_has_markdown_toolbar_and_no_cyberpanel() {
        let html = error_messages_settings_page(None, None);
        assert!(html.contains("403 Forbidden"));
        assert!(html.contains("data-md-action=\"bold\""));
        assert!(html.contains("Preview"));
        assert!(html.contains("/settings/site-messages"));
        assert!(!html.to_lowercase().contains("cyberpanel"));
        assert!(!html.contains('\u{2014}'));
        assert!(!html.contains('\u{2013}'));
    }
}
