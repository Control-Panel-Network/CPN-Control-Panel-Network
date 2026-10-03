//! Create / edit hosting package forms (owned `{username}_{customname}` naming).

use crate::account_mgmt::list_accounts;
use crate::packages::{
    DEFAULT_PACKAGE_ID, Package, package_custom_name_for_edit, package_owner_from_name,
};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub(crate) fn package_form(
    action: &str,
    pkg: Option<&Package>,
    submit: &str,
    creator_username: &str,
) -> String {
    let is_default = pkg.is_some_and(|p| p.id == DEFAULT_PACKAGE_ID);
    let (id, name_value, disk, bw, domains, emails, dbs, ftp, fqdn, notes) = match pkg {
        Some(p) => (
            p.id.as_str(),
            if is_default {
                "Default".to_string()
            } else {
                package_custom_name_for_edit(p)
            },
            p.disk_mb.to_string(),
            p.bandwidth_mb.to_string(),
            p.domains.to_string(),
            p.emails.to_string(),
            p.databases.to_string(),
            p.ftp_accounts.to_string(),
            p.fqdn_enabled,
            p.notes.as_str(),
        ),
        None => (
            "",
            String::new(),
            "1000".into(),
            "1000".into(),
            "20".into(),
            "1000".into(),
            "1000".into(),
            "1000".into(),
            true,
            "",
        ),
    };
    let fqdn_checked = if fqdn { " checked" } else { "" };
    let id_field = if id.is_empty() {
        String::new()
    } else {
        format!(
            r#"<input type="hidden" name="id" value="{}">"#,
            html_escape(id)
        )
    };
    let owner_field = if pkg.is_none() {
        let accounts = list_accounts().unwrap_or_default();
        let mut opts = String::new();
        if accounts.is_empty() {
            opts.push_str(&format!(
                r#"<option value="{user}" selected>{user}</option>"#,
                user = html_escape(creator_username),
            ));
        } else {
            for account in &accounts {
                let selected = if account
                    .username
                    .trim()
                    .eq_ignore_ascii_case(creator_username.trim())
                {
                    " selected"
                } else {
                    ""
                };
                opts.push_str(&format!(
                    r#"<option value="{user}"{selected}>{user}</option>"#,
                    user = html_escape(&account.username),
                    selected = selected,
                ));
            }
        }
        format!(
            r#"<label>Package owner
        <select name="owner" required>{opts}</select>
      </label>
      <p class="muted" style="margin:0;">New packages are stored as <code>username_customname</code> (example: <code>{example}_test</code>). If you already type the owner prefix, it is not added twice.</p>"#,
            opts = opts,
            example = html_escape(creator_username),
        )
    } else if is_default {
        r#"<p class="muted" style="margin:0;">The Default package keeps the exact name <code>Default</code> (no username prefix).</p>"#.into()
    } else {
        let owner = pkg
            .and_then(|p| package_owner_from_name(&p.name))
            .unwrap_or_else(|| creator_username.to_string());
        format!(
            r#"<p class="muted" style="margin:0;">Owner prefix <code>{owner}_</code> stays locked. Edit the custom name only; full name is <code>{owner}_customname</code>.</p>"#,
            owner = html_escape(&owner),
        )
    };
    let name_attrs = if is_default {
        r#" readonly aria-readonly="true""#
    } else {
        r#" required maxlength="100""#
    };
    let name_label = if is_default {
        "Package name"
    } else if pkg.is_some() {
        "Custom name"
    } else {
        "Package name (custom part)"
    };
    format!(
        r#"<form method="post" action="{action}" class="stack-form" style="margin-top:12px;display:grid;gap:12px;max-width:520px;">
      {id_field}
      {owner_field}
      <label>{name_label}
        <input name="name"{name_attrs} value="{name}">
      </label>
      <label>Disk space (MB, -1 = unlimited)
        <input name="disk_mb" type="number" required value="{disk}">
      </label>
      <label>Bandwidth (MB, -1 = unlimited)
        <input name="bandwidth_mb" type="number" required value="{bw}">
      </label>
      <label>Domains (-1 = unlimited)
        <input name="domains" type="number" required value="{domains}">
      </label>
      <label>Emails (-1 = unlimited)
        <input name="emails" type="number" required value="{emails}">
      </label>
      <label>Databases (-1 = unlimited)
        <input name="databases" type="number" required value="{dbs}">
      </label>
      <label>FTP accounts (-1 = unlimited)
        <input name="ftp_accounts" type="number" required value="{ftp}">
      </label>
      <label style="display:flex;align-items:center;gap:8px;">
        <input type="checkbox" name="fqdn_enabled" value="1"{fqdn_checked}>
        Allow FQDN / subdomain creation
      </label>
      <label>Notes
        <textarea name="notes" rows="3">{notes}</textarea>
      </label>
      {sidebar}
      <button type="submit" class="btn-primary">{submit}</button>
      <p class="muted"><a href="/packages">Back to packages</a></p>
    </form>"#,
        action = html_escape(action),
        id_field = id_field,
        owner_field = owner_field,
        name_label = html_escape(name_label),
        name_attrs = name_attrs,
        name = html_escape(&name_value),
        disk = html_escape(&disk),
        bw = html_escape(&bw),
        domains = html_escape(&domains),
        emails = html_escape(&emails),
        dbs = html_escape(&dbs),
        ftp = html_escape(&ftp),
        fqdn_checked = fqdn_checked,
        notes = html_escape(notes),
        sidebar = package_sidebar_fields(pkg),
        submit = html_escape(submit),
    )
}

fn package_sidebar_fields(pkg: Option<&Package>) -> String {
    let selected = pkg
        .map(|p| p.sidebar_hidden_nav_ids.clone())
        .unwrap_or_default();
    let mut checks = String::new();
    for item in crate::sidebar_visibility::controllable_nav_items() {
        let checked = if selected.iter().any(|id| id == item.id) {
            " checked"
        } else {
            ""
        };
        checks.push_str(&format!(
            r#"<label class="cpn-check-item">
          <input type="checkbox" name="sidebar_hidden_nav_ids" value="{id}"{checked}>
          <span>Hide {label}</span>
        </label>"#,
            id = html_escape(item.id),
            checked = checked,
            label = html_escape(item.label),
        ));
    }
    format!(
        r#"<fieldset class="cpn-check-fieldset">
      <legend>Sidebar visibility (plan)</legend>
      <p class="muted" style="margin:0 0 10px;">Accounts on this package cannot see or open checked sections (403 on direct URL). Dashboard stays available. Owner/admin keeps full access unless separately restricted in ACL.</p>
      <div class="cpn-check-grid">{checks}</div>
    </fieldset>"#,
        checks = checks,
    )
}
