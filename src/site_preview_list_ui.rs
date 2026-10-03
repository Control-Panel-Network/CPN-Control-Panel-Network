//! Websites list Site preview cards (thumbnail + actions).

use crate::panel_ops_ssl_public::{
    inspect_public_ssl, offers_origin_backup, origin_backup_form, ssl_list_badge_html,
};
use crate::panel_user_prefs::load_user_minimalist_mode;
use crate::site_preview_microlink::enabled_image_url;
use crate::site_preview_thumb::{PreviewFreshness, freshness, image_path, load_meta};
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
.site-ssl-badge.cf { background:rgba(59,130,246,.2); color:#93c5fd; }
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

fn site_action_buttons(site: &SiteRecord, show_origin_backup: bool) -> String {
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
    let backup = if show_origin_backup {
        let next = if crate::backups::is_subdomain_site(&site.domain) {
            "/subdomains"
        } else {
            "/websites"
        };
        origin_backup_form(&site.domain, next, "Issue origin backup")
    } else {
        String::new()
    };
    format!(
        r#"<div class="site-card-actions">
            <a class="btn-primary" style="min-height:36px;padding:0 14px;font-size:13px;" href="/websites/manage?domain={domain}">Manage</a>
            <a class="btn-secondary" style="min-height:36px;padding:0 12px;border-radius:999px;background:#f2f4f7;color:#344054;font-weight:700;display:inline-flex;align-items:center;font-size:13px;" href="/preview/{domain}/">Open preview</a>
            <a class="btn-secondary" style="min-height:36px;padding:0 12px;border-radius:999px;background:#f2f4f7;color:#344054;font-weight:700;display:inline-flex;align-items:center;font-size:13px;" href="/websites/manage?domain={domain}&amp;tab=files">File manager</a>
            {backup}
            {suspend}
            <form method="post" action="/websites/delete" class="inline-form" onsubmit="return confirm('Delete site {domain}? Document files under /home are kept.');">
              <input type="hidden" name="domain" value="{domain}">
              <button type="submit" class="btn-danger" style="min-height:36px;padding:0 12px;font-size:13px;">Delete</button>
            </form>
          </div>"#
    )
}

/// True when an authenticated cached shot exists for this domain.
fn cached_shot_present(domain: &str) -> bool {
    load_meta(domain).ok
        && image_path(domain)
            .map(|path| path.is_file())
            .unwrap_or(false)
}

/// Thumbnail `<img>` markup.
///
/// Cached local captures are served from the authenticated panel route. Without
/// a cached shot, public domains load the Microlink screenshot URL directly and
/// fall back to the panel placeholder when that request fails (labs without
/// public DNS, offline browser, or remote previews switched off).
fn preview_image_tag(domain_raw: &str, bust: u64, extra_style: &str) -> String {
    let domain = html_escape(domain_raw);
    let panel_src = format!("/websites/site-preview/image?domain={domain}&amp;v={bust}");
    let style = if extra_style.is_empty() {
        String::new()
    } else {
        format!(r#" style="{}""#, html_escape(extra_style))
    };
    if !cached_shot_present(domain_raw)
        && let Some(remote) = enabled_image_url(domain_raw, bust)
    {
        return format!(
            r#"<img src="{remote}" alt="Site preview for {domain}" loading="lazy" width="640" height="400"{style} data-preview-fallback="{panel_src}" onerror="if(this.dataset.previewFallback){{this.src=this.dataset.previewFallback;this.removeAttribute('data-preview-fallback');}}">"#,
            remote = html_escape(&remote),
        );
    }
    format!(
        r#"<img src="{panel_src}" alt="Site preview for {domain}" loading="lazy" width="640" height="400"{style}>"#
    )
}

fn preview_slot(site: &SiteRecord, auto_capture: bool) -> String {
    let domain = html_escape(&site.domain);
    let visit = public_site_url(&site.domain).unwrap_or_else(|_| format!("http://{}", site.domain));
    let visit_e = html_escape(&visit);
    let state = freshness(&site.domain);
    if auto_capture && matches!(state, PreviewFreshness::Missing | PreviewFreshness::Stale) {
        spawn_background_capture(site.domain.clone());
    }
    let meta = load_meta(&site.domain);
    let has_cached = cached_shot_present(&site.domain);
    let status_hint = if has_cached && state == PreviewFreshness::Fresh {
        "Cached thumbnail"
    } else if has_cached {
        "Cached thumbnail, refresh for a newer shot"
    } else if enabled_image_url(&site.domain, 0).is_some() {
        "Screenshot service thumbnail until a local capture is cached"
    } else if !meta.error.is_empty() {
        "Preview unavailable"
    } else {
        "Placeholder until capture finishes"
    };
    let bust = meta.captured_at;
    format!(
        r#"<div class="site-preview-slot">
  <div class="site-preview-frame">
    {img}
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
        img = preview_image_tag(&site.domain, bust, ""),
        visit = visit_e,
        hint = html_escape(status_hint),
    )
}

fn site_card(site: &SiteRecord, show_docroots: bool, auto_capture: bool, viewer: &str) -> String {
    let domain = html_escape(&site.domain);
    let status = if site.enabled { "Active" } else { "Suspended" };
    let wired = if site.vhost_wired {
        "vhost wired"
    } else {
        "files ready"
    };
    let ssl_view = inspect_public_ssl(&site.domain);
    let ssl = ssl_list_badge_html(&ssl_view);
    let disk_bytes = crate::panel_website_resources::approx_dir_bytes(
        std::path::Path::new(&site.docroot),
        4_000,
    );
    let disk_label = disk_bytes
        .map(|b| crate::panel_storage_fmt::format_bytes_for_user(viewer, b))
        .unwrap_or_else(|| "n/a".into());
    let bw = crate::panel_website_bandwidth::bandwidth_for_site(site, viewer);
    let bw_label = html_escape(&bw.label);
    let doc_meta = if show_docroots {
        format!(
            r#"<div><span>Document root</span><strong><details><summary>Show path</summary><code>{doc}</code></details></strong></div>"#,
            doc = html_escape(&site.docroot),
        )
    } else {
        String::new()
    };
    let parent_meta = match crate::sites::resolve_parent_domain(&site.domain)
        .ok()
        .flatten()
    {
        Some(parent) => format!(
            r#"<div><span>Parent</span><strong><a href="/websites/manage?domain={p}">{p}</a></strong></div>"#,
            p = html_escape(&parent),
        ),
        None => String::new(),
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
      <div><span>Disk</span><strong>{disk}</strong></div>
      <div><span>Bandwidth</span><strong>{bw}</strong></div>
      {parent_meta}
      {doc_meta}
    </div>
    {actions}
  </div>
</article>"#,
        preview = preview_slot(site, auto_capture),
        owner = html_escape(&site.owner),
        disk = html_escape(&disk_label),
        bw = bw_label,
        parent_meta = parent_meta,
        doc_meta = doc_meta,
        actions = site_action_buttons(site, offers_origin_backup(site, &ssl_view)),
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
        out.push_str(&site_card(site, show_docroots, auto_capture, username));
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
    let visit = public_site_url(&site.domain).unwrap_or_else(|_| format!("http://{}", site.domain));
    let meta = load_meta(&site.domain);
    format!(
        r#"<aside class="manage-site-preview" style="margin:0 0 14px;display:grid;grid-template-columns:minmax(140px,220px) 1fr;gap:14px;align-items:center;background:var(--m-card,#1b1e27);border:1px solid var(--m-line,#2a2f3a);border-radius:14px;padding:12px;">
  <div class="site-preview-frame" style="aspect-ratio:16/10;border-radius:10px;overflow:hidden;border:1px solid var(--m-line,#2a2f3a);">
    {img}
  </div>
  <div>
    <strong style="display:block;font-size:14px;">Site preview</strong>
    <p class="manage-muted" style="margin:6px 0 10px;">Cached homepage thumbnail (24h). Public domains fall back to the Microlink screenshot service until a local capture is cached; labs without public DNS may need Refresh with local vhost mapping.</p>
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
        img = preview_image_tag(
            &site.domain,
            meta.captured_at,
            "width:100%;height:100%;object-fit:cover;object-position:top center;display:block;",
        ),
        visit = html_escape(&visit),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styles_mention_site_preview_slot() {
        assert!(site_preview_list_styles().contains("site-preview-slot"));
        assert!(site_preview_list_styles().contains("site-preview-frame"));
    }

    #[test]
    fn public_domain_without_cache_uses_screenshot_service() {
        crate::account::with_test_data_dir(|| {
            let html = preview_image_tag("example.com", 0, "");
            assert!(html.contains("api.microlink.io"));
            assert!(html.contains("data-preview-fallback"));
            assert!(html.contains("/websites/site-preview/image?domain=example.com"));
        });
    }

    #[test]
    fn lab_domain_keeps_panel_route_only() {
        crate::account::with_test_data_dir(|| {
            let html = preview_image_tag("lab.test", 0, "");
            assert!(!html.contains("microlink"));
            assert!(html.contains("/websites/site-preview/image?domain=lab.test"));
        });
    }

    #[test]
    fn cached_local_capture_wins_over_remote() {
        crate::account::with_test_data_dir(|| {
            crate::site_preview_thumb::write_cached_image(
                "example.com",
                b"\x89PNG\r\n\x1a\nlocal-shot",
                "chromium",
            )
            .unwrap();
            let html = preview_image_tag("example.com", 42, "");
            assert!(!html.contains("microlink"));
            assert!(html.contains("v=42"));
        });
    }
}
