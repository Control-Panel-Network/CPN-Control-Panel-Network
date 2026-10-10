//! Reseller Center Users tab: searchable, filterable, paginated hierarchy list.

use crate::account_lifecycle::status_label;
use crate::account_mgmt::list_accounts;
use crate::model::AccountPublic;
use crate::panel_list_search::{list_filter_summary, normalize_list_q};
use crate::panel_reseller::{account_is_reseller, can_manage_account, list_resellers};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn query_escape(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => out.push(ch),
            ' ' => out.push('+'),
            _ => {
                for b in ch.encode_utf8(&mut [0; 4]).bytes() {
                    out.push_str(&format!("%{b:02X}"));
                }
            }
        }
    }
    out
}

#[derive(Debug, Clone)]
pub struct ResellerUsersOpts {
    pub q: String,
    pub role: String,
    pub parent: String,
    pub status: String,
    pub page: usize,
    pub per_page: usize,
}

impl Default for ResellerUsersOpts {
    fn default() -> Self {
        Self {
            q: String::new(),
            role: "all".into(),
            parent: String::new(),
            status: "all".into(),
            page: 1,
            per_page: 5,
        }
    }
}

pub fn reseller_users_role_from_query(raw: &str) -> &'static str {
    match raw.trim().to_ascii_lowercase().as_str() {
        "reseller" | "resellers" => "reseller",
        "child" | "children" | "user" => "child",
        _ => "all",
    }
}

pub fn reseller_users_status_from_query(raw: &str) -> &'static str {
    match raw.trim().to_ascii_lowercase().as_str() {
        "active" | "enabled" => "active",
        "disabled" | "inactive" => "disabled",
        _ => "all",
    }
}

pub fn reseller_users_per_page_from_query(raw: &str) -> usize {
    match raw.trim().parse::<usize>().unwrap_or(5) {
        n @ (5 | 10 | 25 | 50) => n,
        _ => 5,
    }
}

pub fn reseller_users_page_from_query(raw: &str) -> usize {
    raw.trim().parse::<usize>().unwrap_or(1).max(1)
}

pub fn reseller_users_opts(
    q: Option<&str>,
    role: Option<&str>,
    parent: Option<&str>,
    status: Option<&str>,
    page: Option<&str>,
    per_page: Option<&str>,
) -> ResellerUsersOpts {
    ResellerUsersOpts {
        q: normalize_list_q(q),
        role: reseller_users_role_from_query(role.unwrap_or("")).to_string(),
        parent: parent.unwrap_or("").trim().to_string(),
        status: reseller_users_status_from_query(status.unwrap_or("")).to_string(),
        page: reseller_users_page_from_query(page.unwrap_or("1")),
        per_page: reseller_users_per_page_from_query(per_page.unwrap_or("5")),
    }
}

fn is_child(acct: &AccountPublic) -> bool {
    !acct.parent_reseller.trim().is_empty()
}

fn is_reseller_row(acct: &AccountPublic) -> bool {
    account_is_reseller(&acct.username)
}

fn hierarchy_role(acct: &AccountPublic) -> &'static str {
    if is_reseller_row(acct) {
        "Reseller"
    } else if is_child(acct) {
        "Child"
    } else {
        "User"
    }
}

fn truncate_email(email: &str, max: usize) -> String {
    let chars: Vec<char> = email.chars().collect();
    if chars.len() <= max {
        return email.to_string();
    }
    let keep = max.saturating_sub(1).max(1);
    format!("{}…", chars.into_iter().take(keep).collect::<String>())
}

fn scoped_accounts(viewer: &str, admin: bool) -> Vec<AccountPublic> {
    let all = list_accounts().unwrap_or_default();
    if admin {
        all.into_iter()
            .filter(|a| is_reseller_row(a) || is_child(a))
            .collect()
    } else {
        all.into_iter()
            .filter(|a| {
                a.username.eq_ignore_ascii_case(viewer)
                    || a.parent_reseller.eq_ignore_ascii_case(viewer)
            })
            .collect()
    }
}

