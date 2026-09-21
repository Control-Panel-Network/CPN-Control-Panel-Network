//! Manage DNS list toolbar (Pagination / Scrollbar) and URL helpers.

use crate::panel_ops_cloudflare_api::CfDnsRecord;
use crate::panel_plugins_spa::list_mode_from_query;

/// List display options for Manage DNS (defaults: page mode, 10 per page, name asc).
#[derive(Debug, Clone)]
pub struct CfTableOpts {
    pub filter_type: String,
    pub page: usize,
    pub per_page: usize,
    pub mode: String,
    pub sort: String,
    pub order: String,
}

impl Default for CfTableOpts {
    fn default() -> Self {
        Self {
            filter_type: String::new(),
            page: 1,
            per_page: 10,
            mode: "page".into(),
            sort: "name".into(),
            order: "asc".into(),
        }
    }
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn dns_per_page_from_query(raw: &str) -> usize {
    let t = raw.trim().to_ascii_lowercase();
    if t.is_empty() || t == "all" {
        return 10;
    }
    match t.parse::<usize>().unwrap_or(10) {
        n @ (10 | 25 | 50 | 100) => n,
        _ => 10,
    }
}

pub fn dns_page_from_query(raw: &str) -> usize {
    raw.trim().parse::<usize>().unwrap_or(1).max(1)
}

pub fn dns_mode_from_query(raw: &str) -> &'static str {
    list_mode_from_query(raw)
}

pub fn dns_sort_from_query(raw: &str) -> &'static str {
    match raw.trim().to_ascii_lowercase().as_str() {
        "type" => "type",
        "ttl" => "ttl",
        "value" | "content" => "value",
        "priority" | "pri" => "priority",
        "proxy" | "proxied" => "proxy",
        _ => "name",
    }
}

pub fn dns_order_from_query(raw: &str) -> &'static str {
    if raw.trim().eq_ignore_ascii_case("desc") {
        "desc"
    } else {
        "asc"
    }
}

/// Default order when selecting a new sort column (TTL/Priority start high-first).
pub fn dns_default_order_for_sort(sort: &str) -> &'static str {
    match dns_sort_from_query(sort) {
        "ttl" | "priority" => "desc",
        _ => "asc",
    }
}

fn urlencoding_path(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}

/// Build a Manage DNS URL from list options (preserves filter, mode, sort, order).
pub fn manage_list_url(domain: &str, opts: &CfTableOpts) -> String {
    let mut url = format!(
        "/dns/cloudflare?tab=manage&domain={}",
        urlencoding_path(domain)
    );
    let ft = opts.filter_type.trim();
    if !ft.is_empty() && !ft.eq_ignore_ascii_case("all") {
        url.push_str("&type=");
        url.push_str(&urlencoding_path(ft));
    }
    let mode = dns_mode_from_query(&opts.mode);
    if mode == "scroll" {
        url.push_str("&mode=scroll");
    } else {
        url.push_str("&mode=page");
        url.push_str(&format!("&per_page={}", opts.per_page));
        url.push_str(&format!("&page={}", opts.page.max(1)));
    }
    let sort = dns_sort_from_query(&opts.sort);
    let order = dns_order_from_query(&opts.order);
    if sort != "name" || order != "asc" {
        url.push_str(&format!("&sort={sort}&order={order}"));
    }
    url
}

/// URL for a column header click (toggle order when already sorting that column).
pub fn manage_sort_url(domain: &str, opts: &CfTableOpts, column: &str) -> String {
    let column = dns_sort_from_query(column);
    let mut next = opts.clone();
    next.page = 1;
    next.sort = column.into();
    if dns_sort_from_query(&opts.sort) == column {
        next.order = if dns_order_from_query(&opts.order) == "asc" {
            "desc".into()
        } else {
            "asc".into()
        };
    } else {
        next.order = dns_default_order_for_sort(column).into();
    }
    manage_list_url(domain, &next)
}

