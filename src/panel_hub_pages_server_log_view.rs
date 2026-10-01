//! Server > Logs viewer pages: one searchable, paginated page per host log.
//!
//! The controls reuse the Activity Board list classes (search, matching count, per-page, page
//! indicator, Prev/Next, Go to page) and entries use the Overview (Panel activity) table markup
//! inside an `.activity-list` container, so every tab looks the same: a table on wide panels and
//! stacked labelled cards (Time / Source / Level / Details) on narrow ones, with no horizontal
//! scrolling on any screen size.

use crate::panel_admin::is_panel_admin;
use crate::panel_hub_http::urlencoding_simple;
use crate::panel_hubs::feature_shell;
use crate::panel_server_logs::{
    HostLogKind, LogPage, PAGE_SIZES, Scope, query_host_log_scoped, scope_for,
};
use crate::panel_server_logs_rows::{rows_styles, rows_table};
use crate::panel_server_logs_tabs::log_tabs;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn view_styles() -> &'static str {
    r#"<style>
.log-controls a.btn-secondary,.log-controls span.btn-secondary{display:inline-flex;align-items:center;text-decoration:none;}
.log-controls span.btn-secondary.is-disabled{opacity:.55;cursor:not-allowed;}
.log-controls form{margin:0;}
.log-meta{margin:0 0 10px;font-size:13px;color:var(--muted);overflow-wrap:anywhere;}
.log-meta details{display:inline;}
.log-meta ul{margin:6px 0 0;padding-left:18px;}
</style>"#
}

