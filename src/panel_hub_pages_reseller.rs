//! Reseller Center LIVE page (hierarchy, pool quotas, branding).

use crate::account_mgmt::list_accounts;
use crate::packages::is_panel_admin;
use crate::panel_hub_pages_reseller_stats::reseller_stats_cards_html;
use crate::panel_hubs::feature_shell;
use crate::panel_reseller::{
    ResellerBranding, ResellerQuotas, ResellerRecord, account_is_reseller, child_usernames,
    format_quota_cell, get_reseller, list_resellers, pool_committed,
};
use crate::panel_reseller_csrf::reseller_csrf_token;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn notice_block(notice: Option<&str>, error: Option<&str>) -> String {
    let mut out = String::new();
    if let Some(msg) = notice.filter(|m| !m.trim().is_empty()) {
        out.push_str(&format!(
            r#"<p class="panel-notice ok" role="status">{}</p>"#,
            html_escape(msg)
        ));
    }
    if let Some(msg) = error.filter(|m| !m.trim().is_empty()) {
        out.push_str(&format!(
            r#"<p class="panel-notice error" role="status">{}</p>"#,
            html_escape(msg)
        ));
    }
    out
}

fn quota_inputs(prefix: &str, q: &ResellerQuotas) -> String {
    format!(
        r#"<div class="reseller-quota-grid" style="display:grid;grid-template-columns:repeat(auto-fit,minmax(140px,1fr));gap:10px;">
  <label>Websites<input name="{p}_websites" type="number" required value="{w}"></label>
  <label>Mailboxes<input name="{p}_mailboxes" type="number" required value="{m}"></label>
  <label>Databases<input name="{p}_databases" type="number" required value="{d}"></label>
  <label>FTP accounts<input name="{p}_ftp" type="number" required value="{f}"></label>
  <label>Storage (MB)<input name="{p}_storage" type="number" required value="{s}"></label>
  <label>Bandwidth (MB)<input name="{p}_bandwidth" type="number" required value="{b}"></label>
</div>
<p class="muted" style="margin:8px 0 0;">Limits: <code>-1</code> unlimited, <code>0</code> none, positive = cap. Child package caps must fit the pool.</p>"#,
        p = html_escape(prefix),
        w = q.websites,
        m = q.mailboxes,
        d = q.databases,
        f = q.ftp_accounts,
        s = q.storage_mb,
        b = q.bandwidth_mb,
    )
}

fn quota_table(q: &ResellerQuotas, committed_label: &str, committed: Option<&str>) -> String {
    let mut rows = String::new();
    let items = [
        ("Websites", format_quota_cell(q.websites, "")),
        ("Mailboxes", format_quota_cell(q.mailboxes, "")),
        ("Databases", format_quota_cell(q.databases, "")),
        ("FTP", format_quota_cell(q.ftp_accounts, "")),
        ("Storage", format_quota_cell(q.storage_mb, "MB")),
        ("Bandwidth", format_quota_cell(q.bandwidth_mb, "MB")),
    ];
    for (label, value) in items {
        rows.push_str(&format!(
            "<tr><th scope=\"row\">{}</th><td>{}</td></tr>",
            html_escape(label),
            html_escape(&value)
        ));
    }
    let committed_row = committed
        .map(|c| {
            format!(
                r#"<p class="muted" style="margin-top:8px;">{label}: {c}</p>"#,
                label = html_escape(committed_label),
                c = html_escape(c)
            )
        })
        .unwrap_or_default();
    format!(
        r#"<div class="table-wrap"><table class="data-table"><tbody>{rows}</tbody></table></div>{committed_row}"#
    )
}

fn branding_form(csrf: &str, username: &str, branding: &ResellerBranding, action: &str) -> String {
    format!(
        r##"<form method="post" action="{action}" class="stack-form" style="max-width:560px;display:grid;gap:12px;">
  <input type="hidden" name="csrf" value="{csrf}">
  <input type="hidden" name="username" value="{user}">
  <label>Display name<input name="display_name" type="text" maxlength="80" value="{name}" placeholder="Acme Hosting"></label>
  <label>Tagline<input name="tagline" type="text" maxlength="160" value="{tag}" placeholder="Reliable multi-tenant hosting"></label>
  <label>Logo URL<input name="logo_url" type="text" maxlength="512" value="{logo}" placeholder="https://example.com/logo.png"></label>
  <label>Primary color<input name="primary_color" type="text" maxlength="32" value="{color}" placeholder="#006CFA"></label>
  <button type="submit" class="btn-primary">Save branding</button>
</form>"##,
        action = html_escape(action),
        csrf = html_escape(csrf),
        user = html_escape(username),
        name = html_escape(&branding.display_name),
        tag = html_escape(&branding.tagline),
        logo = html_escape(&branding.logo_url),
        color = html_escape(&branding.primary_color),
    )
}