pub(crate) fn type_hidden_input(filter_type: &str) -> String {
    let ft = filter_type.trim();
    if ft.is_empty() || ft.eq_ignore_ascii_case("all") {
        String::new()
    } else {
        format!(
            r#"<input type="hidden" name="type" value="{}">"#,
            html_escape(ft)
        )
    }
}

pub(crate) fn sort_hidden_inputs(opts: &CfTableOpts) -> String {
    let sort = dns_sort_from_query(&opts.sort);
    let order = dns_order_from_query(&opts.order);
    if sort == "name" && order == "asc" {
        String::new()
    } else {
        format!(
            r#"<input type="hidden" name="sort" value="{sort}">
<input type="hidden" name="order" value="{order}">"#
        )
    }
}

pub(crate) fn list_state_hiddens(opts: &CfTableOpts) -> String {
    format!(
        r#"<input type="hidden" name="filter_type" value="{ft}">
<input type="hidden" name="page" value="{page}">
<input type="hidden" name="per_page" value="{per_page}">
<input type="hidden" name="mode" value="{mode}">
<input type="hidden" name="sort" value="{sort}">
<input type="hidden" name="order" value="{order}">"#,
        ft = html_escape(&opts.filter_type),
        page = opts.page.max(1),
        per_page = opts.per_page,
        mode = html_escape(dns_mode_from_query(&opts.mode)),
        sort = html_escape(dns_sort_from_query(&opts.sort)),
        order = html_escape(dns_order_from_query(&opts.order)),
    )
}

/// Cloudflare TTL: 1 (and 0) means AUTO; sort as numeric 1 so AUTO ranks lowest.
fn ttl_sort_key(ttl: u32) -> u32 {
    if ttl == 0 { 1 } else { ttl }
}

fn priority_sort_key(priority: Option<u16>) -> u32 {
    // Missing priority sorts after real values in ascending order.
    priority.map(|p| u32::from(p)).unwrap_or(u32::MAX)
}

fn cmp_records(a: &CfDnsRecord, b: &CfDnsRecord, sort: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let primary = match dns_sort_from_query(sort) {
        "type" => a
            .record_type
            .to_ascii_lowercase()
            .cmp(&b.record_type.to_ascii_lowercase()),
        "ttl" => ttl_sort_key(a.ttl).cmp(&ttl_sort_key(b.ttl)),
        "value" => a
            .content
            .to_ascii_lowercase()
            .cmp(&b.content.to_ascii_lowercase()),
        "priority" => priority_sort_key(a.priority).cmp(&priority_sort_key(b.priority)),
        "proxy" => a.proxied.cmp(&b.proxied),
        _ => a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()),
    };
    if primary != Ordering::Equal {
        return primary;
    }
    a.name
        .to_ascii_lowercase()
        .cmp(&b.name.to_ascii_lowercase())
        .then_with(|| {
            a.record_type
                .to_ascii_lowercase()
                .cmp(&b.record_type.to_ascii_lowercase())
        })
}

/// Sort filtered records in place according to opts.sort / opts.order.
pub fn sort_dns_records(records: &mut [&CfDnsRecord], opts: &CfTableOpts) {
    let sort = dns_sort_from_query(&opts.sort);
    let desc = dns_order_from_query(&opts.order) == "desc";
    records.sort_by(|a, b| {
        let ord = cmp_records(a, b, sort);
        if desc { ord.reverse() } else { ord }
    });
}

fn toolbar_styles() -> &'static str {
    r#"<style>
