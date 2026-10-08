//! HTML for CPN Panel Plugins (Installed + unified Store).

use crate::apps::list_apps;
use crate::panel_plugins_installed::{InstalledPageOpts, render_installed};
use crate::panel_plugins_markup::{
    domain_picker, html_escape, notice_block, resolve_domain, resolve_store_target,
    section_heading, store_install_target_picker, view_tabs,
};
use crate::panel_plugins_spa::{
    list_mode_from_query, page_from_query, per_page_from_query, plugins_hub_script,
};
use crate::panel_plugins_store::StoreListOpts;
use crate::panel_plugins_unified::{unified_category_pills, unified_store_catalog};
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
    pub status: &'a str,
    pub domain: &'a str,
    pub store_target: &'a str,
    pub notice: Option<&'a str>,
    pub error: Option<&'a str>,
    pub refresh: bool,
    pub mode: &'a str,
    pub page: usize,
    pub per_page: usize,
    pub sites: &'a [SiteRecord],
    pub username: &'a str,
}

fn wrap_hub(inner: String, allow_host: bool) -> String {
    format!(
        r#"<div id="plugins-hub" data-plugins-hub="1" data-store-allow-host="{allow_host}">{inner}</div>
{dialog}
{script}"#,
        inner = inner,
        allow_host = if allow_host { "1" } else { "0" },
        dialog = crate::uninstall_confirm::uninstall_dialog_bundle(),
        script = plugins_hub_script(),
    )
}

pub fn plugins_main(query: PluginsPageQuery<'_>) -> String {
    let allow_host = crate::panel_admin::is_panel_admin(query.username);
    wrap_hub(plugins_main_inner(query), allow_host)
}

fn plugins_main_inner(query: PluginsPageQuery<'_>) -> String {
    let allow_host = crate::panel_admin::is_panel_admin(query.username);
    let (view, mut category) = match query.view {
        "store" | "view-store" => ("store", query.category),
        "host" | "apps" => ("store", query.category),
        _ => ("installed", query.category),
    };
    let legacy_host_cat = category.trim().eq_ignore_ascii_case("host");
    if legacy_host_cat {
        category = "";
    }
    let requested_target = if legacy_host_cat && query.store_target.trim().is_empty() {
        "host"
    } else {
        query.store_target
    };
    let mode = list_mode_from_query(query.mode);
    let per_page = per_page_from_query(&query.per_page.to_string());
    let page = page_from_query(&query.page.to_string());
    let sites = query.sites;
    let mut store_target = resolve_store_target(requested_target, "", sites);
    if !allow_host {
        store_target = "site";
    }
    let domain = if view == "store" && store_target == "host" {
        String::new()
    } else {
        resolve_domain(sites, query.domain)
    };
    let picker = if view == "store" {
        store_install_target_picker(
            sites,
            &domain,
            store_target,
            view,
            category,
            query.q,
            mode,
            per_page,
            allow_host,
        )
    } else {
        domain_picker(sites, &domain, view)
    };

    if view == "store" {
        return render_store(
            PluginsPageQuery { category, ..query },
            &domain,
            store_target,
            &picker,
            mode,
            page,
            per_page,
        );
    }

    render_installed(InstalledPageOpts {
        layout: query.layout,
        domain: query.domain,
        notice: query.notice,
        error: query.error,
        sites,
        username: query.username,
        q: query.q,
        category: query.category,
        status: query.status,
        mode,
        page,
        per_page,
    })
}

fn render_store(
    query: PluginsPageQuery<'_>,
    domain: &str,
    store_target: &str,
    picker: &str,
    mode: &str,
    page: usize,
    per_page: usize,
) -> String {
    let catalog = fetch_catalog(query.refresh);
    let installed = if domain.is_empty() {
        Vec::new()
    } else {
        let mut installed = list_installed(domain).unwrap_or_default();
        for act in crate::plugin_activation::activated_as_installed(domain) {
            if !installed.iter().any(|p| p.manifest.id == act.manifest.id) {
                installed.push(act);
            }
        }
        installed
    };
    let ids: Vec<String> = installed.iter().map(|p| p.manifest.id.clone()).collect();
    let install_path = if domain.is_empty() {
        plugins_install_path_display(None)
    } else {
        plugins_install_path_display(Some(domain))
    };
    let apps = list_apps();
    let (body, cache_note) = match catalog {
        Ok((entries, fetched_at)) => {
            let next = catalog_next_refresh_unix(fetched_at);
            let note = format!(
                "Cached 1 hour. Last refresh: {}. Next: {}.",
                format_unix_local(fetched_at),
                format_unix_local(next),
            );
            let host_n = apps.len();
            let plugin_n = entries.len();
            let count_label =
                format!("{host_n} host packages + {plugin_n} community plugins (one catalog)");
            let body = format!(
                r#"<p class="plugin-count">{count}</p>
          <form method="get" action="/plugins" class="plugin-search-row">
            <input type="hidden" name="view" value="store">
            <input type="hidden" name="target" value="{store_target}">
            <input type="hidden" name="domain" value="{domain}">
            <input type="hidden" name="mode" value="{mode}">
            <input type="hidden" name="per_page" value="{per_page}">
            <input type="hidden" name="category" value="{category}">
            <label for="q">Search</label>
            <input class="plugin-search" id="q" name="q" type="search" value="{q}" placeholder="Search name, id, description, category, keywords...">
            <button type="submit" class="btn-primary">Search</button>
            <button type="submit" class="btn-secondary" name="refresh" value="1">Refresh catalog</button>
          </form>
          {pills}
          {rows}"#,
                count = html_escape(&count_label),
                store_target = html_escape(store_target),
                domain = html_escape(domain),
                mode = html_escape(mode),
                per_page = per_page,
                category = html_escape(query.category),
                q = html_escape(query.q),
                pills = unified_category_pills(
                    &apps,
                    &entries,
                    query.category,
                    domain,
                    store_target,
                    mode,
                    per_page,
                    query.q,
                ),
                rows = unified_store_catalog(
                    &entries,
                    &ids,
                    StoreListOpts {
                        query: query.q,
                        category: query.category,
                        domain,
                        mode,
                        page,
                        per_page,
                        username: query.username,
                        store_target,
                    },
                ),
            );
            (body, note)
        }
        Err(error) => (
            format!(
                r#"<p class="panel-notice error" role="alert">Could not load community catalog: {err}</p>
          <p class="muted">Host packages still appear when the catalog is available again. Catalog URL: <a href="{url}" target="_blank" rel="noopener noreferrer">{url}</a></p>
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
        <h2>Store</h2>
        {picker}
        <p class="plugin-store-meta">One catalog. <strong>Install target</strong> is independent of category chips: <strong>Host</strong> (owner/admin) is server-wide; <strong>CPN only</strong> is your signed-in account under <code>/var/lib/cpn/user-plugins/</code>; <strong>Site</strong> is per-domain plugins under <code>{path}</code>. Cards show Host / CPN / Site badges. Catalog: <a href="{url}" target="_blank" rel="noopener noreferrer">{url}</a>. {cache}</p>
        <p class="plugin-risk-notice" role="note">Third-party plugins run with site privileges. Review each package before install. Fail2ban and other Security plugins ship from Control-Panel-Network/CPN-Plugins.</p>
        {body}
      </article>"#,
        heading = section_heading(
            "Plugins",
            "Installed host packages and site plugins, plus the CPN Store.",
        ),
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
