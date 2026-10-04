//! WordPress panel HTML (list, install, manage tabs).

use crate::backups::is_subdomain_site;
use crate::panel_list_search::{
    domain_matches_q, list_filter_summary, list_search_form, normalize_list_q,
};
use crate::wordpress::{WordpressSite, list_wordpress_sites};
use crate::wordpress_manage::{PluginRow, ThemeRow, WordpressSiteSnapshot};
use crate::wordpress_wpcli::{WpCliStatus, format_wp_cli_binary};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct WordpressInstallDraft {
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub site_url: String,
    #[serde(default)]
    pub admin_user: String,
    #[serde(default)]
    pub admin_password: String,
    #[serde(default)]
    pub admin_email: String,
    #[serde(default)]
    pub plugin_sources: String,
    #[serde(default)]
    pub create_site_if_missing: bool,
}

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
        format!(
            r#"<span class="badge-ok">{label}</span>"#,
            label = html_escape(on_label)
        )
    } else {
        format!(
            r#"<span class="badge-off">{label}</span>"#,
            label = html_escape(off_label)
        )
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
        binary = dash_or(
            status
                .binary
                .as_deref()
                .map(format_wp_cli_binary)
                .as_deref()
                .unwrap_or("-"),
        ),
        version = dash_or(status.version.as_deref().unwrap_or("-")),
    )
}

