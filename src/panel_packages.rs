//! HTML for CPN Packages list / create / edit / assign.

use crate::account_mgmt::list_accounts;
use crate::packages::{
    Package, PackageUsage, accounts_assigned_to, format_limit_display, is_panel_admin, is_unlimited,
    list_packages, package_custom_name_for_edit, package_for_account, usage_for_account,
};
use crate::panel_dashboard_activity_list::{activity_list_script, wrap_activity_table_sized};
use crate::panel_package_form::package_form;

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

fn limit_cell(viewer: &str, limit: i64, unit: &str) -> String {
    if is_unlimited(limit) {
        crate::panel_storage_fmt::unlimited_html().into()
    } else if unit == "MB" {
        html_escape(&crate::panel_storage_fmt::format_mb_limit_for_user(
            viewer, limit,
        ))
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

fn action_button(label: &str, style: &str) -> String {
    format!(
        r#"<button type="submit" class="linkish" style="background:none;border:0;{style}font-weight:600;cursor:pointer;padding:0;">{label}</button>"#,
        label = html_escape(label),
        style = style,
    )
}

fn package_rows(packages: &[Package], owner_username: &str) -> String {
    if packages.is_empty() {
        return r#"<p class="empty-state">No packages yet.</p>"#.into();
    }
    let mut rows = String::from(
        r#"<div class="table-wrap"><table class="data-table" id="packages-table">
      <thead><tr>
        <th style="width:2.5rem;"><input type="checkbox" id="pkg-select-all" aria-label="Select all packages"></th>
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
        let custom = package_custom_name_for_edit(pkg);
        let dup_default = format!("{custom}-Copy");
        rows.push_str(&format!(
            r#"<tr>
          <td data-label="Select">
            <input type="checkbox" class="pkg-row-check" value="{id}" aria-label="Select {name}">
          </td>
          <td data-label="Package name"><strong>{name}</strong>{assigned_note}<div class="muted" style="font-size:12px;">{id}</div></td>
          <td data-label="Disk space">{disk}</td><td data-label="Bandwidth">{bw}</td>
          <td data-label="Domains">{domains}</td><td data-label="Emails">{emails}</td>
          <td data-label="Databases">{dbs}</td><td data-label="FTP accounts">{ftp}</td>
          <td data-label="FQDN status">{fqdn}</td>
          <td data-label="Actions">
            <a href="/packages/edit?id={id}">Edit</a>
            &nbsp;|&nbsp;
            <form method="post" action="/packages/duplicate" class="inline-form" style="display:inline;" data-owner="{owner}" onsubmit="return cpnPkgDuplicate(this);">
              <input type="hidden" name="id" value="{id}">
              <input type="hidden" name="new_name" value="">
              <input type="hidden" data-default-name="{dup_default}">
              {dup_btn}
            </form>
            &nbsp;|&nbsp;
            <form method="post" action="/packages/delete" class="inline-form" style="display:inline;" onsubmit="return confirm('Delete package {name}?');">
              <input type="hidden" name="id" value="{id}">
              {del_btn}
            </form>
          </td>
        </tr>"#,
            name = html_escape(&pkg.name),
            assigned_note = assigned_note,
            id = html_escape(&pkg.id),
            owner = html_escape(owner_username),
            dup_default = html_escape(&dup_default),
            disk = limit_cell(owner_username, pkg.disk_mb, "MB"),
            bw = limit_cell(owner_username, pkg.bandwidth_mb, "MB"),
            domains = limit_cell(owner_username, pkg.domains, ""),
            emails = limit_cell(owner_username, pkg.emails, ""),
            dbs = limit_cell(owner_username, pkg.databases, ""),
            ftp = limit_cell(owner_username, pkg.ftp_accounts, ""),
            fqdn = fqdn_cell(pkg.fqdn_enabled),
            dup_btn = action_button("Duplicate", "color:inherit;"),
            del_btn = action_button("Delete", "color:#d92d20;"),
        ));
    }
    rows.push_str("</tbody></table></div>");
    format!(
        r#"{}<script>{}</script>"#,
        wrap_activity_table_sized("packages", "Filter package name or id", &rows, 10),
        activity_list_script(),
    )
}

fn bulk_toolbar() -> String {
    r#"<form id="packages-bulk-form" method="post" action="/packages/bulk" style="margin:0 0 14px;" onsubmit="return cpnPkgBulkPrepare(this);">
      <input type="hidden" name="package_ids" id="pkg-ids-joined" value="">
      <div style="display:flex;flex-wrap:wrap;gap:8px;align-items:center;margin-bottom:10px;">
        <span class="muted" id="pkg-selected-count" aria-live="polite">0 selected</span>
        <button type="submit" name="action" value="fqdn_enable" class="btn-secondary" onclick="return cpnPkgBulkConfirm(this);">Enable FQDN</button>
        <button type="submit" name="action" value="fqdn_disable" class="btn-secondary" onclick="return cpnPkgBulkConfirm(this);">Disable FQDN</button>
        <button type="submit" name="action" value="delete" class="btn-secondary" style="color:#d92d20;" onclick="return cpnPkgBulkConfirm(this);">Delete selected</button>
        <button type="button" class="btn-secondary" id="pkg-toggle-bulk-edit" aria-expanded="false" aria-controls="pkg-bulk-edit">Bulk edit fields</button>
      </div>
      <div id="pkg-bulk-edit" hidden style="display:none;grid-template-columns:repeat(auto-fit,minmax(140px,1fr));gap:10px;margin-bottom:8px;padding:12px;border:1px solid var(--border, #d0d5dd);border-radius:8px;">
        <p class="muted" style="grid-column:1/-1;margin:0;">Leave a field blank to keep each package value. Names are not changed. 0 or -1 = unlimited.</p>
        <label>Disk MB<input name="disk_mb" type="number" min="-1" placeholder="unchanged"></label>
        <label>Bandwidth MB<input name="bandwidth_mb" type="number" min="-1" placeholder="unchanged"></label>
        <label>Domains<input name="domains" type="number" min="-1" placeholder="unchanged"></label>
        <label>Emails<input name="emails" type="number" min="-1" placeholder="unchanged"></label>
        <label>Databases<input name="databases" type="number" min="-1" placeholder="unchanged"></label>
        <label>FTP accounts<input name="ftp_accounts" type="number" min="-1" placeholder="unchanged"></label>
        <label>FQDN
          <select name="fqdn_enabled">
            <option value="">Unchanged</option>
            <option value="1">Enable</option>
            <option value="0">Disable</option>
          </select>
        </label>
        <label style="grid-column:1/-1;">Notes
          <textarea name="notes" rows="2" placeholder="Optional shared notes"></textarea>
        </label>
        <label style="display:flex;align-items:center;gap:8px;">
          <input type="checkbox" name="apply_notes" value="1"> Apply notes to selection
        </label>
        <div style="grid-column:1/-1;">
          <button type="submit" name="action" value="update" class="btn-primary" onclick="return cpnPkgBulkConfirm(this);">Apply to selected</button>
        </div>
      </div>
    </form>
    <script>
    (function(){
      function checks(){return Array.prototype.slice.call(document.querySelectorAll('.pkg-row-check'));}
      function selectedIds(){return checks().filter(function(c){return c.checked;}).map(function(c){return c.value;});}
      function refresh(){
        var list=checks(), n=list.filter(function(c){return c.checked;}).length;
        var el=document.getElementById('pkg-selected-count');
        if(el) el.textContent=n+' selected';
        var all=document.getElementById('pkg-select-all');
        if(all){all.checked=list.length>0&&n===list.length; all.indeterminate=n>0&&n<list.length;}
      }
      var all=document.getElementById('pkg-select-all');
      if(all){all.addEventListener('change',function(){checks().forEach(function(c){c.checked=all.checked;});refresh();});}
      checks().forEach(function(c){c.addEventListener('change',refresh);});
      var toggle=document.getElementById('pkg-toggle-bulk-edit');
      var panel=document.getElementById('pkg-bulk-edit');
      if(toggle&&panel){
        toggle.addEventListener('click',function(){
          var open=panel.getAttribute('hidden')===null;
          if(open){panel.setAttribute('hidden','');panel.style.display='none';toggle.setAttribute('aria-expanded','false');}
          else{panel.removeAttribute('hidden');panel.style.display='grid';toggle.setAttribute('aria-expanded','true');}
        });
      }
      refresh();
      window.cpnPkgDuplicate=function(form){
        var owner=form.getAttribute('data-owner')||'';
        var hint=form.querySelector('[data-default-name]');
        var suggested=hint?hint.getAttribute('data-default-name'):'';
        var msg=owner
          ?('New package custom name (saved as '+owner+'_customname)')
          :'New package custom name';
        var name=window.prompt(msg, suggested||'');
        if(!name||!String(name).trim()) return false;
        form.querySelector('input[name="new_name"]').value=String(name).trim();
        return true;
      };
      window.cpnPkgBulkPrepare=function(form){
        var joined=document.getElementById('pkg-ids-joined');
        if(joined) joined.value=selectedIds().join(',');
        return true;
      };
      window.cpnPkgBulkConfirm=function(btn){
        var n=selectedIds().length;
        if(n<1){alert('Select at least one package.');return false;}
        var action=btn&&btn.value?btn.value:'';
        if(action==='delete') return confirm('Delete '+n+' selected package(s)? Assigned packages and Default stay blocked.');
        if(action==='update') return confirm('Apply bulk fields to '+n+' package(s)?');
        return true;
      };
    })();
    </script>"#
        .into()
}

/// Percent used plus an over-limit badge for the monthly bandwidth line.
fn bandwidth_note(usage: &PackageUsage) -> String {
    let limit = usage.bandwidth_mb_limit;
    if is_unlimited(limit) {
        return String::new();
    }
    if limit < 0 {
        return r#" <span class="badge-warn">Over limit</span>"#.into();
    }
    let pct = (usage.bandwidth_mb_used.saturating_mul(100)) / (limit as u64);
    if usage.bandwidth_mb_used >= limit as u64 {
        format!(
            r#" ({pct}%) <span class="badge-warn">Over limit: new websites are blocked until next month or a larger package</span>"#
        )
    } else {
        format!(" ({pct}%)")
    }
}

fn usage_card(viewer: &str, usage: &PackageUsage) -> String {
    format!(
        r#"<div class="panel-card" style="margin-bottom:18px;">
      <h2 style="margin:0 0 8px;font-size:18px;">Your package: {name}</h2>
      <p class="muted" style="margin:0 0 12px;">Limits apply to websites, mailboxes, databases, and FTP accounts you own.</p>
      <ul style="margin:0;padding-left:18px;line-height:1.7;">
        <li>Domains: {d}</li>
        <li>Emails: {e}</li>
        <li>Databases: {db}</li>
        <li>FTP accounts: {f}</li>
        <li>Disk: {disk}</li>
        <li>Bandwidth (this month): {bw}{bw_note}</li>
        <li>FQDN / subdomains: {fqdn}</li>
      </ul>
    </div>"#,
        name = html_escape(&usage.package_name),
        d = html_escape(&crate::panel_storage_fmt::format_used_count(
            usage.domains_used,
            usage.domains_limit,
        )),
        e = html_escape(&crate::panel_storage_fmt::format_used_count(
            usage.emails_used,
            usage.emails_limit,
        )),
        db = html_escape(&crate::panel_storage_fmt::format_used_count(
            usage.databases_used,
            usage.databases_limit,
        )),
        f = html_escape(&crate::panel_storage_fmt::format_used_count(
            usage.ftp_used,
            usage.ftp_limit,
        )),
        disk = html_escape(&crate::panel_storage_fmt::format_used_mb_limit_for_user(
            viewer,
            usage.disk_mb_used,
            usage.disk_mb_limit,
        )),
        bw = html_escape(&crate::panel_storage_fmt::format_used_mb_limit_for_user(
            viewer,
            usage.bandwidth_mb_used,
            usage.bandwidth_mb_limit,
        )),
        bw_note = bandwidth_note(usage),
        fqdn = if usage.fqdn_enabled {
            "Enabled"
        } else {
            "Disabled"
        },
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
        "Manage hosting packages: duplicate, multi-select, and bulk update limits.",
    );
    let notices = format!(
        "{}{}",
        notice_block("ok", notice),
        notice_block("error", error)
    );
    if !is_panel_admin(username) {
        let usage = usage_for_account(username).ok();
        let card = usage
            .as_ref()
            .map(|u| usage_card(username, u))
            .unwrap_or_else(|| {
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
      {toolbar}
      {rows}
    </div>
    {assign}"#,
        heading = heading,
        notices = notices,
        toolbar = bulk_toolbar(),
        rows = package_rows(&packages, username),
        assign = assign_form(&packages),
    )
}

pub fn packages_new_main(username: &str, notice: Option<&str>, error: Option<&str>) -> String {
    format!(
        "{}{}{}{}",
        section_heading(
            "Create package",
            "Define resource limits for an account package."
        ),
        notice_block("ok", notice),
        notice_block("error", error),
        package_form("/packages/create", None, "Create package", username),
    )
}

pub fn packages_edit_main(
    pkg: &Package,
    actor_username: &str,
    notice: Option<&str>,
    error: Option<&str>,
) -> String {
    format!(
        "{}{}{}{}",
        section_heading(
            &format!("Edit {}", pkg.name),
            "Update resource limits for this hosting package."
        ),
        notice_block("ok", notice),
        notice_block("error", error),
        package_form(
            "/packages/update",
            Some(pkg),
            "Save changes",
            actor_username,
        ),
    )
}
