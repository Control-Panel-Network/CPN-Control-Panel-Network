//! Dashboard Sites switcher + collapsible Tools groups (CPN-branded).

use crate::panel_icons::{hub_icon_html, nav_icon_html};
use crate::panel_ops_ssl_le::ssl_status_for_domain;
use crate::sites::{list_sites, site_home_from_record};
use crate::website_preview::{preview_mode_url, ssl_material_present};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Short SSL label for the dashboard site jumper (Active / None / Pending / …).
fn dash_ssl_label(domain: &str) -> String {
    let row = ssl_status_for_domain(domain);
    if row.provider == "none" || row.provider_label.eq_ignore_ascii_case("None") {
        return "None".into();
    }
    if row.has_cert || ssl_material_present(domain) {
        return format!("Active ({})", row.provider_label);
    }
    if !row.last_error.trim().is_empty() {
        return format!("{} · error", row.provider_label);
    }
    if row.needs_issue {
        return format!("{} · pending", row.provider_label);
    }
    format!("{} · no cert", row.provider_label)
}

struct ToolLink {
    label: &'static str,
    href: &'static str,
    icon_id: &'static str,
}

struct ToolGroup {
    id: &'static str,
    title: &'static str,
    icon_id: &'static str,
    tools: &'static [ToolLink],
}

const TOOL_GROUPS: &[ToolGroup] = &[
    ToolGroup {
        id: "domains",
        title: "Domains",
        icon_id: "websites",
        tools: &[
            ToolLink {
                label: "Websites",
                href: "/websites",
                icon_id: "websites",
            },
            ToolLink {
                label: "Cloudflare DNS",
                href: "/dns/cloudflare",
                icon_id: "server",
            },
            ToolLink {
                label: "DNS Zones",
                href: "/server/dns/zones",
                icon_id: "server",
            },
        ],
    },
    ToolGroup {
        id: "email",
        title: "Email",
        icon_id: "email",
        tools: &[
            ToolLink {
                label: "Email hub",
                href: "/email",
                icon_id: "email",
            },
            ToolLink {
                label: "Accounts",
                href: "/email/accounts",
                icon_id: "email",
            },
            ToolLink {
                label: "Forwarders",
                href: "/email/forwarders",
                icon_id: "email",
            },
        ],
    },
    ToolGroup {
        id: "files",
        title: "Files",
        icon_id: "server",
        tools: &[
            ToolLink {
                label: "File Manager",
                href: "/server/files",
                icon_id: "server",
            },
            ToolLink {
                label: "FTP / SFTP",
                href: "/ftp",
                icon_id: "databases",
            },
            ToolLink {
                label: "Backups",
                href: "/backups",
                icon_id: "backups",
            },
        ],
    },
    ToolGroup {
        id: "databases",
        title: "Databases",
        icon_id: "databases",
        tools: &[
            ToolLink {
                label: "Databases & FTP",
                href: "/databases",
                icon_id: "databases",
            },
            ToolLink {
                label: "phpMyAdmin",
                href: "/databases/phpmyadmin",
                icon_id: "databases",
            },
        ],
    },
    ToolGroup {
        id: "security",
        title: "Security",
        icon_id: "security",
        tools: &[
            ToolLink {
                label: "Manage SSL",
                href: "/security/ssl",
                icon_id: "security",
            },
            ToolLink {
                label: "Firewall",
                href: "/security/firewall",
                icon_id: "security",
            },
            ToolLink {
                label: "Fail2ban",
                href: "/security/fail2ban",
                icon_id: "security",
            },
        ],
    },
    ToolGroup {
        id: "software",
        title: "Software",
        icon_id: "plugins",
        tools: &[
            ToolLink {
                label: "Plugins",
                href: "/plugins",
                icon_id: "plugins",
            },
            ToolLink {
                label: "Apps",
                href: "/apps",
                icon_id: "plugins",
            },
            ToolLink {
                label: "Packages",
                href: "/packages",
                icon_id: "settings",
            },
        ],
    },
];

pub fn dashboard_tools_styles() -> &'static str {
    r#"
