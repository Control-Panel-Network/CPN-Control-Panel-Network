//! Websites list Site preview cards (thumbnail + actions).

use crate::panel_ops_ssl_inspect::{SslValidityKind, inspect_domain_ssl};
use crate::panel_user_prefs::load_user_minimalist_mode;
use crate::site_preview_thumb::{PreviewFreshness, freshness, load_meta};
use crate::site_preview_thumb_routes::spawn_background_capture;
use crate::sites::SiteRecord;
use crate::website_preview::public_site_url;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn site_preview_list_styles() -> &'static str {
    r#"
.site-cards { display:grid; gap:14px; }
.site-card {
  display:grid; grid-template-columns:minmax(160px,240px) 1fr; gap:16px;
  padding:14px; border:1px solid var(--hairline, #2a2f3a); border-radius:14px;
  background:var(--panel, #1a1d26); align-items:start;
}
@media (max-width:720px) {
  .site-card { grid-template-columns:1fr; }
}
.site-preview-slot { display:grid; gap:8px; min-width:0; }
.site-preview-frame {
  position:relative; aspect-ratio:16/10; border-radius:10px; overflow:hidden;
  border:1px solid var(--hairline, #2a2f3a); background:#0b0d12;
}
.site-preview-frame img {
  width:100%; height:100%; object-fit:cover; object-position:top center; display:block;
  background:#0b0d12;
}
.site-preview-actions { display:flex; flex-wrap:wrap; gap:8px; align-items:center; }
.site-preview-actions a, .site-preview-actions button {
  font-size:12px; font-weight:700; min-height:32px; padding:0 10px; border-radius:8px;
  border:1px solid var(--hairline, #2a2f3a); background:transparent; color:inherit;
  cursor:pointer; text-decoration:none; display:inline-flex; align-items:center;
}
.site-preview-actions .visit-link { color:#3b82f6; border-color:transparent; padding:0; }
.site-card-main { display:grid; gap:10px; min-width:0; }
.site-card-head { display:flex; flex-wrap:wrap; gap:10px; align-items:center; justify-content:space-between; }
.site-card-head h3 { margin:0; font-size:18px; letter-spacing:-.02em; word-break:break-word; }
.site-ssl-badge {
  display:inline-flex; align-items:center; gap:4px; padding:3px 9px; border-radius:999px;
  font-size:10px; font-weight:800; letter-spacing:.04em; text-transform:uppercase;
  background:rgba(18,183,106,.16); color:#6ce9a6;
}
.site-ssl-badge.off { background:rgba(152,162,179,.14); color:#98a2b3; }
.site-meta-grid {
  display:grid; grid-template-columns:repeat(auto-fill,minmax(120px,1fr)); gap:8px;
}
.site-meta-grid div {
  border:1px solid var(--hairline, #2a2f3a); border-radius:10px; padding:8px 10px;
}
.site-meta-grid span { display:block; font-size:11px; color:#98a2b3; font-weight:600; }
.site-meta-grid strong { display:block; margin-top:4px; font-size:13px; word-break:break-word; }
.site-card-actions { display:flex; flex-wrap:wrap; gap:6px; justify-content:flex-start; }
"#
}

fn site_action_buttons(site: &SiteRecord) -> String {
    let domain = html_escape(&site.domain);
    let suspend = if site.enabled {
        format!(
            r#"<form method="post" action="/websites/suspend" class="inline-form" onsubmit="return confirm('Suspend {domain}?');">
              <input type="hidden" name="domain" value="{domain}">
              <button type="submit" class="btn-warn" style="min-height:36px;padding:0 12px;border:0;border-radius:999px;background:#fffaeb;color:#b54708;font-weight:700;cursor:pointer;">Suspend</button>
            </form>"#
        )
    } else {
        format!(
            r#"<form method="post" action="/websites/resume" class="inline-form">
              <input type="hidden" name="domain" value="{domain}">
              <button type="submit" class="btn-secondary" style="min-height:36px;padding:0 12px;border:0;border-radius:999px;background:#f2f4f7;color:#344054;font-weight:700;cursor:pointer;">Resume</button>
            </form>"#
        )
    };
    format!(
        r#"<div class="site-card-actions">
            <a class="btn-primary" style="min-height:36px;padding:0 14px;font-size:13px;" href="/websites/manage?domain={domain}">Manage</a>
            <a class="btn-secondary" style="min-height:36px;padding:0 12px;border-radius:999px;background:#f2f4f7;color:#344054;font-weight:700;display:inline-flex;align-items:center;font-size:13px;" href="/preview/{domain}/">Open preview</a>
            <a class="btn-secondary" style="min-height:36px;padding:0 12px;border-radius:999px;background:#f2f4f7;color:#344054;font-weight:700;display:inline-flex;align-items:center;font-size:13px;" href="/websites/manage?domain={domain}&amp;tab=files">File manager</a>
            {suspend}
            <form method="post" action="/websites/delete" class="inline-form" onsubmit="return confirm('Delete site {domain}? Document files under /home are kept.');">
              <input type="hidden" name="domain" value="{domain}">
              <button type="submit" class="btn-danger" style="min-height:36px;padding:0 12px;font-size:13px;">Delete</button>
            </form>
          </div>"#
    )
}

fn preview_slot(site: &SiteRecord, auto_capture: bool) -> String {
    let domain = html_escape(&site.domain);
    let visit = public_site_url(&site.domain)
        .unwrap_or_else(|_| format!("http://{}", site.domain));
    let visit_e = html_escape(&visit);
    let state = freshness(&site.domain);
    if auto_capture && matches!(state, PreviewFreshness::Missing | PreviewFreshness::Stale) {
        spawn_background_capture(site.domain.clone());
    }
    let meta = load_meta(&site.domain);
    let status_hint = if state == PreviewFreshness::Fresh {
        "Cached thumbnail"
    } else if !meta.error.is_empty() {
        "Preview unavailable"
    } else {
        "Placeholder until capture finishes"
    };
    let bust = meta.captured_at;
    format!(
        r#"<div class="site-preview-slot">
  <div class="site-preview-frame">
    <img src="/websites/site-preview/image?domain={domain}&amp;v={bust}" alt="Site preview for {domain}" loading="lazy" width="640" height="400">
  </div>
  <div class="site-preview-actions">
    <a class="visit-link" href="{visit}" target="_blank" rel="noopener noreferrer">Visit site</a>
    <form method="post" action="/websites/site-preview/refresh" class="inline-form">
      <input type="hidden" name="domain" value="{domain}">
      <input type="hidden" name="next" value="/websites">
      <button type="submit" title="{hint}">Refresh preview</button>
    </form>
  </div>
</div>"#,
        visit = visit_e,
        hint = html_escape(status_hint),
    )
}

fn ssl_badge_html(domain: &str) -> String {
    let insight = inspect_domain_ssl(domain);
    let short = insight.kind.short_label();
    let tip = insight
        .expires_display
        .as_deref()
        .map(|d| format!("Expires {d}"))
        .unwrap_or_else(|| insight.detail.clone());
    let (bg, fg, class) = match insight.kind {
        SslValidityKind::Valid => ("rgba(18,183,106,.18)", "#6ce9a6", ""),
        SslValidityKind::ExpiringSoon => ("rgba(247,144,9,.2)", "#fdb022", ""),
        SslValidityKind::Expired | SslValidityKind::Invalid | SslValidityKind::Mismatch => {
            ("rgba(240,68,56,.18)", "#f97066", "")
        }
        SslValidityKind::None => ("rgba(152,162,179,.16)", "#98a2b3", " off"),
    };
    format!(
        r#"<span class="site-ssl-badge{class}" title="{tip}" style="background:{bg};color:{fg};">{short}</span>"#,
        tip = html_escape(&tip),
        short = html_escape(short),
        class = class,
        bg = bg,
        fg = fg,
    )
}

fn site_card(site: &SiteRecord, show_docroots: bool, auto_capture: bool) -> String {
    let domain = html_escape(&site.domain);
    let status = if site.enabled { "Active" } else { "Suspended" };
    let wired = if site.vhost_wired {
        "vhost wired"
    } else {
        "files ready"
    };
    let ssl = ssl_badge_html(&site.domain);
    let doc_meta = if show_docroots {
        format!(
            r#"<div><span>Document root</span><strong><details><summary>Show path</summary><code>{doc}</code></details></strong></div>"#,
            doc = html_escape(&site.docroot),
        )
    } else {
        String::new()
    };
    format!(
        r#"<article class="site-card">
  {preview}
  <div class="site-card-main">
    <div class="site-card-head">
      <h3>{domain}</h3>
      {ssl}
    </div>
    <p class="muted" style="margin:0;">{wired}</p>
    <div class="site-meta-grid">
      <div><span>State</span><strong>{status}</strong></div>
      <div><span>Owner</span><strong>{owner}</strong></div>
      {doc_meta}
    </div>
    {actions}
  </div>
</article>"#,
        preview = preview_slot(site, auto_capture),
        owner = html_escape(&site.owner),
        actions = site_action_buttons(site),
    )
}

/// Render the Sites list as preview cards.
pub fn site_preview_cards(sites: &[SiteRecord], show_docroots: bool, username: &str) -> String {
    if sites.is_empty() {
        return r#"<p class="empty-state">No sites yet. Create one below or use <code>cpn site create</code>.</p>
        <p class="muted">New sites store files under the domain home (for example <code>/home/example.com/public_html</code>).</p>"#
            .into();
    }
    let minimalist = load_user_minimalist_mode(username);
    let auto_capture = !minimalist;
    let mut out = String::from(r#"<div class="site-cards">"#);
    for site in sites {
        out.push_str(&site_card(site, show_docroots, auto_capture));
    }
    out.push_str("</div>");
    if minimalist {
        out.push_str(
            r#"<p class="muted" style="margin-top:10px;">Minimalist mode: Site preview captures run only when you click Refresh preview.</p>"#,
        );
    }
    out
}

/// Compact Site preview block for Manage Overview.
pub fn manage_overview_preview(site: &SiteRecord) -> String {
    let domain = html_escape(&site.domain);
    let visit = public_site_url(&site.domain)
        .unwrap_or_else(|_| format!("http://{}", site.domain));
    let meta = load_meta(&site.domain);
    format!(
        r#"<aside class="manage-site-preview" style="margin:0 0 14px;display:grid;grid-template-columns:minmax(140px,220px) 1fr;gap:14px;align-items:center;background:var(--m-card,#1b1e27);border:1px solid var(--m-line,#2a2f3a);border-radius:14px;padding:12px;">
  <div class="site-preview-frame" style="aspect-ratio:16/10;border-radius:10px;overflow:hidden;border:1px solid var(--m-line,#2a2f3a);">
    <img src="/websites/site-preview/image?domain={domain}&amp;v={bust}" alt="Site preview for {domain}" loading="lazy" width="640" height="400" style="width:100%;height:100%;object-fit:cover;object-position:top center;display:block;">
  </div>
  <div>
    <strong style="display:block;font-size:14px;">Site preview</strong>
    <p class="manage-muted" style="margin:6px 0 10px;">Cached homepage thumbnail (24h). Labs without public DNS may need Refresh with local vhost mapping.</p>
    <div style="display:flex;flex-wrap:wrap;gap:8px;">
      <a class="manage-btn" href="{visit}" target="_blank" rel="noopener noreferrer">Visit site</a>
      <form method="post" action="/websites/site-preview/refresh" class="inline-form">
        <input type="hidden" name="domain" value="{domain}">
        <input type="hidden" name="next" value="/websites/manage?domain={domain}&amp;tab=overview">
        <button type="submit" class="manage-btn">Refresh preview</button>
      </form>
    </div>
  </div>
</aside>"#,
        visit = html_escape(&visit),
        bust = meta.captured_at,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styles_mention_site_preview_slot() {
        assert!(site_preview_list_styles().contains("site-preview-slot"));
        assert!(!site_preview_list_styles().to_lowercase().contains("cyberpanel"));
    }
}
