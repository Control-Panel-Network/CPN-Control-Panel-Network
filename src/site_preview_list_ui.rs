//! Websites list Site preview cards (thumbnail + actions).

use crate::panel_ops_ssl_public::{
    inspect_public_ssl, offers_origin_backup, origin_backup_form, ssl_list_badge_html,
};
use crate::panel_user_prefs::load_user_minimalist_mode;
use crate::site_preview_thumb::{PreviewFreshness, cached_shot_usable, freshness, load_meta};
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
  padding:14px; border:1px solid var(--hairline, #e0e0e0); border-radius:14px;
  background:var(--canvas, #fff); color:var(--ink, #1d1d1f); align-items:start;
}
@media (max-width:720px) {
  .site-card { grid-template-columns:1fr; }
}
.site-preview-slot { display:grid; gap:8px; min-width:0; }
.site-preview-frame {
  position:relative; aspect-ratio:16/10; border-radius:10px; overflow:hidden;
  border:1px solid var(--hairline, #e0e0e0); background:var(--surface-soft, #fafafc);
}
.site-preview-frame img {
  width:100%; height:100%; object-fit:cover; object-position:top center; display:block;
  background:var(--surface-soft, #fafafc);
}
.site-preview-actions { display:flex; flex-wrap:wrap; gap:8px; align-items:center; }
.site-preview-actions a, .site-preview-actions button {
  font-size:12px; font-weight:700; min-height:32px; padding:0 10px; border-radius:8px;
  border:1px solid var(--hairline, #e0e0e0); background:transparent; color:var(--ink, #1d1d1f);
  cursor:pointer; text-decoration:none; display:inline-flex; align-items:center;
}
.site-preview-actions .visit-link { color:var(--blue, #0066cc); border-color:transparent; padding:0; }
.site-card-main { display:grid; gap:10px; min-width:0; }
.site-card-head { display:flex; flex-wrap:wrap; gap:10px; align-items:center; justify-content:space-between; }
.site-card-head h3 {
  margin:0; font-size:18px; letter-spacing:-.02em; word-break:break-word;
  color:var(--ink, #1d1d1f);
}
.site-card .muted { color:var(--muted, #6e6e73); }
.site-ssl-badge {
  display:inline-flex; align-items:center; gap:4px; padding:3px 9px; border-radius:999px;
  font-size:10px; font-weight:800; letter-spacing:.04em; text-transform:uppercase;
  background:rgba(24,134,75,.14); color:var(--green, #18864b);
}
.site-ssl-badge.off { background:rgba(110,110,115,.12); color:var(--muted, #6e6e73); }
.site-ssl-badge.cf { background:rgba(0,102,204,.14); color:var(--blue, #0066cc); }
.site-meta-grid {
  display:grid; grid-template-columns:repeat(auto-fill,minmax(140px,1fr)); gap:8px;
}
.site-meta-grid div {
  border:1px solid var(--hairline, #e0e0e0); border-radius:10px; padding:8px 10px;
  background:var(--surface-soft, #fafafc);
}
.site-meta-grid span {
  display:block; font-size:11px; color:var(--muted, #6e6e73); font-weight:600;
}
.site-meta-grid strong {
  display:block; margin-top:4px; font-size:13px; word-break:break-word;
  color:var(--ink, #1d1d1f); font-weight:700;
}
.site-meta-grid a { color:var(--blue, #0066cc); }
.site-card-actions { display:flex; flex-wrap:wrap; gap:6px; justify-content:flex-start; }
[data-color-mode="dark"] .site-card .muted,
[data-color-mode="dark"] .site-meta-grid span {
  color:#b7c0cc;
}
[data-color-mode="dark"] .site-ssl-badge {
  background:rgba(18,183,106,.16); color:#6ce9a6;
}
[data-color-mode="dark"] .site-ssl-badge.off {
  background:rgba(152,162,179,.14); color:#98a2b3;
}
[data-color-mode="dark"] .site-ssl-badge.cf {
  background:rgba(59,130,246,.2); color:#93c5fd;
}
[data-color-mode="dark"] .site-preview-frame,
[data-color-mode="dark"] .site-preview-frame img {
  background:#0b0d12;
}
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
            <a class="btn-secondary" style="min-height:36px;padding:0 12px;border-radius:999px;background:#f2f4f7;color:#344054;font-weight:700;display:inline-flex;align-items:center;font-size:13px;" href="/preview/{domain}/" target="_blank" rel="noopener noreferrer">Open preview</a>
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
    cached_shot_usable(domain)
}

/// Thumbnail `<img>` markup uses the authenticated panel cache only.
/// Refresh preview captures locally so list pages do not spend remote quota.
fn preview_image_tag(domain_raw: &str, bust: u64, extra_style: &str) -> String {
    let domain = html_escape(domain_raw);
    let panel_src = format!("/websites/site-preview/image?domain={domain}&amp;v={bust}");
    let style = if extra_style.is_empty() {
        String::new()
    } else {
        format!(r#" style="{}""#, html_escape(extra_style))
    };
    format!(
        r#"<img src="{panel_src}" alt="Site preview for {domain}" loading="lazy" width="640" height="400"{style}>"#
    )
}

fn preview_slot(site: &SiteRecord, auto_capture: bool) -> String {
    let domain = html_escape(&site.domain);
    let visit = public_site_url(&site.domain).unwrap_or_else(|_| format!("http://{}", site.domain));
    let visit_e = html_escape(&visit);
    let state = freshness(&site.domain);
    let has_cached = cached_shot_present(&site.domain);
    // List loads: never N+1 remote APIs. Background fill is local Chromium only
    // and only when there is no usable disk cache yet.
    if auto_capture && !has_cached && matches!(state, PreviewFreshness::Missing) {
        spawn_background_capture(site.domain.clone());
    }
    let meta = load_meta(&site.domain);
    let status_hint = if has_cached && state == PreviewFreshness::Fresh {
        "Cached thumbnail (up to 7 days)"
    } else if has_cached {
        "Cached thumbnail, refresh for a newer shot"
    } else if !meta.error.is_empty() {
        "Preview unavailable"
    } else {
        "Placeholder until capture finishes"
    };
    let bust = meta.captured_at;
    let next = if crate::backups::is_subdomain_site(&site.domain) {
        "/subdomains"
    } else {
        "/websites"
    };
    format!(
        r#"<div class="site-preview-slot">
  <div class="site-preview-frame">
    {img}
  </div>
  <div class="site-preview-actions">
    <a class="visit-link" href="{visit}" target="_blank" rel="noopener noreferrer">Visit site</a>
    <form method="post" action="/websites/site-preview/refresh" class="inline-form">
      <input type="hidden" name="domain" value="{domain}">
      <input type="hidden" name="next" value="{next}">
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
    let disk_bytes = crate::panel_website_resources::site_used_bytes(site, 6_000);
    let pkg = crate::packages::package_for_account(&site.owner).ok();
    let disk_limit = pkg
        .as_ref()
        .map(|p| p.disk_mb)
        .unwrap_or(crate::packages::UNLIMITED);
    let disk_used = disk_bytes.unwrap_or(0);
    let disk_label =
        crate::panel_storage_fmt::format_used_limit_for_user(viewer, disk_used, disk_limit);
    let pkg_allow = crate::panel_storage_fmt::format_mb_limit_for_user(viewer, disk_limit);
    let disk_tip = format!(
        "Used storage for this site home and document root (not the whole account). Package disk allowance: {pkg_allow}."
    );
    let bw = crate::panel_website_bandwidth::bandwidth_for_site(site, viewer);
    let bw_label = html_escape(&bw.label);
    let bw_tip = html_escape(&bw.hint);
    let php = site
        .php_version
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .unwrap_or_else(crate::php_defaults::default_php_branch_for_sites);
    let package_name = pkg
        .as_ref()
        .map(|p| p.name.clone())
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "Default".into());
    let ip_meta = site
        .internal_ip
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|ip| {
            format!(
                r#"<div><span>IP</span><strong>{}</strong></div>"#,
                html_escape(ip)
            )
        })
        .unwrap_or_default();
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
      <div><span>Package</span><strong>{package}</strong></div>
      <div><span>PHP</span><strong>{php}</strong></div>
      {ip_meta}
      <div title="{disk_tip}"><span>Disk</span><strong>{disk}</strong></div>
      <div title="{bw_tip}"><span>Package bandwidth</span><strong>{bw}</strong></div>
      {parent_meta}
      {doc_meta}
    </div>
    {actions}
  </div>
</article>"#,
        preview = preview_slot(site, auto_capture),
        owner = html_escape(&site.owner),
        package = html_escape(&package_name),
        php = html_escape(&php),
        ip_meta = ip_meta,
        disk_tip = html_escape(&disk_tip),
        disk = html_escape(&disk_label),
        bw_tip = bw_tip,
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
    <p class="manage-muted" style="margin:6px 0 10px;">Cached homepage thumbnail (7 days). Refresh preview captures with a local headless browser. Public sites whose document root is still the CPN placeholder, or has no index file, are captured from the live URL.</p>
    <div style="display:flex;flex-wrap:wrap;gap:8px;">
      <a class="manage-btn" href="/preview/{domain}/" target="_blank" rel="noopener noreferrer">Open preview</a>
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
        let css = site_preview_list_styles();
        assert!(css.contains("site-preview-slot"));
        assert!(css.contains("site-preview-frame"));
        assert!(css.contains("background:var(--canvas, #fff)"));
        assert!(css.contains("color:var(--ink, #1d1d1f)"));
        assert!(css.contains(".site-card .muted"));
        assert!(css.contains("color:var(--muted, #6e6e73)"));
        assert!(!css.contains("var(--panel"));
    }

    #[test]
    fn public_domain_without_cache_uses_panel_route() {
        crate::account::with_test_data_dir(|| {
            let html = preview_image_tag("example.com", 0, "");
            assert!(!html.contains("api.microlink.io"));
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

    #[test]
    fn list_cards_show_package_php_and_used_of() {
        crate::account::with_test_data_dir(|| {
            let site = SiteRecord {
                schema_version: 5,
                domain: "card.example".into(),
                owner: "Admin".into(),
                docroot: "/tmp/cpn-card-missing/public_html".into(),
                enabled: true,
                engine: None,
                notes: String::new(),
                created_at_unix: 0,
                updated_at_unix: 0,
                vhost_wired: false,
                ssl: Default::default(),
                internal_ip: Some("10.0.2.15".into()),
                owner_suspend_message: String::new(),
                suspended_by: None,
                php_version: Some("8.5".into()),
                aliases: Vec::new(),
                staging_of: None,
            };
            let html = site_preview_cards(&[site], false, "Admin");
            assert!(html.contains("<span>Package</span>"), "{html}");
            assert!(html.contains("<span>PHP</span>"), "{html}");
            assert!(html.contains("8.5"), "{html}");
            assert!(html.contains("Package bandwidth"), "{html}");
            assert!(html.contains("Used "), "{html}");
            assert!(
                html.contains(
                    r#"href="/preview/card.example/" target="_blank" rel="noopener noreferrer""#
                ),
                "{html}"
            );
            assert!(html.contains("<span>IP</span>"), "{html}");
            assert!(!html.contains(" B<"), "{html}");
            assert!(!html.contains(" B</"), "{html}");
        });
    }

    #[test]
    fn list_does_not_spawn_when_cached_shot_exists() {
        crate::account::with_test_data_dir(|| {
            crate::site_preview_thumb::write_cached_image(
                "cached-list.example",
                b"\x89PNG\r\n\x1a\ncached",
                "chromium",
            )
            .unwrap();
            let site = SiteRecord {
                schema_version: 5,
                domain: "cached-list.example".into(),
                owner: "Admin".into(),
                docroot: "/tmp/cpn-card-missing/public_html".into(),
                enabled: true,
                engine: None,
                notes: String::new(),
                created_at_unix: 0,
                updated_at_unix: 0,
                vhost_wired: false,
                ssl: Default::default(),
                internal_ip: None,
                owner_suspend_message: String::new(),
                suspended_by: None,
                php_version: Some("8.5".into()),
                aliases: Vec::new(),
                staging_of: None,
            };
            let html = site_preview_cards(&[site], false, "Admin");
            assert!(html.contains("/websites/site-preview/image?domain=cached-list.example"));
            assert!(!html.contains("api.microlink.io"));
            assert!(html.contains("Cached thumbnail"));
        });
    }

    #[test]
    fn manage_overview_mentions_seven_day_cache() {
        crate::account::with_test_data_dir(|| {
            let site = SiteRecord {
                schema_version: 5,
                domain: "manage-preview.example".into(),
                owner: "Admin".into(),
                docroot: "/tmp/cpn-card-missing/public_html".into(),
                enabled: true,
                engine: None,
                notes: String::new(),
                created_at_unix: 0,
                updated_at_unix: 0,
                vhost_wired: false,
                ssl: Default::default(),
                internal_ip: None,
                owner_suspend_message: String::new(),
                suspended_by: None,
                php_version: None,
                aliases: Vec::new(),
                staging_of: None,
            };
            let html = manage_overview_preview(&site);
            assert!(html.contains("7 days"));
            assert!(!html.contains("24h"));
        });
    }
}