fn matches_filters(acct: &AccountPublic, opts: &ResellerUsersOpts, admin: bool) -> bool {
    match opts.role.as_str() {
        "reseller" if !is_reseller_row(acct) => return false,
        "child" if !is_child(acct) => return false,
        _ => {}
    }
    match opts.status.as_str() {
        "active" if acct.disabled => return false,
        "disabled" if !acct.disabled => return false,
        _ => {}
    }
    if !opts.parent.trim().is_empty() {
        let parent = opts.parent.trim();
        let parent_match = if is_reseller_row(acct) {
            acct.username.eq_ignore_ascii_case(parent)
        } else {
            acct.parent_reseller.eq_ignore_ascii_case(parent)
        };
        if !parent_match {
            return false;
        }
    }
    if opts.q.is_empty() {
        return true;
    }
    let q = &opts.q;
    let role = hierarchy_role(acct).to_ascii_lowercase();
    let status = status_label(acct.disabled).to_ascii_lowercase();
    let parent = acct.parent_reseller.to_ascii_lowercase();
    acct.username.to_ascii_lowercase().contains(q)
        || acct.recovery_email.to_ascii_lowercase().contains(q)
        || role.contains(q.as_str())
        || status.contains(q.as_str())
        || (admin && parent.contains(q.as_str()))
}

fn list_href(opts: &ResellerUsersOpts, page: usize) -> String {
    format!(
        "/account/users/reseller?tab=users&q={q}&role={role}&parent={parent}&status={status}&page={page}&per_page={pp}",
        q = query_escape(&opts.q),
        role = query_escape(&opts.role),
        parent = query_escape(&opts.parent),
        status = query_escape(&opts.status),
        page = page,
        pp = opts.per_page,
    )
}

