//! WordPress panel HTML (list, install, manage tabs).

use crate::wordpress::{WordpressSite, list_wordpress_sites};
use crate::wordpress_manage::{PluginRow, ThemeRow, WordpressSiteSnapshot};
use crate::wordpress_wpcli::WpCliStatus;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn dash_or(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "-".to_string()
    } else {
        html_escape(trimmed)
    }
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

fn bool_badge(on: bool, on_label: &str, off_label: &str) -> String {
    if on {
        format!(r#"<span class="badge-ok">{label}</span>"#, label = html_escape(on_label))
    } else {
        format!(r#"<span class="badge-off">{label}</span>"#, label = html_escape(off_label))
    }
}

fn wp_cli_status_card(status: &WpCliStatus) -> String {
    let state = if status.available {
        ("ok", "Available")
    } else {
        ("warn", "Missing")
    };
    format!(
        r#"<article class="status-card">
          <div class="status-card-heading"><h2>WP-CLI</h2><strong class="{cls}">{state_label}</strong></div>
          <ul>
            <li><strong class="{cls}">Status</strong><span>{detail}</span></li>
            <li><strong>Binary</strong><span>{binary}</span></li>
            <li><strong>Version</strong><span>{version}</span></li>
          </ul>
          <form method="post" action="/wordpress/ensure-wpcli" class="inline-form" style="margin-top:14px;">
            <button type="submit" class="btn-secondary">Ensure WP-CLI</button>
          </form>
        </article>"#,
        cls = state.0,
        state_label = state.1,
        detail = html_escape(&status.detail),
        binary = dash_or(status.binary.as_deref().unwrap_or("-")),
        version = dash_or(status.version.as_deref().unwrap_or("-")),
    )
}

fn site_table_rows(sites: &[WordpressSite]) -> String {
    if sites.is_empty() {
        return r#"<p class="empty-state">No WordPress sites registered yet. Install WordPress on an existing site or run Scan.</p>"#.into();
    }
    let mut rows = String::from(
        r#"<div class="table-wrap"><table class="data-table">
      <thead><tr>
        <th>Domain</th><th>Title</th><th>WP version</th><th>Theme</th><th>Plugins</th><th>Owner</th><th>Actions</th>
      </tr></thead><tbody>"#,
    );
    for site in sites {
        let domain_q = html_escape(&site.domain);
        rows.push_str(&format!(
            r#"<tr>
          <td><strong>{domain}</strong><div class="muted">{url}</div></td>
          <td>{title}</td>
          <td>{wp_version}</td>
          <td>{theme}</td>
          <td>{plugin_count}</td>
          <td>{owner}</td>
          <td>
            <a class="btn-primary" style="min-height:34px;padding:0 12px;font-size:13px;" href="/wordpress/manage?domain={domain_q}">Manage</a>
          </td>
        </tr>"#,
            domain = html_escape(&site.domain),
            url = dash_or(&site.site_url),
            title = dash_or(&site.title),
            wp_version = dash_or(&site.wp_version),
            theme = dash_or(&site.theme),
            plugin_count = site.plugin_count,
            owner = dash_or(&site.owner),
            domain_q = domain_q,
        ));
    }
    rows.push_str("</tbody></table></div>");
    rows
}

pub fn wordpress_list_page(
    notice: Option<&str>,
    error: Option<&str>,
    wp_cli: &WpCliStatus,
) -> String {
    let sites = list_wordpress_sites();
    format!(
        r#"{heading}
      {ok}
      {err}
      <div class="resource-grid">
        {wp_cli_card}
        <article class="section-card">
          <h2>Quick actions</h2>
          <p class="muted">Install WordPress on a CPN site docroot, scan for existing installs, or manage registered sites.</p>
          <div style="display:flex;flex-wrap:wrap;gap:10px;margin-top:14px;">
            <a class="btn-primary" href="/wordpress/install">Install WordPress</a>
            <form method="post" action="/wordpress/scan" class="inline-form">
              <button type="submit" class="btn-secondary">Scan sites</button>
            </form>
          </div>
        </article>
      </div>
      <article class="section-card" style="margin-top:18px;">
        <h2>Registered sites ({count})</h2>
        {rows}
      </article>"#,
        heading = section_heading(
            "WordPress",
            "Install and manage WordPress on CPN website document roots. Passwords are never stored in the registry.",
        ),
        ok = notice_block("ok", notice),
        err = notice_block("error", error),
        wp_cli_card = wp_cli_status_card(wp_cli),
        count = sites.len(),
        rows = site_table_rows(&sites),
    )
}

pub fn wordpress_install_page(notice: Option<&str>, error: Option<&str>) -> String {
    format!(
        r#"{heading}
      {ok}
      {err}
      <article class="section-card">
        <h2>Install WordPress</h2>
        <p class="muted">Creates MariaDB database and user, downloads WordPress core, runs wp core install, and optionally pre-installs plugins. The CPN site must exist or enable create site below.</p>
        <form method="post" action="/wordpress/install" class="stack-form" style="max-width:560px;">
          <label>Domain<input type="text" name="domain" required placeholder="blog.example.com" autocomplete="off"></label>
          <label>Site title<input type="text" name="title" required placeholder="My blog"></label>
          <label>Site URL<input type="url" name="site_url" placeholder="https://blog.example.com"></label>
          <label>Admin username<input type="text" name="admin_user" required placeholder="admin" autocomplete="username"></label>
          <label>Admin password<input type="password" name="admin_password" required minlength="8" autocomplete="new-password"></label>
          <label>Admin email<input type="email" name="admin_email" required placeholder="you@example.com" autocomplete="email"></label>
          <label>Pre-install plugins<textarea name="plugin_sources" rows="3" placeholder="akismet, hello-dolly, https://example.com/plugin.zip"></textarea></label>
          <label style="display:flex;align-items:center;gap:8px;font-weight:600;">
            <input type="checkbox" name="create_site_if_missing" value="1"> Create CPN site if missing
          </label>
          <button type="submit" class="btn-primary">Install WordPress</button>
        </form>
      </article>"#,
        heading = section_heading(
            "Install WordPress",
            "One-click WordPress setup on a domain home docroot.",
        ),
        ok = notice_block("ok", notice),
        err = notice_block("error", error),
    )
}

fn manage_tabs(domain: &str, active: &str) -> String {
    let domain_q = html_escape(domain);
    let tabs = [
        ("general", "General"),
        ("plugins", "Plugins"),
        ("themes", "Themes"),
        ("staging", "Staging"),
        ("backups", "Backups"),
        ("database", "Database"),
    ];
    let mut html = String::from(r#"<nav class="manage-tabs" aria-label="WordPress manage tabs">"#);
    for (id, label) in tabs {
        let class = if id == active { "active" } else { "" };
        html.push_str(&format!(
            r#"<a class="{class}" href="/wordpress/manage?domain={domain_q}&amp;tab={id}">{label}</a>"#,
            class = class,
            domain_q = domain_q,
            id = html_escape(id),
            label = html_escape(label),
        ));
    }
    html.push_str("</nav>");
    html
}

fn plugin_rows_html(plugins: &[PluginRow]) -> String {
    if plugins.is_empty() {
        return r#"<p class="muted">No plugins listed.</p>"#.into();
    }
    let mut rows = String::from(
        r#"<div class="table-wrap"><table class="data-table"><thead><tr>
      <th>Name</th><th>Status</th><th>Version</th><th>Update</th></tr></thead><tbody>"#,
    );
    for p in plugins {
        rows.push_str(&format!(
            r#"<tr><td>{name}</td><td>{status}</td><td>{version}</td><td>{update}</td></tr>"#,
            name = dash_or(&p.name),
            status = dash_or(&p.status),
            version = dash_or(&p.version),
            update = dash_or(&p.update),
        ));
    }
    rows.push_str("</tbody></table></div>");
    rows
}

fn theme_rows_html(domain: &str, themes: &[ThemeRow]) -> String {
    let domain_q = html_escape(domain);
    if themes.is_empty() {
        return r#"<p class="muted">No themes listed.</p>"#.into();
    }
    let mut rows = String::from(
        r#"<div class="table-wrap"><table class="data-table"><thead><tr>
      <th>Name</th><th>Status</th><th>Version</th><th>Action</th></tr></thead><tbody>"#,
    );
    for t in themes {
        rows.push_str(&format!(
            r#"<tr><td>{name}</td><td>{status}</td><td>{version}</td><td>{action}</td></tr>"#,
            name = dash_or(&t.name),
            status = dash_or(&t.status),
            version = dash_or(&t.version),
            action = if t.status == "active" {
                "-".to_string()
            } else {
                format!(
                    r#"<form method="post" action="/wordpress/themes/activate" class="inline-form">
                      <input type="hidden" name="domain" value="{domain}">
                      <input type="hidden" name="slug" value="{slug}">
                      <button type="submit" class="btn-secondary" style="min-height:32px;padding:0 10px;">Activate</button>
                    </form>"#,
                    domain = domain_q,
                    slug = html_escape(&t.name),
                )
            },
        ));
    }
    rows.push_str("</tbody></table></div>");
    rows
}

fn tab_general(site: &WordpressSite, domain_q: &str) -> String {
    format!(
        r#"<article class="section-card">
          <h2>General</h2>
          <div class="resource-grid">
            <div class="status-card">
              <h2>Site</h2>
              <ul>
                <li><strong>Domain</strong><span>{domain}</span></li>
                <li><strong>URL</strong><span>{url}</span></li>
                <li><strong>Docroot</strong><span><code>{docroot}</code></span></li>
                <li><strong>WP version</strong><span>{wp_version}</span></li>
                <li><strong>PHP</strong><span>{php_version}</span></li>
                <li><strong>Theme</strong><span>{theme}</span></li>
                <li><strong>Plugins</strong><span>{plugin_count}</span></li>
              </ul>
            </div>
            <div class="status-card">
              <h2>Toggles</h2>
              <ul>
                <li><strong>Search indexing</strong><span>{search}</span></li>
                <li><strong>Debugging</strong><span>{debug}</span></li>
                <li><strong>Maintenance</strong><span>{maint}</span></li>
                <li><strong>Password protection</strong><span>{pw}</span></li>
              </ul>
            </div>
          </div>
          <div style="display:flex;flex-wrap:wrap;gap:10px;margin-top:16px;">
            <form method="post" action="/wordpress/refresh" class="inline-form">
              <input type="hidden" name="domain" value="{domain_q}">
              <button type="submit" class="btn-secondary">Refresh metadata</button>
            </form>
            <a class="btn-secondary" href="/websites/manage?domain={domain_q}">Open site manage</a>
          </div>
          <div class="stack-form" style="max-width:520px;margin-top:18px;">
            <form method="post" action="/wordpress/toggle/search-indexing">
              <input type="hidden" name="domain" value="{domain_q}">
              <input type="hidden" name="enabled" value="{search_next}">
              <button type="submit" class="btn-secondary">{search_btn}</button>
            </form>
            <form method="post" action="/wordpress/toggle/debugging">
              <input type="hidden" name="domain" value="{domain_q}">
              <input type="hidden" name="enabled" value="{debug_next}">
              <button type="submit" class="btn-secondary">{debug_btn}</button>
            </form>
            <form method="post" action="/wordpress/toggle/maintenance">
              <input type="hidden" name="domain" value="{domain_q}">
              <input type="hidden" name="enabled" value="{maint_next}">
              <button type="submit" class="btn-secondary">{maint_btn}</button>
            </form>
          </div>
          <form method="post" action="/wordpress/toggle/password-protection" class="stack-form" style="max-width:420px;margin-top:18px;">
            <input type="hidden" name="domain" value="{domain_q}">
            <label><input type="checkbox" name="enabled" value="1"> Enable HTTP basic protection</label>
            <label>Username<input type="text" name="username" autocomplete="off"></label>
            <label>Password<input type="password" name="password" autocomplete="new-password"></label>
            <button type="submit" class="btn-secondary">Apply protection</button>
          </form>
          <form method="post" action="/wordpress/delete" class="stack-form" style="max-width:420px;margin-top:22px;" onsubmit="return confirm('Remove WordPress registry entry? Optionally delete core files.');">
            <input type="hidden" name="domain" value="{domain_q}">
            <label style="display:flex;align-items:center;gap:8px;font-weight:600;">
              <input type="checkbox" name="remove_files" value="1"> Also delete WordPress files from docroot
            </label>
            <button type="submit" class="btn-danger">Delete WordPress</button>
          </form>
        </article>"#,
        domain = html_escape(&site.domain),
        url = dash_or(&site.site_url),
        docroot = html_escape(&site.docroot),
        wp_version = dash_or(&site.wp_version),
        php_version = dash_or(&site.php_version),
        theme = dash_or(&site.theme),
        plugin_count = site.plugin_count,
        search = bool_badge(site.search_indexing, "Indexed", "Discouraged"),
        debug = bool_badge(site.debugging, "On", "Off"),
        maint = bool_badge(site.maintenance, "On", "Off"),
        pw = bool_badge(site.password_protection, "On", "Off"),
        domain_q = domain_q,
        search_next = if site.search_indexing { "0" } else { "1" },
        debug_next = if site.debugging { "0" } else { "1" },
        maint_next = if site.maintenance { "0" } else { "1" },
        search_btn = if site.search_indexing {
            "Discourage search engines"
        } else {
            "Allow search indexing"
        },
        debug_btn = if site.debugging {
            "Disable WP_DEBUG"
        } else {
            "Enable WP_DEBUG"
        },
        maint_btn = if site.maintenance {
            "Clear maintenance mode"
        } else {
            "Enable maintenance mode"
        },
    )
}

fn tab_plugins(site: &WordpressSite, plugins: &[PluginRow]) -> String {
    let domain_q = html_escape(&site.domain);
    format!(
        r#"<article class="section-card">
          <h2>Plugins</h2>
          {rows}
          <form method="post" action="/wordpress/plugins/install" class="stack-form" style="max-width:480px;margin-top:18px;">
            <input type="hidden" name="domain" value="{domain_q}">
            <label>Install plugin (slug, ZIP path, or URL)<input type="text" name="source" required placeholder="akismet"></label>
            <button type="submit" class="btn-primary">Install and activate</button>
          </form>
        </article>"#,
        rows = plugin_rows_html(plugins),
        domain_q = domain_q,
    )
}

fn tab_themes(site: &WordpressSite, themes: &[ThemeRow]) -> String {
    format!(
        r#"<article class="section-card">
          <h2>Themes</h2>
          {rows}
        </article>"#,
        rows = theme_rows_html(&site.domain, themes),
    )
}

fn tab_staging(site: &WordpressSite) -> String {
    format!(
        r#"<article class="section-card">
          <h2>Staging</h2>
          <p class="muted">Staging clones and push workflows are planned. For now use Backups and manual docroot copy under <code>{docroot}</code>.</p>
        </article>"#,
        docroot = html_escape(&site.docroot),
    )
}

fn tab_backups(site: &WordpressSite) -> String {
    let domain_q = html_escape(&site.domain);
    format!(
        r#"<article class="section-card">
          <h2>Backups</h2>
          <p class="muted">Use the CPN Backups hub for full-site archives including <code>{docroot}</code> and the MariaDB database <code>{db}</code>.</p>
          <a class="btn-secondary" href="/backups/create?domain={domain_q}">Create backup</a>
        </article>"#,
        docroot = html_escape(&site.docroot),
        db = dash_or(&site.db_name),
        domain_q = domain_q,
    )
}

fn tab_database(site: &WordpressSite) -> String {
    format!(
        r#"<article class="section-card">
          <h2>Database</h2>
          <ul class="muted" style="list-style:none;padding:0;margin:12px 0 0;">
            <li><strong>Database:</strong> <code>{db_name}</code></li>
            <li><strong>User:</strong> <code>{db_user}</code></li>
            <li><strong>Password:</strong> - (not stored in CPN registry)</li>
          </ul>
          <a class="btn-secondary" href="/databases/manager" style="margin-top:14px;display:inline-flex;">Open MariaDB Manager</a>
        </article>"#,
        db_name = dash_or(&site.db_name),
        db_user = dash_or(&site.db_user),
    )
}

pub fn wordpress_manage_page(
    snapshot: &WordpressSiteSnapshot,
    tab: &str,
    notice: Option<&str>,
    error: Option<&str>,
) -> String {
    let site = &snapshot.site;
    let domain_q = html_escape(&site.domain);
    let active_tab = match tab {
        "plugins" | "themes" | "staging" | "backups" | "database" => tab,
        _ => "general",
    };
    let body = match active_tab {
        "plugins" => tab_plugins(site, &snapshot.plugins),
        "themes" => tab_themes(site, &snapshot.themes),
        "staging" => tab_staging(site),
        "backups" => tab_backups(site),
        "database" => tab_database(site),
        _ => tab_general(site, &domain_q),
    };
    format!(
        r#"{heading}
      {ok}
      {err}
      {tabs}
      {body}"#,
        heading = section_heading(
            &format!("WordPress: {}", site.domain),
            "Manage plugins, themes, maintenance, and site metadata.",
        ),
        ok = notice_block("ok", notice),
        err = notice_block("error", error),
        tabs = manage_tabs(&site.domain, active_tab),
        body = body,
    )
}
