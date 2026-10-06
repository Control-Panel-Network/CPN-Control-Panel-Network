//! Websites list Site preview cards (thumbnail + actions).

use crate::panel_user_prefs::load_user_minimalist_mode;
use crate::site_preview_list_cards::{preview_image_tag, site_card};
use crate::site_preview_thumb::load_meta;
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

/// Render the Sites list as preview cards.
pub fn site_preview_cards(
    sites: &[SiteRecord],
    show_docroots: bool,
    username: &str,
    list_next: &str,
) -> String {
    if sites.is_empty() {
        return r#"<p class="empty-state">No sites yet. Create one below or use <code>cpn site create</code>.</p>
        <p class="muted">New sites store files under the domain home (for example <code>/home/example.com/public_html</code>).</p>"#
            .into();
    }
    let minimalist = load_user_minimalist_mode(username);
    let auto_capture = !minimalist;
    let mut out = String::from(r#"<div class="site-cards">"#);
    for site in sites {
        out.push_str(&site_card(
            site,
            show_docroots,
            auto_capture,
            username,
            list_next,
        ));
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
            assert!(html.contains("/websites/manage?domain=manage-preview.example"));
        });
    }

    #[test]
    fn cards_pass_list_next_into_refresh_form() {
        crate::account::with_test_data_dir(|| {
            let site = SiteRecord {
                schema_version: 5,
                domain: "page-two.example".into(),
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
            let html = site_preview_cards(
                &[site],
                false,
                "Admin",
                "/websites/list?mode=page&per_page=5&page=2",
            );
            assert!(
                html.contains(
                    r#"name="next" value="/websites/list?mode=page&amp;per_page=5&amp;page=2""#
                ),
                "{html}"
            );
        });
    }
}