fn tabs(active: HostLogKind) -> String {
    log_tabs(Some(active))
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

fn pager_button(page: &LogPage, label: &str, target: Option<usize>) -> String {
    match target {
        Some(t) => format!(
            r#"<a class="btn-secondary" href="{}">{label}</a>"#,
            html_escape(&page_href(page, t))
        ),
        None => format!(
            r#"<span class="btn-secondary is-disabled" aria-disabled="true">{label}</span>"#
        ),
    }
}

fn controls(page: &LogPage) -> String {
    let sizes: String = PAGE_SIZES
        .iter()
        .map(|n| {
            let sel = if *n == page.per_page { " selected" } else { "" };
            format!(r#"<option value="{n}"{sel}>{n}</option>"#)
        })
        .collect();
    let prev = pager_button(page, "Prev", (page.page > 1).then(|| page.page - 1));
    let next = pager_button(
        page,
        "Next",
        (page.page < page.total_pages).then(|| page.page + 1),
    );
    format!(
        r#"<div class="activity-list-controls log-controls" role="group" aria-label="Search and pagination">
  <form id="log-form" class="activity-list-search-wrap" method="get" action="/server/logs/{slug}">
    <label for="log-q">Search</label>
    <input id="log-q" type="search" class="activity-list-search" name="q" value="{q}" placeholder="Filter time, site or text" maxlength="120" autocomplete="off">
    <button type="submit" class="btn-primary">Apply</button>
    <span class="activity-list-match">{total} matching</span>
  </form>
  <div class="activity-list-pager">
    <label for="log-per">Show
      <select id="log-per" name="per" form="log-form" aria-label="Lines per page">{sizes}</select>
      per page
    </label>
    <span class="activity-list-status">Page {page} / {pages}</span>
    {prev}{next}
    <form class="activity-list-goto" method="get" action="/server/logs/{slug}">
      <input type="hidden" name="q" value="{q}">
      <input type="hidden" name="per" value="{per}">
      <label for="log-goto">Go to page
        <input id="log-goto" class="activity-list-goto-input" type="number" name="page" min="1" max="{pages}" value="{page}" inputmode="numeric">
      </label>
      <button type="submit" class="btn-primary">Go</button>
    </form>
  </div>
</div>
<script>(function(){{var s=document.getElementById('log-per');if(s){{s.addEventListener('change',function(){{if(s.form)s.form.submit();}});}}}})();</script>"#,
        slug = page.kind.slug(),
        q = html_escape(&page.search),
        sizes = sizes,
        total = page.total_lines,
        page = page.page,
        pages = page.total_pages,
        per = page.per_page,
        prev = prev,
        next = next,
    )
}

fn meta(page: &LogPage, scoped_to_sites: bool) -> String {
    let sources = if page.sources.len() == 1 {
        format!("Source: <code>{}</code>", html_escape(&page.sources[0]))
    } else {
        let items: String = page
            .sources
            .iter()
            .map(|s| format!("<li><code>{}</code></li>", html_escape(s)))
            .collect();
        format!(
            "<details><summary>{} sources</summary><ul>{items}</ul></details>",
            page.sources.len()
        )
    };
    let who = if scoped_to_sites {
        " Showing only the websites you manage."
    } else {
        ""
    };
    format!(
        r#"<p class="log-meta">{sources} &middot; newest first &middot; times are shown as written in the log (server time, dd/mm/yyyy 24h) &middot; <a href="{refresh}">Refresh</a>.{who}</p>"#,
        refresh = html_escape(&page_href(page, page.page)),
    )
}

fn lines_view(page: &LogPage, scoped_to_sites: bool) -> String {
    if !page.found() {
        return format!(
            r#"<p class="empty-state">{}</p>"#,
            html_escape(page.kind.empty_hint())
        );
    }
    let meta = meta(page, scoped_to_sites);
    if page.entries.is_empty() {
        let why = if !page.search.is_empty() {
            "No lines match the search."
        } else if !page.note.is_empty() {
            page.note.as_str()
        } else {
            page.kind.quiet_hint()
        };
        return format!(r#"{meta}<p class="empty-state">{}</p>"#, html_escape(why));
    }
    format!(
        r#"{meta}<div role="log" aria-label="{} entries">{table}</div>"#,
        page.kind.title(),
        table = rows_table(page.kind, &page.entries),
    )
}

/// Plain-language 403 for host-wide logs (they can contain other accounts' activity).
fn admin_only(kind: HostLogKind) -> String {
    format!(
        r#"{tabs}<p class="panel-notice error">{title} is host-wide and only available to the panel admin.</p><p class="muted">Per-site access and error logs are under Access Logs and Error Logs for the sites you manage, and under Websites > Manage > Logs.</p>"#,
        tabs = tabs(kind),
        title = html_escape(kind.title()),
    )
}

fn viewer_body(
    username: &str,
    kind: HostLogKind,
    search: &str,
    page: usize,
    per_page: usize,
) -> String {
    let admin = is_panel_admin(username);
    if !admin && !kind.per_site() {
        return format!("{}{}", view_styles(), admin_only(kind));
    }
    let scope: Scope = scope_for(username);
    let data = query_host_log_scoped(kind, &scope, search, page, per_page);
    let mut body = format!(
        r#"{}{}{}<div class="activity-list log-list">{}"#,
        view_styles(),
        rows_styles(),
        tabs(kind),
        controls(&data)
    );
    body.push_str(&lines_view(&data, !admin));
    body.push_str("</div>");
    if kind == HostLogKind::Panel {
        body.push_str(
            r#"<p class="muted" style="margin-top:14px;">Plugin and host package install, activate and uninstall actions are listed on the <a href="/server/logs">Logs overview</a>.</p>"#,
        );
    }
    body
}

/// Full HTML (inside the panel shell) for `/server/logs/{kind}`.
pub fn host_log_page(
    username: &str,
    kind: HostLogKind,
    search: &str,
    page: usize,
    per_page: usize,
) -> String {
    let body = viewer_body(username, kind, search, page, per_page);
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
    use crate::panel_server_logs::{Collected, paginate, paginate_text};

    #[test]
    fn non_admin_sees_admin_only_notice_for_host_wide_logs() {
        let html = host_log_page("not-the-admin-xyz", HostLogKind::Email, "", 1, 10);
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
    fn missing_source_shows_modsec_not_installed_hint() {
        let page = paginate(HostLogKind::ModSec, &Collected::default(), "", 1, 10);
        let html = lines_view(&page, false);
        assert!(html.contains("ModSecurity is not installed"));
    }

    #[test]
    fn empty_source_shows_quiet_hint_not_install_hint() {
        let mut c = Collected::default();
        c.sources.push("/var/log/x.log".into());
        let page = paginate(HostLogKind::Access, &c, "", 1, 10);
        let html = lines_view(&page, false);
        assert!(html.contains("empty so far"));
        assert!(!html.contains("No access log found"));
    }

    #[test]
    fn note_explains_an_existing_but_idle_source() {
        let mut c = Collected::default();
        c.sources.push("ModSecurity module detected".into());
        c.note = "Module installed but not enabled.".into();
        let page = paginate(HostLogKind::ModSec, &c, "", 1, 10);
        assert!(lines_view(&page, false).contains("not enabled"));
        let searched = paginate(HostLogKind::ModSec, &c, "zzz", 1, 10);
        assert!(lines_view(&searched, false).contains("No lines match"));
    }

    #[test]
    fn lines_are_escaped_and_pager_keeps_search() {
        let page = paginate_text(
            HostLogKind::Error,
            "/usr/local/lsws/logs/error.log",
            "<script>alert(1)</script> a&b\nplain a&b",
            "a&b",
            1,
            25,
        );
        let html = lines_view(&page, false);
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
        assert!(controls(&page).contains("value=\"a&amp;b\""));
        assert!(page_href(&page, 2).contains("q=a%26b"));
    }

    #[test]
    fn controls_have_every_required_element_and_default_ten() {
        let text = (1..=35)
            .map(|i| format!("l{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let page = paginate_text(HostLogKind::Panel, "/x", &text, "", 2, 10);
        let html = controls(&page);
        for needle in [
            "type=\"search\"",
            "35 matching",
            "Page 2 / 4",
            ">Prev<",
            ">Next<",
            "Go to page",
            ">Go<",
            "value=\"10\" selected",
        ] {
            assert!(html.contains(needle), "missing {needle}");
        }
        assert!(html.contains("/server/logs/panel?page=1&amp;per=10"));
        assert!(html.contains("/server/logs/panel?page=3&amp;per=10"));
    }

    #[test]
    fn first_page_disables_prev_and_last_page_disables_next() {
        let page = paginate_text(HostLogKind::Panel, "/x", "a\nb", "", 1, 10);
        let html = controls(&page);
        assert!(html.contains("is-disabled\" aria-disabled=\"true\">Prev"));
        assert!(html.contains("is-disabled\" aria-disabled=\"true\">Next"));
    }

    #[test]
    fn entries_show_norwegian_time_scope_and_fold_long_lines() {
        let long = format!("[01/Oct/2026:14:05:09 +0200] {}", "x".repeat(400));
        let mut c = Collected::default();
        c.sources.push("s".into());
        c.push_text("example.com", &long);
        let page = paginate(HostLogKind::Access, &c, "", 1, 10);
        let html = lines_view(&page, false);
        assert!(html.contains("01/10/2026 14:05:09"));
        assert!(html.contains("log-scope\">example.com"));
        assert!(html.contains("Show full line"));
        // Same table markup as the Overview (Panel activity), with labelled cells.
        assert!(html.contains(r#"<table class="data-table">"#));
        assert!(html.contains(r#"data-label="Details""#));
        assert!(html.contains(r#"data-label="Site / source""#));
    }

    #[test]
    fn viewers_default_to_five_per_page_and_keep_larger_choices() {
        let text = (1..=12)
            .map(|i| format!("l{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let page = paginate_text(
            HostLogKind::Email,
            "/x",
            &text,
            "",
            1,
            crate::panel_server_logs::DEFAULT_PER_PAGE,
        );
        assert_eq!(page.per_page, 5);
        assert_eq!(page.total_pages, 3);
        let html = controls(&page);
        assert!(html.contains(r#"<option value="5" selected>5</option>"#));
        for n in [10, 25, 50, 100, 200] {
            assert!(html.contains(&format!(r#"<option value="{n}">{n}</option>"#)));
        }
        assert!(html.contains("Page 1 / 3"));
    }

    #[test]
    fn search_and_pager_chrome_matches_the_overview_classes() {
        let page = paginate_text(HostLogKind::Error, "/x", "a\nb", "", 1, 10);
        let html = controls(&page);
        for class in [
            "activity-list-controls",
            "activity-list-search",
            "activity-list-match",
            "activity-list-pager",
            "activity-list-status",
            "activity-list-goto",
        ] {
            assert!(html.contains(class), "missing {class}");
        }
    }
}