fn reseller_card(admin: bool, csrf: &str, row: &ResellerRecord) -> String {
    let children = child_usernames(&row.username).unwrap_or_default();
    let committed = pool_committed(&row.username).ok();
    let committed_txt = committed.as_ref().map(|c| {
        format!(
            "websites {}, mailboxes {}, DBs {}, FTP {}, storage {}, bandwidth {}",
            format_quota_cell(c.websites, ""),
            format_quota_cell(c.mailboxes, ""),
            format_quota_cell(c.databases, ""),
            format_quota_cell(c.ftp_accounts, ""),
            format_quota_cell(c.storage_mb, "MB"),
            format_quota_cell(c.bandwidth_mb, "MB"),
        )
    });
    let brand_preview = if row.branding.display_name.trim().is_empty() {
        String::new()
    } else {
        format!(
            r#"<p style="margin:8px 0;"><strong>{}</strong> <span class="muted">{}</span></p>"#,
            html_escape(&row.branding.display_name),
            html_escape(&row.branding.tagline)
        )
    };
    let children_list = if children.is_empty() {
        r#"<p class="muted">No child users yet.</p>"#.to_string()
    } else {
        let items: Vec<String> = children
            .iter()
            .map(|u| format!("<li><code>{}</code></li>", html_escape(u)))
            .collect();
        format!("<ul>{}</ul>", items.join(""))
    };
    let mut admin_tools = String::new();
    if admin {
        admin_tools.push_str(&format!(
            r#"<h4 style="margin:16px 0 8px;">Update pool quotas</h4>
<form method="post" action="/account/users/reseller/quotas" class="stack-form" style="display:grid;gap:12px;">
  <input type="hidden" name="csrf" value="{csrf}">
  <input type="hidden" name="username" value="{user}">
  {quota_inputs}
  <button type="submit" class="btn-primary">Save quotas</button>
</form>
<form method="post" action="/account/users/reseller/demote" class="stack-form" style="margin-top:12px;"
      onsubmit="return confirm('Demote this reseller?');">
  <input type="hidden" name="csrf" value="{csrf}">
  <input type="hidden" name="username" value="{user}">
  <button type="submit" class="btn-secondary">Demote reseller</button>
</form>"#,
            csrf = html_escape(csrf),
            user = html_escape(&row.username),
            quota_inputs = quota_inputs("q", &row.quotas),
        ));
    }
    format!(
        r#"<article class="section-card" style="margin-top:16px;">
  <h3 style="margin:0 0 8px;">Reseller <code>{user}</code></h3>
  {brand_preview}
  <h4 style="margin:12px 0 8px;">Pool quotas</h4>
  {quota_table}
  <h4 style="margin:16px 0 8px;">Child users ({n})</h4>
  {children_list}
  <h4 style="margin:16px 0 8px;">Branding</h4>
  {branding}
  {admin_tools}
</article>"#,
        user = html_escape(&row.username),
        brand_preview = brand_preview,
        quota_table = quota_table(
            &row.quotas,
            "Committed child package caps",
            committed_txt.as_deref()
        ),
        n = children.len(),
        children_list = children_list,
        branding = branding_form(
            csrf,
            &row.username,
            &row.branding,
            "/account/users/reseller/branding"
        ),
        admin_tools = admin_tools,
    )
}

