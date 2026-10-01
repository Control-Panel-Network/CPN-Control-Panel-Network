//! Panel-admin gate helpers for account and ACL pages.
//!
//! Admin-only pages answer non-admins with an in-page 403 (no query string), and POST
//! handlers redirect with the short code `error=admin-only` instead of a sentence, so the
//! address bar never carries `%20` or `+`. Pages decode the code back into readable text.

use crate::panel_pages::panel_shell;
use actix_web::HttpResponse;

/// Short error code carried in `?error=` when a non-admin hits an admin-only action.
pub const ADMIN_ONLY_CODE: &str = "admin-only";

/// Account and ACL pages only the bootstrap panel admin may open.
const ADMIN_ONLY_HREFS: &[&str] = &[
    "/account/users/create",
    "/account/acl/create",
    "/account/acl/modify",
    "/account/acl/sidebar",
    // Host-wide logs can hold other accounts' activity (see panel_hub_pages_server_log_view).
    "/server/logs/panel",
    "/server/logs/email",
    "/server/logs/ftp",
    "/server/logs/modsec",
];

/// True when `href` is an admin-only account or ACL page (hidden from non-admin navigation).
pub fn is_admin_only_href(href: &str) -> bool {
    let path = href.split(['?', '#']).next().unwrap_or(href);
    ADMIN_ONLY_HREFS.contains(&path)
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Human text for a known notice/error code; unknown values are returned unchanged.
pub fn decode_notice_code(raw: &str) -> String {
    match raw.trim() {
        ADMIN_ONLY_CODE => {
            "Only the panel admin can manage users and ACL. Sign in with the owner account (the first account created at install) to use this page.".into()
        }
        other => other.to_string(),
    }
}

/// In-page 403 for admin-only pages, naming the signed-in account.
pub fn admin_only_html(viewer: &str, title: &str) -> HttpResponse {
    let who: String = viewer
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '@'))
        .collect();
    let body = format!(
        r#"<div class="dashboard-heading"><div><p class="eyebrow">CPN PANEL</p><h1>{title}</h1></div></div>
<article class="section-card">
  <p class="panel-notice error" role="status">{message}</p>
  <p class="muted">You are signed in as <strong>{who}</strong>. Sign out and sign in with the owner account to manage users and ACL.</p>
  <p><a class="btn-primary" href="/account/users">Back to Users &amp; Plans</a> <a class="btn-secondary" href="/dashboard">Dashboard</a></p>
</article>"#,
        title = escape(title),
        message = escape(&decode_notice_code(ADMIN_ONLY_CODE)),
        who = escape(&who),
    );
    HttpResponse::Forbidden()
        .content_type("text/html; charset=utf-8")
        .body(panel_shell(viewer, "users", title, &body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_only_hrefs_are_detected() {
        assert!(is_admin_only_href("/account/acl/modify"));
        assert!(is_admin_only_href("/account/users/create"));
        assert!(!is_admin_only_href("/account/users/list"));
        assert!(!is_admin_only_href("/account/users/profile"));
    }

    #[test]
    fn code_decodes_without_spaces_in_code() {
        assert!(!ADMIN_ONLY_CODE.contains(' '));
        assert!(decode_notice_code("admin-only").starts_with("Only the panel admin"));
        assert_eq!(decode_notice_code("Password updated"), "Password updated");
    }
}