fn site_table_rows(sites: &[WordpressSite]) -> String {
    if sites.is_empty() {
        return r#"<p class="empty-state">No WordPress sites in this list yet.</p>"#.into();
    }
    let mut rows = String::from(
        r#"<div class="table-wrap"><table class="data-table">
      <thead><tr>
        <th>Domain</th><th>Parent</th><th>Title</th><th>WP version</th><th>Theme</th><th>Plugins</th><th>Owner</th><th>Actions</th>
      </tr></thead><tbody>"#,
    );
    for site in sites {
        let domain_q = html_escape(&site.domain);
        let parent = crate::sites::resolve_parent_domain(&site.domain)
            .ok()
            .flatten()
            .unwrap_or_default();
        let parent_cell = if parent.is_empty() {
            "-".to_string()
        } else {
            format!(
                r#"<a href="/websites/manage?domain={p}">{p}</a>"#,
                p = html_escape(&parent)
            )
        };
        rows.push_str(&format!(
            r#"<tr>
          <td><strong>{domain}</strong><div class="muted">{url}</div></td>
          <td>{parent}</td>
          <td>{title}</td>
          <td>{wp_version}</td>
          <td>{theme}</td>
          <td>{plugin_count}</td>
          <td>{owner}</td>
          <td>
            <a class="btn-primary" style="min-height:34px;padding:0 12px;font-size:13px;" href="/wordpress/manage?domain={domain_q}">Manage</a>
            <form method="post" action="/wordpress/delete" class="inline-form" onsubmit="return confirm('Remove WordPress registry entry for {domain}? Optionally delete core files.');">
              <input type="hidden" name="domain" value="{domain_q}">
              <label><input type="checkbox" name="remove_files" value="1"> Also delete files</label>
              <button type="submit" class="btn-danger">Delete WordPress</button>
            </form>
          </td>
        </tr>"#,
            domain = html_escape(&site.domain),
            parent = parent_cell,
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

fn filtered_wordpress_sites(subsites: bool, q: &str) -> (usize, Vec<WordpressSite>) {
    let all: Vec<_> = list_wordpress_sites()
        .into_iter()
        .filter(|site| is_subdomain_site(&site.domain) == subsites)
        .collect();
    let total = all.len();
    let sites = all
        .into_iter()
        .filter(|site| {
            let parent = if subsites {
                crate::sites::resolve_parent_domain(&site.domain)
                    .ok()
                    .flatten()
            } else {
                None
            };
            domain_matches_q(&site.domain, parent.as_deref(), q)
        })
        .collect();
    (total, sites)
}

pub fn wordpress_list_page(
    notice: Option<&str>,
    error: Option<&str>,
    wp_cli: &WpCliStatus,
    q_raw: Option<&str>,
) -> String {
    let q = normalize_list_q(q_raw);
    let (total, sites) = filtered_wordpress_sites(false, &q);
    let rows = if total == 0 {
        site_table_rows(&sites)
    } else if sites.is_empty() {
        format!(
            r#"<p class="empty-state">No WordPress sites match <strong>{}</strong>. <a href="/wordpress/list">Clear search</a>.</p>"#,
            html_escape(q_raw.unwrap_or("").trim())
        )
    } else {
        site_table_rows(&sites)
    };
    format!(
        r#"{heading}
      {ok}
      {err}
      <div class="resource-grid">
        {wp_cli_card}
        <article class="section-card">
          <h2>Quick actions</h2>
          <p class="muted">Install WordPress on a main CPN website, scan for existing installs, or open WordPress Sub-sites for nested domains.</p>
          <div style="display:flex;flex-wrap:wrap;gap:10px;margin-top:14px;">
            <a class="btn-primary" href="/wordpress/install">Install WordPress</a>
            <a class="btn-secondary" href="/wordpress/subsites">WordPress Sub-sites</a>
            <form method="post" action="/wordpress/scan" class="inline-form">
              <button type="submit" class="btn-secondary">Scan sites</button>
            </form>
          </div>
        </article>
      </div>
      <article class="section-card" style="margin-top:18px;">
        <h2>WordPress sites ({count})</h2>
        <p class="muted">Main domains only. Nested WordPress installs are under <a href="/wordpress/subsites">WordPress Sub-sites</a>.</p>
        {search}
        {filter_summary}
        {rows}
      </article>"#,
        heading = section_heading(
            "WordPress",
            "Install and manage WordPress on main website document roots. Passwords are never stored in the registry.",
        ),
        ok = notice_block("ok", notice),
        err = notice_block("error", error),
        wp_cli_card = wp_cli_status_card(wp_cli),
        count = sites.len(),
        search = list_search_form(
            "/wordpress/list",
            q_raw.unwrap_or("").trim(),
            "Search by domain"
        ),
        filter_summary = list_filter_summary(sites.len(), total, q_raw.unwrap_or("")),
        rows = rows,
    )
}

pub fn wordpress_subsites_list_page(
    notice: Option<&str>,
    error: Option<&str>,
    wp_cli: &WpCliStatus,
    q_raw: Option<&str>,
) -> String {
    let q = normalize_list_q(q_raw);
    let (total, sites) = filtered_wordpress_sites(true, &q);
    let rows = if total == 0 {
        site_table_rows(&sites)
    } else if sites.is_empty() {
        format!(
            r#"<p class="empty-state">No WordPress sub-sites match <strong>{}</strong>. <a href="/wordpress/subsites">Clear search</a>.</p>"#,
            html_escape(q_raw.unwrap_or("").trim())
        )
    } else {
        site_table_rows(&sites)
    };
    format!(
        r#"{heading}
      {ok}
      {err}
      <div class="resource-grid">
        {wp_cli_card}
        <article class="section-card">
          <h2>Quick actions</h2>
          <p class="muted">Install WordPress on a CPN sub-domain, or open main WordPress Sites.</p>
          <div style="display:flex;flex-wrap:wrap;gap:10px;margin-top:14px;">
            <a class="btn-primary" href="/wordpress/subsites/install">Install WordPress Sub-site</a>
            <a class="btn-secondary" href="/wordpress/list">WordPress Sites</a>
            <form method="post" action="/wordpress/subsites/scan" class="inline-form">
              <button type="submit" class="btn-secondary">Scan sites</button>
            </form>
          </div>
        </article>
      </div>
      <article class="section-card" style="margin-top:18px;">
        <h2>WordPress sub-sites ({count})</h2>
        <p class="muted">Sub-domains only. Main WordPress installs are under <a href="/wordpress/list">WordPress Sites</a>.</p>
        {search}
        {filter_summary}
        {rows}
      </article>"#,
        heading = section_heading(
            "WordPress Sub-sites",
            "WordPress installs on nested domains under a parent website.",
        ),
        ok = notice_block("ok", notice),
        err = notice_block("error", error),
        wp_cli_card = wp_cli_status_card(wp_cli),
        count = sites.len(),
        search = list_search_form(
            "/wordpress/subsites",
            q_raw.unwrap_or("").trim(),
            "Search by domain or parent",
        ),
        filter_summary = list_filter_summary(sites.len(), total, q_raw.unwrap_or("")),
        rows = rows,
    )
}

fn wordpress_install_form(
    action: &str,
    domain_hint: &str,
    create_label: &str,
    default_admin_user: &str,
    draft: Option<&WordpressInstallDraft>,
) -> String {
    let d = draft.cloned().unwrap_or_default();
    let admin_user = if d.admin_user.trim().is_empty() {
        default_admin_user.trim()
    } else {
        d.admin_user.trim()
    };
    let admin_placeholder = if default_admin_user.trim().is_empty() {
        "cpnowner"
    } else {
        default_admin_user.trim()
    };
    let create_checked = if d.create_site_if_missing {
        " checked"
    } else {
        ""
    };
    format!(
        r#"<form method="post" action="{action}" class="stack-form cpn-wp-install-form" style="max-width:560px;">
          <label>Domain<input type="text" name="domain" required placeholder="{hint}" autocomplete="off" value="{domain}"></label>
          <label>Site title<input type="text" name="title" required placeholder="My blog" value="{title}"></label>
          <label>Site URL<input type="url" name="site_url" placeholder="https://{hint}" value="{site_url}"></label>
          <label>Admin username<input type="text" name="admin_user" required placeholder="{admin_ph}" autocomplete="username" value="{admin_user}"></label>
          <label>Admin password<input type="password" name="admin_password" required minlength="8" autocomplete="new-password" value="{admin_password}"></label>
          <label>Admin email<input type="email" name="admin_email" required placeholder="you@example.com" autocomplete="email" value="{admin_email}"></label>
          <label>Pre-install plugins (slug or URL)<textarea name="plugin_sources" rows="3" placeholder="akismet, hello-dolly, https://example.com/plugin.zip">{plugin_sources}</textarea></label>
          <label>Plugin ZIP uploads<input type="file" class="cpn-wp-plugin-zips" accept=".zip,application/zip" multiple></label>
          <input type="hidden" name="plugin_zips_json" class="cpn-wp-plugin-zips-json" value="">
          <p class="muted" style="margin-top:-6px;">Optional. Select one or more plugin <code>.zip</code> files (max 40&nbsp;MB each). Installed with WP-CLI after core setup, alongside slug/URL plugins above.</p>
          <label style="display:flex;align-items:center;gap:8px;font-weight:600;">
            <input type="checkbox" name="create_site_if_missing" value="1"{create_checked}> {create_label}
          </label>
          <button type="submit" class="btn-primary">Install WordPress</button>
        </form>
        <script>
        (function(){{
          function bind(form){{
            if (!form || form.getAttribute('data-cpn-wp-zip') === '1') return;
            form.setAttribute('data-cpn-wp-zip', '1');
            var input = form.querySelector('.cpn-wp-plugin-zips');
            var hidden = form.querySelector('.cpn-wp-plugin-zips-json');
            if (!input || !hidden) return;
            form.addEventListener('submit', function(ev){{
              var files = Array.prototype.slice.call(input.files || []);
              if (!files.length) return;
              ev.preventDefault();
              var btn = form.querySelector('button[type=submit]');
              if (btn) {{ btn.disabled = true; btn.textContent = 'Preparing ZIP uploads...'; }}
              var out = [];
              var i = 0;
              function next(){{
                if (i >= files.length) {{
                  hidden.value = JSON.stringify(out);
                  form.submit();
                  return;
                }}
                var f = files[i++];
                if (!/\.zip$/i.test(f.name)) {{ next(); return; }}
                if (f.size > 40*1024*1024) {{
                  alert('Plugin ZIP exceeds 40 MB: ' + f.name);
                  if (btn) {{ btn.disabled = false; btn.textContent = 'Install WordPress'; }}
                  return;
                }}
                var reader = new FileReader();
                reader.onload = function(){{
                  var dataUrl = String(reader.result || '');
                  var b64 = dataUrl.indexOf(',') >= 0 ? dataUrl.split(',')[1] : dataUrl;
                  out.push({{ name: f.name, data: b64 }});
                  next();
                }};
                reader.onerror = function(){{
                  alert('Could not read ' + f.name);
                  if (btn) {{ btn.disabled = false; btn.textContent = 'Install WordPress'; }}
                }};
                reader.readAsDataURL(f);
              }}
              next();
            }});
          }}
          document.querySelectorAll('form.cpn-wp-install-form').forEach(bind);
        }})();
        </script>"#,
        action = html_escape(action),
        hint = html_escape(domain_hint),
        create_label = html_escape(create_label),
        domain = html_escape(&d.domain),
        title = html_escape(&d.title),
        site_url = html_escape(&d.site_url),
        admin_ph = html_escape(admin_placeholder),
        admin_user = html_escape(admin_user),
        admin_password = html_escape(&d.admin_password),
        admin_email = html_escape(&d.admin_email),
        plugin_sources = html_escape(&d.plugin_sources),
        create_checked = create_checked,
    )
}

pub fn wordpress_install_page(
    notice: Option<&str>,
    error: Option<&str>,
    default_admin_user: &str,
    draft: Option<&WordpressInstallDraft>,
) -> String {
    format!(
        r#"{heading}
      {ok}
      {err}
      <article class="section-card">
        <h2>Install WordPress</h2>
        <p class="muted">Installs on a <strong>main</strong> website docroot. For nested domains use <a href="/wordpress/subsites/install">Install WordPress Sub-site</a>.</p>
        <p class="muted">This page creates a <strong>new</strong> WordPress site and can pre-install plugins (slug, URL, or ZIP upload). Full site ZIP restore (existing content + database) belongs under <a href="/backups/restore">Backups / Restore</a>.</p>
        {form}
      </article>"#,
        heading = section_heading(
            "Install WordPress",
            "One-click WordPress setup on a main domain home docroot.",
        ),
        ok = notice_block("ok", notice),
        err = notice_block("error", error),
        form = wordpress_install_form(
            "/wordpress/install",
            "example.com",
            "Create CPN main site if missing",
            default_admin_user,
            draft,
        ),
    )
}

pub fn wordpress_subsites_install_page(
    notice: Option<&str>,
    error: Option<&str>,
    default_admin_user: &str,
    draft: Option<&WordpressInstallDraft>,
) -> String {
    format!(
        r#"{heading}
      {ok}
      {err}
      <article class="section-card">
        <h2>Install WordPress Sub-site</h2>
        <p class="muted">Installs on a <strong>sub-domain</strong> docroot. Parent website must exist first. Main domains use <a href="/wordpress/install">Install WordPress</a>.</p>
        <p class="muted">New WordPress + plugins only. Full site ZIP restore uses <a href="/backups/restore">Backups / Restore</a>.</p>
        {form}
      </article>"#,
        heading = section_heading(
            "Install WordPress Sub-site",
            "One-click WordPress setup on a nested domain home docroot.",
        ),
        ok = notice_block("ok", notice),
        err = notice_block("error", error),
        form = wordpress_install_form(
            "/wordpress/subsites/install",
            "blog.example.com",
            "Create CPN sub-domain if missing (parent must exist)",
            default_admin_user,
            draft,
        ),
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