.dash-sites {
  margin: 0 0 22px; padding: 16px 18px; border-radius: 16px;
  border: 1px solid var(--hairline, #e4e7ec); background: var(--canvas, #fff);
}
.dash-sites h2 { margin: 0 0 6px; font-size: 16px; letter-spacing: -.01em; }
.dash-sites .muted { color: var(--muted); font-size: 13px; margin: 0 0 12px; }
.dash-sites-row { display:flex; flex-wrap:wrap; gap:12px; align-items:flex-end; }
.dash-sites-row label { display:flex; flex-direction:column; gap:6px; font-size:12px; font-weight:700; min-width:min(100%,280px); flex:1; color:var(--ink,#1d1d1f); }
.dash-sites-row input[type=search], .dash-sites-row select {
  min-height:40px; padding:8px 12px; border-radius:10px; border:1px solid var(--hairline,#d0d5dd);
  background:var(--canvas,#fff); color:var(--ink,#1d1d1f); font:inherit; color-scheme:light;
}
.dash-sites-row select option {
  background:#ffffff; color:#111827;
}
.dash-sites-meta { margin-top:12px; display:grid; gap:6px; font-size:13px; color:var(--ink,#1d1d1f); }
.dash-sites-meta strong { color:var(--ink,#1d1d1f); font-weight:700; }
.dash-sites-meta[hidden], .dash-sites-actions[hidden] { display:none !important; }
.dash-sites-actions { display:flex; flex-wrap:wrap; gap:8px; margin-top:12px; }
.dash-sites-actions a {
  display:inline-flex; align-items:center; min-height:36px; padding:0 12px; border-radius:999px;
  border:1px solid var(--hairline,#d0d5dd); text-decoration:none; color:inherit; font-weight:700; font-size:13px;
}
.dash-sites-actions a.primary { background:var(--blue,#2563eb); border-color:transparent; color:#fff; }
.dash-tool-groups { display:flex; flex-direction:column; gap:12px; margin:8px 0 28px; }
.dash-tool-group {
  border:1px solid var(--hairline,#e4e7ec); border-radius:14px; background:var(--canvas,#fff);
  overflow:hidden;
}
.dash-tool-group > summary {
  list-style:none; cursor:pointer; display:flex; align-items:center; gap:10px;
  min-height:48px; padding:10px 14px; font-weight:700; font-size:14px; user-select:none;
}
.dash-tool-group > summary::-webkit-details-marker { display:none; }
.dash-tool-group > summary .nav-icon { width:28px; height:28px; }
.dash-tool-group > summary .dash-chevron {
  margin-left:auto; transition:transform .18s ease; color:var(--muted);
}
.dash-tool-group[open] > summary .dash-chevron { transform:rotate(90deg); }
.dash-tool-grid {
  display:grid; grid-template-columns:repeat(3,minmax(0,1fr)); gap:4px 8px;
  padding:4px 10px 14px; border-top:1px solid var(--hairline,#e4e7ec);
}
.dash-tool-link {
  display:flex; align-items:center; gap:10px; min-height:40px; padding:8px 10px;
  border-radius:10px; text-decoration:none; color:inherit; font-size:13px; font-weight:600;
}
.dash-tool-link:hover { background:rgba(37,99,235,.06); color:var(--blue,#2563eb); }
.dash-tool-link .hub-tile-icon, .dash-tool-link .nav-icon {
  width:28px; height:28px; flex:0 0 28px; border-radius:8px;
}
.dash-tool-link .hub-tile-icon svg, .dash-tool-link .nav-icon svg { width:16px; height:16px; }
@media (max-width:900px) {
  .dash-tool-grid { grid-template-columns:repeat(2,minmax(0,1fr)); }
}
@media (max-width:560px) {
  .dash-tool-grid { grid-template-columns:1fr; }
}
[data-color-mode="dark"] .dash-sites,
[data-color-mode="dark"] .dash-tool-group {
  background:#1c212b; border-color:#2a3140;
}
[data-color-mode="dark"] .dash-sites-row label { color:#e8edf5; }
[data-color-mode="dark"] .dash-sites .muted { color:#b7c0cc; }
[data-color-mode="dark"] .dash-sites-row input[type=search],
[data-color-mode="dark"] .dash-sites-row select {
  background:#12151c; border-color:#3b4558; color:#f3f6fb; color-scheme:dark;
}
[data-color-mode="dark"] .dash-sites-row select option {
  background:#12151c; color:#f3f6fb;
}
[data-color-mode="dark"] .dash-sites-meta,
[data-color-mode="dark"] .dash-sites-meta strong { color:#e8edf5; }
[data-color-mode="dark"] .dash-tool-link:hover { background:rgba(147,197,253,.08); }
"#
}

fn chevron() -> &'static str {
    r#"<svg class="dash-chevron" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="m9 18 6-6-6-6"/></svg>"#
}

pub fn dashboard_sites_panel() -> String {
    let sites = list_sites().unwrap_or_default();
    let mut options = String::from(r#"<option value="">Choose a site…</option>"#);
    let mut meta_json = String::from("{");
    for (i, site) in sites.iter().enumerate() {
        let d = html_escape(&site.domain);
        options.push_str(&format!(r#"<option value="{d}">{d}</option>"#, d = d));
        let ssl = dash_ssl_label(&site.domain);
        let preview = preview_mode_url(&site.domain)
            .unwrap_or_else(|_| format!("/websites/manage?domain={}", site.domain));
        if i > 0 {
            meta_json.push(',');
        }
        let home = site_home_from_record(site)
            .to_string_lossy()
            .trim_end_matches(['/', '\\'])
            .replace('\\', "/");
        let home = if home.is_empty() {
            format!("/home/{}", site.domain)
        } else {
            home
        };
        meta_json.push_str(&format!(
            r#"{dom}:{{"home":{home},"ssl":{ssl},"preview":{prev},"manage":{manage}}}"#,
            dom = serde_json::to_string(&site.domain).unwrap_or_else(|_| "\"\"".into()),
            home = serde_json::to_string(&home).unwrap_or_else(|_| "\"\"".into()),
            ssl = serde_json::to_string(&ssl).unwrap_or_else(|_| "\"\"".into()),
            prev = serde_json::to_string(&preview).unwrap_or_else(|_| "\"\"".into()),
            manage = serde_json::to_string(&format!("/websites/manage?domain={}", site.domain))
                .unwrap_or_else(|_| "\"\"".into()),
        ));
    }
    meta_json.push('}');
    let empty = if sites.is_empty() {
        r#"<p class="muted">No sites yet. Create one under <a href="/websites">Websites</a>.</p>"#
    } else {
        ""
    };
    format!(
        r#"<section class="dash-sites" aria-label="Sites and domains">
  <h2>Sites / Domains</h2>
  <p class="muted">Jump to a hosted site. Search or pick a domain, then Manage or Preview.</p>
  {empty}
  <div class="dash-sites-row">
    <label for="dash-site-filter">Search sites
      <input id="dash-site-filter" type="search" placeholder="Filter domains…" autocomplete="off">
    </label>
    <label for="dash-site-select">Current site
      <select id="dash-site-select">{options}</select>
    </label>
  </div>
  <div class="dash-sites-meta" id="dash-site-meta" hidden>
    <div>Home: <strong id="dash-site-home">-</strong></div>
    <div>SSL: <strong id="dash-site-ssl">-</strong></div>
  </div>
  <div class="dash-sites-actions" id="dash-site-actions" hidden>
    <a class="primary" id="dash-site-manage" href="/websites">Manage</a>
    <a id="dash-site-preview" href="/websites" target="_blank" rel="noopener noreferrer">Preview</a>
    <a id="dash-site-websites" href="/websites">All websites</a>
  </div>
</section>
<script>
(function(){{
  var meta = {meta};
  var sel = document.getElementById('dash-site-select');
  var filter = document.getElementById('dash-site-filter');
  var box = document.getElementById('dash-site-meta');
  var actions = document.getElementById('dash-site-actions');
  if (!sel) return;
  function applyFilter() {{
    var q = (filter && filter.value || '').toLowerCase().trim();
    Array.prototype.forEach.call(sel.options, function(opt, i) {{
      if (i === 0) {{ opt.hidden = false; return; }}
      opt.hidden = q && opt.value.toLowerCase().indexOf(q) === -1;
    }});
  }}
  function sync() {{
    var d = sel.value;
    var info = meta[d];
    if (!info) {{
      box.hidden = true; actions.hidden = true;
      document.getElementById('dash-site-home').textContent = '-';
      document.getElementById('dash-site-ssl').textContent = '-';
      return;
    }}
    box.hidden = false; actions.hidden = false;
    document.getElementById('dash-site-home').textContent = info.home || '-';
    document.getElementById('dash-site-ssl').textContent = info.ssl || '-';
    document.getElementById('dash-site-manage').href = info.manage;
    document.getElementById('dash-site-preview').href = info.preview;
  }}
  sel.addEventListener('change', sync);
  if (filter) filter.addEventListener('input', applyFilter);
  sync();
}})();
</script>"#,
        empty = empty,
        options = options,
        meta = meta_json,
    )
}

pub fn dashboard_tool_groups() -> String {
    let mut out = String::from(
        r#"<h2 class="hub-section-title">Tools</h2>
<div class="dash-tool-groups">"#,
    );
    for (i, group) in TOOL_GROUPS.iter().enumerate() {
        let open = if i < 2 { " open" } else { "" };
        out.push_str(&format!(
            r#"<details class="dash-tool-group"{open}>
  <summary>{icon}<span>{title}</span>{chev}</summary>
  <div class="dash-tool-grid">"#,
            open = open,
            icon = nav_icon_html(group.icon_id),
            title = html_escape(group.title),
            chev = chevron(),
        ));
        for tool in group.tools {
            out.push_str(&format!(
                r#"<a class="dash-tool-link" href="{href}">{icon}<span>{label}</span></a>"#,
                href = html_escape(tool.href),
                icon = hub_icon_html(tool.href),
                label = html_escape(tool.label),
            ));
        }
        out.push_str("</div></details>");
    }
    out.push_str("</div>");
    out
}
