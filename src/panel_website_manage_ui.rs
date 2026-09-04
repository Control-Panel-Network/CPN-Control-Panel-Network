//! Manage dashboard styles and chrome (banner, quick actions, tabs).

use crate::sites::SiteRecord;
use crate::website_preview::{preview_mode_url, public_site_url, ssl_material_present};

pub fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn manage_styles() -> &'static str {
    r#"
.site-manage { --m-bg:#12141a; --m-card:#1b1e27; --m-ink:#f2f4f7; --m-muted:#98a2b3;
  --m-accent:#3b82f6; --m-ok:#12b76a; --m-warn:#f79009; --m-line:#2a2f3a; color:var(--m-ink); }
.site-manage a { color:var(--m-accent); }
.site-manage .manage-banner {
  position:relative; overflow:hidden; border-radius:18px; padding:28px 26px 24px;
  background:linear-gradient(135deg,#1e293b 0%,#0f172a 55%,#1d4ed8 140%);
  border:1px solid rgba(255,255,255,.08); margin-bottom:14px;
}
.site-manage .manage-banner h1 { margin:0; font-size:28px; letter-spacing:-.03em; }
.site-manage .manage-banner p { margin:8px 0 0; color:rgba(242,244,247,.82); max-width:52ch; }
.site-manage .manage-badge {
  display:inline-flex; align-items:center; margin-left:10px; padding:3px 10px;
  border-radius:999px; font-size:11px; font-weight:800; letter-spacing:.04em;
  vertical-align:middle;
}
.site-manage .manage-badge.active { background:rgba(18,183,106,.18); color:#6ce9a6; }
.site-manage .manage-badge.suspended { background:rgba(247,144,9,.18); color:#fdb022; }
.site-manage .manage-banner-actions { display:flex; flex-wrap:wrap; gap:10px; margin-top:18px; }
.site-manage .manage-btn {
  display:inline-flex; align-items:center; gap:8px; min-height:40px; padding:0 14px;
  border-radius:10px; border:1px solid rgba(255,255,255,.14); background:rgba(255,255,255,.08);
  color:var(--m-ink); font-weight:700; font-size:13px; text-decoration:none;
}
.site-manage .manage-btn.primary { background:var(--m-accent); border-color:transparent; color:#fff; }
.site-manage .manage-quick {
  display:flex; flex-wrap:wrap; gap:8px; margin:0 0 14px;
}
.site-manage .manage-quick a, .site-manage .manage-quick span {
  display:inline-flex; align-items:center; gap:6px; min-height:36px; padding:0 12px;
  border-radius:10px; border:1px solid var(--m-line); background:var(--m-card);
  color:var(--m-ink); font-size:12px; font-weight:600; text-decoration:none;
}
.site-manage .manage-quick .scaffold { opacity:.72; cursor:default; }
.site-manage .manage-tabs {
  display:flex; flex-wrap:wrap; gap:4px; border-bottom:1px solid var(--m-line);
  margin:0 0 18px; padding:0;
}
.site-manage .manage-tabs a {
  display:inline-flex; align-items:center; min-height:40px; padding:0 14px;
  color:var(--m-muted); text-decoration:none; font-weight:700; font-size:13px;
  border-bottom:2px solid transparent; margin-bottom:-1px;
}
.site-manage .manage-tabs a.active { color:var(--m-ink); border-bottom-color:var(--m-accent); }
.site-manage .manage-card-grid {
  display:grid; grid-template-columns:repeat(auto-fill,minmax(180px,1fr)); gap:12px; margin-bottom:14px;
}
.site-manage .manage-stat {
  background:var(--m-card); border:1px solid var(--m-line); border-radius:14px; padding:14px 14px 12px;
}
.site-manage .manage-stat span { display:block; color:var(--m-muted); font-size:12px; font-weight:600; }
.site-manage .manage-stat strong { display:block; margin-top:6px; font-size:20px; letter-spacing:-.02em; }
.site-manage .manage-stat .bar {
  margin-top:10px; height:6px; border-radius:999px; background:#2a2f3a; overflow:hidden;
}
.site-manage .manage-stat .bar > i { display:block; height:100%; background:var(--m-accent); border-radius:999px; }
.site-manage .manage-ssl {
  display:flex; flex-wrap:wrap; align-items:center; gap:12px; justify-content:space-between;
  background:var(--m-card); border:1px solid var(--m-line); border-radius:14px; padding:16px;
  margin-bottom:14px;
}
.site-manage .manage-ssl strong { display:block; font-size:15px; }
.site-manage .manage-ssl p { margin:4px 0 0; color:var(--m-muted); font-size:13px; }
.site-manage .manage-charts {
  display:grid; grid-template-columns:repeat(auto-fit,minmax(260px,1fr)); gap:12px;
}
.site-manage .manage-chart {
  background:var(--m-card); border:1px solid var(--m-line); border-radius:14px; padding:14px;
}
.site-manage .manage-chart h3 { margin:0 0 4px; font-size:13px; letter-spacing:.04em; text-transform:uppercase; color:var(--m-muted); }
.site-manage .manage-chart p { margin:0 0 10px; color:var(--m-muted); font-size:12px; }
.site-manage .manage-chart svg { width:100%; height:88px; display:block; }
.site-manage .manage-section-title {
  margin:22px 0 10px; font-size:12px; font-weight:800; letter-spacing:.08em;
  text-transform:uppercase; color:var(--m-muted); border-left:3px solid var(--m-accent); padding-left:10px;
}
.site-manage .manage-tile-grid {
  display:grid; grid-template-columns:repeat(auto-fill,minmax(200px,1fr)); gap:12px;
}
.site-manage .manage-tile {
  display:flex; align-items:center; gap:12px; min-height:78px; padding:14px;
  border-radius:14px; background:var(--m-card); border:1px solid var(--m-line);
  color:inherit; text-decoration:none;
}
.site-manage .manage-tile:hover { border-color:#3b82f6; }
.site-manage .manage-tile-icon {
  width:40px; height:40px; flex:0 0 40px; border-radius:12px; background:#243047;
  border:1px solid #334155;
}
.site-manage .manage-tile strong { display:block; font-size:14px; }
.site-manage .manage-tile span { display:block; color:var(--m-muted); font-size:12px; margin-top:2px; }
.site-manage .manage-muted { color:var(--m-muted); font-size:13px; }
.site-manage .manage-log-pre {
  max-height:360px; overflow:auto; background:#0b0d12; border:1px solid var(--m-line);
  border-radius:12px; padding:12px; font-size:12px; line-height:1.45; white-space:pre-wrap;
}
.site-manage .manage-log-panel { margin-bottom:16px; }
.site-manage .manage-log-panel h3 { margin:0 0 6px; font-size:16px; }
.site-manage code { background:#0b0d12; padding:1px 6px; border-radius:6px; font-size:12px; }
.site-manage .manage-actions-row { display:flex; flex-wrap:wrap; gap:8px; margin:12px 0; }
.site-manage .btn-danger, .site-manage .btn-warn, .site-manage .btn-primary {
  min-height:36px; padding:0 12px; border-radius:999px; border:0; font-weight:700; cursor:pointer;
}
.site-manage .btn-primary { background:var(--m-accent); color:#fff; }
.site-manage .btn-warn { background:#3a2a12; color:#fdb022; }
.site-manage .btn-danger { background:#3f1d22; color:#fda29b; }
.site-manage .inline-form { display:inline; }
@media (max-width:720px) {
  .site-manage .manage-banner h1 { font-size:22px; }
}
"#
}

pub fn notice_block(kind: &str, message: Option<&str>) -> String {
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

pub fn manage_banner(site: &SiteRecord) -> String {
    let status = if site.enabled { "Active" } else { "Suspended" };
    let badge_class = if site.enabled { "active" } else { "suspended" };
    let preview = preview_mode_url(&site.domain).unwrap_or_else(|_| "#".into());
    let domain_q = html_escape(&site.domain);
    format!(
        r#"<div class="manage-banner">
  <h1>{domain}<span class="manage-badge {badge}">{status}</span></h1>
  <p>Manage your website with powerful tools and real-time monitoring.</p>
  <div class="manage-banner-actions">
    <a class="manage-btn primary" href="{preview}">Preview Website</a>
    <a class="manage-btn" href="/websites/manage?domain={domain_q}&amp;tab=files">File Manager</a>
  </div>
</div>"#,
        domain = html_escape(&site.domain),
        badge = badge_class,
        status = status,
        preview = html_escape(&preview),
        domain_q = domain_q,
    )
}

pub fn quick_actions(site: &SiteRecord) -> String {
    let domain_q = html_escape(&site.domain);
    let home = crate::sites::site_home_from_record(site);
    let ssh_hint = html_escape(&format!(
        "SSH/SFTP: use the site owner account. Docroot: {}",
        site.docroot
    ));
    format!(
        r#"<div class="manage-quick" aria-label="Quick actions">
  <span class="scaffold" title="Web terminal ships later">Open Terminal</span>
  <span class="scaffold" title="Git manager ships later">Manage Git</span>
  <span class="scaffold" title="Clone/staging ships later">Clone/Staging</span>
  <a href="/websites/manage?domain={domain_q}&amp;tab=config" title="{ssh_hint}">SSH/SFTP Access</a>
  <a href="/websites/manage?domain={domain_q}&amp;tab=domains">Cron Jobs</a>
  <span class="scaffold" title="Stress test ships later">Stress Test</span>
  <span class="manage-muted" style="align-self:center;margin-left:4px;">Home: <code>{home}</code></span>
</div>"#,
        domain_q = domain_q,
        ssh_hint = ssh_hint,
        home = html_escape(&home.display().to_string()),
    )
}

pub fn tab_bar(domain: &str, active: &str) -> String {
    let tabs = [
        ("overview", "Overview"),
        ("domains", "Domains"),
        ("logs", "Logs"),
        ("config", "Config"),
        ("ssl", "SSL"),
        ("files", "Files"),
        ("apps", "Apps"),
    ];
    let domain_q = html_escape(domain);
    let mut out = String::from(r#"<nav class="manage-tabs" aria-label="Website sections">"#);
    for (id, label) in tabs {
        let class = if id == active {
            " class=\"active\""
        } else {
            ""
        };
        out.push_str(&format!(
            r#"<a href="/websites/manage?domain={domain_q}&amp;tab={id}"{class}>{label}</a>"#
        ));
    }
    out.push_str("</nav>");
    out
}

pub fn ssl_status_card(site: &SiteRecord) -> String {
    let has = ssl_material_present(&site.domain);
    let live = public_site_url(&site.domain).unwrap_or_else(|_| format!("http://{}", site.domain));
    let domain_q = html_escape(&site.domain);
    if has {
        format!(
            r#"<div class="manage-ssl">
  <div>
    <strong>{domain} has SSL material on this host.</strong>
    <p>Certificate files were found under Let's Encrypt or CPN SSL paths.</p>
  </div>
  <div class="manage-actions-row">
    <a class="manage-btn primary" href="/websites/manage?domain={domain_q}&amp;tab=ssl">Renew / Manage SSL</a>
    <a class="manage-btn" href="{live}" target="_blank" rel="noopener noreferrer">Visit live site</a>
  </div>
</div>"#,
            domain = html_escape(&site.domain),
            domain_q = domain_q,
            live = html_escape(&live),
        )
    } else {
        format!(
            r#"<div class="manage-ssl">
  <div>
    <strong>No SSL certificate detected for {domain}.</strong>
    <p>Issue a Let's Encrypt certificate from the SSL tab when certbot is available.</p>
  </div>
  <a class="manage-btn primary" href="/websites/manage?domain={domain_q}&amp;tab=ssl">Issue SSL</a>
</div>"#,
            domain = html_escape(&site.domain),
            domain_q = domain_q,
        )
    }
}

pub fn resource_card(label: &str, value: &str, pct: Option<u8>) -> String {
    let bar = match pct {
        Some(p) => format!(
            r#"<div class="bar" aria-hidden="true"><i style="width:{}%"></i></div>"#,
            p.min(100)
        ),
        None => String::new(),
    };
    format!(
        r#"<div class="manage-stat"><span>{label}</span><strong>{value}</strong>{bar}</div>"#,
        label = html_escape(label),
        value = html_escape(value),
        bar = bar,
    )
}

pub fn tile(href: &str, title: &str, subtitle: &str) -> String {
    format!(
        r#"<a class="manage-tile" href="{href}">
  <span class="manage-tile-icon" aria-hidden="true"></span>
  <span><strong>{title}</strong><span>{subtitle}</span></span>
</a>"#,
        href = html_escape(href),
        title = html_escape(title),
        subtitle = html_escape(subtitle),
    )
}

pub fn section(title: &str, body: &str) -> String {
    format!(
        r#"<h2 class="manage-section-title">{title}</h2>{body}"#,
        title = html_escape(title),
        body = body,
    )
}