/// LIVE Reseller Center body for the signed-in viewer.
pub fn users_reseller_page(viewer: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let admin = is_panel_admin(viewer);
    let csrf = reseller_csrf_token(viewer);
    let mut body = notice_block(notice, error);
    body.push_str(&reseller_stats_cards_html(viewer));
    body.push_str(
        r#"<p class="muted">Manage reseller hierarchy, multi-tenant pool quotas (websites, mailboxes, databases, FTP, storage, bandwidth), and per-reseller branding. Child users are ACL-jailed to their parent reseller.</p>"#,
    );

    if admin {
        let candidates: Vec<String> = list_accounts()
            .unwrap_or_default()
            .into_iter()
            .filter(|a| !is_panel_admin(&a.username) && !account_is_reseller(&a.username))
            .map(|a| a.username)
            .collect();
        let mut opts = String::from(r#"<option value="">Select account</option>"#);
        for u in &candidates {
            opts.push_str(&format!(
                r#"<option value="{v}">{v}</option>"#,
                v = html_escape(u)
            ));
        }
        let default_q = ResellerQuotas {
            websites: 10,
            mailboxes: 50,
            databases: 10,
            ftp_accounts: 10,
            storage_mb: 51200,
            bandwidth_mb: 512000,
        };
        body.push_str(&format!(
            r#"<article class="section-card">
  <h3 style="margin:0 0 8px;">Promote reseller</h3>
  <form method="post" action="/account/users/reseller/promote" class="stack-form" style="display:grid;gap:12px;">
    <input type="hidden" name="csrf" value="{csrf}">
    <label>Account<select name="username" required>{opts}</select></label>
    {quota_inputs}
    <button type="submit" class="btn-primary">Promote to reseller</button>
  </form>
</article>
<article class="section-card" style="margin-top:16px;">
  <h3 style="margin:0 0 8px;">Assign user under reseller</h3>
  <form method="post" action="/account/users/reseller/assign" class="stack-form" style="display:grid;gap:12px;max-width:560px;">
    <input type="hidden" name="csrf" value="{csrf}">
    <label>Child user<input name="child" type="text" required maxlength="128" autocomplete="username"></label>
    <label>Reseller<input name="reseller" type="text" required maxlength="128" autocomplete="username"></label>
    <button type="submit" class="btn-primary">Assign</button>
  </form>
  <form method="post" action="/account/users/reseller/unassign" class="stack-form" style="display:grid;gap:12px;max-width:560px;margin-top:16px;">
    <input type="hidden" name="csrf" value="{csrf}">
    <label>Unassign user<input name="child" type="text" required maxlength="128" autocomplete="username"></label>
    <button type="submit" class="btn-secondary">Unassign</button>
  </form>
</article>"#,
            csrf = html_escape(&csrf),
            opts = opts,
            quota_inputs = quota_inputs("q", &default_q),
        ));
    }

    body.push_str(&format!(
        r#"<article class="section-card" style="margin-top:16px;">
  <h3 style="margin:0 0 8px;">Create child user</h3>
  <p class="muted">Creates a panel account under your reseller (admin may pick the parent).</p>
  <form method="post" action="/account/users/reseller/create-user" class="stack-form" style="display:grid;gap:12px;max-width:560px;">
    <input type="hidden" name="csrf" value="{csrf}">
    {parent_field}
    <label>Username<input name="username" type="text" required maxlength="128" autocomplete="username"></label>
    <label>Email<input name="recovery_email" type="email" required maxlength="254" autocomplete="email"></label>
    <label>Password<input name="password" type="password" maxlength="256" autocomplete="new-password"></label>
    <label style="display:flex;align-items:center;gap:8px;"><input type="checkbox" name="generate" value="1"> Generate password</label>
    <button type="submit" class="btn-primary">Create child user</button>
  </form>
</article>"#,
        csrf = html_escape(&csrf),
        parent_field = if admin {
            r#"<label>Parent reseller<input name="reseller" type="text" required maxlength="128" placeholder="reseller username"></label>"#.to_string()
        } else {
            format!(
                r#"<input type="hidden" name="reseller" value="{}">"#,
                html_escape(viewer)
            )
        },
    ));

    let rows: Vec<ResellerRecord> = if admin {
        list_resellers().unwrap_or_default()
    } else {
        get_reseller(viewer).ok().flatten().into_iter().collect()
    };
    if rows.is_empty() {
        body.push_str(
            r#"<article class="section-card" style="margin-top:16px;"><p class="empty-state">No resellers configured yet.</p></article>"#,
        );
    } else {
        for row in &rows {
            body.push_str(&reseller_card(admin, &csrf, row));
        }
    }

    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Users & Plans", Some("/account/users")),
            ("Reseller Center", None),
        ],
        "Reseller Center",
        "Hierarchy, pool quotas, and branding",
        &body,
        None,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_is_live_not_scaffold() {
        let html = users_reseller_page("cpnowner", None, None);
        assert!(!html.contains("Not configured yet"));
        assert!(!html.contains("scaffolded honestly"));
        assert!(html.contains("Reseller Center"));
        assert!(html.contains("multi-tenant") || html.contains("Create child user"));
        assert!(html.contains("Total Users"));
        assert!(html.contains("Total Websites"));
        assert!(html.contains("Resellers"));
        assert!(!html.to_lowercase().contains("cyberpanel"));
    }
}
