//! Sub-users management page HTML (owner ACL-style secondary logins).

use crate::panel_hubs::feature_shell;
use crate::panel_password_gen::{
    generated_password_notice_html, password_field_and_gen_html, password_gen_script,
    password_gen_styles,
};
use crate::panel_subusers::{SubUserPublic, grantable_nav_items, list_subusers_for_owner};
use crate::panel_subusers_csrf::subusers_csrf_token;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn caps_checkboxes(prefix: &str, selected: &[String]) -> String {
    let mut out = String::from(
        r#"<fieldset style="border:1px solid var(--border, #334);border-radius:8px;padding:12px;">
  <legend>Capabilities</legend>
  <p class="muted" style="margin:0 0 8px;">Sub-users manage the owner panel within these sections. Users, packages, and server tools stay owner-only.</p>
  <div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(160px,1fr));gap:8px;">"#,
    );
    for item in grantable_nav_items() {
        let checked = if selected.iter().any(|s| s == item.id) {
            " checked"
        } else {
            ""
        };
        out.push_str(&format!(
            r#"<label style="display:flex;gap:8px;align-items:center;"><input type="checkbox" name="{prefix}" value="{id}"{checked}> {label}</label>"#,
            prefix = html_escape(prefix),
            id = html_escape(item.id),
            checked = checked,
            label = html_escape(item.label),
        ));
    }
    out.push_str("</div></fieldset>");
    out
}

fn subuser_rows(owner: &str) -> String {
    let rows = match list_subusers_for_owner(owner) {
        Ok(list) if list.is_empty() => {
            return r#"<p class="empty-state">No sub-users yet. Create one below.</p>"#.into();
        }
        Ok(list) => list,
        Err(error) => {
            return format!(
                r#"<p class="panel-notice error" role="status">Could not list sub-users: {}</p>"#,
                html_escape(&error)
            );
        }
    };
    let csrf = subusers_csrf_token(owner);
    let mut body = String::from(
        r#"<div class="table-wrap"><table class="data-table">
<thead><tr><th scope="col">Username</th><th scope="col">Email</th><th scope="col">Capabilities</th><th scope="col">Status</th><th scope="col">Actions</th></tr></thead>
<tbody>"#,
    );
    for row in rows {
        body.push_str(&subuser_row_html(&csrf, &row));
    }
    body.push_str("</tbody></table></div>");
    body
}

fn subuser_row_html(csrf: &str, row: &SubUserPublic) -> String {
    let caps = if row.allowed_nav_ids.is_empty() {
        "none".into()
    } else {
        row.allowed_nav_ids.join(", ")
    };
    let status = if row.disabled { "disabled" } else { "active" };
    format!(
        r#"<tr>
  <td><code>{user}</code></td>
  <td>{email}</td>
  <td>{caps}</td>
  <td>{status}</td>
  <td>
    <details>
      <summary>Edit capabilities</summary>
      <form method="post" action="/account/users/sub-users/capabilities" class="stack-form" style="margin-top:8px;display:grid;gap:8px;max-width:420px;">
        <input type="hidden" name="csrf" value="{csrf}">
        <input type="hidden" name="username" value="{user}">
        {caps_form}
        <button type="submit" class="btn-secondary">Save capabilities</button>
      </form>
    </details>
    <form method="post" action="/account/users/sub-users/delete" class="inline-form" style="display:inline;margin-left:8px;"
          onsubmit="return confirm('Delete sub-user {user}?');">
      <input type="hidden" name="csrf" value="{csrf}">
      <input type="hidden" name="username" value="{user}">
      <button type="submit" class="btn-secondary">Delete</button>
    </form>
  </td>
</tr>"#,
        user = html_escape(&row.username),
        email = html_escape(&row.recovery_email),
        caps = html_escape(&caps),
        status = html_escape(status),
        csrf = html_escape(csrf),
        caps_form = caps_checkboxes("allowed_nav_ids", &row.allowed_nav_ids),
    )
}

pub fn users_subusers_page(
    owner: &str,
    notice: Option<&str>,
    error: Option<&str>,
    generated_password: Option<&str>,
) -> String {
    let csrf = subusers_csrf_token(owner);
    let policy = crate::account::default_password_policy();
    let mut body = String::new();
    body.push_str(
        r#"<p class="muted">Sub-users are secondary logins on <strong>your</strong> panel. They are not separate reseller child accounts. Grant section access (ACL-style); site tools for granted website capabilities use your ownership via site ACL.</p>"#,
    );
    if let Some(password) = generated_password {
        body.push_str(&generated_password_notice_html(password, true));
        body.push_str(&password_gen_styles());
        body.push_str(&password_gen_script());
    }
    body.push_str(&subuser_rows(owner));
    body.push_str(&format!(
        r#"
<h3 style="margin-top:24px;">Create sub-user</h3>
<form method="post" action="/account/users/sub-users/create" class="stack-form" style="max-width:560px;display:grid;gap:12px;">
  <input type="hidden" name="csrf" value="{csrf}">
  <label>Username<input name="username" type="text" required autocomplete="username" maxlength="128"></label>
  <label>Email<input name="recovery_email" type="email" required autocomplete="email" maxlength="254"></label>
  {pw}
  {caps}
  <button type="submit" class="btn-primary">Create sub-user</button>
</form>
{pw_styles}
{pw_script}"#,
        csrf = html_escape(&csrf),
        pw = password_field_and_gen_html("Password (leave blank to generate)", policy.min_length),
        caps = caps_checkboxes("allowed_nav_ids", &["websites".into()]),
        pw_styles = password_gen_styles(),
        pw_script = password_gen_script(),
    ));
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Users & Plans", Some("/account/users")),
            ("Sub-users", None),
        ],
        "Sub-users",
        "ACL-style secondary logins for your panel.",
        &body,
        notice,
        error,
    )
}
