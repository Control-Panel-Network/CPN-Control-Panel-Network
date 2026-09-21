//! Collect / filter installed plugin rows for the Installed hub.

use crate::apps::{AppStateKind, list_apps};
use crate::apps_webmail::is_active_webmail;
use crate::backups::is_subdomain_site;
use crate::host_packages_catalog::meta_for;
use crate::panel_admin::is_panel_admin;
use crate::panel_apps::{host_action_buttons, host_nav_links};
use crate::panel_plugins_markup::{html_escape, installed_one_card};
use crate::plugin_activation::{is_host_owned_install, list_host_installed_plugins};
use crate::plugins::{InstalledPlugin, list_installed};
use crate::sites::SiteRecord;
use crate::uninstall_confirm::{plugin_uninstall_impacts, uninstall_form_attrs};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scope {
    Host,
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

fn status_filter_ok(active: bool, status: &str) -> bool {
    match status.trim().to_ascii_lowercase().as_str() {
        "" | "all" => true,
        "active" | "activated" | "enabled" => active,
        "deactivated" | "inactive" | "disabled" => !active,
        _ => true,
    }
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

fn category_match(want: &str, category: &str, is_host: bool) -> bool {
    let want = want.trim().to_ascii_lowercase();
    if want.is_empty() || want == "all" {
        return true;
    }
    if want == "host" {
        return is_host;
    }
    if want == "site" {
        return !is_host;
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
    String::new()
}

fn host_scoped_card(item: &InstalledPlugin, is_admin: bool) -> String {
    let m = &item.manifest;
    let active_badge = if m.enabled {
        r#"<span class="plugin-badge installed">Active</span>"#
    } else {
        r#"<span class="plugin-badge">Deactivated</span>"#
    };
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
          </div>
          <p class="plugin-desc">{desc}</p>
          <p class="plugin-meta">Id: <code>{id}</code> · path: <code>{path}</code></p>
          <div class="plugin-actions">{actions}</div>
        </article>"#,
        name = html_escape(&m.name),
        cat = html_escape(&m.category),
        ver = html_escape(&m.version),
        active = active_badge,
        desc = html_escape(&m.description),
        id = html_escape(&m.id),
        path = html_escape(&item.path.display().to_string()),
        actions = actions,
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
        if !status_filter_ok(active, status) {
            continue;
        }
        if !category_match(category, meta.category, true) {
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
        if !status_filter_ok(m.enabled, status) {
            continue;
        }
        if !category_match(category, &m.category, true) {
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
            card_html: host_scoped_card(&item, is_admin),
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
            if !status_filter_ok(m.enabled, status) {
                continue;
            }
            if !category_match(category, &m.category, host_owned) {
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
                card_html: installed_one_card(&p, &site.domain, username),
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
