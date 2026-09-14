//! HTML for CPN Panel Plugins (Installed + Store + Host packages).

use crate::panel_plugins_markup::{
    StoreListOpts, category_pills, domain_picker, html_escape, installed_cards, notice_block,
    resolve_domain, section_heading, store_catalog, urlencoding_simple, view_tabs,
};
use crate::panel_plugins_spa::{
    list_mode_from_query, page_from_query, per_page_from_query, plugins_hub_script,
};
use crate::plugins::{
    catalog_next_refresh_unix, catalog_repo_url, fetch_catalog, format_unix_local, list_installed,
    plugins_install_path_display,
};
use crate::sites::SiteRecord;

pub struct PluginsPageQuery<'a> {
    pub view: &'a str,
    pub layout: &'a str,
    pub q: &'a str,
    pub category: &'a str,
    pub domain: &'a str,
    pub notice: Option<&'a str>,
    pub error: Option<&'a str>,
    pub refresh: bool,
    pub mode: &'a str,
    pub page: usize,
    pub per_page: usize,
    pub sites: &'a [SiteRecord],
}

fn wrap_hub(inner: String) -> String {
    format!(
        r#"<div id="plugins-hub" data-plugins-hub="1">{inner}</div>
{script}"#,
        inner = inner,
        script = plugins_hub_script(),
    )
}

pub fn plugins_main(query: PluginsPageQuery<'_>) -> String {
    wrap_hub(plugins_main_inner(query))
}

fn plugins_main_inner(query: PluginsPageQuery<'_>) -> String {
    let view = match query.view {
        "store" | "view-store" => "store",
        "host" | "apps" => "host",
        _ => "installed",
    };
    let mode = list_mode_from_query(query.mode);
    let per_page = per_page_from_query(&query.per_page.to_string());
    let page = page_from_query(&query.page.to_string());
    let sites = query.sites;
    let domain = resolve_domain(sites, query.domain);
    let picker = domain_picker(sites, &domain, view);

    if view == "host" {
        let apps_body = crate::panel_apps::apps_main(crate::panel_apps::AppsPageQuery {
            domain: &domain,
            notice: query.notice,
            error: query.error,
            sites,
            q: query.q,
            category: query.category,
            mode,
            page,
            per_page,
        });
        return format!(
            r#"{heading}
      {tabs}
      <article class="section-card">
        <h2>Host packages</h2>
        <p class="muted">Former Apps page: databases, phpMyAdmin, Email stack, and webmail clients. CLI <code>cpn app</code> remains an alias.</p>
        {apps_body}
      </article>"#,
            heading = section_heading(
                "Plugins",
                "Installed plugins, Plugin Store, and host packages.",
            ),
            tabs = view_tabs(view, &domain),
            apps_body = apps_body,
        );
    }

    if view == "store" {
        return render_store(query, &domain, &picker, mode, page, per_page);
    }

    if domain.is_empty() {
        return format!(
            r#"{heading}
      {ok}
      {err}
      {tabs}
      <article class="section-card">
        <h2>Installed Plugins</h2>
        {picker}
        <p class="muted">Plugins install under <code>/home/&lt;domain&gt;/plugins/&lt;plugin-id&gt;/</code> (nested for subdomains). Open Plugin Store to browse the catalog before creating a site.</p>
      </article>"#,
            heading = section_heading("Plugins", "Installed plugins and the CPN Plugin Store."),
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
        heading = section_heading("Plugins", "Installed plugins and the CPN Plugin Store."),
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

fn render_store(
    query: PluginsPageQuery<'_>,
    domain: &str,
    picker: &str,
    mode: &str,
    page: usize,
    per_page: usize,
) -> String {
    let catalog = fetch_catalog(query.refresh);
    let installed = if domain.is_empty() {
        Vec::new()
    } else {
        list_installed(domain).unwrap_or_default()
    };
    let ids: Vec<String> = installed.iter().map(|p| p.manifest.id.clone()).collect();
    let install_path = if domain.is_empty() {
        plugins_install_path_display(None)
    } else {
        plugins_install_path_display(Some(domain))
    };
    let (body, cache_note) = match catalog {
        Ok((entries, fetched_at)) => {
            let next = catalog_next_refresh_unix(fetched_at);
            let note = format!(
                "Cached 1 hour. Last refresh: {}. Next: {}.",
                format_unix_local(fetched_at),
                format_unix_local(next),
            );
            let count_label = if entries.len() == 1 {
                "1 plugin in catalog".to_string()
            } else {
                format!("{} plugins in catalog", entries.len())
            };
            (
                format!(
                    r#"<p class="plugin-count">{count}</p>
          <form method="get" action="/plugins" class="plugin-search-row">
            <input type="hidden" name="view" value="store">
            <input type="hidden" name="domain" value="{domain}">
            <input type="hidden" name="mode" value="{mode}">
            <input type="hidden" name="per_page" value="{per_page}">
            <label for="q">Search</label>
            <input class="plugin-search" id="q" name="q" type="search" value="{q}" placeholder="Search plugins by name or description...">
            <button type="submit" class="btn-primary">Search</button>
            <button type="submit" class="btn-secondary" name="refresh" value="1">Refresh catalog</button>
          </form>
          {pills}
          {rows}"#,
                    count = html_escape(&count_label),
                    domain = html_escape(domain),
                    mode = html_escape(mode),
                    per_page = per_page,
                    q = html_escape(query.q),
                    pills = category_pills(&entries, query.category, domain, mode, per_page),
                    rows = store_catalog(
                        &entries,
                        &ids,
                        StoreListOpts {
                            query: query.q,
                            category: query.category,
                            domain,
                            mode,
                            page,
                            per_page,
                        },
                    ),
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
                domain = html_escape(domain),
            ),
            "Catalog cache unavailable.".into(),
        ),
    };
    format!(
        r#"{heading}
      {ok}
      {err}
      {tabs}
      <article class="section-card">
        <h2>Plugin Store</h2>
        {picker}
        <p class="plugin-store-meta">Install into <code>{path}</code>. Catalog: <a href="{url}" target="_blank" rel="noopener noreferrer">{url}</a>. {cache}</p>
        <p class="plugin-risk-notice" role="note">Third-party plugins run with site privileges. Review each package before install. Fail2ban and other Security plugins appear here from Control-Panel-Network/CPN-Plugins.</p>
        {body}
      </article>"#,
        heading = section_heading("Plugins", "Installed plugins and the CPN Plugin Store."),
        ok = notice_block("ok", query.notice),
        err = notice_block("error", query.error),
        tabs = view_tabs("store", domain),
        picker = picker,
        path = html_escape(&install_path),
        cache = html_escape(&cache_note),
        url = html_escape(catalog_repo_url()),
        body = body,
    )
}
