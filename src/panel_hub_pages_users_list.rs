//! List Users: search (`q=`), sortable columns, pagination (default 5).

use crate::account_lifecycle::status_label;
use crate::account_mgmt::list_accounts;
use crate::model::AccountPublic;
use crate::packages::is_panel_admin;
use crate::panel_hubs::feature_shell;
use crate::panel_list_search::{list_filter_summary, normalize_list_q};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[derive(Debug, Clone)]
pub struct UsersListOpts {
    pub q: String,
    pub sort: String,
    pub order: String,
    pub page: usize,
    pub per_page: usize,
}

pub fn users_sort_from_query(raw: &str) -> &'static str {
    match raw.trim().to_ascii_lowercase().as_str() {
        "email" | "recovery_email" => "email",
        "role" => "role",
        "status" => "status",
        _ => "username",
    }
}

pub fn users_order_from_query(raw: &str) -> &'static str {
    if raw.trim().eq_ignore_ascii_case("desc") {
        "desc"
    } else {
        "asc"
    }
}

pub fn users_per_page_from_query(raw: &str) -> usize {
    match raw.trim().parse::<usize>().unwrap_or(5) {
        n @ (5 | 10 | 20 | 50) => n,
        _ => 5,
    }
}

pub fn users_page_from_query(raw: &str) -> usize {
    raw.trim().parse::<usize>().unwrap_or(1).max(1)
}

pub fn users_list_opts(
    q: Option<&str>,
    sort: Option<&str>,
    order: Option<&str>,
    page: Option<&str>,
    per_page: Option<&str>,
) -> UsersListOpts {
    UsersListOpts {
        q: normalize_list_q(q),
        sort: users_sort_from_query(sort.unwrap_or("")).to_string(),
        order: users_order_from_query(order.unwrap_or("")).to_string(),
        page: users_page_from_query(page.unwrap_or("1")),
        per_page: users_per_page_from_query(per_page.unwrap_or("5")),
    }
}

fn role_label(username: &str) -> &'static str {
    if is_panel_admin(username) {
        "Admin"
    } else {
        "User"
    }
}

fn account_matches(acct: &AccountPublic, q: &str) -> bool {
    if q.is_empty() {
        return true;
    }
    let role = role_label(&acct.username).to_ascii_lowercase();
    let status = status_label(acct.disabled).to_ascii_lowercase();
    acct.username.to_ascii_lowercase().contains(q)
        || acct.recovery_email.to_ascii_lowercase().contains(q)
        || role.contains(q)
        || status.contains(q)
}

fn sort_accounts(rows: &mut [AccountPublic], sort: &str, order: &str) {
    let desc = order == "desc";
    rows.sort_by(|a, b| {
        let ord = match sort {
            "email" => a
                .recovery_email
                .to_ascii_lowercase()
                .cmp(&b.recovery_email.to_ascii_lowercase()),
            "role" => role_label(&a.username).cmp(role_label(&b.username)),
            "status" => status_label(a.disabled).cmp(status_label(b.disabled)),
            _ => a
                .username
                .to_ascii_lowercase()
                .cmp(&b.username.to_ascii_lowercase()),
        };
        if desc { ord.reverse() } else { ord }
    });
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

fn list_href(opts: &UsersListOpts, page: usize, sort: &str, order: &str) -> String {
    format!(
        "/account/users/list?q={q}&sort={sort}&order={order}&page={page}&per_page={pp}",
        q = query_escape(&opts.q),
        sort = html_escape(sort),
        order = html_escape(order),
        page = page,
        pp = opts.per_page,
    )
}

fn sort_href(opts: &UsersListOpts, col: &str) -> String {
    let current = users_sort_from_query(&opts.sort);
    let next_order = if current == col && opts.order == "asc" {
        "desc"
    } else {
        "asc"
    };
    list_href(opts, 1, col, next_order)
}

fn sort_ind(opts: &UsersListOpts, col: &str) -> &'static str {
    if users_sort_from_query(&opts.sort) != col {
        return "";
    }
    if opts.order == "desc" {
        " ▼"
    } else {
        " ▲"
    }
}

