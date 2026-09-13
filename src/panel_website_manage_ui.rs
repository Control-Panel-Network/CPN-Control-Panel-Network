//! Manage dashboard styles and chrome (banner, quick actions, tabs).

use crate::panel_ops_ssl_inspect::{SslValidityKind, inspect_domain_ssl, ssl_status_badge_html};
use crate::sites::SiteRecord;
use crate::website_preview::{preview_mode_url, public_site_url};

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
.site-manage .manage-badge.ssl-badge { gap:5px; margin-left:8px; }
.site-manage .manage-badge.ssl-valid { background:rgba(18,183,106,.18); color:#6ce9a6; }
.site-manage .manage-badge.ssl-expiring { background:rgba(247,144,9,.2); color:#fdb022; }
.site-manage .manage-badge.ssl-expired,
.site-manage .manage-badge.ssl-invalid,
.site-manage .manage-badge.ssl-mismatch { background:rgba(240,68,56,.2); color:#fda29b; }
.site-manage .manage-badge.ssl-none { background:rgba(152,162,179,.16); color:#98a2b3; }
.site-manage .ssl-lock { flex-shrink:0; }
.site-manage .manage-ssl.ssl-valid { border-color:rgba(18,183,106,.35); }
.site-manage .manage-ssl.ssl-expiring { border-color:rgba(247,144,9,.4); }
.site-manage .manage-ssl.ssl-expired,
.site-manage .manage-ssl.ssl-invalid,
.site-manage .manage-ssl.ssl-mismatch { border-color:rgba(240,68,56,.4); }
.site-manage .manage-ssl .ssl-meta { margin:6px 0 0; color:var(--m-muted); font-size:13px; line-height:1.45; }
.site-manage .manage-ssl .ssl-meta strong { display:inline; font-size:inherit; }
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
  display:flex; flex-wrap:nowrap; gap:4px; border-bottom:1px solid var(--m-line);
  margin:0 0 18px; padding:0 0 2px; overflow-x:auto; -webkit-overflow-scrolling:touch;
  overscroll-behavior-x:contain; scrollbar-width:thin;
}
.site-manage .manage-tabs a {
  display:inline-flex; align-items:center; flex:0 0 auto; min-height:40px; padding:0 14px;
  color:var(--m-muted); text-decoration:none; font-weight:700; font-size:13px;
  border-bottom:2px solid transparent; margin-bottom:-1px; white-space:nowrap;
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
  text-align:left; color:inherit; width:100%;
}
.site-manage .manage-chart-head {
  display:flex; align-items:center; justify-content:space-between; gap:10px; margin-bottom:4px;
}
.site-manage .manage-chart-head h3 { margin:0; }
.site-manage .manage-chart-details {
  flex:0 0 auto; min-height:28px; padding:0 10px; border-radius:999px; border:1px solid var(--m-line);
  background:#12151c; color:var(--m-ink); font:inherit; font-size:12px; font-weight:700; cursor:pointer;
}
.site-manage .manage-chart-details:hover, .site-manage .manage-chart-details:focus-visible {
  border-color:var(--m-accent); outline:none;
}
.site-manage .manage-chart h3 { margin:0 0 4px; font-size:13px; letter-spacing:.04em; text-transform:uppercase; color:var(--m-muted); }
.site-manage .manage-chart p, .site-manage .manage-chart-summary { margin:0 0 10px; color:var(--m-muted); font-size:12px; line-height:1.4; }
.site-manage .manage-chart svg, .site-manage .manage-metric-svg { width:100%; height:120px; display:block; }
.site-manage .manage-chart-svg { position:relative; }
.site-manage .manage-metric-svg .metric-hit { cursor:pointer; }
.site-manage .manage-metric-svg .metric-dot.is-selected {
  stroke:#fff; stroke-width:2;
}
.site-manage .manage-chart-readout {
  margin-top:8px; min-height:36px; display:flex; align-items:center; gap:10px;
  padding:8px 12px; border-radius:10px; background:#0b0d12; border:1px solid var(--m-line);
  color:#f5f7fb; font-size:14px; font-weight:600; letter-spacing:.01em;
}
.site-manage .manage-chart-readout.is-active { border-color:var(--m-accent); }
.site-manage .manage-chart-readout-hint { color:var(--m-muted); font-weight:500; font-size:12px; }
.site-manage .manage-chart-readout-value { font-variant-numeric:tabular-nums; }
.site-manage .manage-chart-readout-dialog { margin:0 0 12px; }
.site-manage .manage-stat-hint {
  display:block; margin-top:6px; font-style:normal; color:var(--m-muted); font-size:11px; line-height:1.35;
}
.site-manage .manage-metric-dialog {
  border:1px solid var(--m-line); border-radius:16px; padding:0; max-width:min(920px,96vw);
  width:100%; background:var(--m-card); color:var(--m-ink);
}
.site-manage .manage-metric-dialog::backdrop { background:rgba(0,0,0,.55); }
.site-manage .manage-metric-dialog-inner { margin:0; padding:16px; }
.site-manage .manage-metric-dialog-inner header {
  display:flex; align-items:center; justify-content:space-between; gap:12px; margin-bottom:8px;
}
.site-manage .manage-metric-dialog-inner h2 { margin:0; font-size:18px; }
.site-manage .manage-metric-dialog-inner svg { width:100%; height:220px; display:block; margin:8px 0 12px; }
.site-manage .manage-metric-stats {
  list-style:none; margin:0; padding:0; display:grid; grid-template-columns:repeat(auto-fit,minmax(120px,1fr)); gap:8px;
}
.site-manage .manage-metric-stats li {
  background:#0b0d12; border:1px solid var(--m-line); border-radius:10px; padding:10px 12px; font-size:13px;
}
.site-manage .manage-section-title {
  margin:22px 0 10px; font-size:12px; font-weight:800; letter-spacing:.08em;
  text-transform:uppercase; color:var(--m-muted); border-left:3px solid var(--m-accent); padding-left:10px;
}
.site-manage .manage-tile-grid {
  display:grid; grid-template-columns:repeat(auto-fill,minmax(min(100%,200px),1fr)); gap:12px;
}
.site-manage .manage-tile {
  display:flex; align-items:center; gap:12px; min-height:78px; padding:14px;
  border-radius:14px; background:var(--m-card); border:1px solid var(--m-line);
  color:inherit; text-decoration:none;
}
.site-manage .manage-tile:hover { border-color:#3b82f6; }
/* Size/centering for .manage-tile-icon comes from panel_icons::icon_tone_styles */
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
  .site-manage .manage-banner { padding:20px 16px 18px; }
  .site-manage .manage-banner h1 { font-size:22px; }
  .site-manage .manage-tile-grid,
  .site-manage .manage-card-grid { grid-template-columns:1fr; }
  .site-manage .manage-charts { grid-template-columns:1fr; }
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

pub fn manage_banner(site: &SiteRecord, username: &str) -> String {
    let status = if site.enabled { "Active" } else { "Suspended" };
    let badge_class = if site.enabled { "active" } else { "suspended" };
    let preview = preview_mode_url(&site.domain).unwrap_or_else(|_| "#".into());
    let domain_q = html_escape(&site.domain);
    let design = crate::panel_theme_chrome::manage_design_controls(username);
    let php_badge = site
        .php_version
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| {
            format!(
                r#"<span class="manage-badge" style="background:rgba(59,130,246,.18);color:#93c5fd;">PHP {}</span>"#,
                html_escape(v)
            )
        })
        .unwrap_or_default();
    let ssl_badge = ssl_status_badge_html(&inspect_domain_ssl(&site.domain));
    format!(
        r#"<div class="manage-banner">
  <h1>{domain}<span class="manage-badge {badge}">{status}</span>{php}{ssl}</h1>
  <p>Manage your website with powerful tools and real-time monitoring.</p>
  <div class="manage-banner-actions">
    <a class="manage-btn primary" href="{preview}">Preview Website</a>
    <a class="manage-btn" href="/websites/manage?domain={domain_q}&amp;tab=files">File Manager</a>
    {design}
  </div>
</div>"#,
        domain = html_escape(&site.domain),
        badge = badge_class,
        status = status,
        php = php_badge,
        ssl = ssl_badge,
        preview = html_escape(&preview),
        domain_q = domain_q,
        design = design,
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
  <a href="/websites/manage?domain={domain_q}&amp;tab=terminal" title="Web terminal in site home">Open Terminal</a>
  <a href="/websites/manage?domain={domain_q}&amp;tab=git" title="Git status, pull, commit, push">Manage Git</a>
  <a href="/websites/manage?domain={domain_q}&amp;tab=clone" title="Clone files to staging or new site">Clone/Staging</a>
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
        ("plugins", "Plugins"),
        ("terminal", "Terminal"),
        ("git", "Git"),
        ("clone", "Clone"),
    ];
    let domain_q = html_escape(domain);
    let mut out = String::from(r#"<nav class="manage-tabs" aria-label="Website sections">"#);
    let active_norm = match active {
        "apps" | "applications" | "plugin" => "plugins",
        "term" | "shell" => "terminal",
        "staging" => "clone",
        other => other,
    };
    for (id, label) in tabs {
        let class = if id == active_norm {
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
    let insight = inspect_domain_ssl(&site.domain);
    let live = public_site_url(&site.domain).unwrap_or_else(|_| format!("http://{}", site.domain));
    let domain_q = html_escape(&site.domain);
    let provider = html_escape(site.ssl.provider.label());
    let badge = ssl_status_badge_html(&insight);
    let expires = insight
        .expires_display
        .as_deref()
        .map(|d| format!("Expires: <strong>{}</strong>", html_escape(d)))
        .unwrap_or_else(|| "Expires: <strong>n/a</strong>".into());
    let issuer = if insight.issuer.is_empty() {
        String::new()
    } else {
        format!(
            " · Issuer: <strong>{}</strong>",
            html_escape(&insight.issuer)
        )
    };
    let sans = if insight.sans.is_empty() {
        String::new()
    } else {
        format!(
            " · SANs: <strong>{}</strong>",
            html_escape(
                &insight
                    .sans
                    .iter()
                    .take(4)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        )
    };
    let headline = match insight.kind {
        SslValidityKind::None => format!(
            "No SSL certificate detected for {}.",
            html_escape(&site.domain)
        ),
        other => format!(
            "{} for {}.",
            html_escape(other.label()),
            html_escape(&site.domain)
        ),
    };
    let actions = if insight.kind == SslValidityKind::None {
        format!(
            r#"<a class="manage-btn primary" href="/websites/manage?domain={domain_q}&amp;tab=ssl">Manage SSL</a>"#
        )
    } else {
        format!(
            r#"<div class="manage-actions-row">
    <a class="manage-btn primary" href="/websites/manage?domain={domain_q}&amp;tab=ssl">Renew / Manage SSL</a>
    <a class="manage-btn" href="{live}" target="_blank" rel="noopener noreferrer">Visit live site</a>
  </div>"#,
            domain_q = domain_q,
            live = html_escape(&live),
        )
    };
    format!(
        r#"<div class="manage-ssl ssl-{kind}">
  <div>
    {badge}
    <strong style="margin-top:8px;">{headline}</strong>
    <p class="ssl-meta">Provider: <strong>{provider}</strong>. {expires}{issuer}{sans}</p>
    <p>{detail}</p>
  </div>
  {actions}
</div>"#,
        kind = insight.kind.as_str(),
        badge = badge,
        headline = headline,
        provider = provider,
        expires = expires,
        issuer = issuer,
        sans = sans,
        detail = html_escape(&insight.detail),
        actions = actions,
    )
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
  {icon}
  <span><strong>{title}</strong><span>{subtitle}</span></span>
</a>"#,
        href = html_escape(href),
        icon = crate::panel_icons::manage_icon_html(href),
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
