//! Markup helpers for the Plugins hub (tabs, installed cards, store catalog).

use crate::panel_plugins_spa::plugins_hub_styles;
use crate::plugins::InstalledPlugin;
use crate::sites::SiteRecord;

pub(crate) fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub(crate) fn urlencoding_simple(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 3);
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

pub(crate) fn section_heading(title: &str, blurb: &str) -> String {
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

pub(crate) fn notice_block(kind: &str, message: Option<&str>) -> String {
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

pub(crate) fn view_tabs(active: &str, domain: &str) -> String {
    let installed = if active == "installed" { " active" } else { "" };
    let store = if active == "store" { " active" } else { "" };
    let host = if active == "host" { " active" } else { "" };
    let domain_q = if domain.is_empty() {
        String::new()
    } else {
        format!("&amp;domain={}", urlencoding_simple(domain))
    };
    format!(
        r#"<div class="plugin-tabs" role="tablist" aria-label="Plugins views">
        <a class="plugin-tab{installed}" href="/plugins?view=installed{domain_q}" role="tab">Installed</a>
        <a class="plugin-tab{store}" href="/plugins?view=store{domain_q}" role="tab">Plugin Store</a>
        <a class="plugin-tab{host}" href="/plugins?view=host{domain_q}" role="tab">Host packages</a>
      </div>
      <style>
        .plugin-tabs {{ display:flex; flex-wrap:wrap; gap:8px; margin:0 0 18px; }}
        .plugin-tab {{
          display:inline-flex; align-items:center; min-height:40px; padding:0 16px;
          border-radius:999px; border:1px solid var(--hairline); background:var(--canvas);
          color:var(--ink); font-size:14px; font-weight:600;
        }}
        .plugin-tab.active {{ background:#e7f1ff; color:#0b3d91; border-color:#93c5fd; }}
        .plugin-stats {{ display:flex; flex-wrap:wrap; gap:16px; margin:0 0 14px; font-size:14px; color:var(--ink); }}
        .plugin-stats strong {{ color:var(--ink); }}
        .plugin-grid {{
          display:grid; grid-template-columns:repeat(auto-fill,minmax(260px,1fr)); gap:16px; margin-top:14px;
        }}
        .plugin-card {{
          display:flex; flex-direction:column; gap:10px; padding:18px;
          border:1px solid var(--hairline); border-radius:16px; background:var(--canvas);
          color:var(--ink);
        }}
        .plugin-card h3 {{ margin:0; font-size:17px; letter-spacing:-.02em; color:var(--ink); }}
        .plugin-card .plugin-desc {{ margin:0; color:var(--ink); opacity:.88; font-size:14px; line-height:1.45; }}
        .plugin-card .plugin-meta {{ margin:0; color:var(--ink); opacity:.8; font-size:13px; }}
        .plugin-badges {{ display:flex; flex-wrap:wrap; gap:6px; }}
        .plugin-badge {{
          display:inline-flex; align-items:center; min-height:24px; padding:0 8px;
          border-radius:999px; font-size:11px; font-weight:700; letter-spacing:.04em; text-transform:uppercase;
          background:#eef2f6; color:#0f172a;
        }}
        .plugin-badge.featured {{ background:#fef3c7; color:#92400e; }}
        .plugin-badge.free {{ background:#dcfce7; color:#14532d; }}
        .plugin-badge.paid {{ background:#ede9fe; color:#4c1d95; }}
        .plugin-badge.cat {{ background:#dbeafe; color:#1e3a8a; }}
        .plugin-badge.installed {{ background:#dbeafe; color:#1e3a8a; }}
        .plugin-dates {{ margin:0; color:var(--ink); opacity:.75; font-size:12px; line-height:1.4; }}
        .plugin-actions {{ display:flex; flex-wrap:wrap; gap:8px; margin-top:auto; }}
        .btn-secondary, .btn-warn {{
          display:inline-flex; align-items:center; justify-content:center; min-height:40px; padding:0 14px;
          border:0; border-radius:999px; font-weight:700; cursor:pointer; font:inherit; text-decoration:none;
        }}
        .btn-secondary {{ background:#e2e8f0; color:#0f172a; }}
        .btn-warn {{ background:#fee2e2; color:#7f1d1d; border:1px solid #fecaca; }}
        .plugin-search-row {{
          display:flex; flex-wrap:wrap; gap:10px; align-items:center; margin:12px 0 10px; max-width:none;
        }}
        .plugin-search-row label {{
          position:absolute; width:1px; height:1px; padding:0; margin:-1px; overflow:hidden;
          clip:rect(0,0,0,0); white-space:nowrap; border:0;
        }}
        .plugin-search {{
          flex:1 1 240px; min-width:180px; max-width:480px; box-sizing:border-box; border:1px solid #94a3b8;
          border-radius:10px; padding:10px 12px; font:inherit; margin:0; color:var(--ink); background:var(--canvas);
        }}
        .plugin-search-row .btn-primary,
        .plugin-search-row .btn-secondary {{ min-height:40px; }}
        .plugin-store-meta {{ margin:8px 0 0; font-size:.92rem; color:var(--ink); max-width:none; }}
        .plugin-store-meta a {{ color:var(--blue); font-weight:600; }}
        .plugin-risk-notice {{
          margin:12px 0 0; padding:12px 14px; border-radius:12px;
          border:1px solid #1d4ed8; background:#eff6ff; color:#0f172a; font-weight:600; line-height:1.45;
        }}
        .plugin-count {{ margin:0 0 8px; font-size:.95rem; color:var(--ink); font-weight:700; }}
        .category-pills {{ display:flex; flex-wrap:wrap; gap:8px; margin:0 0 10px; }}
        .category-pills a {{
          display:inline-flex; align-items:center; min-height:34px; padding:0 12px; border-radius:999px;
          border:1px solid var(--hairline); background:var(--canvas); font-size:13px; color:var(--ink); font-weight:600;
        }}
        .category-pills a.active {{ background:#1d4ed8; color:#ffffff; border-color:#1d4ed8; }}
        .plugin-links {{ display:flex; gap:12px; font-size:13px; }}
        .plugin-links a {{ color:var(--blue); font-weight:600; }}
        .domain-picker {{ display:flex; flex-wrap:wrap; gap:10px; align-items:end; margin:0 0 10px; }}
        .domain-picker select {{
          min-width:220px; border:1px solid #94a3b8; border-radius:10px; padding:10px 12px; font:inherit;
          color:var(--ink); background:var(--canvas);
        }}
        [data-color-mode="dark"] .plugin-tab {{ color:#e2e8f0; background:#1a1d26; }}
        [data-color-mode="dark"] .plugin-tab.active {{ background:rgba(59,130,246,.25); color:#bfdbfe; border-color:#60a5fa; }}
        [data-color-mode="dark"] .plugin-search,
        [data-color-mode="dark"] .domain-picker select {{
          background:#1a1d26; border-color:#475569; color:#f1f5f9;
        }}
        [data-color-mode="dark"] .category-pills a {{ color:#e2e8f0; }}
        [data-color-mode="dark"] .category-pills a.active {{
          background:#2563eb; color:#ffffff; border-color:#60a5fa;
        }}
        [data-color-mode="dark"] .btn-secondary {{ background:#334155; color:#f8fafc; }}
        [data-color-mode="dark"] .btn-warn {{ background:#7f1d1d; color:#fef2f2; border-color:#f87171; }}
        [data-color-mode="dark"] .plugin-badge {{ background:#334155; color:#f8fafc; }}
        [data-color-mode="dark"] .plugin-badge.featured {{ background:rgba(245,158,11,.28); color:#fde68a; }}
        [data-color-mode="dark"] .plugin-badge.free {{ background:rgba(22,163,74,.28); color:#bbf7d0; }}
        [data-color-mode="dark"] .plugin-badge.paid {{ background:rgba(124,58,237,.28); color:#ddd6fe; }}
        [data-color-mode="dark"] .plugin-badge.cat,
        [data-color-mode="dark"] .plugin-badge.installed {{ background:rgba(37,99,235,.3); color:#bfdbfe; }}
        [data-color-mode="dark"] .plugin-risk-notice {{
          background:#0b1220; border-color:#60a5fa; color:#e2e8f0;
        }}
        [data-color-mode="dark"] .plugin-card {{ color:#f1f5f9; }}
        {hub_styles}
      </style>"#,
        installed = installed,
        store = store,
        host = host,
        domain_q = domain_q,
        hub_styles = plugins_hub_styles(),
    )
}

pub(crate) fn domain_picker(sites: &[SiteRecord], selected: &str, view: &str) -> String {
    if sites.is_empty() {
        return r#"<p class="panel-notice error" role="status">Create a website first. Plugins install under <code>/home/&lt;domain&gt;/plugins/</code>.</p>"#.into();
    }
    let mut options = String::new();
    for site in sites {
        let sel = if site.domain == selected {
            " selected"
        } else {
            ""
        };
        options.push_str(&format!(
            r#"<option value="{domain}"{sel}>{domain}</option>"#,
            domain = html_escape(&site.domain),
            sel = sel,
        ));
    }
    format!(
        r#"<form method="get" action="/plugins" class="domain-picker">
        <input type="hidden" name="view" value="{view}">
        <div>
          <label for="domain"><strong>Site</strong></label><br>
          <select id="domain" name="domain">{options}</select>
        </div>
        <button type="submit" class="btn-secondary">Apply</button>
      </form>"#,
        view = html_escape(view),
        options = options,
    )
}

pub(crate) fn resolve_domain(sites: &[SiteRecord], requested: &str) -> String {
    let req = requested.trim().to_lowercase();
    if !req.is_empty() && sites.iter().any(|s| s.domain == req) {
        return req;
    }
    sites.first().map(|s| s.domain.clone()).unwrap_or_default()
}

fn badge_pricing(pricing: &str) -> String {
    let lower = pricing.to_ascii_lowercase();
    if lower.contains("paid") || lower.contains("premium") {
        r#"<span class="plugin-badge paid">Paid</span>"#.into()
    } else {
        r#"<span class="plugin-badge free">Free</span>"#.into()
    }
}

pub(crate) fn installed_cards(plugins: &[InstalledPlugin], layout: &str, domain: &str) -> String {
    if plugins.is_empty() {
        return r#"<p class="empty-state">No plugins installed for this site yet. Open the Plugin Store to install from the community catalog.</p>"#
            .into();
    }
    if layout == "table" {
        return installed_table(plugins, domain);
    }
    let mut cards = String::from(r#"<div class="plugin-grid">"#);
    for item in plugins {
        let m = &item.manifest;
        let active = if m.enabled { "Yes" } else { "No" };
        let toggle = toggle_form(m.enabled, &m.id, domain);
        cards.push_str(&format!(
            r#"<article class="plugin-card">
          <h3>{name}</h3>
          <div class="plugin-badges">
            <span class="plugin-badge cat">{cat}</span>
            <span class="plugin-badge">v{ver}</span>
            {pricing}
          </div>
          <p class="plugin-desc">{desc}</p>
          <p class="plugin-meta">Status: Installed · Active: {active}</p>
          <div class="plugin-actions">
            <a class="btn-secondary" href="/plugins/settings?domain={domain_q}&amp;id={id}">Settings</a>
            {toggle}
            <form method="post" action="/plugins/uninstall" class="inline-form" onsubmit="return confirm('Uninstall {name}?');">
              <input type="hidden" name="id" value="{id}">
              <input type="hidden" name="domain" value="{domain}">
              <button type="submit" class="btn-danger">Uninstall</button>
            </form>
          </div>
          <div class="plugin-links">
            <a href="/plugins/dashboard?domain={domain_q}&amp;id={id}">Dashboard</a>
            <a href="/plugins?view=installed&amp;domain={domain_q}&amp;notice={help}">Help</a>
            <a href="/plugins?view=installed&amp;domain={domain_q}&amp;notice={about}">About</a>
          </div>
        </article>"#,
            name = html_escape(&m.name),
            id = html_escape(&m.id),
            cat = html_escape(&m.category),
            ver = html_escape(&m.version),
            pricing = badge_pricing(&m.pricing),
            desc = html_escape(&m.description),
            active = active,
            toggle = toggle,
            domain = html_escape(domain),
            domain_q = urlencoding_simple(domain),
            help = urlencoding_simple(&format!(
                "Help for {}: see plugin docs in the install folder.",
                m.name
            )),
            about = urlencoding_simple(&format!(
                "{} v{} by {}. Installed under {}.",
                m.name,
                m.version,
                m.author,
                item.path.display()
            )),
        ));
    }
    cards.push_str("</div>");
    cards
}

fn toggle_form(enabled: bool, id: &str, domain: &str) -> String {
    if enabled {
        format!(
            r#"<form method="post" action="/plugins/disable" class="inline-form">
            <input type="hidden" name="id" value="{id}">
            <input type="hidden" name="domain" value="{domain}">
            <button type="submit" class="btn-warn">Deactivate</button>
          </form>"#,
            id = html_escape(id),
            domain = html_escape(domain),
        )
    } else {
        format!(
            r#"<form method="post" action="/plugins/enable" class="inline-form">
            <input type="hidden" name="id" value="{id}">
            <input type="hidden" name="domain" value="{domain}">
            <button type="submit" class="btn-primary">Activate</button>
          </form>"#,
            id = html_escape(id),
            domain = html_escape(domain),
        )
    }
}

fn installed_table(plugins: &[InstalledPlugin], domain: &str) -> String {
    let mut rows = String::from(
        r#"<div class="table-wrap"><table class="data-table">
        <thead><tr><th>Plugin</th><th>Category</th><th>Version</th><th>Status</th><th>Actions</th></tr></thead><tbody>"#,
    );
    for item in plugins {
        let m = &item.manifest;
        let active = if m.enabled { "Active" } else { "Inactive" };
        let toggle = toggle_form(m.enabled, &m.id, domain);
        rows.push_str(&format!(
            r#"<tr>
            <td><strong>{name}</strong><div class="muted">{id}</div></td>
            <td>{cat}</td>
            <td>v{ver}</td>
            <td>{active}</td>
            <td class="plugin-actions">
              <a class="btn-secondary" href="/plugins/settings?domain={domain_q}&amp;id={id}">Settings</a>
              {toggle}
              <form method="post" action="/plugins/uninstall" class="inline-form" onsubmit="return confirm('Uninstall {name}?');">
                <input type="hidden" name="id" value="{id}">
                <input type="hidden" name="domain" value="{domain}">
                <button type="submit" class="btn-danger">Uninstall</button>
              </form>
            </td>
          </tr>"#,
            name = html_escape(&m.name),
            id = html_escape(&m.id),
            cat = html_escape(&m.category),
            ver = html_escape(&m.version),
            active = active,
            toggle = toggle,
            domain = html_escape(domain),
            domain_q = urlencoding_simple(domain),
        ));
    }
    rows.push_str("</tbody></table></div>");
    rows
}

pub(crate) use crate::panel_plugins_store::{StoreListOpts, category_pills, store_catalog};
