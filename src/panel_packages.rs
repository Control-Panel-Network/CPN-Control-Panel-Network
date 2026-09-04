//! HTML for CPN Packages list / create / edit / assign.

use crate::account_mgmt::list_accounts;
use crate::packages::{
    Package, PackageUsage, accounts_assigned_to, format_limit_display, is_panel_admin,
    list_packages, package_for_account, usage_for_account,
};

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

fn limit_cell(limit: i64, unit: &str) -> String {
    if limit == crate::packages::UNLIMITED {
        r#"<span class="badge-ok">Unlimited</span>"#.into()
    } else {
        html_escape(&format_limit_display(limit, unit))
    }
}

fn fqdn_cell(enabled: bool) -> String {
    if enabled {
        r#"<span class="status-dot ok"></span> Enabled"#.into()
    } else {
        r#"<span class="status-dot off"></span> Disabled"#.into()
    }
}

fn package_rows(packages: &[Package]) -> String {
    if packages.is_empty() {
        return r#"<p class="empty-state">No packages yet.</p>"#.into();
    }
    let mut rows = String::from(
        r#"<div class="table-wrap"><table class="data-table">
      <thead><tr>
        <th>Package name</th><th>Disk space</th><th>Bandwidth</th><th>Domains</th>
        <th>Emails</th><th>Databases</th><th>FTP accounts</th><th>FQDN status</th><th>Actions</th>
      </tr></thead><tbody>"#,
    );
    for pkg in packages {
        let assigned = accounts_assigned_to(&pkg.id).unwrap_or_default();
        let assigned_note = if assigned.is_empty() {
            String::new()
        } else {
            format!(
                r#"<div class="muted" style="font-size:12px;margin-top:4px;">Assigned: {}</div>"#,
                html_escape(&assigned.join(", "))
            )
        };
        rows.push_str(&format!(
            r#"<tr>
          <td><strong>{name}</strong>{assigned_note}<div class="muted" style="font-size:12px;">{id}</div></td>
          <td>{disk}</td><td>{bw}</td><td>{domains}</td><td>{emails}</td>
          <td>{dbs}</td><td>{ftp}</td><td>{fqdn}</td>
          <td>
            <a href="/packages/edit?id={id}">Edit</a>
            &nbsp;|&nbsp;
            <form method="post" action="/packages/delete" class="inline-form" style="display:inline;" onsubmit="return confirm('Delete package {name}?');">
              <input type="hidden" name="id" value="{id}">
              <button type="submit" class="linkish" style="background:none;border:0;color:#d92d20;font-weight:600;cursor:pointer;padding:0;">Delete</button>
            </form>
          </td>
        </tr>"#,
            name = html_escape(&pkg.name),
            assigned_note = assigned_note,
            id = html_escape(&pkg.id),
            disk = limit_cell(pkg.disk_mb, "MB"),
            bw = limit_cell(pkg.bandwidth_mb, "MB"),
            domains = limit_cell(pkg.domains, ""),
            emails = limit_cell(pkg.emails, ""),
            dbs = limit_cell(pkg.databases, ""),
            ftp = limit_cell(pkg.ftp_accounts, ""),
            fqdn = fqdn_cell(pkg.fqdn_enabled),
        ));
    }
    rows.push_str("</tbody></table></div>");
    rows
}

fn usage_card(usage: &PackageUsage) -> String {
    format!(
        r#"<div class="panel-card" style="margin-bottom:18px;">
      <h2 style="margin:0 0 8px;font-size:18px;">Your package: {name}</h2>
      <p class="muted" style="margin:0 0 12px;">Limits apply to websites, mailboxes, databases, and FTP accounts you own.</p>
      <ul style="margin:0;padding-left:18px;line-height:1.7;">
        <li>Domains: {d_used} / {d_limit}</li>
        <li>Emails: {e_used} / {e_limit}</li>
        <li>Databases: {db_used} / {db_limit}</li>
        <li>FTP accounts: {f_used} / {f_limit}</li>
        <li>Disk: {disk_used} MB / {disk_limit}</li>
        <li>Bandwidth limit: {bw} (metering later)</li>
        <li>FQDN / subdomains: {fqdn}</li>
      </ul>
    </div>"#,
        name = html_escape(&usage.package_name),
        d_used = usage.domains_used,
        d_limit = html_escape(&format_limit_display(usage.domains_limit, "")),
        e_used = usage.emails_used,
        e_limit = html_escape(&format_limit_display(usage.emails_limit, "")),
        db_used = usage.databases_used,
        db_limit = html_escape(&format_limit_display(usage.databases_limit, "")),
        f_used = usage.ftp_used,
        f_limit = html_escape(&format_limit_display(usage.ftp_limit, "")),
        disk_used = usage.disk_mb_used,
        disk_limit = html_escape(&format_limit_display(usage.disk_mb_limit, "MB")),
        bw = html_escape(&format_limit_display(usage.bandwidth_mb_limit, "MB")),
        fqdn = if usage.fqdn_enabled {
            "Enabled"
        } else {
            "Disabled"
        },
    )
}

