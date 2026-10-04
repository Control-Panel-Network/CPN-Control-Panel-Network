//! Create / edit hosting package forms (owned `{username}_{customname}` naming).

use crate::account_mgmt::list_accounts;
use crate::packages::{
    DEFAULT_PACKAGE_ID, Package, is_unlimited, package_custom_name_for_edit,
    package_owner_from_name,
};
use crate::panel_storage_fmt::unlimited_html;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn limit_field(name: &str, label: &str, value: &str, hint: &str) -> String {
    let mark = if value.parse::<i64>().ok().is_some_and(is_unlimited) {
        format!(" {}", unlimited_html())
    } else {
        String::new()
    };
    format!(
        r#"<label>{label}{mark}
        <input name="{name}" type="number" min="-1" required value="{value}">
        <span class="muted" style="font-weight:500;">{hint}</span>
      </label>"#,
        label = html_escape(label),
        mark = mark,
        name = html_escape(name),
        value = html_escape(value),
        hint = html_escape(hint),
    )
}

pub(crate) fn package_form(
    action: &str,
    pkg: Option<&Package>,
    submit: &str,
    creator_username: &str,
) -> String {
    let is_default = pkg.is_some_and(|p| p.id == DEFAULT_PACKAGE_ID);
    let (
        id,
        name_value,
        disk,
        bw,
        domains,
        emails,
        dbs,
        ftp,
        mailing_lists,
        autoresponders,
        forwarders,
        email_filters,
        alias_domains,
        subdomains,
        fqdn,
        notes,
    ) = match pkg {
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
            p.mailing_lists.to_string(),
            p.autoresponders.to_string(),
            p.forwarders.to_string(),
            p.email_filters.to_string(),
            p.alias_domains.to_string(),
            p.subdomains.to_string(),
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
            "1000".into(),
            "1000".into(),
            "1000".into(),
            "1000".into(),
            "-1".into(),
            "20".into(),
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
    // Labels match Account Statistics meters (same package fields; form units stay MB for storage/bandwidth).
    let unlimited_hint = "-1 = unlimited; 0 = none allowed";
    let mb_hint = "Value in MB. -1 = unlimited; 0 = none allowed";
    format!(
        r#"<form method="post" action="{action}" class="stack-form" style="margin-top:12px;display:grid;gap:12px;max-width:520px;">
      {id_field}
      {owner_field}
      <label>{name_label}
        <input name="name"{name_attrs} value="{name}">
      </label>
      <fieldset class="cpn-check-fieldset" style="margin:0;padding:12px;border:1px solid var(--border,#d0d5dd);border-radius:8px;">
        <legend>Package limits</legend>
        <p class="muted" style="margin:0 0 10px;">Same meters as Account Statistics: Websites, Mailboxes, Databases, FTP accounts, Storage, Bandwidth, Alias domains, Sub-domains, mailing lists, autoresponders, forwarders, and email filters. Storage and Bandwidth are entered in MB. Saving this package updates dashboard meters for accounts assigned to it.</p>
        <div style="display:grid;gap:12px;">
          {websites_field}
          {mailboxes_field}
          {dbs_field}
          {ftp_field}
          {storage_field}
          {bw_field}
          {alias_field}
          {subdomains_field}
          {mailing_lists_field}
          {autoresponders_field}
          {forwarders_field}
          {email_filters_field}
        </div>
      </fieldset>
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
        websites_field = limit_field("domains", "Websites", &domains, unlimited_hint),
        mailboxes_field = limit_field("emails", "Mailboxes", &emails, unlimited_hint),
        dbs_field = limit_field("databases", "Databases", &dbs, unlimited_hint),
        ftp_field = limit_field(
            "ftp_accounts",
            "FTP accounts",
            &ftp,
            "Includes jailed SFTP. -1 = unlimited; 0 = none allowed",
        ),
        storage_field = limit_field("disk_mb", "Storage (MB)", &disk, mb_hint),
        bw_field = limit_field("bandwidth_mb", "Bandwidth (MB)", &bw, mb_hint),
        alias_field = limit_field(
            "alias_domains",
            "Alias domains",
            &alias_domains,
            unlimited_hint,
        ),
        subdomains_field = limit_field("subdomains", "Sub-domains", &subdomains, unlimited_hint),
        mailing_lists_field = limit_field(
            "mailing_lists",
            "Mailing lists",
            &mailing_lists,
            unlimited_hint
        ),
        autoresponders_field = limit_field(
            "autoresponders",
            "Autoresponders",
            &autoresponders,
            unlimited_hint,
        ),
        forwarders_field = limit_field("forwarders", "Forwarders", &forwarders, unlimited_hint),
        email_filters_field = limit_field(
            "email_filters",
            "Email filters",
            &email_filters,
            unlimited_hint
        ),
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