fn pager_html(opts: &UsersListOpts, page: usize, total_pages: usize, filtered: usize) -> String {
    let prev = if page > 1 {
        format!(
            r#"<a class="btn-secondary" href="{}">Prev</a>"#,
            html_escape(&list_href(opts, page - 1, &opts.sort, &opts.order))
        )
    } else {
        r#"<button type="button" class="btn-secondary" disabled>Prev</button>"#.into()
    };
    let next = if page < total_pages {
        format!(
            r#"<a class="btn-secondary" href="{}">Next</a>"#,
            html_escape(&list_href(opts, page + 1, &opts.sort, &opts.order))
        )
    } else {
        r#"<button type="button" class="btn-secondary" disabled>Next</button>"#.into()
    };
    let mut size_opts = String::new();
    for n in [5usize, 10, 20, 50] {
        let sel = if n == opts.per_page { " selected" } else { "" };
        size_opts.push_str(&format!(r#"<option value="{n}"{sel}>{n}</option>"#));
    }
    format!(
        r#"<div class="activity-list-pager users-list-pager" style="display:flex;flex-wrap:wrap;gap:8px;align-items:center;margin:12px 0;">
  <form method="get" action="/account/users/list" style="display:inline-flex;flex-wrap:wrap;gap:8px;align-items:center;">
    <input type="hidden" name="q" value="{q}">
    <input type="hidden" name="sort" value="{sort}">
    <input type="hidden" name="order" value="{order}">
    <input type="hidden" name="page" value="1">
    <label>Show
      <select name="per_page" onchange="this.form.submit()">{size_opts}</select>
      per page
    </label>
  </form>
  <span class="activity-list-status">Page {page} / {pages}</span>
  <span class="muted">{filtered} matching</span>
  {prev}{next}
  <form method="get" action="/account/users/list" class="activity-list-goto">
    <input type="hidden" name="q" value="{q}">
    <input type="hidden" name="sort" value="{sort}">
    <input type="hidden" name="order" value="{order}">
    <input type="hidden" name="per_page" value="{pp}">
    <label>Go to page
      <input name="page" type="number" min="1" max="{pages}" value="{page}" inputmode="numeric">
    </label>
    <button type="submit" class="btn-primary">Go</button>
  </form>
</div>"#,
        q = html_escape(&opts.q),
        sort = html_escape(&opts.sort),
        order = html_escape(&opts.order),
        size_opts = size_opts,
        page = page,
        pages = total_pages,
        filtered = filtered,
        prev = prev,
        next = next,
        pp = opts.per_page,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn users_list_page(
    viewer: &str,
    notice: Option<&str>,
    error: Option<&str>,
    q: Option<&str>,
    sort: Option<&str>,
    order: Option<&str>,
    page: Option<&str>,
    per_page: Option<&str>,
) -> String {
    let admin = is_panel_admin(viewer);
    let opts = users_list_opts(q, sort, order, page, per_page);
    let mut accounts = list_accounts().unwrap_or_default();
    if !admin {
        accounts.retain(|a| a.username.eq_ignore_ascii_case(viewer));
    }
    let total = accounts.len();
    accounts.retain(|a| account_matches(a, &opts.q));
    sort_accounts(&mut accounts, &opts.sort, &opts.order);
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

    let mut body = String::new();
    if !admin {
        body.push_str(
            r#"<p class="muted">Showing your account only. Panel admin can list every user.</p>"#,
        );
    }
    body.push_str(&format!(
        r#"<form method="get" action="/account/users/list" class="plugin-search-row cpn-list-search" role="search" style="display:flex;flex-wrap:wrap;gap:10px;align-items:center;margin:12px 0;">
  <input type="hidden" name="sort" value="{sort}">
  <input type="hidden" name="order" value="{order}">
  <input type="hidden" name="per_page" value="{pp}">
  <input type="hidden" name="page" value="1">
  <label class="visually-hidden" for="cpn-users-q">Search</label>
  <input class="plugin-search" id="cpn-users-q" name="q" type="search" value="{q}" placeholder="Search username, email, role, or status" maxlength="120" autocomplete="off" aria-label="Search username, email, role, or status" style="flex:1 1 220px;min-height:40px;border-radius:10px;padding:10px 12px;">
  <button type="submit" class="btn-primary">Search</button>
</form>
{summary}"#,
        sort = html_escape(&opts.sort),
        order = html_escape(&opts.order),
        pp = opts.per_page,
        q = html_escape(q.unwrap_or("").trim()),
        summary = list_filter_summary(filtered, total, q.unwrap_or("")),
    ));
    if total == 0 {
        body.push_str(r#"<p class="empty-state">No panel accounts found.</p>"#);
    } else if page_rows.is_empty() {
        body.push_str(&format!(
            r#"<p class="empty-state">No users match this filter. <a href="/account/users/list">Clear search</a>.</p>"#
        ));
    } else {
        body.push_str(&pager_html(&opts, page, total_pages, filtered));
        body.push_str(
            r#"<div class="table-wrap"><table class="data-table" id="users-table">
        <thead><tr>"#,
        );
        for (col, label) in [
            ("username", "Username"),
            ("email", "Email"),
            ("role", "Role"),
            ("status", "Status"),
        ] {
            body.push_str(&format!(
                r#"<th><a class="cf-sort" href="{href}">{label}<span class="cf-sort-ind">{ind}</span></a></th>"#,
                href = html_escape(&sort_href(&opts, col)),
                label = label,
                ind = sort_ind(&opts, col),
            ));
        }
        body.push_str("<th></th></tr></thead><tbody>");
        for acct in page_rows {
            let role = role_label(&acct.username);
            body.push_str(&format!(
                r#"<tr>
              <td data-label="Username"><strong>{}</strong></td>
              <td data-label="Email">{}</td>
              <td data-label="Role">{}</td>
              <td data-label="Status">{}</td>
              <td data-label="">
                <button type="button" class="btn-secondary users-manage-open" data-username="{u}" style="min-height:36px;">Manage user</button>
              </td>
            </tr>"#,
                html_escape(&acct.username),
                html_escape(&acct.recovery_email),
                role,
                status_label(acct.disabled),
                u = html_escape(&acct.username),
            ));
        }
        body.push_str("</tbody></table></div>");
        body.push_str(&pager_html(&opts, page, total_pages, filtered));
    }
    if admin {
        body.push_str(
            r#"<p style="margin-top:16px;"><a class="btn-primary" href="/account/users/create">Create user</a></p>"#,
        );
    }
    body.push_str(crate::panel_hub_pages_users_manage::manage_modal_shell());
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Users & Plans", Some("/account/users")),
            ("List Users", None),
        ],
        "List Users",
        "Panel accounts on this host.",
        &body,
        notice,
        error,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::{default_password_policy, with_test_data_dir};
    use crate::account_mgmt::create_account;

    #[test]
    fn query_helpers_default_to_five_and_username_asc() {
        assert_eq!(users_per_page_from_query(""), 5);
        assert_eq!(users_per_page_from_query("10"), 10);
        assert_eq!(users_sort_from_query("email"), "email");
        assert_eq!(users_order_from_query("DESC"), "desc");
        let opts = users_list_opts(Some("Adm"), Some("role"), Some("desc"), Some("2"), Some("5"));
        assert_eq!(opts.q, "adm");
        assert_eq!(opts.sort, "role");
        assert_eq!(opts.order, "desc");
        assert_eq!(opts.page, 2);
        assert_eq!(opts.per_page, 5);
    }

    #[test]
    fn list_html_uses_email_label_search_and_manage() {
        with_test_data_dir(|| {
            unsafe {
                std::env::set_var("CPN_RESERVED_USERNAMES_OFFLINE", "1");
            }
            create_account(
                "cpnowner",
                None,
                true,
                "info@example.com",
                default_password_policy(),
                "en",
            )
            .expect("create");
            let html = users_list_page("cpnowner", None, None, None, None, None, None, None);
            assert!(html.contains(">Email<") || html.contains("Email"));
            assert!(!html.contains("Recovery email"));
            assert!(html.contains("name=\"q\""));
            assert!(html.contains("Manage user"));
            assert!(!html.contains(">Modify user<"));
            assert!(html.contains("Go to page"));
            assert!(html.contains("data-page-size") || html.contains("per_page"));
            unsafe {
                std::env::remove_var("CPN_RESERVED_USERNAMES_OFFLINE");
            }
        });
    }
}
