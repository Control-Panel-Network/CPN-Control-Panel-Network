//! List HTML for `/websites` and `/subdomains` (GET `q=` filter).

use crate::backups::is_subdomain_site;
use crate::panel_list_search::{
    domain_matches_q, list_filter_summary, list_search_form, normalize_list_q,
};
use crate::panel_prefs::load_panel_ui_prefs;
use crate::site_preview_list_ui::{site_preview_cards, site_preview_list_styles};
use crate::sites::{list_sites, resolve_parent_domain};

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

/// List Websites page body (apex / main domains only; create lives at `/websites/create`).
pub fn websites_main(
    username: &str,
    notice: Option<&str>,
    error: Option<&str>,
    q_raw: Option<&str>,
) -> String {
    let q = normalize_list_q(q_raw);
    let all: Vec<_> = list_sites()
        .unwrap_or_default()
        .into_iter()
        .filter(|site| !is_subdomain_site(&site.domain))
        .collect();
    let total = all.len();
    let sites: Vec<_> = all
        .into_iter()
        .filter(|site| domain_matches_q(&site.domain, None, &q))
        .collect();
    let prefs = load_panel_ui_prefs();
    let show = prefs.show_document_roots;
    let toggle_label = if show {
        "Hide document roots"
    } else {
        "Show document roots"
    };
    let toggle_value = if show { "0" } else { "1" };
    let remote = prefs.remote_site_previews;
    let remote_label = if remote {
        "Turn off screenshot service"
    } else {
        "Turn on screenshot service"
    };
    let remote_value = if remote { "0" } else { "1" };
    let remote_hint = if remote {
        "Public domains without a cached local capture load a thumbnail from the Microlink screenshot API, so the site hostname is sent to that service."
    } else {
        "Screenshot service is off: thumbnails come only from local captures on this host."
    };
    let rows = if total == 0 {
        r#"<p class="empty-state">No main websites yet. Create one below, or open <a href="/subdomains">Sub-domains</a> for nested sites.</p>
        <p class="muted">Main websites store files under the domain home (for example <code>/home/example.com/public_html</code>).</p>"#
            .to_string()
    } else if sites.is_empty() {
        format!(
            r#"<p class="empty-state">No websites match <strong>{}</strong>. <a href="/websites">Clear search</a>.</p>"#,
            html_escape(q_raw.unwrap_or("").trim())
        )
    } else {
        site_preview_cards(&sites, show, username)
    };
    format!(
        r#"<style>{preview_css}</style>
      {heading}
      {ok}
      {err}
      <article class="section-card">
        <h2>Websites ({count})</h2>
        <p class="muted">Main domains only. Sub-domains are listed under <a href="/subdomains">Sub-domains</a>. Each site shows a Site preview thumbnail, Manage, Visit, SSL status, and File manager.</p>
        <p class="muted">SSL: <strong>Valid</strong> is an origin cert on this host (expiry dd/mm/yyyy). <strong>CF SSL</strong> is Cloudflare CA or orange-cloud proxy with no origin files. <strong>NONE</strong> is neither. Issue origin backup for Let's Encrypt on this server.</p>
        <p style="margin:12px 0;"><a class="btn-primary" href="/websites/create">Create Website</a>
          <a class="btn-secondary" style="margin-left:8px;min-height:40px;padding:0 14px;border-radius:999px;background:#f2f4f7;color:#344054;font-weight:700;display:inline-flex;align-items:center;text-decoration:none;" href="/subdomains">List Sub-domains</a></p>
        {search}
        {filter_summary}
        <div style="display:flex;flex-wrap:wrap;gap:8px;margin:12px 0;">
          <form method="post" action="/websites/prefs" class="inline-form">
            <input type="hidden" name="show_document_roots" value="{toggle_value}">
            <button type="submit" class="btn-secondary" style="min-height:40px;padding:0 14px;border:0;border-radius:999px;background:#f2f4f7;color:#344054;font-weight:700;cursor:pointer;">{toggle_label}</button>
          </form>
          <form method="post" action="/websites/preview-prefs" class="inline-form">
            <input type="hidden" name="remote_site_previews" value="{remote_value}">
            <button type="submit" class="btn-secondary" style="min-height:40px;padding:0 14px;border:0;border-radius:999px;background:#f2f4f7;color:#344054;font-weight:700;cursor:pointer;" title="{remote_hint}">{remote_label}</button>
          </form>
        </div>
        <p class="muted" style="margin:0 0 12px;">{remote_hint}</p>
        {rows}
      </article>"#,
        preview_css = site_preview_list_styles(),
        heading = section_heading(
            "Websites",
            "Manage main website domains under /home. Sub-domains have their own list.",
        ),
        ok = notice_block("ok", notice),
        err = notice_block("error", error),
        count = sites.len(),
        search = list_search_form("/websites", q_raw.unwrap_or("").trim(), "Search by domain"),
        filter_summary = list_filter_summary(sites.len(), total, q_raw.unwrap_or("")),
        toggle_value = toggle_value,
        toggle_label = toggle_label,
        remote_value = remote_value,
        remote_label = remote_label,
        remote_hint = html_escape(remote_hint),
        rows = rows,
    )
}