fn pager_html(
    opts: &ResellerUsersOpts,
    page: usize,
    total_pages: usize,
    filtered: usize,
) -> String {
    let prev = if page > 1 {
        format!(
            r#"<a class="btn-secondary" href="{}">Prev</a>"#,
            html_escape(&list_href(opts, page - 1))
        )
    } else {
        r#"<button type="button" class="btn-secondary" disabled>Prev</button>"#.into()
    };
    let next = if page < total_pages {
        format!(
            r#"<a class="btn-secondary" href="{}">Next</a>"#,
            html_escape(&list_href(opts, page + 1))
        )
    } else {
        r#"<button type="button" class="btn-secondary" disabled>Next</button>"#.into()
    };
    let mut size_opts = String::new();
    for n in [5usize, 10, 25, 50] {
        let sel = if n == opts.per_page { " selected" } else { "" };
        size_opts.push_str(&format!(r#"<option value="{n}"{sel}>{n}</option>"#));
    }
    format!(
        r#"<div class="activity-list-pager reseller-users-pager" style="display:flex;flex-wrap:wrap;gap:8px;align-items:center;margin:12px 0;">
  <form method="get" action="/account/users/reseller" style="display:inline-flex;flex-wrap:wrap;gap:8px;align-items:center;">
    <input type="hidden" name="tab" value="users">
    <input type="hidden" name="q" value="{q}">
    <input type="hidden" name="role" value="{role}">
    <input type="hidden" name="parent" value="{parent}">
    <input type="hidden" name="status" value="{status}">
    <input type="hidden" name="page" value="1">
    <label>Show
      <select name="per_page" onchange="this.form.submit()">{size_opts}</select>
      per page
    </label>
  </form>
  <span class="activity-list-status">Page {page} / {pages}</span>
  <span class="muted">{filtered} matching</span>
  {prev}{next}
  <form method="get" action="/account/users/reseller" class="activity-list-goto">
    <input type="hidden" name="tab" value="users">
    <input type="hidden" name="q" value="{q}">
    <input type="hidden" name="role" value="{role}">
    <input type="hidden" name="parent" value="{parent}">
    <input type="hidden" name="status" value="{status}">
    <input type="hidden" name="per_page" value="{pp}">
    <label>Go to page
      <input name="page" type="number" min="1" max="{pages}" value="{page}" inputmode="numeric">
    </label>
    <button type="submit" class="btn-primary">Go</button>
  </form>
</div>"#,
        q = html_escape(&opts.q),
        role = html_escape(&opts.role),
        parent = html_escape(&opts.parent),
        status = html_escape(&opts.status),
        size_opts = size_opts,
        page = page,
        pages = total_pages,
        filtered = filtered,
        prev = prev,
        next = next,
        pp = opts.per_page,
    )
}

fn parent_filter_options(viewer: &str, admin: bool, selected: &str) -> String {
    let mut opts = String::from(r#"<option value="">All parents</option>"#);
    let parents: Vec<String> = if admin {
        list_resellers()
            .unwrap_or_default()
            .into_iter()
            .map(|r| r.username)
            .collect()
    } else {
        vec![viewer.to_string()]
    };
    for u in parents {
        let sel = if u.eq_ignore_ascii_case(selected) {
            " selected"
        } else {
            ""
        };
        opts.push_str(&format!(
            r#"<option value="{v}"{sel}>{v}</option>"#,
            v = html_escape(&u),
            sel = sel,
        ));
    }
    opts
}

fn row_actions(viewer: &str, admin: bool, csrf: &str, acct: &AccountPublic) -> String {
    let mut actions = String::new();
    if can_manage_account(viewer, &acct.username) {
        actions.push_str(&format!(
            r#"<button type="button" class="btn-secondary users-manage-open" data-username="{u}" style="min-height:36px;">Manage</button>"#,
            u = html_escape(&acct.username),
        ));
    }
    if admin && is_reseller_row(acct) {
        actions.push_str(&format!(
            r#" <form method="post" action="/account/users/reseller/demote" style="display:inline;"
      onsubmit="return confirm('Demote this reseller?');">
  <input type="hidden" name="csrf" value="{csrf}">
  <input type="hidden" name="username" value="{u}">
  <button type="submit" class="btn-secondary" style="min-height:36px;">Demote</button>
</form>"#,
            csrf = html_escape(csrf),
            u = html_escape(&acct.username),
        ));
    }
    if admin && is_child(acct) {
        actions.push_str(&format!(
            r#" <form method="post" action="/account/users/reseller/unassign" style="display:inline;"
      onsubmit="return confirm('Unassign this child user?');">
  <input type="hidden" name="csrf" value="{csrf}">
  <input type="hidden" name="child" value="{u}">
  <button type="submit" class="btn-secondary" style="min-height:36px;">Unassign</button>
</form>"#,
            csrf = html_escape(csrf),
            u = html_escape(&acct.username),
        ));
    }
    if actions.is_empty() {
        r#"<span class="muted">n/a</span>"#.into()
    } else {
        format!(
            r#"<div style="display:flex;flex-wrap:wrap;gap:6px;align-items:center;">{actions}</div>"#
        )
    }
}

/// Users tab body: search, filters, table, pagination (default 5).
pub fn reseller_users_panel(
    viewer: &str,
    admin: bool,
    csrf: &str,
    opts: &ResellerUsersOpts,
) -> String {
    let mut accounts = scoped_accounts(viewer, admin);
    let total = accounts.len();
    accounts.retain(|a| matches_filters(a, opts, admin));
    accounts.sort_by(|a, b| {
        a.username
            .to_ascii_lowercase()
            .cmp(&b.username.to_ascii_lowercase())
    });
    let filtered = accounts.len();
    let total_pages = filtered.max(1).div_ceil(opts.per_page);
    let page = opts.page.min(total_pages).max(1);
    let start = (page - 1) * opts.per_page;
    let page_rows: &[AccountPublic] = if start >= accounts.len() {
        &[]
    } else {
        let end = (start + opts.per_page).min(accounts.len());
        &accounts[start..end]
    };

    let scope_note = if admin {
        r#"<p class="muted" style="margin:0 0 8px;">Showing all resellers and their child users. Create accounts on the Resellers tab.</p>"#
    } else {
        r#"<p class="muted" style="margin:0 0 8px;">Showing your reseller account and child users under your hierarchy.</p>"#
    };

    let role_sel = |id: &str| -> String {
        if opts.role == id {
            " selected".into()
        } else {
            String::new()
        }
    };
    let status_sel = |id: &str| -> String {
        if opts.status == id {
            " selected".into()
        } else {
            String::new()
        }
    };

    let mut body = String::from(scope_note);
    body.push_str(&format!(
        r#"<style>
.reseller-users-table td[data-label="Email"] {{ max-width:220px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }}
@media (max-width:719.98px) {{
  .reseller-users-table thead {{ display:none; }}
  .reseller-users-table tr {{ display:block; border:1px solid var(--hairline,#334155); border-radius:10px; margin:0 0 10px; padding:10px; }}
  .reseller-users-table td {{ display:flex; justify-content:space-between; gap:12px; padding:6px 0; border:0; white-space:normal; max-width:none; }}
  .reseller-users-table td::before {{ content:attr(data-label); font-weight:600; color:var(--muted,#94a3b8); }}
}}
</style>
<form method="get" action="/account/users/reseller" class="plugin-search-row cpn-list-search" role="search" style="display:flex;flex-wrap:wrap;gap:10px;align-items:center;margin:12px 0;">
  <input type="hidden" name="tab" value="users">
  <input type="hidden" name="page" value="1">
  <input type="hidden" name="per_page" value="{pp}">
  <label class="visually-hidden" for="cpn-reseller-users-q">Search</label>
  <input class="plugin-search" id="cpn-reseller-users-q" name="q" type="search" value="{q}" placeholder="{ph}" maxlength="120" autocomplete="off" aria-label="{ph}" style="flex:1 1 220px;min-height:40px;border-radius:10px;padding:10px 12px;">
  <label>Role
    <select name="role" onchange="this.form.submit()">
      <option value="all"{role_all}>All roles</option>
      <option value="reseller"{role_reseller}>Reseller</option>
      <option value="child"{role_child}>Child</option>
    </select>
  </label>
  <label>Parent
    <select name="parent" onchange="this.form.submit()">{parent_opts}</select>
  </label>
  <label>Status
    <select name="status" onchange="this.form.submit()">
      <option value="all"{st_all}>All</option>
      <option value="active"{st_active}>Active</option>
      <option value="disabled"{st_disabled}>Disabled</option>
    </select>
  </label>
  <button type="submit" class="btn-primary">Search</button>
</form>
{summary}"#,
        pp = opts.per_page,
        q = html_escape(opts.q.as_str()),
        ph = html_escape(if admin {
            "Search username, email, or parent reseller"
        } else {
            "Search username or email"
        }),
        role_all = role_sel("all"),
        role_reseller = role_sel("reseller"),
        role_child = role_sel("child"),
        parent_opts = parent_filter_options(viewer, admin, &opts.parent),
        st_all = status_sel("all"),
        st_active = status_sel("active"),
        st_disabled = status_sel("disabled"),
        summary = list_filter_summary(filtered, total, &opts.q),
    ));

    if total == 0 {
        body.push_str(
            r#"<article class="section-card"><p class="empty-state">No reseller users yet. Promote a reseller or create a child on the Resellers tab.</p></article>"#,
        );
    } else if page_rows.is_empty() {
        body.push_str(
            r#"<p class="empty-state">No users match this filter. <a href="/account/users/reseller?tab=users">Clear filters</a>.</p>"#,
        );
    } else {
        body.push_str(&pager_html(opts, page, total_pages, filtered));
        body.push_str(
            r#"<div class="table-wrap"><table class="data-table reseller-users-table">
  <thead><tr>
    <th>Username</th><th>Email</th><th>Role / parent</th><th>Status</th><th>Actions</th>
  </tr></thead><tbody>"#,
        );
        for acct in page_rows {
            let role = hierarchy_role(acct);
            let parent_cell = if is_reseller_row(acct) {
                format!(r#"{} <span class="muted">(self)</span>"#, html_escape(role))
            } else if acct.parent_reseller.trim().is_empty() {
                html_escape(role)
            } else {
                format!(
                    r#"{} · <code>{}</code>"#,
                    html_escape(role),
                    html_escape(&acct.parent_reseller)
                )
            };
            let email_full = acct.recovery_email.as_str();
            let email_show = truncate_email(email_full, 28);
            body.push_str(&format!(
                r#"<tr>
  <td data-label="Username"><strong>{user}</strong></td>
  <td data-label="Email" title="{email_title}">{email}</td>
  <td data-label="Role / parent">{parent}</td>
  <td data-label="Status">{status}</td>
  <td data-label="Actions">{actions}</td>
</tr>"#,
                user = html_escape(&acct.username),
                email_title = html_escape(email_full),
                email = html_escape(&email_show),
                parent = parent_cell,
                status = status_label(acct.disabled),
                actions = row_actions(viewer, admin, csrf, acct),
            ));
        }
        body.push_str("</tbody></table></div>");
        body.push_str(&pager_html(opts, page, total_pages, filtered));
    }

    body.push_str(crate::panel_hub_pages_users_manage::manage_modal_shell());
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_helpers_default_five_and_filters() {
        assert_eq!(reseller_users_per_page_from_query(""), 5);
        assert_eq!(reseller_users_per_page_from_query("25"), 25);
        assert_eq!(reseller_users_per_page_from_query("20"), 5);
        assert_eq!(reseller_users_role_from_query("children"), "child");
        assert_eq!(reseller_users_status_from_query("disabled"), "disabled");
        let opts = reseller_users_opts(
            Some("Acme"),
            Some("reseller"),
            Some("rseller1"),
            Some("active"),
            Some("2"),
            Some("10"),
        );
        assert_eq!(opts.q, "acme");
        assert_eq!(opts.role, "reseller");
        assert_eq!(opts.parent, "rseller1");
        assert_eq!(opts.status, "active");
        assert_eq!(opts.page, 2);
        assert_eq!(opts.per_page, 10);
    }

    #[test]
    fn truncate_email_keeps_short() {
        assert_eq!(truncate_email("a@b.co", 28), "a@b.co");
        let long = "verylong.email.address@example.com";
        let out = truncate_email(long, 28);
        assert!(out.ends_with('…'));
        assert!(out.chars().count() <= 28);
    }
}
