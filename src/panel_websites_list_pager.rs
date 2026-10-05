//! Pagination toolbar for `/websites/list` and `/subdomains` (default 5 per page).

use crate::panel_list_search::normalize_list_q;
use crate::panel_plugins_spa::list_mode_from_query;

/// List display options (defaults: page mode, 5 per page).
#[derive(Debug, Clone)]
pub struct SitesListOpts {
    pub q: String,
    pub page: usize,
    pub per_page: usize,
    pub mode: String,
}

impl Default for SitesListOpts {
    fn default() -> Self {
        Self {
            q: String::new(),
            page: 1,
            per_page: 5,
            mode: "page".into(),
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

pub fn sites_per_page_from_query(raw: &str) -> usize {
    match raw.trim().parse::<usize>().unwrap_or(5) {
        n @ (5 | 10 | 25 | 50) => n,
        _ => 5,
    }
}

pub fn sites_page_from_query(raw: &str) -> usize {
    raw.trim().parse::<usize>().unwrap_or(1).max(1)
}

pub fn sites_mode_from_query(mode_raw: &str, per_page_raw: &str) -> &'static str {
    let per = per_page_raw.trim();
    if per.eq_ignore_ascii_case("all") || list_mode_from_query(mode_raw) == "scroll" {
        "scroll"
    } else {
        "page"
    }
}

pub fn sites_list_opts(
    q: Option<&str>,
    page: Option<&str>,
    per_page: Option<&str>,
    mode: Option<&str>,
) -> SitesListOpts {
    let per_raw = per_page.unwrap_or("5");
    let mode_raw = mode.unwrap_or("page");
    SitesListOpts {
        q: normalize_list_q(q),
        page: sites_page_from_query(page.unwrap_or("1")),
        per_page: sites_per_page_from_query(per_raw),
        mode: sites_mode_from_query(mode_raw, per_raw).to_string(),
    }
}

/// Build a list URL preserving search, mode, and page size.
pub fn sites_list_url(action: &str, opts: &SitesListOpts) -> String {
    let mut url = action.to_string();
    let mut first = true;
    let mut push = |key: &str, value: &str| {
        if value.is_empty() {
            return;
        }
        url.push(if first { '?' } else { '&' });
        first = false;
        url.push_str(key);
        url.push('=');
        url.push_str(&query_escape(value));
    };
    if !opts.q.is_empty() {
        push("q", &opts.q);
    }
    let mode = sites_mode_from_query(&opts.mode, "");
    if mode == "scroll" {
        push("mode", "scroll");
    } else {
        push("mode", "page");
        push("per_page", &opts.per_page.to_string());
        push("page", &opts.page.max(1).to_string());
    }
    url
}

/// Slice filtered items for the current page (or all items in scroll mode).
pub fn paginate_slice<'a, T>(
    items: &'a [T],
    opts: &SitesListOpts,
) -> (usize, usize, usize, &'a [T]) {
    let filtered = items.len();
    let mode = sites_mode_from_query(&opts.mode, "");
    if mode == "scroll" || filtered == 0 {
        return (1, 1, filtered, items);
    }
    let per_page = opts.per_page.max(1);
    let total_pages = filtered.div_ceil(per_page).max(1);
    let page = opts.page.min(total_pages).max(1);
    let start = (page - 1) * per_page;
    if start >= filtered {
        return (page, total_pages, filtered, &[]);
    }
    let end = (start + per_page).min(filtered);
    (page, total_pages, filtered, &items[start..end])
}

fn toolbar_styles() -> &'static str {
    r#"<style>
.plugin-list-toolbar{display:flex;flex-wrap:wrap;gap:12px;align-items:center;margin:12px 0 8px;padding:10px 12px;border:1px solid var(--hairline,#cbd5e1);border-radius:12px;background:var(--canvas,#fff)}
.plugin-mode-switch{display:inline-flex;border:1px solid var(--hairline,#94a3b8);border-radius:999px;overflow:hidden}
.plugin-mode-switch a{min-height:34px;padding:0 14px;border:0;background:transparent;color:var(--ink,#1d1d1f);font:inherit;font-weight:600;cursor:pointer;text-decoration:none;display:inline-flex;align-items:center}
.plugin-mode-switch a.active{background:#1d4ed8;color:#fff}
.plugin-pager{display:flex;flex-wrap:wrap;gap:8px;align-items:center;margin-left:auto}
.plugin-pager label{font-size:13px;font-weight:600;color:var(--ink,#1d1d1f)}
.plugin-pager select,.plugin-pager input[type=number]{min-height:34px;border:1px solid #94a3b8;border-radius:8px;padding:4px 8px;font:inherit;color:var(--ink,#1d1d1f);background:var(--canvas,#fff);max-width:88px;color-scheme:light}
.plugin-pager .page-status{font-size:13px;font-weight:700;color:var(--ink,#1d1d1f)}
.plugin-pager .btn-secondary,.plugin-pager .btn-primary{min-height:34px}
@media (max-width:640px){
  .plugin-list-toolbar{flex-direction:column;align-items:stretch}
  .plugin-pager{margin-left:0;width:100%}
  .plugin-pager select,.plugin-pager input[type=number]{max-width:none}
}
[data-color-mode=dark] .plugin-list-toolbar{border-color:#333;background:var(--canvas,#111)}
[data-color-mode=dark] .plugin-mode-switch{border-color:#444}
[data-color-mode=dark] .plugin-mode-switch a{color:var(--ink,#eee)}
[data-color-mode=dark] .plugin-mode-switch a.active{background:#2563eb;color:#fff}
[data-color-mode=dark] .plugin-pager label,[data-color-mode=dark] .plugin-pager .page-status{color:var(--ink,#eee)}
[data-color-mode=dark] .plugin-pager select,[data-color-mode=dark] .plugin-pager input[type=number]{background:#1a1d26;border-color:#475569;color:#f1f5f9;color-scheme:dark}
</style>"#
}

fn q_hidden(q: &str) -> String {
    if q.trim().is_empty() {
        String::new()
    } else {
        format!(
            r#"<input type="hidden" name="q" value="{}">"#,
            html_escape(q.trim())
        )
    }
}

/// Pagination / Scrollbar toolbar with per-page dropdown and go-to-page.
pub fn sites_list_toolbar(
    action: &str,
    opts: &SitesListOpts,
    page: usize,
    total_pages: usize,
    filtered: usize,
) -> String {
    sites_list_toolbar_id(action, opts, page, total_pages, filtered, "top")
}

/// Bottom pager (same controls, unique element ids).
pub fn sites_list_toolbar_bottom(
    action: &str,
    opts: &SitesListOpts,
    page: usize,
    total_pages: usize,
    filtered: usize,
) -> String {
    sites_list_toolbar_id(action, opts, page, total_pages, filtered, "bottom")
}

fn sites_list_toolbar_id(
    action: &str,
    opts: &SitesListOpts,
    page: usize,
    total_pages: usize,
    filtered: usize,
    id_suffix: &str,
) -> String {
    let mode = sites_mode_from_query(&opts.mode, "");
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
    let page_href = sites_list_url(action, &page_opts);
    let scroll_href = sites_list_url(action, &scroll_opts);
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
    let prev_href = sites_list_url(action, &prev_opts);
    let next_href = sites_list_url(action, &next_opts);
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
    for size in [5usize, 10, 25, 50] {
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
    let action_e = html_escape(action);
    let q_h = q_hidden(&opts.q);
    let id = html_escape(id_suffix);
    format!(
        r#"{styles}
<div class="plugin-list-toolbar" role="group" aria-label="List display">
  <div class="plugin-mode-switch" role="group" aria-label="List mode">
    <a class="{page_cls}" href="{page_href}">Pagination</a>
    <a class="{scroll_cls}" href="{scroll_href}">Scrollbar</a>
  </div>
  <span class="muted">{filtered} matching</span>
  <div class="plugin-pager"{pager_style}>
    <form method="get" action="{action_e}" style="display:inline-flex;flex-wrap:wrap;gap:8px;align-items:center;margin:0;">
      <input type="hidden" name="mode" value="page">
      <input type="hidden" name="page" value="1">
      {q_h}
      <label for="sites-per-page-{id}">Show</label>
      <select id="sites-per-page-{id}" name="per_page" aria-label="Items per page" onchange="if(this.value==='all'){{this.form.mode.value='scroll';}}else{{this.form.mode.value='page';}} this.form.submit()">{options}</select>
      <span>per page</span>
    </form>
    <span class="page-status">Page {page} / {total_pages}</span>
    <a class="btn-secondary" href="{prev_href}"{prev_dis}>Prev</a>
    <a class="btn-secondary" href="{next_href}"{next_dis}>Next</a>
    <form method="get" action="{action_e}" style="display:inline-flex;flex-wrap:wrap;gap:6px;align-items:center;margin:0;">
      <input type="hidden" name="mode" value="page">
      <input type="hidden" name="per_page" value="{per_page}">
      {q_h}
      <label for="sites-goto-page-{id}">Go to page</label>
      <input id="sites-goto-page-{id}" name="page" type="number" min="1" max="{total_pages}" value="{page}" inputmode="numeric">
      <button type="submit" class="btn-primary">Go</button>
    </form>
  </div>
</div>"#,
        styles = toolbar_styles(),
        page_cls = page_cls,
        scroll_cls = scroll_cls,
        page_href = html_escape(&page_href),
        scroll_href = html_escape(&scroll_href),
        filtered = filtered,
        pager_style = pager_style,
        options = options,
        page = page,
        total_pages = total_pages.max(1),
        prev_href = html_escape(&prev_href),
        next_href = html_escape(&next_href),
        prev_dis = prev_dis,
        next_dis = next_dis,
        action_e = action_e,
        per_page = opts.per_page,
        q_h = q_h,
        id = id,
    )
}

/// Hidden fields so search keeps page size / mode and resets to page 1.
pub fn sites_search_extras(opts: &SitesListOpts) -> String {
    let mode = sites_mode_from_query(&opts.mode, "");
    if mode == "scroll" {
        r#"<input type="hidden" name="mode" value="scroll">
<input type="hidden" name="page" value="1">"#
            .into()
    } else {
        format!(
            r#"<input type="hidden" name="mode" value="page">
<input type="hidden" name="per_page" value="{pp}">
<input type="hidden" name="page" value="1">"#,
            pp = opts.per_page
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SitesListOpts, paginate_slice, sites_list_opts, sites_list_toolbar, sites_list_url,
        sites_mode_from_query, sites_page_from_query, sites_per_page_from_query,
    };

    #[test]
    fn per_page_defaults_to_five() {
        assert_eq!(sites_per_page_from_query(""), 5);
        assert_eq!(sites_per_page_from_query("7"), 5);
        assert_eq!(sites_per_page_from_query("10"), 10);
        assert_eq!(sites_per_page_from_query("25"), 25);
        assert_eq!(sites_per_page_from_query("50"), 50);
    }

    #[test]
    fn mode_all_and_scroll() {
        assert_eq!(sites_mode_from_query("page", "all"), "scroll");
        assert_eq!(sites_mode_from_query("scroll", "5"), "scroll");
        assert_eq!(sites_mode_from_query("page", "5"), "page");
        assert_eq!(sites_mode_from_query("scrollbar", ""), "scroll");
    }

    #[test]
    fn page_clamps_and_paginates() {
        assert_eq!(sites_page_from_query(""), 1);
        assert_eq!(sites_page_from_query("0"), 1);
        let items: Vec<u8> = (1..=12).collect();
        let opts = SitesListOpts {
            q: String::new(),
            page: 2,
            per_page: 5,
            mode: "page".into(),
        };
        let (page, total_pages, filtered, slice) = paginate_slice(&items, &opts);
        assert_eq!(page, 2);
        assert_eq!(total_pages, 3);
        assert_eq!(filtered, 12);
        assert_eq!(slice, &[6, 7, 8, 9, 10]);
    }

    #[test]
    fn scroll_shows_all() {
        let items: Vec<u8> = (1..=8).collect();
        let opts = sites_list_opts(None, Some("3"), Some("all"), Some("page"));
        let (page, total_pages, filtered, slice) = paginate_slice(&items, &opts);
        assert_eq!(opts.mode, "scroll");
        assert_eq!(page, 1);
        assert_eq!(total_pages, 1);
        assert_eq!(filtered, 8);
        assert_eq!(slice.len(), 8);
    }

    #[test]
    fn url_preserves_search_and_page() {
        let opts = SitesListOpts {
            q: "blog".into(),
            page: 2,
            per_page: 10,
            mode: "page".into(),
        };
        let url = sites_list_url("/websites/list", &opts);
        assert!(url.contains("q=blog"));
        assert!(url.contains("page=2"));
        assert!(url.contains("per_page=10"));
        assert!(!url.to_lowercase().contains("cyberpanel"));
    }

    #[test]
    fn toolbar_has_goto_and_sizes() {
        let opts = SitesListOpts::default();
        let html = sites_list_toolbar("/websites/list", &opts, 1, 3, 12);
        assert!(html.contains("Go to page"));
        assert!(html.contains(r#"value="5""#));
        assert!(html.contains(r#"value="10""#));
        assert!(html.contains(r#"value="25""#));
        assert!(html.contains(r#"value="50""#));
        assert!(html.contains(r#"value="all""#));
        assert!(html.contains("Page 1 / 3"));
        assert!(!html.to_lowercase().contains("cyberpanel"));
    }
}