/// List Sub-domains page body (nested FQDNs only; create lives at `/subdomains/create`).
pub fn subdomains_main(
    username: &str,
    notice: Option<&str>,
    error: Option<&str>,
    q_raw: Option<&str>,
) -> String {
    let q = normalize_list_q(q_raw);
    let all: Vec<_> = list_sites()
        .unwrap_or_default()
        .into_iter()
        .filter(|site| is_subdomain_site(&site.domain))
        .collect();
    let total = all.len();
    let sites: Vec<_> = all
        .into_iter()
        .filter(|site| {
            let parent = resolve_parent_domain(&site.domain).ok().flatten();
            domain_matches_q(&site.domain, parent.as_deref(), &q)
        })
        .collect();
    let prefs = load_panel_ui_prefs();
    let show = prefs.show_document_roots;
    let rows = if total == 0 {
        r#"<p class="empty-state">No sub-domains yet. Create one under an existing website, or open <a href="/websites">List Websites</a>.</p>
        <p class="muted">Sub-domains nest under the parent home (for example <code>/home/example.com/blog.example.com</code>).</p>"#
            .to_string()
    } else if sites.is_empty() {
        format!(
            r#"<p class="empty-state">No sub-domains match <strong>{}</strong>. <a href="/subdomains">Clear search</a>.</p>"#,
            html_escape(q_raw.unwrap_or("").trim())
        )
    } else {
        site_preview_cards(&sites, show, username)
    };
    format!(
        r#"<style>{preview_css}</style>
      {heading}
      {ok}
      {err}
      <article class="section-card">
        <h2>Sub-domains ({count})</h2>
        <p class="muted">Nested sites only. Main domains are listed under <a href="/websites">Websites</a>. Parent links appear on each card when a parent site exists.</p>
        <p style="margin:12px 0;"><a class="btn-primary" href="/subdomains/create">Create Sub-domain</a>
          <a class="btn-secondary" style="margin-left:8px;min-height:40px;padding:0 14px;border-radius:999px;background:#f2f4f7;color:#344054;font-weight:700;display:inline-flex;align-items:center;text-decoration:none;" href="/websites">List Websites</a></p>
        {search}
        {filter_summary}
        {rows}
      </article>"#,
        preview_css = site_preview_list_styles(),
        heading = section_heading(
            "Sub-domains",
            "Manage nested sites under an existing parent domain.",
        ),
        ok = notice_block("ok", notice),
        err = notice_block("error", error),
        count = sites.len(),
        search = list_search_form(
            "/subdomains",
            q_raw.unwrap_or("").trim(),
            "Search by domain or parent",
        ),
        filter_summary = list_filter_summary(sites.len(), total, q_raw.unwrap_or("")),
        rows = rows,
    )
}
