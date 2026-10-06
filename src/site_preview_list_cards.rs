//! Site preview list cards (thumbnail slot, meta, actions).

use crate::backups::is_subdomain_site;
use crate::panel_ops_ssl_public::{
    inspect_public_ssl, offers_origin_backup, origin_backup_form, ssl_list_badge_html,
};
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

fn default_list_next(site: &SiteRecord) -> &'static str {
    if is_subdomain_site(&site.domain) {
        "/subdomains"
    } else {
        "/websites/list"
    }
}

fn return_next(list_next: &str, site: &SiteRecord) -> String {
    let trimmed = list_next.trim();
    if trimmed.is_empty() {
        default_list_next(site).to_string()
    } else {
        trimmed.to_string()
    }
}

fn site_action_buttons(site: &SiteRecord, show_origin_backup: bool, list_next: &str) -> String {
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
        origin_backup_form(
            &site.domain,
            &return_next(list_next, site),
            "Issue origin backup",
        )
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

fn cached_shot_present(domain: &str) -> bool {
    cached_shot_usable(domain)
}

pub(crate) fn preview_image_tag(domain_raw: &str, bust: u64, extra_style: &str) -> String {
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

fn preview_slot(site: &SiteRecord, auto_capture: bool, list_next: &str) -> String {
    let domain = html_escape(&site.domain);
    let visit = public_site_url(&site.domain).unwrap_or_else(|_| format!("http://{}", site.domain));
    let visit_e = html_escape(&visit);
    let state = freshness(&site.domain);
    let has_cached = cached_shot_present(&site.domain);
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
    let next = html_escape(&return_next(list_next, site));
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

pub(crate) fn site_card(
    site: &SiteRecord,
    show_docroots: bool,
    auto_capture: bool,
    viewer: &str,
    list_next: &str,
) -> String {
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
        preview = preview_slot(site, auto_capture, list_next),
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
        actions = site_action_buttons(site, offers_origin_backup(site, &ssl_view), list_next),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_site(domain: &str) -> SiteRecord {
        SiteRecord {
            schema_version: 5,
            domain: domain.into(),
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
        }
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
    fn refresh_next_keeps_page_and_search() {
        crate::account::with_test_data_dir(|| {
            let site = sample_site("card.example");
            let next = "/websites/list?q=blog&mode=page&per_page=5&page=2";
            let html = site_card(&site, false, false, "Admin", next);
            assert!(
                html.contains(r#"name="next" value="/websites/list?q=blog&amp;mode=page&amp;per_page=5&amp;page=2""#),
                "{html}"
            );
            assert!(html.contains("<span>Package</span>"), "{html}");
            assert!(html.contains("8.5"), "{html}");
            assert!(
                html.contains(
                    r#"href="/preview/card.example/" target="_blank" rel="noopener noreferrer""#
                ),
                "{html}"
            );
            assert!(!html.contains(" B<"), "{html}");
        });
    }

    #[test]
    fn subdomain_refresh_keeps_page() {
        crate::account::with_test_data_dir(|| {
            let site = sample_site("blog.example.com");
            let next = "/subdomains?mode=page&per_page=5&page=2";
            let html = preview_slot(&site, false, next);
            assert!(
                html.contains(
                    r#"name="next" value="/subdomains?mode=page&amp;per_page=5&amp;page=2""#
                ),
                "{html}"
            );
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
            let site = sample_site("cached-list.example");
            let html = preview_slot(&site, true, "/websites/list?page=2");
            assert!(html.contains("/websites/site-preview/image?domain=cached-list.example"));
            assert!(!html.contains("api.microlink.io"));
            assert!(html.contains("Cached thumbnail"));
            assert!(html.contains(r#"name="next" value="/websites/list?page=2""#));
        });
    }
}
