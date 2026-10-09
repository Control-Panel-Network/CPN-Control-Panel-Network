//! Collect / filter installed plugin rows for the Installed hub.

use crate::apps::{AppStateKind, list_apps};
use crate::apps_webmail::is_active_webmail;
use crate::backups::is_subdomain_site;
use crate::host_packages_catalog::meta_for;
use crate::panel_admin::is_panel_admin;
use crate::panel_apps::{host_action_buttons, host_nav_links};
use crate::panel_plugins_markup::{html_escape, installed_one_card, update_available_badge};
use crate::plugin_activation::{is_host_owned_install, list_host_installed_plugins};
use crate::plugin_cpn_scope::list_cpn_installed_plugins;
use crate::plugins::{InstalledPlugin, fetch_catalog, list_installed};
use crate::releases::compare_versions;
use crate::sites::SiteRecord;
use crate::uninstall_confirm::{plugin_uninstall_impacts, uninstall_form_attrs};
use std::cmp::Ordering;
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scope {
    Host,
    Cpn,
    Domain,
    Subdomain,
}

pub(crate) struct FlatItem {
    pub scope: Scope,
    pub domain: String,
    pub name: String,
    #[allow(dead_code)] // retained for Installed filters / future SPA payloads
    pub id: String,
    pub category: String,
    #[allow(dead_code)] // retained for Installed status filters / future SPA payloads
    pub active: bool,
    pub card_html: String,
}

fn status_filter_ok(active: bool, status: &str, update_available: bool) -> bool {
    match status.trim().to_ascii_lowercase().as_str() {
        "" | "all" => true,
        "active" | "activated" | "enabled" => active,
        "deactivated" | "inactive" | "disabled" => !active,
        "updates" | "update" | "upgrades" | "upgrade" => update_available,
        _ => true,
    }
}

fn catalog_version_map() -> HashMap<String, String> {
    match fetch_catalog(false) {
        Ok((entries, _)) => entries
            .into_iter()
            .map(|e| (e.id.to_ascii_lowercase(), e.version))
            .collect(),
        Err(_) => HashMap::new(),
    }
}

/// True when the catalog version is newer than the installed version.
pub(crate) fn plugin_update_available(
    catalog: &HashMap<String, String>,
    plugin_id: &str,
    installed_version: &str,
) -> bool {
    let key = plugin_id.trim().to_ascii_lowercase();
    let Some(catalog_ver) = catalog.get(&key) else {
        return false;
    };
    let catalog_ver = catalog_ver.trim();
    let installed = installed_version.trim();
    if catalog_ver.is_empty() || installed.is_empty() {
        return false;
    }
    compare_versions(catalog_ver, installed) == Ordering::Greater
}

fn text_match(q: &str, name: &str, id: &str, category: &str) -> bool {
    let q = q.trim().to_ascii_lowercase();
    if q.is_empty() {
        return true;
    }
    name.to_ascii_lowercase().contains(&q)
        || id.to_ascii_lowercase().contains(&q)
        || category.to_ascii_lowercase().contains(&q)
}

fn category_match(want: &str, category: &str, scope_kind: &str) -> bool {
    let want = want.trim().to_ascii_lowercase();
    if want.is_empty() || want == "all" {
        return true;
    }
    if want == "host" {
        return scope_kind == "host";
    }
    if want == "cpn" || want == "cpn only" || want == "cpn_only" {
        return scope_kind == "cpn";
    }
    if want == "site" {
        return scope_kind == "site";
    }
    category.eq_ignore_ascii_case(want.trim())
}

fn host_active(status: &crate::apps::AppStatus) -> bool {
    if matches!(
        status.id,
        crate::apps::AppId::Snappymail
            | crate::apps::AppId::Tachyon
            | crate::apps::AppId::Roundcube
            | crate::apps::AppId::Nextsnapmail
            | crate::apps::AppId::Sogo
    ) {
        return is_active_webmail(status.id);
    }
    matches!(
        status.state,
        AppStateKind::Running | AppStateKind::Installed
    )
}

