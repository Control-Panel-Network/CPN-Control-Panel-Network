//! Email Accounts list UI: actions only (no embedded create form).
//! Create lives at `/email/create`. Soft-deleted restore is owner-only.

use crate::account::{default_password_policy, password_policy_hint_html};
use crate::mail_accounts::{MailSmtpMode, list_accounts_public};
use crate::mail_accounts_lifecycle::{list_restorable_mailboxes, restore_days_remaining};
use crate::panel_admin::is_panel_admin;
use crate::panel_webmail::{webmail_label, webmail_open_path, webmail_ready};
use crate::postfix_fallback::postfix_is_ready;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn section_heading(title: &str, blurb: &str) -> String {
    format!(
        r#"
      <div class="dashboard-heading">
        <div>
          <p class="eyebrow">CPN PANEL</p>
          <h1>{title}</h1>
          <p>{blurb}</p>
        </div>
      </div>"#,
        title = html_escape(title),
        blurb = html_escape(blurb),
    )
}

fn notice_block(kind: &str, message: Option<&str>) -> String {
    let Some(message) = message.filter(|value| !value.is_empty()) else {
        return String::new();
    };
    let class = if kind == "error" {
        "panel-notice error"
    } else {
        "panel-notice ok"
    };
    format!(
        r#"<p class="{class}" role="status">{msg}</p>"#,
        msg = html_escape(message)
    )
}

fn webmail_href_for(address: &str) -> Option<String> {
    let path = webmail_open_path()?;
    let base = path.trim_end_matches('/');
    let enc = urlencoding_form(address);
    Some(match crate::panel_webmail::detect_webmail_client() {
        Some(crate::model::MailSystem::Roundcube) => format!("{base}/?_user={enc}"),
        Some(crate::model::MailSystem::Snappymail | crate::model::MailSystem::Tachyon) => {
            format!("{base}/?Email={enc}")
        }
        _ => format!("{base}/"),
    })
}

fn urlencoding_form(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 3);
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn modify_modal_html() -> String {
    let policy = default_password_policy();
    let hint = password_policy_hint_html(&policy);
    let min_len = policy.min_length;
    format!(
        r#"<dialog id="mailbox-modify-modal" class="cpn-modal" style="border:1px solid #d0d5dd;border-radius:12px;padding:0;max-width:440px;width:calc(100% - 24px);">
  <form method="post" action="/email/accounts/modify" class="stack-form" style="padding:20px;margin:0;" id="mailbox-modify-form">
    <h2 style="margin:0 0 8px;font-size:1.15rem;">Modify mailbox</h2>
    <p class="muted" id="mailbox-modify-address" style="margin:0 0 12px;"></p>
    <input type="hidden" name="id" id="mailbox-modify-id" value="">
    <input type="hidden" name="return_to" value="/email/accounts">
    <label for="mailbox-modify-local">Email username (before @)</label>
    <input id="mailbox-modify-local" name="local_part" type="text" autocomplete="off" maxlength="64" pattern="[A-Za-z0-9._+-]+">
    <label for="mailbox-modify-password">New password (optional)</label>
    <input id="mailbox-modify-password" name="mailbox_password" type="password" autocomplete="new-password" minlength="{min_len}" maxlength="256">
    <label for="mailbox-modify-password2">Confirm new password</label>
    <input id="mailbox-modify-password2" name="mailbox_password_confirm" type="password" autocomplete="new-password" minlength="{min_len}" maxlength="256">
    <p class="muted" style="margin:8px 0 0;">{hint}</p>
    <div style="display:flex;gap:8px;justify-content:flex-end;margin-top:16px;">
      <button type="button" class="btn-secondary" id="mailbox-modify-cancel">Cancel</button>
      <button type="submit" class="btn-primary">Save changes</button>
    </div>
  </form>
</dialog>
<script>
(function () {{
  var modal = document.getElementById("mailbox-modify-modal");
  if (!modal) return;
  var idInput = document.getElementById("mailbox-modify-id");
  var localInput = document.getElementById("mailbox-modify-local");
  var addrLabel = document.getElementById("mailbox-modify-address");
  var cancel = document.getElementById("mailbox-modify-cancel");
  var form = document.getElementById("mailbox-modify-form");
  document.querySelectorAll("[data-mailbox-modify]").forEach(function (btn) {{
    btn.addEventListener("click", function () {{
      var id = btn.getAttribute("data-id") || "";
      var address = btn.getAttribute("data-address") || "";
      var local = address.indexOf("@") >= 0 ? address.split("@")[0] : "";
      if (idInput) idInput.value = id;
      if (localInput) localInput.value = local;
      if (addrLabel) addrLabel.textContent = address;
      if (modal.showModal) modal.showModal();
    }});
  }});
  if (cancel) cancel.addEventListener("click", function () {{ if (modal.close) modal.close(); }});
  if (form) form.addEventListener("submit", function (ev) {{
    var p1 = document.getElementById("mailbox-modify-password");
    var p2 = document.getElementById("mailbox-modify-password2");
    if (p1 && p2 && p1.value && p1.value !== p2.value) {{
      ev.preventDefault();
      alert("Password and confirmation do not match.");
    }}
  }});
}})();
</script>"#,
        min_len = min_len,
        hint = hint,
    )
}