.plugin-list-toolbar{display:flex;flex-wrap:wrap;gap:12px;align-items:center;margin:12px 0 8px;padding:10px 12px;border:1px solid var(--hairline,#333);border-radius:12px;background:var(--canvas,#111)}
.plugin-mode-switch{display:inline-flex;border:1px solid var(--hairline,#444);border-radius:999px;overflow:hidden}
.plugin-mode-switch a{min-height:34px;padding:0 14px;border:0;background:transparent;color:var(--ink,#eee);font:inherit;font-weight:600;cursor:pointer;text-decoration:none;display:inline-flex;align-items:center}
.plugin-mode-switch a.active{background:#1d4ed8;color:#fff}
.plugin-pager{display:flex;flex-wrap:wrap;gap:8px;align-items:center;margin-left:auto}
.plugin-pager label{font-size:13px;font-weight:600;color:var(--ink,#eee)}
.plugin-pager select,.plugin-pager input[type=number]{min-height:34px;border:1px solid #94a3b8;border-radius:8px;padding:4px 8px;font:inherit;color:var(--ink,#1d1d1f);background:var(--canvas,#fff);max-width:88px;color-scheme:light}
.plugin-pager .page-status{font-size:13px;font-weight:700;color:var(--ink,#eee)}
.cf-table-scroll{margin-top:10px}
.cf-table-scroll.is-scroll{max-height:min(70vh,720px);overflow:auto;border:1px solid var(--hairline,#333);border-radius:12px;padding:8px}
.cf-table th a.cf-sort{color:inherit;text-decoration:none;display:inline-flex;align-items:center;gap:4px}
.cf-table th a.cf-sort:hover{text-decoration:underline}
.cf-table th a.cf-sort .cf-sort-ind{font-size:11px;opacity:.85}
[data-color-mode=dark] .plugin-mode-switch a.active{background:#2563eb;color:#fff}
[data-color-mode=dark] .plugin-pager select,[data-color-mode=dark] .plugin-pager input[type=number]{background:#1a1d26;border-color:#475569;color:#f1f5f9;color-scheme:dark}
</style>"#
}

pub(crate) fn dns_list_toolbar(
    domain: &str,
    opts: &CfTableOpts,
    page: usize,
    total_pages: usize,
    filtered: usize,
    total: usize,
) -> String {
    let mode = dns_mode_from_query(&opts.mode);
    let page_cls = if mode == "page" {
        "plugin-mode-btn active"
    } else {
        "plugin-mode-btn"
    };
    let scroll_cls = if mode == "scroll" {
        "plugin-mode-btn active"
    } else {
        "plugin-mode-btn"
    };
    let mut page_opts = opts.clone();
    page_opts.mode = "page".into();
    page_opts.page = 1;
    let mut scroll_opts = opts.clone();
    scroll_opts.mode = "scroll".into();
    scroll_opts.page = 1;
    let page_href = manage_list_url(domain, &page_opts);
    let scroll_href = manage_list_url(domain, &scroll_opts);
    let pager_style = if mode == "scroll" {
        " style=\"display:none\""
    } else {
        ""
    };
    let prev = if page > 1 { page - 1 } else { 1 };
    let next = if page < total_pages {
        page + 1
    } else {
        total_pages.max(1)
    };
    let mut prev_opts = opts.clone();
    prev_opts.mode = "page".into();
    prev_opts.page = prev;
    let mut next_opts = opts.clone();
    next_opts.mode = "page".into();
    next_opts.page = next;
    let prev_href = manage_list_url(domain, &prev_opts);
    let next_href = manage_list_url(domain, &next_opts);
    let prev_dis = if page <= 1 {
        " aria-disabled=\"true\" tabindex=\"-1\" style=\"pointer-events:none;opacity:.45\""
    } else {
        ""
    };
    let next_dis = if page >= total_pages.max(1) {
        " aria-disabled=\"true\" tabindex=\"-1\" style=\"pointer-events:none;opacity:.45\""
    } else {
        ""
    };
    let mut options = String::new();
    for size in [10usize, 25, 50, 100] {
        let sel = if mode == "page" && size == opts.per_page {
            " selected"
        } else {
            ""
        };
        options.push_str(&format!(
            r#"<option value="{size}"{sel}>{size}</option>"#,
            size = size,
            sel = sel
        ));
    }
    let all_sel = if mode == "scroll" { " selected" } else { "" };
    options.push_str(&format!(
        r#"<option value="all"{all_sel}>All</option>"#,
        all_sel = all_sel
    ));
    let filter_label =
        if opts.filter_type.is_empty() || opts.filter_type.eq_ignore_ascii_case("all") {
            format!("Showing {filtered} of {total} records")
        } else {
            format!(
                "Showing {filtered} of {total} · {}",
                html_escape(&opts.filter_type.to_ascii_uppercase())
            )
        };
    let sort_hiddens = sort_hidden_inputs(opts);
    format!(
        r#"{styles}
<div class="plugin-list-toolbar" role="group" aria-label="DNS records display">
  <div class="plugin-mode-switch" role="group" aria-label="List mode">
    <a class="{page_cls}" href="{page_href}">Pagination</a>
    <a class="{scroll_cls}" href="{scroll_href}">Scrollbar</a>
  </div>
  <span class="muted" id="cf-filter-count">{filter_label}</span>
  <div class="plugin-pager"{pager_style}>
    <form method="get" action="/dns/cloudflare" style="display:inline-flex;flex-wrap:wrap;gap:8px;align-items:center;margin:0;">
      <input type="hidden" name="tab" value="manage">
      <input type="hidden" name="domain" value="{dom}">
      <input type="hidden" name="mode" value="page">
      {type_hidden}
      {sort_hiddens}
      <label for="cf-per-page">Show</label>
      <select id="cf-per-page" name="per_page" aria-label="Records per page" onchange="this.form.submit()">{options}</select>
      <span>per page</span>
    </form>
    <span class="page-status">Page {page} / {total_pages}</span>
    <a class="btn-secondary" href="{prev_href}"{prev_dis}>Prev</a>
    <a class="btn-secondary" href="{next_href}"{next_dis}>Next</a>
    <form method="get" action="/dns/cloudflare" style="display:inline-flex;flex-wrap:wrap;gap:6px;align-items:center;margin:0;">
      <input type="hidden" name="tab" value="manage">
      <input type="hidden" name="domain" value="{dom}">
      <input type="hidden" name="mode" value="page">
      <input type="hidden" name="per_page" value="{per_page}">
      {type_hidden}
      {sort_hiddens}
      <label for="cf-goto-page">Go to page</label>
      <input id="cf-goto-page" name="page" type="number" min="1" max="{total_pages}" value="{page}">
      <button type="submit" class="btn-primary">Go</button>
    </form>
  </div>
</div>"#,
        styles = toolbar_styles(),
        page_cls = page_cls,
        scroll_cls = scroll_cls,
        page_href = html_escape(&page_href),
        scroll_href = html_escape(&scroll_href),
        filter_label = filter_label,
        pager_style = pager_style,
        options = options,
        page = page,
        total_pages = total_pages.max(1),
        prev_href = html_escape(&prev_href),
        next_href = html_escape(&next_href),
        prev_dis = prev_dis,
        next_dis = next_dis,
        dom = html_escape(domain),
        per_page = opts.per_page,
        type_hidden = type_hidden_input(&opts.filter_type),
        sort_hiddens = sort_hiddens,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        dns_default_order_for_sort, dns_mode_from_query, dns_order_from_query, dns_page_from_query,
        dns_per_page_from_query, dns_sort_from_query,
    };

    #[test]
    fn dns_per_page_defaults_to_ten() {
        assert_eq!(dns_per_page_from_query(""), 10);
        assert_eq!(dns_per_page_from_query("7"), 10);
        assert_eq!(dns_per_page_from_query("4"), 10);
        assert_eq!(dns_per_page_from_query("25"), 25);
        assert_eq!(dns_per_page_from_query("100"), 100);
    }

    #[test]
    fn dns_page_and_mode() {
        assert_eq!(dns_page_from_query(""), 1);
        assert_eq!(dns_page_from_query("3"), 3);
        assert_eq!(dns_mode_from_query("scroll"), "scroll");
        assert_eq!(dns_mode_from_query(""), "page");
    }

    #[test]
    fn dns_sort_and_order() {
        assert_eq!(dns_sort_from_query("TTL"), "ttl");
        assert_eq!(dns_sort_from_query("bogus"), "name");
        assert_eq!(dns_order_from_query("DESC"), "desc");
        assert_eq!(dns_order_from_query(""), "asc");
        assert_eq!(dns_default_order_for_sort("ttl"), "desc");
        assert_eq!(dns_default_order_for_sort("name"), "asc");
    }
}