fn host_card_html(status: &crate::apps::AppStatus, is_admin: bool) -> String {
    let meta = meta_for(status.id);
    let active = host_active(status);
    let active_badge = if active {
        r#"<span class="plugin-badge installed">Active</span>"#
    } else {
        r#"<span class="plugin-badge">Deactivated</span>"#
    };
    let mut actions = host_nav_links(status.id);
    actions.push_str(&host_action_buttons(status, "", is_admin, "installed"));
    format!(
        r#"<article class="plugin-card">
          <h3>{label}</h3>
          <div class="plugin-badges">
            <span class="plugin-badge">Host</span>
            <span class="plugin-badge cat">{cat}</span>
            {active}
            <span class="plugin-badge installed">{state}</span>
          </div>
          <p class="plugin-desc">{desc}</p>
          <p class="plugin-meta">{detail}</p>
          <div class="plugin-actions">{actions}</div>
        </article>"#,
        label = html_escape(status.id.label()),
        cat = html_escape(meta.category),
        active = active_badge,
        state = html_escape(status.state.label()),
        desc = html_escape(meta.description),
        detail = html_escape(&status.detail),
        actions = actions,
    )
}

fn host_scoped_nav_links(id: &str) -> String {
    let id = id.trim().to_ascii_lowercase();
    if id == "mragent" || id.contains("mragent") {
        return r#"<a class="btn-primary" href="/plugins/settings?domain=_host&amp;id=mrAgent">Settings</a>
          <a class="btn-secondary" href="/plugins/mr-agent?domain=_host">Open chat</a>"#
            .into();
    }
    if id.contains("fail2ban") {
        return r#"<a class="btn-primary" href="/security/fail2ban">Manage</a>"#.into();
    }
    if id.contains("clam") {
        return r#"<a class="btn-primary" href="/security/malware-scan">Manage</a>"#.into();
    }
    if id.contains("bimi") {
        return r#"<a class="btn-secondary" href="/email/bimi">Manage</a>"#.into();
    }
    if id.contains("mta") && id.contains("sts") {
        return r#"<a class="btn-secondary" href="/email/mta-sts">Manage</a>"#.into();
    }
    if id.eq_ignore_ascii_case("protonMail") || id.contains("proton") {
        return r#"<a class="btn-secondary" href="/email/proton">Manage</a>"#.into();
    }
    String::new()
}

