//! HTML for CPN Panel Plugins (Installed + Store). User-facing copy is CPN-only.

use crate::plugins::{
    CatalogEntry, InstalledPlugin, catalog_next_refresh_unix, catalog_repo_url, fetch_catalog,
    format_unix_local, list_installed, plugins_install_path_display,
};
use crate::sites::SiteRecord;

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

fn view_tabs(active: &str, domain: &str) -> String {
    let installed = if active == "store" { "" } else { " active" };
    let store = if active == "store" { " active" } else { "" };
    let domain_q = if domain.is_empty() {
        String::new()
    } else {
        format!("&amp;domain={}", urlencoding_simple(domain))
    };
    format!(
        r#"<div class="plugin-tabs" role="tablist" aria-label="Plugins views">
        <a class="plugin-tab{installed}" href="/plugins?view=installed{domain_q}" role="tab">Installed</a>
        <a class="plugin-tab{store}" href="/plugins?view=store{domain_q}" role="tab">Plugin Store</a>
      </div>
      <style>
        .plugin-tabs {{ display:flex; flex-wrap:wrap; gap:8px; margin:0 0 18px; }}
        .plugin-tab {{
          display:inline-flex; align-items:center; min-height:40px; padding:0 16px;
          border-radius:999px; border:1px solid var(--hairline); background:var(--canvas);
          color:var(--muted); font-size:14px; font-weight:600;
        }}
        .plugin-tab.active {{ background:#e7f1ff; color:var(--blue); border-color:#c9ddf7; }}
        .plugin-stats {{ display:flex; flex-wrap:wrap; gap:16px; margin:0 0 14px; font-size:14px; }}
        .plugin-stats strong {{ color:var(--ink); }}
        .plugin-grid {{
          display:grid; grid-template-columns:repeat(auto-fill,minmax(260px,1fr)); gap:16px; margin-top:14px;
        }}
        .plugin-card {{
          display:flex; flex-direction:column; gap:10px; padding:18px;
          border:1px solid var(--hairline); border-radius:16px; background:var(--canvas);
        }}
        .plugin-card h3 {{ margin:0; font-size:17px; letter-spacing:-.02em; }}
        .plugin-badges {{ display:flex; flex-wrap:wrap; gap:6px; }}
        .plugin-badge {{
          display:inline-flex; align-items:center; min-height:24px; padding:0 8px;
          border-radius:999px; font-size:11px; font-weight:700; letter-spacing:.04em; text-transform:uppercase;
          background:#f2f4f7; color:#475467;
        }}
        .plugin-badge.free {{ background:#ecfdf3; color:#067647; }}
        .plugin-badge.paid {{ background:#f4ebff; color:#6941c6; }}
        .plugin-badge.cat {{ background:#eff8ff; color:#175cd3; }}
        .plugin-actions {{ display:flex; flex-wrap:wrap; gap:8px; margin-top:auto; }}
        .btn-secondary, .btn-warn {{
          display:inline-flex; align-items:center; justify-content:center; min-height:40px; padding:0 14px;
          border:0; border-radius:999px; font-weight:700; cursor:pointer; font:inherit; text-decoration:none;
        }}
        .btn-secondary {{ background:#f2f4f7; color:#344054; }}
        .btn-warn {{ background:#fffaeb; color:#b54708; }}
        .plugin-search {{
          width:100%; max-width:420px; box-sizing:border-box; border:1px solid #d0d5dd;
          border-radius:10px; padding:11px 12px; font:inherit; margin:0 0 12px;
        }}
        .category-pills {{ display:flex; flex-wrap:wrap; gap:8px; margin:0 0 14px; }}
        .category-pills a {{
          display:inline-flex; align-items:center; min-height:34px; padding:0 12px; border-radius:999px;
          border:1px solid var(--hairline); background:var(--canvas); font-size:13px; color:var(--muted);
        }}
        .category-pills a.active {{ background:#e7f1ff; color:var(--blue); font-weight:600; }}
        .plugin-links {{ display:flex; gap:12px; font-size:13px; }}
        .plugin-links a {{ color:var(--blue); }}
        .domain-picker {{ display:flex; flex-wrap:wrap; gap:10px; align-items:end; margin:0 0 16px; }}
        .domain-picker select {{
          min-width:220px; border:1px solid #d0d5dd; border-radius:10px; padding:10px 12px; font:inherit;
        }}
      </style>"#
    )
}

fn domain_picker(sites: &[SiteRecord], selected: &str, view: &str) -> String {
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
          <select id="domain" name="domain" onchange="this.form.submit()">{options}</select>
        </div>
        <noscript><button type="submit" class="btn-secondary">Apply</button></noscript>
      </form>"#,
        view = html_escape(view),
        options = options,
    )
}

fn resolve_domain(sites: &[SiteRecord], requested: &str) -> String {
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

fn installed_cards(plugins: &[InstalledPlugin], layout: &str, domain: &str) -> String {
    if plugins.is_empty() {
        return r#"<p class="empty-state">No plugins installed for this site yet. Open the Plugin Store to install from the community catalog.</p>"#
            .into();
    }
    if layout == "table" {
        let mut rows = String::from(
            r#"<div class="table-wrap"><table class="data-table">
        <thead><tr><th>Plugin</th><th>Category</th><th>Version</th><th>Status</th><th>Actions</th></tr></thead><tbody>"#,
        );
        for item in plugins {
            let m = &item.manifest;
            let active = if m.enabled { "Active" } else { "Inactive" };
            let toggle = if m.enabled {
                format!(
                    r#"<form method="post" action="/plugins/disable" class="inline-form">
              <input type="hidden" name="id" value="{id}">
              <input type="hidden" name="domain" value="{domain}">
              <button type="submit" class="btn-warn">Deactivate</button>
            </form>"#,
                    id = html_escape(&m.id),
                    domain = html_escape(domain),
                )
            } else {
                format!(
                    r#"<form method="post" action="/plugins/enable" class="inline-form">
              <input type="hidden" name="id" value="{id}">
              <input type="hidden" name="domain" value="{domain}">
              <button type="submit" class="btn-primary">Activate</button>
            </form>"#,
                    id = html_escape(&m.id),
                    domain = html_escape(domain),
                )
            };
            rows.push_str(&format!(
                r#"<tr>
            <td><strong>{name}</strong><div class="muted">{id}</div></td>
            <td>{cat}</td>
            <td>v{ver}</td>
            <td>{active}</td>
            <td class="plugin-actions">
              <a class="btn-secondary" href="/plugins?view=installed&amp;domain={domain_q}&amp;notice={settings}">Settings</a>
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
                settings = urlencoding_simple("Settings UI for this plugin is not wired yet."),
            ));
        }
        rows.push_str("</tbody></table></div>");
        return rows;
    }

    let mut cards = String::from(r#"<div class="plugin-grid">"#);
    for item in plugins {
        let m = &item.manifest;
        let active = if m.enabled { "Yes" } else { "No" };
        let toggle = if m.enabled {
            format!(
                r#"<form method="post" action="/plugins/disable" class="inline-form">
            <input type="hidden" name="id" value="{id}">
            <input type="hidden" name="domain" value="{domain}">
            <button type="submit" class="btn-warn">Deactivate</button>
          </form>"#,
                id = html_escape(&m.id),
                domain = html_escape(domain),
            )
        } else {
            format!(
                r#"<form method="post" action="/plugins/enable" class="inline-form">
            <input type="hidden" name="id" value="{id}">
            <input type="hidden" name="domain" value="{domain}">
            <button type="submit" class="btn-primary">Activate</button>
          </form>"#,
                id = html_escape(&m.id),
                domain = html_escape(domain),
            )
        };
        cards.push_str(&format!(
            r#"<article class="plugin-card">
          <h3>{name}</h3>
          <div class="plugin-badges">
            <span class="plugin-badge cat">{cat}</span>
            <span class="plugin-badge">v{ver}</span>
            {pricing}
          </div>
          <p class="muted">{desc}</p>
          <p class="muted">Status: Installed · Active: {active}</p>
          <div class="plugin-actions">
            <a class="btn-secondary" href="/plugins?view=installed&amp;domain={domain_q}&amp;notice={settings}">Settings</a>
            {toggle}
            <form method="post" action="/plugins/uninstall" class="inline-form" onsubmit="return confirm('Uninstall {name}?');">
              <input type="hidden" name="id" value="{id}">
              <input type="hidden" name="domain" value="{domain}">
              <button type="submit" class="btn-danger">Uninstall</button>
            </form>
          </div>
          <div class="plugin-links">
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
            settings = urlencoding_simple("Settings UI for this plugin is not wired yet."),
            help = urlencoding_simple(&format!("Help for {}: see plugin docs in the install folder.", m.name)),
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

fn store_rows(
    entries: &[CatalogEntry],
    installed_ids: &[String],
    query: &str,
    category: &str,
    domain: &str,
) -> String {
    let q = query.trim().to_ascii_lowercase();
    let cat = category.trim().to_ascii_lowercase();
    let filtered: Vec<&CatalogEntry> = entries
        .iter()
        .filter(|entry| {
            let cat_ok =
                cat.is_empty() || cat == "all" || entry.category.to_ascii_lowercase() == cat;
            if !cat_ok {
                return false;
            }
            if q.is_empty() {
                return true;
            }
            entry.name.to_ascii_lowercase().contains(&q)
                || entry.description.to_ascii_lowercase().contains(&q)
                || entry.id.to_ascii_lowercase().contains(&q)
        })
        .collect();
    if filtered.is_empty() {
        return r#"<p class="empty-state">No plugins match this search.</p>"#.into();
    }
    let mut rows = String::from(
        r#"<div class="table-wrap"><table class="data-table">
      <thead><tr><th>Plugin</th><th>Category</th><th>Version</th><th>Pricing</th><th>Action</th></tr></thead><tbody>"#,
    );
    for entry in filtered {
        let installed = installed_ids.iter().any(|id| id == &entry.id);
        let action = if installed {
            r#"<span class="plugin-badge free">Installed</span>"#.to_string()
        } else {
            format!(
                r#"<form method="post" action="/plugins/install" class="inline-form">
            <input type="hidden" name="id" value="{id}">
            <input type="hidden" name="domain" value="{domain}">
            <button type="submit" class="btn-primary">Install</button>
          </form>"#,
                id = html_escape(&entry.id),
                domain = html_escape(domain),
            )
        };
        rows.push_str(&format!(
            r#"<tr>
          <td><strong>{name}</strong><div class="muted">{desc}</div></td>
          <td><span class="plugin-badge cat">{cat}</span></td>
          <td>{ver}</td>
          <td>{pricing}</td>
          <td>{action}</td>
        </tr>"#,
            name = html_escape(&entry.name),
            desc = html_escape(&entry.description),
            cat = html_escape(&entry.category),
            ver = html_escape(&entry.version),
            pricing = badge_pricing(&entry.pricing),
            action = action,
        ));
    }
    rows.push_str("</tbody></table></div>");
    rows
}

fn category_pills(entries: &[CatalogEntry], active: &str, domain: &str) -> String {
    let mut cats: Vec<String> = entries.iter().map(|e| e.category.clone()).collect();
    cats.sort_by_key(|c| c.to_ascii_lowercase());
    cats.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    let domain_q = format!("&amp;domain={}", urlencoding_simple(domain));
    let mut out = String::from(r#"<div class="category-pills">"#);
    out.push_str(&format!(
        r#"<a class="{cls}" href="/plugins?view=store{domain_q}">All categories</a>"#,
        cls = if active.is_empty() || active.eq_ignore_ascii_case("all") {
            "active"
        } else {
            ""
        },
        domain_q = domain_q,
    ));
    for cat in cats {
        let cls = if cat.eq_ignore_ascii_case(active) {
            "active"
        } else {
            ""
        };
        out.push_str(&format!(
            r#"<a class="{cls}" href="/plugins?view=store&amp;category={enc}{domain_q}">{label}</a>"#,
            cls = cls,
            enc = urlencoding_simple(&cat),
            domain_q = domain_q,
            label = html_escape(&cat),
        ));
    }
    out.push_str("</div>");
    out
}

fn urlencoding_simple(value: &str) -> String {
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

pub struct PluginsPageQuery<'a> {
    pub view: &'a str,
    pub layout: &'a str,
    pub q: &'a str,
    pub category: &'a str,
    pub domain: &'a str,
    pub notice: Option<&'a str>,
    pub error: Option<&'a str>,
    pub refresh: bool,
    pub sites: &'a [SiteRecord],
}

pub fn plugins_main(query: PluginsPageQuery<'_>) -> String {
    let view = if query.view == "store" {
        "store"
    } else {
        "installed"
    };
    let sites = query.sites;
    let domain = resolve_domain(sites, query.domain);
    let picker = domain_picker(sites, &domain, view);
    if domain.is_empty() {
        return format!(
            r#"{heading}
      {ok}
      {err}
      {tabs}
      <article class="section-card">
        <h2>Plugins</h2>
        {picker}
        <p class="muted">Plugins install under <code>/home/&lt;domain&gt;/plugins/&lt;plugin-id&gt;/</code> (nested for subdomains). Only sites you own or are granted appear here.</p>
      </article>"#,
            heading = section_heading("Plugins", "Installed plugins and the CPN Plugin Store.",),
            ok = notice_block("ok", query.notice),
            err = notice_block("error", query.error),
            tabs = view_tabs(view, ""),
            picker = picker,
        );
    }

    let installed = list_installed(&domain).unwrap_or_default();
    let installed_count = installed.len();
    let active_count = installed.iter().filter(|p| p.manifest.enabled).count();
    let install_path = plugins_install_path_display(Some(&domain));

    if view == "store" {
        let catalog = fetch_catalog(query.refresh);
        let (body, cache_note) = match catalog {
            Ok((entries, fetched_at)) => {
                let ids: Vec<String> = installed.iter().map(|p| p.manifest.id.clone()).collect();
                let next = catalog_next_refresh_unix(fetched_at);
                let note = format!(
                    "Plugin store data is cached for 1 hour to reduce GitHub API pressure. Last refresh: {}. Next refresh after: {}.",
                    format_unix_local(fetched_at),
                    format_unix_local(next),
                );
                (
                    format!(
                        r#"<form method="get" action="/plugins" class="stack-form" style="max-width:none;">
            <input type="hidden" name="view" value="store">
            <input type="hidden" name="domain" value="{domain}">
            <label for="q">Search</label>
            <input class="plugin-search" id="q" name="q" type="search" value="{q}" placeholder="Search plugins by name or description...">
            <button type="submit" class="btn-primary">Search</button>
          </form>
          {pills}
          {rows}
          <form method="get" action="/plugins" style="margin-top:16px;">
            <input type="hidden" name="view" value="store">
            <input type="hidden" name="domain" value="{domain}">
            <input type="hidden" name="refresh" value="1">
            <button type="submit" class="btn-secondary">Refresh catalog</button>
          </form>"#,
                        domain = html_escape(&domain),
                        q = html_escape(query.q),
                        pills = category_pills(&entries, query.category, &domain),
                        rows = store_rows(&entries, &ids, query.q, query.category, &domain),
                    ),
                    note,
                )
            }
            Err(error) => (
                format!(
                    r#"<p class="panel-notice error" role="alert">Could not load catalog: {err}</p>
          <p class="muted">Catalog URL: <a href="{url}" target="_blank" rel="noopener noreferrer">{url}</a></p>
          <form method="get" action="/plugins">
            <input type="hidden" name="view" value="store">
            <input type="hidden" name="domain" value="{domain}">
            <input type="hidden" name="refresh" value="1">
            <button type="submit" class="btn-primary">Retry refresh</button>
          </form>"#,
                    err = html_escape(&error),
                    url = html_escape(catalog_repo_url()),
                    domain = html_escape(&domain),
                ),
                "Catalog cache unavailable.".into(),
            ),
        };
        return format!(
            r#"{heading}
      {ok}
      {err}
      {tabs}
      <article class="section-card">
        <h2>Plugin Store</h2>
        {picker}
        <p class="muted">Install into <code>{path}</code> for the selected site.</p>
        <p class="panel-notice" role="note">{cache}</p>
        <p class="muted"><strong>Use at your own risk.</strong> Plugins are third-party contributions. Review them before enabling on production hosts.</p>
        <p class="muted">Catalog: <a href="{url}" target="_blank" rel="noopener noreferrer">{url}</a></p>
        {body}
      </article>"#,
            heading = section_heading("Plugins", "Installed plugins and the CPN Plugin Store.",),
            ok = notice_block("ok", query.notice),
            err = notice_block("error", query.error),
            tabs = view_tabs(view, &domain),
            picker = picker,
            path = html_escape(&install_path),
            cache = html_escape(&cache_note),
            url = html_escape(catalog_repo_url()),
            body = body,
        );
    }

    let layout = if query.layout == "table" {
        "table"
    } else {
        "grid"
    };
    let domain_q = urlencoding_simple(&domain);
    format!(
        r#"{heading}
      {ok}
      {err}
      {tabs}
      <article class="section-card">
        <h2>Installed Plugins</h2>
        {picker}
        <p class="muted">Plugins for <strong>{domain}</strong> live under <code>{path}</code>.</p>
        <div class="plugin-stats">
          <span>Installed: <strong>{installed}</strong></span>
          <span>Active: <strong>{active}</strong></span>
        </div>
        <div class="plugin-tabs">
          <a class="plugin-tab{grid}" href="/plugins?view=installed&amp;layout=grid&amp;domain={domain_q}">Grid view</a>
          <a class="plugin-tab{table}" href="/plugins?view=installed&amp;layout=table&amp;domain={domain_q}">Table view</a>
          <a class="plugin-tab" href="/plugins?view=store&amp;domain={domain_q}">Open Plugin Store</a>
        </div>
        {cards}
      </article>"#,
        heading = section_heading("Plugins", "Installed plugins and the CPN Plugin Store.",),
        ok = notice_block("ok", query.notice),
        err = notice_block("error", query.error),
        tabs = view_tabs(view, &domain),
        picker = picker,
        domain = html_escape(&domain),
        path = html_escape(&install_path),
        installed = installed_count,
        active = active_count,
        grid = if layout == "grid" { " active" } else { "" },
        table = if layout == "table" { " active" } else { "" },
        domain_q = domain_q,
        cards = installed_cards(&installed, layout, &domain),
    )
}
