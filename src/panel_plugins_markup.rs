//! Markup helpers for the Plugins hub (tabs, installed cards, store catalog).

use crate::panel_admin::is_panel_admin;
use crate::panel_plugins_spa::plugins_hub_styles;
use crate::plugin_activation::is_host_owned_install;
use crate::plugins::InstalledPlugin;
use crate::site_acl::{SitePerm, can_manage_site};
use crate::sites::SiteRecord;
use crate::uninstall_confirm::{plugin_uninstall_impacts, uninstall_form_attrs};

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
    let store = if active == "store" || active == "host" {
        " active"
    } else {
        ""
    };
    let domain_q = if domain.is_empty() {
        String::new()
    } else {
        format!("&amp;domain={}", urlencoding_simple(domain))
    };
    format!(
        r#"<div class="plugin-tabs" role="tablist" aria-label="Plugins views">
        <a class="plugin-tab{installed}" href="/plugins?view=installed{domain_q}" role="tab">Installed</a>
        <a class="plugin-tab{store}" href="/plugins?view=store{domain_q}" role="tab">Store</a>
      </div>
      <style>
        .plugin-tabs {{ display:flex; flex-wrap:wrap; gap:8px; margin:0 0 18px; }}
        .plugin-tab {{
          display:inline-flex; align-items:center; min-height:40px; padding:0 16px;
          border-radius:999px; border:1px solid var(--hairline); background:var(--canvas);
          color:var(--ink); font-size:14px; font-weight:600;
        }}
        .plugin-tab.active {{ background:#e7f1ff; color:#0b3d91; border-color:#93c5fd; }}
        .store-scope-toggle {{ display:flex; flex-wrap:wrap; align-items:center; gap:8px; margin:0 0 12px; }}
        .store-scope-label {{ font-size:13px; font-weight:600; color:var(--ink); margin-right:4px; }}
        .store-scope-btn {{
          display:inline-flex; align-items:center; min-height:36px; padding:0 14px;
          border-radius:999px; border:1px solid var(--hairline); background:var(--canvas);
          color:var(--ink); font-size:13px; font-weight:600; text-decoration:none;
        }}
        .store-scope-btn.active {{ background:#e7f1ff; color:#0b3d91; border-color:#93c5fd; }}
        .store-scope-hint {{ margin:8px 0 0; font-size:13px; }}
        .store-scope-toggle[hidden], #store-host-hint[hidden], #store-cpn-hint[hidden], #store-site-picker[hidden] {{ display:none; }}
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
        domain_q = domain_q,
        hub_styles = plugins_hub_styles(),
    )
}

pub(crate) fn resolve_store_target(
    requested: &str,
    category: &str,
    sites: &[SiteRecord],
) -> &'static str {
    match requested.trim().to_ascii_lowercase().as_str() {
        "host" => "host",
        "cpn" | "cpn_only" | "cpn-only" | "account" => "cpn",
        "site" => "site",
        _ => {
            if category.trim().eq_ignore_ascii_case("host") || sites.is_empty() {
                "host"
            } else {
                "site"
            }
        }
    }
}

fn store_target_href(
    view: &str,
    target: &str,
    category: &str,
    domain: &str,
    mode: &str,
    per_page: usize,
    q: &str,
) -> String {
    let mut href = format!(
        "/plugins?view={}&target={}",
        urlencoding_simple(view),
        urlencoding_simple(target)
    );
    if !category.trim().is_empty() {
        href.push_str(&format!(
            "&category={}",
            urlencoding_simple(category.trim())
        ));
    }
    if target == "site" && !domain.trim().is_empty() {
        href.push_str(&format!("&domain={}", urlencoding_simple(domain.trim())));
    }
    href.push_str(&format!(
        "&mode={}&per_page={}",
        urlencoding_simple(mode),
        per_page
    ));
    if !q.trim().is_empty() {
        href.push_str(&format!("&q={}", urlencoding_simple(q.trim())));
    }
    href
}

pub(crate) fn store_scope_query_suffix(
    domain: &str,
    target: &str,
    mode: &str,
    per_page: usize,
    q: &str,
) -> String {
    let mut out = format!(
        "&amp;mode={}&amp;per_page={}",
        urlencoding_simple(mode),
        per_page
    );
    match target {
        "host" => out.push_str("&amp;target=host"),
        "cpn" => out.push_str("&amp;target=cpn"),
        _ => {
            out.push_str("&amp;target=site");
            if !domain.is_empty() {
                out.push_str(&format!("&amp;domain={}", urlencoding_simple(domain)));
            }
        }
    }
    if !q.trim().is_empty() {
        out.push_str(&format!("&amp;q={}", urlencoding_simple(q)));
    }
    out
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn store_install_target_picker(
    sites: &[SiteRecord],
    selected_domain: &str,
    target: &str,
    view: &str,
    category: &str,
    q: &str,
    mode: &str,
    per_page: usize,
    show_host: bool,
) -> String {
    let host_active = if target == "host" { " active" } else { "" };
    let cpn_active = if target == "cpn" { " active" } else { "" };
    let site_active = if target == "site" { " active" } else { "" };
    let host_href = store_target_href(view, "host", category, "", mode, per_page, q);
    let cpn_href = store_target_href(view, "cpn", category, "", mode, per_page, q);
    let site_href = store_target_href(view, "site", category, selected_domain, mode, per_page, q);
    let host_btn = if show_host {
        format!(
            r#"<a class="store-scope-btn{host_active}" href="{host_href}" data-store-target-btn="host">Host</a>"#,
            host_active = host_active,
            host_href = html_escape(&host_href),
        )
    } else {
        String::new()
    };
    let toggle = format!(
        r#"<div class="store-scope-toggle" role="group" aria-label="Install target">
        <span class="store-scope-label">Install target</span>
        {host_btn}
        <a class="store-scope-btn{cpn_active}" href="{cpn_href}" data-store-target-btn="cpn">CPN only</a>
        <a class="store-scope-btn{site_active}" href="{site_href}" data-store-target-btn="site">Site</a>
      </div>"#,
        host_btn = host_btn,
        cpn_active = cpn_active,
        cpn_href = html_escape(&cpn_href),
        site_active = site_active,
        site_href = html_escape(&site_href),
    );
    let host_hint = if show_host {
        format!(
            r#"<p id="store-host-hint" class="muted store-scope-hint"{hidden}>Host packages install once on this server for every user (Postfix, Tachyon, Roundcube). Sites only Activate or Deactivate them. The site dropdown stays hidden for Host.</p>"#,
            hidden = if target == "host" { "" } else { " hidden" },
        )
    } else {
        String::new()
    };
    let cpn_hint = format!(
        r#"<p id="store-cpn-hint" class="muted store-scope-hint"{hidden}>CPN only installs for your signed-in CPN account under <code>/var/lib/cpn/user-plugins/&lt;user&gt;/</code> (not a public website feature). Use this for panel tools such as Auto Ban Security Alerts. Host stays owner/admin only.</p>"#,
        hidden = if target == "cpn" { "" } else { " hidden" },
    );
    if sites.is_empty() {
        return format!(
            r#"{toggle}
        {host_hint}
        {cpn_hint}
        <p class="muted">No websites yet. CPN-only plugins still install for your account. Create a site to install domain plugins under <code>/home/&lt;domain&gt;/plugins/</code>.</p>"#,
            toggle = toggle,
            host_hint = host_hint,
            cpn_hint = cpn_hint,
        );
    }
    let mut options = String::new();
    for site in sites {
        let sel = if site.domain == selected_domain {
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
    let site_hidden = if target == "site" { "" } else { " hidden" };
    format!(
        r#"{toggle}
      {host_hint}
      {cpn_hint}
      <div id="store-site-picker"{site_hidden}>
      <form method="get" action="/plugins" class="domain-picker">
        <input type="hidden" name="view" value="{view}">
        <input type="hidden" name="target" value="site">
        <input type="hidden" name="mode" value="{mode}">
        <input type="hidden" name="per_page" value="{per_page}">
        <input type="hidden" name="category" value="{category}">
        <input type="hidden" name="q" value="{q}">
        <div>
          <label for="domain"><strong>Site</strong> (for site plugins)</label><br>
          <select id="domain" name="domain">{options}</select>
        </div>
        <button type="submit" class="btn-secondary">Apply</button>
      </form>
      <p class="muted store-scope-hint">Site installs live under the selected domain or sub-domain (<code>/home/&lt;domain&gt;/plugins/</code>) for public site features (BIMI, MTA-STS). CPN only is for your panel account, not a random subdomain.</p>
      </div>"#,
        toggle = toggle,
        host_hint = host_hint,
        cpn_hint = cpn_hint,
        site_hidden = site_hidden,
        view = html_escape(view),
        mode = html_escape(mode),
        per_page = per_page,
        category = html_escape(category),
        q = html_escape(q),
        options = options,
    )
}

#[cfg(test)]
mod store_target_tests {
    use super::*;
    use crate::sites::SiteRecord;

    #[test]
    fn resolve_store_target_defaults() {
        let sites = vec![SiteRecord {
            schema_version: 1,
            domain: "example.com".into(),
            owner: "admin".into(),
            docroot: "/home/example.com/public_html".into(),
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
            aliases: vec![],
            staging_of: None,
        }];
        assert_eq!(resolve_store_target("", "Host", &sites), "host");
        assert_eq!(resolve_store_target("", "", &sites), "site");
        assert_eq!(resolve_store_target("host", "", &sites), "host");
        assert_eq!(resolve_store_target("cpn", "", &sites), "cpn");
        assert_eq!(resolve_store_target("", "", &[]), "host");
    }

    #[test]
    fn store_picker_shows_host_toggle() {
        let html = store_install_target_picker(&[], "", "host", "store", "", "", "page", 4, true);
        assert!(html.contains("Install target"));
        assert!(html.contains("data-store-target-btn=\"host\""));
        assert!(html.contains("data-store-target-btn=\"cpn\""));
        assert!(html.contains("store-scope-btn active"));
        assert!(html.contains(">Host</a>"));
        let no_host =
            store_install_target_picker(&[], "", "cpn", "store", "", "", "page", 4, false);
        assert!(!no_host.contains("data-store-target-btn=\"host\""));
        assert!(no_host.contains("data-store-target-btn=\"cpn\""));
        assert!(no_host.contains("data-store-target-btn=\"site\""));
        assert!(no_host.contains("CPN only installs for your signed-in CPN account"));
    }
}

pub(crate) fn domain_picker(sites: &[SiteRecord], selected: &str, view: &str) -> String {
    if sites.is_empty() {
        return r#"<p class="muted">No websites yet. Host packages still appear under Installed / Host. Create a site to install domain or sub-domain plugins under <code>/home/&lt;domain&gt;/plugins/</code>.</p>"#.into();
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

pub(crate) fn installed_one_card(item: &InstalledPlugin, domain: &str, username: &str) -> String {
    let html = installed_cards(std::slice::from_ref(item), "grid", domain, username);
    html.replace(r#"<div class="plugin-grid">"#, "")
        .replacen("</div>", "", 1)
}

pub(crate) fn installed_cards(
    plugins: &[InstalledPlugin],
    layout: &str,
    domain: &str,
    username: &str,
) -> String {
    if plugins.is_empty() {
        return r#"<p class="empty-state">No plugins installed for this site yet. Open the Store to install from the community catalog.</p>"#
            .into();
    }
    if layout == "table" {
        return installed_table(plugins, domain, username);
    }
    let admin = is_panel_admin(username);
    let mut cards = String::from(r#"<div class="plugin-grid">"#);
    for item in plugins {
        let m = &item.manifest;
        let host_owned = is_host_owned_install(domain, &m.id) || m.source == "host-activation";
        let active = if m.enabled { "Yes" } else { "No" };
        let status = if host_owned {
            "Activated (host)"
        } else {
            "Installed"
        };
        let active_badge = if m.enabled {
            r#"<span class="plugin-badge installed">Active</span>"#
        } else {
            r#"<span class="plugin-badge">Deactivated</span>"#
        };
        let scope_badge = if host_owned {
            r#"<span class="plugin-badge">Host</span>"#
        } else {
            r#"<span class="plugin-badge cat">Site</span>"#
        };
        let toggle = toggle_form(m.enabled, &m.id, domain, host_owned);
        let can_uninstall_site = !host_owned
            && (admin || can_manage_site(username, domain, SitePerm::Uninstall).unwrap_or(false));
        // Host-installed packages: Uninstall from Host is panel-admin only.
        // Site plugins: Uninstall only with site Uninstall ACL (owners always allowed).
        let uninstall = if host_owned && !admin {
            String::new()
        } else if host_owned && admin {
            let impacts = plugin_uninstall_impacts(domain, &m.id, &m.name);
            let form_attrs = uninstall_form_attrs(&m.name, &impacts);
            format!(
                r#"<form method="post" action="/plugins/uninstall-host" {form_attrs}>
              <input type="hidden" name="id" value="{id}">
              <input type="hidden" name="confirm" value="">
              <button type="submit" class="btn-danger">Uninstall from Host</button>
            </form>"#,
                form_attrs = form_attrs,
                id = html_escape(&m.id),
            )
        } else if can_uninstall_site {
            let impacts = plugin_uninstall_impacts(domain, &m.id, &m.name);
            let form_attrs = uninstall_form_attrs(&m.name, &impacts);
            format!(
                r#"<form method="post" action="/plugins/uninstall" {form_attrs}>
              <input type="hidden" name="id" value="{id}">
              <input type="hidden" name="domain" value="{domain}">
              <input type="hidden" name="confirm" value="">
              <button type="submit" class="btn-danger">Uninstall</button>
            </form>"#,
                form_attrs = form_attrs,
                id = html_escape(&m.id),
                domain = html_escape(domain),
            )
        } else {
            String::new()
        };
        cards.push_str(&format!(
            r#"<article class="plugin-card">
          <h3>{name}</h3>
          <div class="plugin-badges">
            {scope}
            <span class="plugin-badge cat">{cat}</span>
            <span class="plugin-badge">v{ver}</span>
            {pricing}
            {active_badge}
          </div>
          <p class="plugin-desc">{desc}</p>
          <p class="plugin-meta">Status: {status} · Active: {active}</p>
          <div class="plugin-actions">
            <a class="btn-secondary" href="/plugins/settings?domain={domain_q}&amp;id={id}">Settings</a>
            {toggle}
            {uninstall}
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
            scope = scope_badge,
            active_badge = active_badge,
            desc = html_escape(&m.description),
            status = status,
            active = active,
            toggle = toggle,
            uninstall = uninstall,
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

fn toggle_form(enabled: bool, id: &str, domain: &str, host_owned: bool) -> String {
    let (action, label, cls) = if enabled {
        if host_owned {
            ("/plugins/deactivate-host", "Deactivate", "btn-warn")
        } else {
            ("/plugins/disable", "Deactivate", "btn-warn")
        }
    } else if host_owned {
        ("/plugins/activate-host", "Activate", "btn-primary")
    } else {
        ("/plugins/enable", "Activate", "btn-primary")
    };
    format!(
        r#"<form method="post" action="{action}" class="inline-form">
            <input type="hidden" name="id" value="{id}">
            <input type="hidden" name="domain" value="{domain}">
            <button type="submit" class="{cls}">{label}</button>
          </form>"#,
        action = action,
        id = html_escape(id),
        domain = html_escape(domain),
        cls = cls,
        label = label,
    )
}

fn installed_table(plugins: &[InstalledPlugin], domain: &str, username: &str) -> String {
    let admin = is_panel_admin(username);
    let mut rows = String::from(
        r#"<div class="table-wrap"><table class="data-table">
        <thead><tr><th>Plugin</th><th>Category</th><th>Version</th><th>Status</th><th>Actions</th></tr></thead><tbody>"#,
    );
    for item in plugins {
        let m = &item.manifest;
        let host_owned = is_host_owned_install(domain, &m.id) || m.source == "host-activation";
        let active = if m.enabled {
            if host_owned {
                "Activated (host)"
            } else {
                "Active"
            }
        } else {
            "Inactive"
        };
        let toggle = toggle_form(m.enabled, &m.id, domain, host_owned);
        let can_uninstall_site = !host_owned
            && (admin || can_manage_site(username, domain, SitePerm::Uninstall).unwrap_or(false));
        let uninstall = if host_owned && !admin {
            String::new()
        } else if host_owned && admin {
            let impacts = plugin_uninstall_impacts(domain, &m.id, &m.name);
            let form_attrs = uninstall_form_attrs(&m.name, &impacts);
            format!(
                r#"<form method="post" action="/plugins/uninstall-host" {form_attrs}>
                <input type="hidden" name="id" value="{id}">
                <input type="hidden" name="confirm" value="">
                <button type="submit" class="btn-danger">Uninstall from Host</button>
              </form>"#,
                form_attrs = form_attrs,
                id = html_escape(&m.id),
            )
        } else if can_uninstall_site {
            let impacts = plugin_uninstall_impacts(domain, &m.id, &m.name);
            let form_attrs = uninstall_form_attrs(&m.name, &impacts);
            format!(
                r#"<form method="post" action="/plugins/uninstall" {form_attrs}>
                <input type="hidden" name="id" value="{id}">
                <input type="hidden" name="domain" value="{domain}">
                <input type="hidden" name="confirm" value="">
                <button type="submit" class="btn-danger">Uninstall</button>
              </form>"#,
                form_attrs = form_attrs,
                id = html_escape(&m.id),
                domain = html_escape(domain),
            )
        } else {
            String::new()
        };
        rows.push_str(&format!(
            r#"<tr>
            <td><strong>{name}</strong><div class="muted">{id}</div></td>
            <td>{cat}</td>
            <td>v{ver}</td>
            <td>{active}</td>
            <td class="plugin-actions">
              <a class="btn-secondary" href="/plugins/settings?domain={domain_q}&amp;id={id}">Settings</a>
              {toggle}
              {uninstall}
            </td>
          </tr>"#,
            name = html_escape(&m.name),
            id = html_escape(&m.id),
            cat = html_escape(&m.category),
            ver = html_escape(&m.version),
            active = active,
            toggle = toggle,
            uninstall = uninstall,
            domain_q = urlencoding_simple(domain),
        ));
    }
    rows.push_str("</tbody></table></div>");
    rows
}