fn mailbox_rows_html() -> String {
    let accounts = list_accounts_public();
    if accounts.is_empty() {
        return r#"<p class="muted">No mailboxes yet. Use <a href="/email/create">Create mailbox</a> to add one.</p>"#.into();
    }
    let webmail_ready = webmail_ready() || crate::panel_webmail::sogo_is_active_webmail();
    let mut rows = String::from(
        r#"<div class="table-wrap"><table class="data-table"><thead><tr>
<th align="left">Address</th><th align="left">SMTP</th><th align="left">Valid</th><th align="left">State</th><th align="left">Actions</th>
</tr></thead><tbody>"#,
    );
    for acct in &accounts {
        let valid = if acct.smtp_valid { "Valid" } else { "Invalid" };
        let err = acct
            .smtp_error
            .as_ref()
            .map(|e| format!(r#" <span class="muted">({})</span>"#, html_escape(e)))
            .unwrap_or_default();
        let mode = match acct.smtp_mode {
            MailSmtpMode::External => "external",
            MailSmtpMode::PostfixLocal => "postfix_local",
        };
        let open_webmail = if webmail_ready {
            if let Some(href) = webmail_href_for(&acct.address) {
                format!(
                    r#"<a class="btn-secondary" href="{href}" target="_blank" rel="noopener noreferrer">Open in webmail</a> "#,
                    href = html_escape(&href),
                )
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        let modify = format!(
            r#"<button type="button" class="btn-secondary" data-mailbox-modify data-id="{id}" data-address="{addr}">Modify</button> "#,
            id = html_escape(&acct.id),
            addr = html_escape(&acct.address),
        );
        let toggle = if acct.enabled {
            format!(
                r#"<form method="post" action="/email/accounts/disable" class="inline-form" style="display:inline;" onsubmit="return confirm('Disable mailbox {addr}? Outbound and login stay blocked until you enable it again.');"><input type="hidden" name="id" value="{id}"><input type="hidden" name="return_to" value="/email/accounts"><button type="submit" class="btn-secondary">Disable</button></form> "#,
                id = html_escape(&acct.id),
                addr = html_escape(&acct.address),
            )
        } else {
            format!(
                r#"<form method="post" action="/email/accounts/enable" class="inline-form" style="display:inline;" onsubmit="return confirm('Enable mailbox {addr}?');"><input type="hidden" name="id" value="{id}"><input type="hidden" name="return_to" value="/email/accounts"><button type="submit" class="btn-primary">Enable</button></form> "#,
                id = html_escape(&acct.id),
                addr = html_escape(&acct.address),
            )
        };
        let delete = format!(
            r#"<form method="post" action="/email/accounts/delete" class="inline-form" style="display:inline;" onsubmit="return confirm('Delete mailbox {addr}? The panel owner can restore it for 14 days.');"><input type="hidden" name="id" value="{id}"><input type="hidden" name="return_to" value="/email/accounts"><button type="submit" class="btn-secondary">Delete</button></form>"#,
            id = html_escape(&acct.id),
            addr = html_escape(&acct.address),
        );
        rows.push_str(&format!(
            r#"<tr><td>{addr}<br><span class="muted">{domain}</span></td><td>{mode}: {summary}</td><td><strong>{valid}</strong>{err}</td><td>{state}</td><td style="white-space:normal;">{open}{modify}{toggle}{delete}</td></tr>"#,
            addr = html_escape(&acct.address),
            domain = html_escape(if acct.domain.is_empty() {
                "-"
            } else {
                &acct.domain
            }),
            mode = mode,
            summary = html_escape(&acct.smtp_summary),
            valid = valid,
            err = err,
            state = if acct.enabled { "Enabled" } else { "Disabled" },
            open = open_webmail,
            modify = modify,
            toggle = toggle,
            delete = delete,
        ));
    }
    rows.push_str("</tbody></table></div>");
    rows
}

fn deleted_restore_section(username: &str) -> String {
    if !is_panel_admin(username) {
        return String::new();
    }
    let deleted = list_restorable_mailboxes();
    if deleted.is_empty() {
        return r#"<article class="section-card" style="margin-top:18px;">
        <h2>Deleted mailboxes</h2>
        <p class="muted">No soft-deleted mailboxes in the 14-day restore window.</p>
      </article>"#
            .into();
    }
    let mut rows = String::from(
        r#"<div class="table-wrap"><table class="data-table"><thead><tr>
<th align="left">Address</th><th align="left">Days left</th><th></th>
</tr></thead><tbody>"#,
    );
    for acct in &deleted {
        let days = restore_days_remaining(acct).unwrap_or(1);
        rows.push_str(&format!(
            r#"<tr><td>{addr}</td><td>{days}</td><td>
<form method="post" action="/email/accounts/restore" class="inline-form" style="display:inline;" onsubmit="return confirm('Restore mailbox {addr}?');">
<input type="hidden" name="id" value="{id}">
<input type="hidden" name="return_to" value="/email/accounts">
<button type="submit" class="btn-primary">Restore</button>
</form></td></tr>"#,
            addr = html_escape(&acct.address),
            days = days,
            id = html_escape(&acct.id),
        ));
    }
    rows.push_str("</tbody></table></div>");
    format!(
        r#"<article class="section-card" style="margin-top:18px;">
        <h2>Deleted mailboxes (owner restore)</h2>
        <p class="muted">Soft-deleted mailboxes can be restored for up to 14 days. After that they are purged from the panel registry.</p>
        {rows}
      </article>"#,
        rows = rows,
    )
}

/// Email Accounts page body (list + actions; create is only via `/email/create`).
pub fn email_accounts_main(
    username: &str,
    selected_mail: Option<crate::model::MailSystem>,
    mail_client_ready: bool,
    mail_backend_ready: bool,
    notice: Option<&str>,
    error: Option<&str>,
) -> String {
    use crate::http_helpers::smtp_status_public;
    use crate::install_webmail_runtime::webmail_health_url;

    let _ = crate::mail_accounts_lifecycle::purge_expired_deleted_mailboxes();

    let mail = selected_mail
        .map(|value| value.label())
        .or_else(|| crate::panel_webmail::detect_webmail_client().map(|m| m.label()))
        .unwrap_or("Not selected");
    let client_ready = if mail_client_ready || webmail_ready() {
        "Ready"
    } else {
        "Not installed"
    };
    let live_backend = crate::apps_email::mail_backend_ready_fast();
    let backend_ready = if mail_backend_ready || live_backend {
        "Ready"
    } else {
        "Not verified"
    };
    let smtp = smtp_status_public();
    let postfix_ready = postfix_is_ready();
    let smtp_line = if smtp.configured {
        format!(
            "{}:{} ({}) from {}",
            smtp.host.as_deref().unwrap_or("-"),
            smtp.port.unwrap_or(0),
            smtp.tls_mode.as_deref().unwrap_or("-"),
            smtp.from_address.as_deref().unwrap_or("-"),
        )
    } else if postfix_ready {
        "Using local Postfix fallback (127.0.0.1)".into()
    } else {
        "Outbound SMTP is not configured".into()
    };
    let mta_line = if postfix_ready {
        "Postfix running (default local MTA)"
    } else {
        "Postfix not detected"
    };
    let webmail = if webmail_ready()
        || crate::panel_webmail::sogo_is_active_webmail()
        || (mail_client_ready
            && matches!(
                selected_mail,
                Some(
                    crate::model::MailSystem::Snappymail
                        | crate::model::MailSystem::Tachyon
                        | crate::model::MailSystem::Roundcube
                )
            )) {
        let open = webmail_open_path().unwrap_or_else(|| webmail_health_url().to_string());
        let label = webmail_label();
        format!(
            r#"<p><a class="btn-primary" href="{open}" target="_blank" rel="noopener noreferrer">Open {label}</a>
        <a class="btn-secondary" href="/email/webmail">Webmail settings</a></p>
        <p class="muted">Panel path opens through CPN; backend health URL: <code>{health}</code></p>"#,
            open = html_escape(&open),
            label = html_escape(label),
            health = html_escape(&webmail_health_url()),
        )
    } else if matches!(selected_mail, Some(crate::model::MailSystem::Thunderbird)) {
        "<p class=\"muted\">Thunderbird is a desktop client only. No local webmail URL is provisioned.</p>"
            .into()
    } else {
        "<p class=\"muted\">Install Tachyon (default), SnappyMail, Roundcube, or SOGo to enable Open Webmail.</p>"
            .into()
    };

    let accounts = list_accounts_public();
    let mailbox_addresses: Vec<String> = accounts.iter().map(|a| a.address.clone()).collect();
    let client_panel =
        crate::panel_hub_pages_mail_client::mail_client_configs_for_mailboxes(&mailbox_addresses);

    format!(
        r#"{heading}
      {ok}
      {err}
      {stack}
      <article class="section-card">
        <h2>Mail stack</h2>
        <ul class="kv-list">
          <li><span>Selected client</span><strong>{mail}</strong></li>
          <li><span>Webmail client</span><strong>{client}</strong></li>
          <li><span>IMAP/SMTP backend</span><strong>{backend}</strong></li>
          <li><span>Local MTA</span><strong>{mta}</strong></li>
          <li><span>Outbound SMTP</span><strong>{smtp}</strong></li>
        </ul>
        <p class="muted">If no external SMTP was set during install, Postfix is the default local MTA.</p>
        {webmail}
      </article>
      {client_panel}
      <article class="section-card" style="margin-top:18px;">
        <h2>Mailboxes</h2>
        <p>Manage existing mailboxes here. Create new ones on the Create Email page only.</p>
        <p style="margin:12px 0;"><a class="btn-primary" href="/email/create">Create mailbox</a></p>
        {rows}
      </article>
      {deleted}
      {modal}"#,
        heading = section_heading(
            "Email Accounts",
            "Mail stack status, client settings, and mailbox actions.",
        ),
        ok = notice_block("ok", notice),
        err = notice_block("error", error),
        stack = crate::panel_sections::email_mail_stack_notice_html(),
        mail = html_escape(mail),
        client = client_ready,
        backend = backend_ready,
        mta = mta_line,
        smtp = html_escape(&smtp_line),
        webmail = webmail,
        client_panel = client_panel,
        rows = mailbox_rows_html(),
        deleted = deleted_restore_section(username),
        modal = modify_modal_html(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accounts_page_has_no_create_form() {
        let html = email_accounts_main("admin", None, false, false, None, None);
        assert!(html.contains("href=\"/email/create\""));
        assert!(!html.contains("action=\"/email/accounts/create\""));
        assert!(!html.contains("name=\"smtp_mode\""));
        assert!(!html.contains("External SMTP"));
        assert!(!html.contains("name=\"smtp_host\""));
        // Modify modal is always present; per-row Delete appears when mailboxes exist.
        assert!(html.contains("mailbox-modify-modal"));
        assert!(html.contains("/email/accounts/modify"));
        assert!(html.contains("/email/accounts/delete") || html.contains("No mailboxes yet"));
    }
}