fn host_scoped_card(item: &InstalledPlugin, is_admin: bool, update_available: bool) -> String {
    let m = &item.manifest;
    let active_badge = if m.enabled {
        r#"<span class="plugin-badge installed">Active</span>"#
    } else {
        r#"<span class="plugin-badge">Deactivated</span>"#
    };
    let update_badge = update_available_badge(update_available);
    let mut actions = host_scoped_nav_links(&m.id);
    if is_admin {
        let impacts = plugin_uninstall_impacts("", &m.id, &m.name);
        let form_attrs = uninstall_form_attrs(&m.name, &impacts);
        actions.push_str(&format!(
            r#"<form method="post" action="/plugins/uninstall-host" {form_attrs}>
              <input type="hidden" name="id" value="{id}">
              <input type="hidden" name="return_view" value="installed">
              <input type="hidden" name="confirm" value="">
              <button type="submit" class="btn-danger">Uninstall from Host</button>
            </form>"#,
            form_attrs = form_attrs,
            id = html_escape(&m.id),
        ));
    }
    if actions.is_empty() {
        actions.push_str(r#"<span class="muted">Installed on Host</span>"#);
    }
    format!(
        r#"<article class="plugin-card">
          <h3>{name}</h3>
          <div class="plugin-badges">
            <span class="plugin-badge">Host</span>
            <span class="plugin-badge cat">{cat}</span>
            <span class="plugin-badge">v{ver}</span>
            {active}
            {update}
          </div>
          <p class="plugin-desc">{desc}</p>
          <p class="plugin-meta">Id: <code>{id}</code> · path: <code>{path}</code></p>
          <div class="plugin-actions">{actions}</div>
        </article>"#,
        name = html_escape(&m.name),
        cat = html_escape(&m.category),
        ver = html_escape(&m.version),
        active = active_badge,
        update = update_badge,
        desc = html_escape(&m.description),
        id = html_escape(&m.id),
        path = html_escape(&item.path.display().to_string()),
        actions = actions,
    )
}

fn cpn_scoped_card(item: &InstalledPlugin, update_available: bool) -> String {
    let m = &item.manifest;
    let active_badge = if m.enabled {
        r#"<span class="plugin-badge installed">Active</span>"#
    } else {
        r#"<span class="plugin-badge">Deactivated</span>"#
    };
    let update_badge = update_available_badge(update_available);
    let domain_q = html_escape(&m.domain);
    let id = html_escape(&m.id);
    let toggle_action = if m.enabled {
        "/plugins/disable-cpn"
    } else {
        "/plugins/enable-cpn"
    };
    let toggle_label = if m.enabled { "Deactivate" } else { "Activate" };
    let toggle_cls = if m.enabled { "btn-warn" } else { "btn-primary" };
    let impacts = plugin_uninstall_impacts(&m.domain, &m.id, &m.name);
    let form_attrs = uninstall_form_attrs(&m.name, &impacts);
    format!(
        r#"<article class="plugin-card">
          <h3>{name}</h3>
          <div class="plugin-badges">
            <span class="plugin-badge cat">CPN</span>
            <span class="plugin-badge cat">{cat}</span>
            <span class="plugin-badge">v{ver}</span>
            {active}
            {update}
          </div>
          <p class="plugin-desc">{desc}</p>
          <p class="plugin-meta">Id: <code>{id}</code> · path: <code>{path}</code> (account-scoped, not a public site app)</p>
          <div class="plugin-actions">
            <a class="btn-secondary" href="/plugins/settings?domain={domain_q}&amp;id={id}">Settings</a>
            <form method="post" action="{toggle_action}" class="inline-form">
              <input type="hidden" name="id" value="{id}">
              <button type="submit" class="{toggle_cls}">{toggle_label}</button>
            </form>
            <form method="post" action="/plugins/uninstall-cpn" {form_attrs}>
              <input type="hidden" name="id" value="{id}">
              <input type="hidden" name="return_view" value="installed">
              <input type="hidden" name="confirm" value="">
              <button type="submit" class="btn-danger">Uninstall from CPN</button>
            </form>
          </div>
        </article>"#,
        name = html_escape(&m.name),
        cat = html_escape(&m.category),
        ver = html_escape(&m.version),
        active = active_badge,
        update = update_badge,
        desc = html_escape(&m.description),
        id = id,
        path = html_escape(&item.path.display().to_string()),
        domain_q = domain_q,
        toggle_action = toggle_action,
        toggle_cls = toggle_cls,
        toggle_label = toggle_label,
        form_attrs = form_attrs,
    )
}

fn site_plugins_for(domain: &str) -> Vec<InstalledPlugin> {
    let mut installed = list_installed(domain).unwrap_or_default();
    for act in crate::plugin_activation::activated_as_installed(domain) {
        if !installed.iter().any(|p| p.manifest.id == act.manifest.id) {
            installed.push(act);
        }
    }
    installed
}

pub(crate) fn collect_installed_flat(
    sites: &[SiteRecord],
    username: &str,
    q: &str,
    category: &str,
    status: &str,
) -> Vec<FlatItem> {
    let is_admin = is_panel_admin(username);
    let catalog = catalog_version_map();
    let mut out = Vec::new();
    for status_app in list_apps() {
        if !matches!(
            status_app.state,
            AppStateKind::Installed | AppStateKind::Running
        ) {
            continue;
        }
        let meta = meta_for(status_app.id);
        let active = host_active(&status_app);
        // Host engines are system packages; catalog version updates apply to CPN Plugins only.
        let update_available = false;
        if !status_filter_ok(active, status, update_available) {
            continue;
        }
        if !category_match(category, meta.category, "host") {
            continue;
        }
        if !text_match(
            q,
            status_app.id.label(),
            status_app.id.as_str(),
            meta.category,
        ) {
            continue;
        }
        out.push(FlatItem {
            scope: Scope::Host,
            domain: String::new(),
            name: status_app.id.label().to_string(),
            id: status_app.id.as_str().to_string(),
            category: meta.category.to_string(),
            active,
            card_html: host_card_html(&status_app, is_admin),
        });
    }
    for item in list_host_installed_plugins() {
        let m = &item.manifest;
        let update_available = plugin_update_available(&catalog, &m.id, &m.version);
        if !status_filter_ok(m.enabled, status, update_available) {
            continue;
        }
        if !category_match(category, &m.category, "host") {
            continue;
        }
        if !text_match(q, &m.name, &m.id, &m.category) {
            continue;
        }
        out.push(FlatItem {
            scope: Scope::Host,
            domain: String::new(),
            name: m.name.clone(),
            id: m.id.clone(),
            category: m.category.clone(),
            active: m.enabled,
            card_html: host_scoped_card(&item, is_admin, update_available),
        });
    }
    for item in list_cpn_installed_plugins(username) {
        let m = &item.manifest;
        let update_available = plugin_update_available(&catalog, &m.id, &m.version);
        if !status_filter_ok(m.enabled, status, update_available) {
            continue;
        }
        if !category_match(category, &m.category, "cpn") {
            continue;
        }
        if !text_match(q, &m.name, &m.id, &m.category) {
            continue;
        }
        out.push(FlatItem {
            scope: Scope::Cpn,
            domain: m.domain.clone(),
            name: m.name.clone(),
            id: m.id.clone(),
            category: m.category.clone(),
            active: m.enabled,
            card_html: cpn_scoped_card(&item, update_available),
        });
    }
    for site in sites {
        let scope = if is_subdomain_site(&site.domain) {
            Scope::Subdomain
        } else {
            Scope::Domain
        };
        for p in site_plugins_for(&site.domain) {
            let m = &p.manifest;
            let host_owned =
                is_host_owned_install(&site.domain, &m.id) || m.source == "host-activation";
            let update_available = plugin_update_available(&catalog, &m.id, &m.version);
            if !status_filter_ok(m.enabled, status, update_available) {
                continue;
            }
            if !category_match(
                category,
                &m.category,
                if host_owned { "host" } else { "site" },
            ) {
                continue;
            }
            if !text_match(q, &m.name, &m.id, &m.category) {
                continue;
            }
            out.push(FlatItem {
                scope,
                domain: site.domain.clone(),
                name: m.name.clone(),
                id: m.id.clone(),
                category: m.category.clone(),
                active: m.enabled,
                card_html: installed_one_card(&p, &site.domain, username, update_available),
            });
        }
    }
    out.sort_by(|a, b| {
        (
            a.scope as u8,
            a.domain.to_ascii_lowercase(),
            a.name.to_ascii_lowercase(),
        )
            .cmp(&(
                b.scope as u8,
                b.domain.to_ascii_lowercase(),
                b.name.to_ascii_lowercase(),
            ))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_filter_updates_requires_newer_catalog() {
        assert!(status_filter_ok(true, "updates", true));
        assert!(!status_filter_ok(true, "updates", false));
        assert!(status_filter_ok(false, "upgrade", true));
        assert!(status_filter_ok(true, "active", true));
        assert!(!status_filter_ok(false, "active", true));
        assert!(status_filter_ok(false, "deactivated", false));
        assert!(status_filter_ok(true, "", false));
    }

    #[test]
    fn plugin_update_compares_catalog_greater() {
        let mut map = HashMap::new();
        map.insert("mragent".into(), "1.2.0".into());
        assert!(plugin_update_available(&map, "mrAgent", "1.1.0"));
        assert!(!plugin_update_available(&map, "mrAgent", "1.2.0"));
        assert!(!plugin_update_available(&map, "mrAgent", "1.3.0"));
        assert!(!plugin_update_available(&map, "missing", "1.0.0"));
        assert!(!plugin_update_available(&map, "mrAgent", ""));
    }

    #[test]
    fn status_filter_updates_applies_to_cpn_and_site_scopes() {
        // Same filter gate for Host catalog plugins, CPN-only, and Site installs.
        assert!(status_filter_ok(true, "updates", true));
        assert!(status_filter_ok(false, "updates", true));
        assert!(!status_filter_ok(true, "updates", false));
    }
}