fn package_form(action: &str, pkg: Option<&Package>, submit: &str) -> String {
    let (id, name, disk, bw, domains, emails, dbs, ftp, fqdn, notes) = match pkg {
        Some(p) => (
            p.id.as_str(),
            p.name.as_str(),
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
            "",
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
    format!(
        r#"<form method="post" action="{action}" class="stack-form" style="margin-top:12px;display:grid;gap:12px;max-width:520px;">
      {id_field}
      <label>Package name
        <input name="name" required maxlength="128" value="{name}">
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
      <button type="submit" class="btn-primary">{submit}</button>
      <p class="muted"><a href="/packages">Back to packages</a></p>
    </form>"#,
        action = html_escape(action),
        id_field = id_field,
        name = html_escape(name),
        disk = html_escape(&disk),
        bw = html_escape(&bw),
        domains = html_escape(&domains),
        emails = html_escape(&emails),
        dbs = html_escape(&dbs),
        ftp = html_escape(&ftp),
        fqdn_checked = fqdn_checked,
        notes = html_escape(notes),
        submit = html_escape(submit),
    )
}

fn assign_form(packages: &[Package]) -> String {
    let accounts = list_accounts().unwrap_or_default();
    if accounts.is_empty() {
        return r#"<p class="muted">Create a panel account before assigning packages.</p>"#.into();
    }
    let mut account_opts = String::new();
    for account in &accounts {
        let pkg = package_for_account(&account.username)
            .map(|p| p.name)
            .unwrap_or_else(|_| "Default".into());
        account_opts.push_str(&format!(
            r#"<option value="{user}">{user} (current: {pkg})</option>"#,
            user = html_escape(&account.username),
            pkg = html_escape(&pkg),
        ));
    }
    let mut package_opts = String::new();
    for pkg in packages {
        package_opts.push_str(&format!(
            r#"<option value="{id}">{name}</option>"#,
            id = html_escape(&pkg.id),
            name = html_escape(&pkg.name),
        ));
    }
    format!(
        r#"<div class="panel-card" style="margin-top:22px;">
      <h2 style="margin:0 0 8px;font-size:18px;">Assign package</h2>
      <p class="muted">Packages apply per account owner. Site ACL stays domain-keyed.</p>
      <form method="post" action="/packages/assign" class="stack-form">
        <label>Account
          <select name="username" required>{account_opts}</select>
        </label>
        <label>Package
          <select name="package_id" required>{package_opts}</select>
        </label>
        <button type="submit" class="btn-primary">Assign</button>
      </form>
    </div>"#
    )
}

/// Admin list + create shortcut, or member usage view.
pub fn packages_main(username: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let _ = crate::packages::ensure_default_package();
    let heading = section_heading(
        "List Packages",
        "Manage hosting packages: edit resource limits or delete packages.",
    );
    let notices = format!(
        "{}{}",
        notice_block("ok", notice),
        notice_block("error", error)
    );
    if !is_panel_admin(username) {
        let usage = usage_for_account(username).ok();
        let card = usage.as_ref().map(usage_card).unwrap_or_else(|| {
            r#"<p class="panel-notice error">Could not load your package limits.</p>"#.into()
        });
        return format!(
            "{heading}{notices}<div class=\"panel-card\"><h2 style=\"margin:0 0 12px;font-size:18px;\">| Your limits</h2>{card}</div>"
        );
    }
    let packages = list_packages().unwrap_or_default();
    format!(
        r#"{heading}{notices}
    <div class="panel-card">
      <div class="panel-card-head">
        <h2 style="margin:0;font-size:18px;">Hosting Packages</h2>
        <a class="btn-primary" href="/packages/new">Create package</a>
      </div>
      {rows}
    </div>
    {assign}"#,
        heading = heading,
        notices = notices,
        rows = package_rows(&packages),
        assign = assign_form(&packages),
    )
}

pub fn packages_new_main(notice: Option<&str>, error: Option<&str>) -> String {
    format!(
        "{}{}{}{}",
        section_heading(
            "Create package",
            "Define resource limits for an account package."
        ),
        notice_block("ok", notice),
        notice_block("error", error),
        package_form("/packages/create", None, "Create package"),
    )
}

pub fn packages_edit_main(pkg: &Package, notice: Option<&str>, error: Option<&str>) -> String {
    format!(
        "{}{}{}{}",
        section_heading(
            &format!("Edit {}", pkg.name),
            "Update resource limits for this hosting package."
        ),
        notice_block("ok", notice),
        notice_block("error", error),
        package_form("/packages/update", Some(pkg), "Save changes"),
    )
}
