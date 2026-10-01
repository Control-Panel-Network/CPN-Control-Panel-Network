//! Server > Logs viewer pages: one searchable, paginated page per host log.

use crate::panel_admin::is_panel_admin;
use crate::panel_hub_http::urlencoding_simple;
use crate::panel_hubs::feature_shell;
use crate::panel_server_logs::{HostLogKind, LogPage, PAGE_SIZES, query_host_log};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn view_styles() -> &'static str {
    r#"<style>
.log-tabs{display:flex;flex-wrap:wrap;gap:8px;margin:0 0 14px;}
.log-tabs a{display:inline-flex;align-items:center;min-height:34px;padding:0 14px;border-radius:999px;
  border:1px solid var(--hairline);font-weight:700;font-size:13px;text-decoration:none;color:var(--ink);background:var(--canvas);}
.log-tabs a.active{background:#1d4ed8;border-color:#1d4ed8;color:#fff;}
.log-controls{display:flex;flex-wrap:wrap;gap:10px 14px;align-items:center;margin:0 0 12px;padding:10px 12px;
  border:1px solid var(--hairline);border-radius:10px;}
.log-controls form{display:flex;flex-wrap:wrap;gap:8px;align-items:center;margin:0;}
.log-controls label{font-size:13px;font-weight:600;}
.log-controls input[type=search]{min-height:34px;min-width:200px;border:1px solid #94a3b8;border-radius:8px;padding:4px 10px;font:inherit;color:var(--ink);background:var(--canvas);}
.log-controls select,.log-controls input[type=number]{min-height:34px;border:1px solid #94a3b8;border-radius:8px;padding:4px 8px;font:inherit;color:var(--ink);background:var(--canvas);}
.log-controls input[type=number]{width:84px;}
.log-pager{display:flex;flex-wrap:wrap;gap:8px;align-items:center;margin-left:auto;}
.log-pager a,.log-pager span.btn-off{display:inline-flex;align-items:center;min-height:34px;padding:0 14px;border-radius:999px;font-weight:700;font-size:13px;text-decoration:none;}
.log-pager a{background:#e2e8f0;color:#0f172a;}
.log-pager span.btn-off{background:#f1f5f9;color:#64748b;}
[data-color-mode="dark"] .log-pager a{background:#334155;color:#f8fafc;}
[data-color-mode="dark"] .log-pager span.btn-off{background:#1e293b;color:#94a3b8;}
[data-color-mode="dark"] .log-controls input[type=search],[data-color-mode="dark"] .log-controls select,
[data-color-mode="dark"] .log-controls input[type=number]{background:#1a1d26;border-color:#475569;color:#f1f5f9;}
.log-meta{margin:0 0 8px;font-size:13px;color:var(--muted);word-break:break-all;}
.log-view{margin:0;padding:12px 14px;border:1px solid var(--hairline);border-radius:10px;background:var(--canvas);
  font-size:12px;line-height:1.5;white-space:pre-wrap;word-break:break-all;overflow-wrap:anywhere;max-width:100%;}
.log-view .log-line{display:block;padding:2px 0;border-bottom:1px solid var(--hairline);}
.log-view .log-line:last-child{border-bottom:0;}
@media (max-width:719.98px){.log-pager{margin-left:0;width:100%;}.log-controls input[type=search]{min-width:0;flex:1 1 160px;}}
</style>"#
}

fn tabs(active: HostLogKind) -> String {
    let mut out = String::from(r#"<nav class="log-tabs" aria-label="Log sources">"#);
    out.push_str(r#"<a href="/server/logs">Overview</a>"#);
    for kind in HostLogKind::ALL {
        let class = if kind == active {
            r#" class="active" aria-current="page""#
        } else {
            ""
        };
        out.push_str(&format!(
            r#"<a{class} href="/server/logs/{slug}">{title}</a>"#,
            slug = kind.slug(),
            title = kind.title(),
        ));
    }
    out.push_str("</nav>");
    out
}

fn page_href(page: &LogPage, target: usize) -> String {
    let mut href = format!(
        "/server/logs/{}?page={}&per={}",
        page.kind.slug(),
        target,
        page.per_page
    );
    if !page.search.is_empty() {
        href.push_str("&q=");
        href.push_str(&urlencoding_simple(&page.search));
    }
    href
}

fn controls(page: &LogPage) -> String {
    let sizes: String = PAGE_SIZES
        .iter()
        .map(|n| {
            let sel = if *n == page.per_page { " selected" } else { "" };
            format!(r#"<option value="{n}"{sel}>{n}</option>"#)
        })
        .collect();
    let prev = if page.page > 1 {
        format!(
            r#"<a href="{}">Prev</a>"#,
            html_escape(&page_href(page, page.page - 1))
        )
    } else {
        r#"<span class="btn-off">Prev</span>"#.into()
    };
    let next = if page.page < page.total_pages {
        format!(
            r#"<a href="{}">Next</a>"#,
            html_escape(&page_href(page, page.page + 1))
        )
    } else {
        r#"<span class="btn-off">Next</span>"#.into()
    };
    format!(
        r#"<div class="log-controls" role="group" aria-label="Search and pagination">
  <form method="get" action="/server/logs/{slug}">
    <label for="log-q">Search</label>
    <input id="log-q" type="search" name="q" value="{q}" placeholder="Filter lines" maxlength="120" autocomplete="off">
    <label for="log-per">Show</label>
    <select id="log-per" name="per">{sizes}</select>
    <button type="submit" class="btn-primary">Apply</button>
  </form>
  <div class="log-pager">
    <strong>Page {page} / {pages}</strong>
    {prev}{next}
    <form method="get" action="/server/logs/{slug}">
      <input type="hidden" name="q" value="{q}">
      <input type="hidden" name="per" value="{per}">
      <label for="log-goto">Go to page</label>
      <input id="log-goto" type="number" name="page" min="1" max="{pages}" value="{page}" inputmode="numeric">
      <button type="submit" class="btn-primary">Go</button>
    </form>
  </div>
</div>"#,
        slug = page.kind.slug(),
        q = html_escape(&page.search),
        sizes = sizes,
        page = page.page,
        pages = page.total_pages,
        per = page.per_page,
        prev = prev,
        next = next,
    )
}

fn lines_view(page: &LogPage) -> String {
    if page.source.is_empty() {
        return format!(
            r#"<p class="empty-state">{}</p>"#,
            html_escape(page.kind.empty_hint())
        );
    }
    let meta = format!(
        r#"<p class="log-meta">Source: <code>{src}</code> &middot; {total} matching lines, newest first &middot; <a href="{refresh}">Refresh</a></p>"#,
        src = html_escape(&page.source),
        total = page.total_lines,
        refresh = html_escape(&page_href(page, page.page)),
    );
    if page.lines.is_empty() {
        let why = if page.search.is_empty() {
            "The log is empty."
        } else {
            "No lines match the search."
        };
        return format!(r#"{meta}<p class="empty-state">{why}</p>"#);
    }
    let mut body = String::from(r#"<div class="log-view" role="log">"#);
    for line in &page.lines {
        body.push_str(&format!(
            r#"<code class="log-line">{}</code>"#,
            html_escape(line)
        ));
    }
    body.push_str("</div>");
    format!("{meta}{body}")
}

/// Plain-language 403 for host-wide logs (they can contain other accounts' activity).
fn admin_only(kind: HostLogKind) -> String {
    format!(
        r#"{tabs}<p class="panel-notice error">{title} is host-wide and only available to the panel admin.</p><p class="muted">Per-site access and error logs are under Websites > Manage > Logs for sites you can manage.</p>"#,
        tabs = tabs(kind),
        title = html_escape(kind.title()),
    )
}

/// Full HTML (inside the panel shell) for `/server/logs/{kind}`.
pub fn host_log_page(
    username: &str,
    kind: HostLogKind,
    search: &str,
    page: usize,
    per_page: usize,
) -> String {
    let body = if is_panel_admin(username) {
        let data = query_host_log(kind, search, page, per_page);
        let mut body = format!("{}{}{}", view_styles(), tabs(kind), controls(&data));
        body.push_str(&lines_view(&data));
        if kind == HostLogKind::Panel {
            body.push_str(
                r#"<p class="muted" style="margin-top:14px;">Plugin and host package install, activate and uninstall actions are listed on the <a href="/server/logs">Logs overview</a>.</p>"#,
            );
        }
        body
    } else {
        format!("{}{}", view_styles(), admin_only(kind))
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("Logs", Some("/server/logs")),
            (kind.title(), None),
        ],
        kind.title(),
        kind.subtitle(),
        &body,
        None,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_admin_sees_admin_only_notice() {
        let html = host_log_page("not-the-admin-xyz", HostLogKind::Email, "", 1, 50);
        assert!(html.contains("only available to the panel admin"));
        assert!(!html.contains(r#"role="log""#));
    }

    #[test]
    fn tabs_mark_active_and_link_every_log() {
        let html = tabs(HostLogKind::Ftp);
        for kind in HostLogKind::ALL {
            assert!(html.contains(&format!("/server/logs/{}", kind.slug())));
        }
        assert_eq!(html.matches("aria-current=\"page\"").count(), 1);
    }

    #[test]
    fn empty_source_shows_honest_hint() {
        let page = crate::panel_server_logs::paginate_text(HostLogKind::ModSec, "", "", "", 1, 50);
        let html = lines_view(&page);
        assert!(html.contains("ModSecurity audit log"));
    }

    #[test]
    fn lines_are_escaped_and_pager_keeps_search() {
        let page = crate::panel_server_logs::paginate_text(
            HostLogKind::Error,
            "/usr/local/lsws/logs/error.log",
            "<script>alert(1)</script> a&b\nplain a&b",
            "a&b",
            1,
            25,
        );
        let html = lines_view(&page);
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
        assert!(controls(&page).contains("value=\"a&amp;b\""));
        assert!(page_href(&page, 2).contains("q=a%26b"));
    }
}
