//! Installed Plugins view: Host / Domain / Sub-domain with search and pagination.

use crate::panel_plugins_installed_data::{FlatItem, Scope, collect_installed_flat};
use crate::panel_plugins_markup::{
    domain_picker, html_escape, notice_block, resolve_domain, section_heading, urlencoding_simple,
    view_tabs,
};
use crate::panel_plugins_spa::{
    list_mode_from_query, page_from_query, per_page_from_query, store_list_toolbar,
};
use crate::sites::SiteRecord;

pub(crate) struct InstalledPageOpts<'a> {
    pub layout: &'a str,
    pub domain: &'a str,
    pub notice: Option<&'a str>,
    pub error: Option<&'a str>,
    pub sites: &'a [SiteRecord],
    pub username: &'a str,
    pub q: &'a str,
    pub category: &'a str,
    pub status: &'a str,
    pub mode: &'a str,
    pub page: usize,
    pub per_page: usize,
}

fn installed_pills(opts: &InstalledPageOpts<'_>, cats: &[String], domain: &str) -> String {
    let mut qs = format!(
        "&amp;mode={}&amp;per_page={}&amp;layout={}",
        urlencoding_simple(opts.mode),
        opts.per_page,
        urlencoding_simple(opts.layout),
    );
    if !domain.is_empty() {
        qs.push_str(&format!("&amp;domain={}", urlencoding_simple(domain)));
    }
    if !opts.q.trim().is_empty() {
        qs.push_str(&format!("&amp;q={}", urlencoding_simple(opts.q)));
    }
    if !opts.status.trim().is_empty() {
        qs.push_str(&format!("&amp;status={}", urlencoding_simple(opts.status)));
    }
    let mut out = String::from(r#"<div class="category-pills">"#);
    for (label, cat) in [("All", ""), ("Host", "Host"), ("Site", "Site")] {
        let cls = if cat.is_empty() {
            if opts.category.is_empty() || opts.category.eq_ignore_ascii_case("all") {
                "active"
            } else {
                ""
            }
        } else if opts.category.eq_ignore_ascii_case(cat) {
            "active"
        } else {
            ""
        };
        let href = if cat.is_empty() {
            format!("/plugins?view=installed{qs}")
        } else {
            format!(
                "/plugins?view=installed&amp;category={enc}{qs}",
                enc = urlencoding_simple(cat)
            )
        };
        out.push_str(&format!(
            r#"<a class="{cls}" href="{href}">{label}</a>"#,
            cls = cls,
            href = href,
            label = label,
        ));
    }
    for cat in cats {
        let cls = if cat.eq_ignore_ascii_case(opts.category) {
            "active"
        } else {
            ""
        };
        out.push_str(&format!(
            r#"<a class="{cls}" href="/plugins?view=installed&amp;category={enc}{qs}">{label}</a>"#,
            cls = cls,
            enc = urlencoding_simple(cat),
            qs = qs,
            label = html_escape(cat),
        ));
    }
    out.push_str("</div>");

    let mut status_qs = qs.clone();
    if !opts.category.trim().is_empty() {
        status_qs.push_str(&format!(
            "&amp;category={}",
            urlencoding_simple(opts.category)
        ));
    }
    out.push_str(r#"<div class="category-pills" style="margin-top:8px;">"#);
    for (label, st) in [
        ("All statuses", ""),
        ("Active", "active"),
        ("Deactivated", "deactivated"),
    ] {
        let cls = if st.is_empty() {
            if opts.status.is_empty() || opts.status.eq_ignore_ascii_case("all") {
                "active"
            } else {
                ""
            }
        } else if opts.status.eq_ignore_ascii_case(st) {
            "active"
        } else {
            ""
        };
        let href = if st.is_empty() {
            format!("/plugins?view=installed{status_qs}")
        } else {
            format!(
                "/plugins?view=installed&amp;status={enc}{status_qs}",
                enc = urlencoding_simple(st)
            )
        };
        out.push_str(&format!(
            r#"<a class="{cls}" href="{href}">{label}</a>"#,
            cls = cls,
            href = href,
            label = label,
        ));
    }
    out.push_str("</div>");
    out
}

fn render_scope_group(title: &str, blurb: &str, items: &[&FlatItem]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let mut cards = String::from(r#"<div class="plugin-grid">"#);
    for item in items {
        cards.push_str(&item.card_html);
    }
    cards.push_str("</div>");
    format!(
        r#"<section class="installed-scope" style="margin-top:22px;">
        <h2>{title}</h2>
        <p class="muted">{blurb}</p>
        {cards}
      </section>"#,
        title = html_escape(title),
        blurb = html_escape(blurb),
        cards = cards,
    )
}

fn empty_domain_sections(sites_empty: bool, total: usize, host_only_page: bool) -> String {
    if total == 0 {
        return String::new();
    }
    if sites_empty && host_only_page {
        return r#"<section class="installed-scope" style="margin-top:22px;"><h2>Domain</h2><p class="muted">Plugins installed for apex / primary domains.</p><p class="empty-state">No sites in this scope yet.</p></section>
        <section class="installed-scope" style="margin-top:22px;"><h2>Sub-domain</h2><p class="muted">Plugins installed for nested subdomain sites.</p><p class="empty-state">No sites in this scope yet.</p></section>"#.into();
    }
    String::new()
}

pub(crate) fn render_installed(opts: InstalledPageOpts<'_>) -> String {
    let layout = if opts.layout == "table" {
        "table"
    } else {
        "grid"
    };
    let domain = resolve_domain(opts.sites, opts.domain);
    let picker = domain_picker(opts.sites, &domain, "installed");
    let flat = collect_installed_flat(
        opts.sites,
        opts.username,
        opts.q,
        opts.category,
        opts.status,
    );
    let mut cats: Vec<String> = flat.iter().map(|i| i.category.clone()).collect();
    cats.sort_by_key(|c| c.to_ascii_lowercase());
    cats.dedup_by(|a, b| a.eq_ignore_ascii_case(b));

    let mode = list_mode_from_query(opts.mode);
    // Keep the user-selected page size for the toolbar/forms (same as Store).
    let page_size = per_page_from_query(&opts.per_page.to_string());
    let slice_size = if mode == "scroll" {
        flat.len().max(1)
    } else {
        page_size
    };
    let total = flat.len();
    let total_pages = if mode == "scroll" {
        1
    } else {
        total.div_ceil(page_size).max(1)
    };
    let page = page_from_query(&opts.page.to_string()).min(total_pages);
    let start = if mode == "scroll" {
        0
    } else {
        (page - 1) * page_size
    };
    let end = if mode == "scroll" {
        total
    } else {
        (start + slice_size).min(total)
    };
    let page_items: Vec<&FlatItem> = if total == 0 {
        Vec::new()
    } else {
        flat[start..end].iter().collect()
    };
    let host_items: Vec<&FlatItem> = page_items
        .iter()
        .copied()
        .filter(|i| i.scope == Scope::Host)
        .collect();
    let domain_items: Vec<&FlatItem> = page_items
        .iter()
        .copied()
        .filter(|i| i.scope == Scope::Domain)
        .collect();
    let sub_items: Vec<&FlatItem> = page_items
        .iter()
        .copied()
        .filter(|i| i.scope == Scope::Subdomain)
        .collect();

    let domain_q = if domain.is_empty() {
        String::new()
    } else {
        format!("&amp;domain={}", urlencoding_simple(&domain))
    };
    let toolbar = store_list_toolbar(mode, page_size, page, total_pages, total);
    let scroll_cls = if mode == "scroll" {
        "plugin-grid-scroll is-scroll"
    } else {
        "plugin-grid-scroll"
    };
    let empty = if total == 0 {
        r#"<p class="empty-state">No installed packages match this search or filter.</p>"#
    } else {
        ""
    };
    let host_only_page = !host_items.is_empty() && domain_items.is_empty() && sub_items.is_empty();
    // Show empty Domain/Sub placeholders on scroll or page 1 so scope stays clear.
    let show_empty_scopes = mode == "scroll" || page <= 1;

    format!(
        r#"{heading}
      {ok}
      {err}
      {tabs}
      <article class="section-card">
        <h2>Installed</h2>
        <p class="muted">Host packages stay visible even with zero websites. Site plugins live under <code>/home/&lt;domain&gt;/plugins/&lt;plugin-id&gt;/</code> (nested for subdomains). Badges show Active or Deactivated.</p>
        {picker}
        <form method="get" action="/plugins" class="plugin-search-row">
          <input type="hidden" name="view" value="installed">
          <input type="hidden" name="domain" value="{domain}">
          <input type="hidden" name="layout" value="{layout}">
          <input type="hidden" name="mode" value="{mode}">
          <input type="hidden" name="per_page" value="{page_size}">
          <input type="hidden" name="category" value="{category}">
          <input type="hidden" name="status" value="{status}">
          <label for="iq">Search installed</label>
          <input class="plugin-search" id="iq" name="q" type="search" value="{q}" placeholder="Search by name or id...">
          <button type="submit" class="btn-primary">Search</button>
        </form>
        {pills}
        {toolbar}
        <div class="plugin-tabs" style="margin-top:8px;">
          <a class="plugin-tab{grid}" href="/plugins?view=installed&amp;layout=grid{domain_q}&amp;q={qenc}&amp;category={catenc}&amp;status={stenc}&amp;mode={mode}&amp;per_page={page_size}&amp;page={page}">Grid view</a>
          <a class="plugin-tab{table}" href="/plugins?view=installed&amp;layout=table{domain_q}&amp;q={qenc}&amp;category={catenc}&amp;status={stenc}&amp;mode={mode}&amp;per_page={page_size}&amp;page={page}">Table view</a>
          <a class="plugin-tab" href="/plugins?view=store{domain_q}">Open Store</a>
        </div>
        <div class="{scroll_cls}">
          {empty}
          {host_sec}
          {domain_sec}
          {sub_sec}
          {empty_scopes}
        </div>
      </article>"#,
        heading = section_heading(
            "Plugins",
            "Installed host packages and site plugins, plus the CPN Store.",
        ),
        ok = notice_block("ok", opts.notice),
        err = notice_block("error", opts.error),
        tabs = view_tabs("installed", &domain),
        picker = picker,
        domain = html_escape(&domain),
        layout = html_escape(layout),
        mode = html_escape(mode),
        page_size = page_size,
        page = page,
        category = html_escape(opts.category),
        status = html_escape(opts.status),
        q = html_escape(opts.q),
        pills = installed_pills(&opts, &cats, &domain),
        grid = if layout == "grid" { " active" } else { "" },
        table = if layout == "table" { " active" } else { "" },
        domain_q = domain_q,
        qenc = urlencoding_simple(opts.q),
        catenc = urlencoding_simple(opts.category),
        stenc = urlencoding_simple(opts.status),
        toolbar = toolbar,
        scroll_cls = scroll_cls,
        empty = empty,
        host_sec = render_scope_group(
            "Host",
            "Host engines, webmail clients, and host-scoped catalog plugins.",
            &host_items,
        ),
        domain_sec = render_scope_group(
            "Domain",
            "Plugins installed for apex / primary domains.",
            &domain_items,
        ),
        sub_sec = render_scope_group(
            "Sub-domain",
            "Plugins installed for nested subdomain sites.",
            &sub_items,
        ),
        empty_scopes = if show_empty_scopes {
            empty_domain_sections(opts.sites.is_empty(), total, host_only_page)
        } else {
            String::new()
        },
    )
}
