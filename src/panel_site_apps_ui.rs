//! Website Manage Apps tab cards (CMS Made Simple, Redis, Node, Python).

use crate::apps::AppStateKind;
use crate::panel_admin::is_panel_admin;
use crate::panel_site_apps::snapshot_site_apps;
use crate::panel_site_apps_cmsms::CMSMS_INSTALLER;
use crate::panel_site_apps_runtime::RuntimeKind;
use crate::panel_site_tools_security::site_tools_csrf_token;
use crate::panel_website_manage_ui::{html_escape, section};
use crate::sites::SiteRecord;
use crate::website_preview::public_site_url;

fn host_label(state: AppStateKind) -> &'static str {
    match state {
        AppStateKind::Running => "Running on host",
        AppStateKind::Installed => "Installed on host",
        AppStateKind::NotInstalled => "Not installed on host",
    }
}

fn option_list(versions: &[(String, String)], selected: &str) -> String {
    let mut out = String::new();
    if versions.is_empty() {
        out.push_str(r#"<option value="">No host binary found</option>"#);
        return out;
    }
    for (path, ver) in versions {
        let sel = if path == selected { " selected" } else { "" };
        out.push_str(&format!(
            r#"<option value="{p}"{sel}>{v} ({p})</option>"#,
            p = html_escape(path),
            v = html_escape(ver),
            sel = sel,
        ));
    }
    out
}

pub fn tab_apps(site: &SiteRecord, username: &str) -> String {
    let snap = snapshot_site_apps(site);
    let domain = html_escape(&site.domain);
    let csrf = html_escape(&site_tools_csrf_token(username, &site.domain));
    let admin = is_panel_admin(username);
    let site_url =
        public_site_url(&site.domain).unwrap_or_else(|_| format!("http://{}", site.domain));
    let site_url_q = html_escape(&site_url);
    let installer_open = format!(
        "{}/{}",
        site_url.trim_end_matches('/'),
        CMSMS_INSTALLER.trim_end_matches(".php")
    );

    let mut cmsms_actions = String::new();
    if snap.wordpress_in_docroot && !snap.cmsms.installed {
        cmsms_actions.push_str(
            r#"<p class="manage-muted">WordPress files are in this document root. Use WordPress for that stack.</p>"#,
        );
    } else if snap.cmsms.installed {
        cmsms_actions.push_str(&format!(
            r#"<a class="manage-btn primary" href="{site_url_q}" target="_blank" rel="noopener noreferrer">Open</a>"#
        ));
        if snap.cmsms.installer_present {
            cmsms_actions.push_str(&format!(
                r#"<form method="post" action="/websites/apps" class="inline-form" onsubmit="return confirm('Remove the CMS Made Simple installer from this document root?');">
  <input type="hidden" name="domain" value="{domain}">
  <input type="hidden" name="csrf" value="{csrf}">
  <input type="hidden" name="action" value="cmsms_remove_installer">
  <button type="submit" class="btn-warn">Remove installer</button>
</form>"#
            ));
        }
    } else if snap.cmsms.installer_present {
        cmsms_actions.push_str(&format!(
            r#"<a class="manage-btn primary" href="{open}" target="_blank" rel="noopener noreferrer">Open installer</a>
<form method="post" action="/websites/apps" class="inline-form" onsubmit="return confirm('Remove the CMS Made Simple installer from this document root?');">
  <input type="hidden" name="domain" value="{domain}">
  <input type="hidden" name="csrf" value="{csrf}">
  <input type="hidden" name="action" value="cmsms_remove_installer">
  <button type="submit" class="btn-warn">Remove installer</button>
</form>"#,
            open = html_escape(&installer_open),
        ));
    } else {
        cmsms_actions.push_str(&format!(
            r#"<form method="post" action="/websites/apps" class="inline-form">
  <input type="hidden" name="domain" value="{domain}">
  <input type="hidden" name="csrf" value="{csrf}">
  <input type="hidden" name="action" value="cmsms_install">
  <button type="submit" class="btn-primary">Install</button>
</form>"#
        ));
    }

    let mut redis_actions = String::new();
    match snap.redis_host {
        AppStateKind::Running | AppStateKind::Installed => {
            if snap.redis_attached {
                redis_actions
                    .push_str(r#"<span class="manage-badge active">Attached to site</span>"#);
                redis_actions.push_str(&format!(
                    r#"<form method="post" action="/websites/apps" class="inline-form" onsubmit="return confirm('Detach Redis from this site? The host Redis install is not removed.');">
  <input type="hidden" name="domain" value="{domain}">
  <input type="hidden" name="csrf" value="{csrf}">
  <input type="hidden" name="action" value="redis_detach">
  <button type="submit" class="btn-warn">Detach</button>
</form>"#
                ));
            } else {
                redis_actions.push_str(&format!(
                    r#"<form method="post" action="/websites/apps" class="inline-form">
  <input type="hidden" name="domain" value="{domain}">
  <input type="hidden" name="csrf" value="{csrf}">
  <input type="hidden" name="action" value="redis_activate">
  <button type="submit" class="btn-primary" title="Attach this site to the host Redis. Nothing is installed again.">Activate</button>
</form>"#
                ));
            }
        }
        AppStateKind::NotInstalled => {
            if admin {
                redis_actions.push_str(&format!(
                    r#"<form method="post" action="/websites/apps" class="inline-form">
  <input type="hidden" name="domain" value="{domain}">
  <input type="hidden" name="csrf" value="{csrf}">
  <input type="hidden" name="action" value="redis_install_host">
  <button type="submit" class="btn-primary">Install on host</button>
</form>"#
                ));
            } else {
                redis_actions.push_str(
                    r#"<p class="manage-muted">Redis is a host engine. Ask an owner to install it from Plugins (Host), then Activate it here.</p>"#,
                );
            }
        }
    }

    let cmsms_card = format!(
        r#"<article class="manage-stat" id="cmsms">
  <span>CMS Made Simple</span>
  <strong>{status}</strong>
  <p class="manage-muted">{detail}</p>
  <div class="manage-actions-row">{actions}</div>
</article>"#,
        status = html_escape(if snap.cmsms.installed {
            "Installed"
        } else if snap.cmsms.installer_present {
            "Installer ready"
        } else {
            "Not installed"
        }),
        detail = html_escape(&snap.cmsms.detail),
        actions = cmsms_actions,
    );

    let redis_card = format!(
        r#"<article class="manage-stat" id="redis">
  <span>Redis</span>
  <strong>{host}</strong>
  <p class="manage-muted">{detail}</p>
  <div class="manage-actions-row">{actions}</div>
</article>"#,
        host = html_escape(host_label(snap.redis_host)),
        detail = html_escape(&snap.redis_detail),
        actions = redis_actions,
    );

    let node_card = runtime_card(site, &domain, &csrf, &snap.node);
    let python_card = runtime_card(site, &domain, &csrf, &snap.python);

    let grid = format!(
        r#"<div class="manage-card-grid" style="grid-template-columns:repeat(auto-fit,minmax(260px,1fr));">{cmsms}{redis}{node}{python}</div>
<p class="manage-muted">Host engines install once. Site Apps Activate or attach when the host is already running. Site users cannot uninstall host packages. WordPress stays under WordPress.</p>"#,
        cmsms = cmsms_card,
        redis = redis_card,
        node = node_card,
        python = python_card,
    );
    section("Apps", &grid)
}

fn runtime_card(
    _site: &SiteRecord,
    domain: &str,
    csrf: &str,
    st: &crate::panel_site_apps_runtime::RuntimeStatus,
) -> String {
    let kind = st.kind.as_str();
    let label = st.kind.label();
    let start_or_stop = if st.running {
        format!(
            r#"<form method="post" action="/websites/apps" class="inline-form" onsubmit="return confirm('Stop the {label} app for this site?');">
  <input type="hidden" name="domain" value="{domain}">
  <input type="hidden" name="csrf" value="{csrf}">
  <input type="hidden" name="action" value="{kind}_stop">
  <button type="submit" class="btn-warn">Stop</button>
</form>"#
        )
    } else {
        format!(
            r#"<form method="post" action="/websites/apps" class="inline-form">
  <input type="hidden" name="domain" value="{domain}">
  <input type="hidden" name="csrf" value="{csrf}">
  <input type="hidden" name="action" value="{kind}_start">
  <button type="submit" class="btn-primary">Start</button>
</form>"#
        )
    };
    let venv = if st.kind == RuntimeKind::Python {
        format!(
            r#"<form method="post" action="/websites/apps" class="inline-form">
  <input type="hidden" name="domain" value="{domain}">
  <input type="hidden" name="csrf" value="{csrf}">
  <input type="hidden" name="action" value="python_venv">
  <button type="submit" class="btn-secondary">Create venv</button>
</form>"#
        )
    } else {
        String::new()
    };
    format!(
        r#"<article class="manage-stat" id="{kind}">
  <span>{label}</span>
  <strong>{state}</strong>
  <p class="manage-muted">{detail}</p>
  <form method="post" action="/websites/apps" style="margin-top:10px;">
    <input type="hidden" name="domain" value="{domain}">
    <input type="hidden" name="csrf" value="{csrf}">
    <input type="hidden" name="action" value="{kind}_save">
    <label class="manage-muted" for="{kind}-bin">Version</label>
    <select id="{kind}-bin" name="version_bin" style="width:100%;margin:4px 0 8px;">{opts}</select>
    <label class="manage-muted" for="{kind}-path">App path (under site home)</label>
    <input id="{kind}-path" name="app_rel" value="{app}" maxlength="160" style="width:100%;margin:4px 0 8px;">
    <label class="manage-muted" for="{kind}-entry">Entry file</label>
    <input id="{kind}-entry" name="entry" value="{entry}" maxlength="80" style="width:100%;margin:4px 0 8px;">
    <button type="submit" class="btn-secondary">Save</button>
  </form>
  <div class="manage-actions-row">{start}{venv}</div>
</article>"#,
        state = if st.running { "Running" } else { "Stopped" },
        detail = html_escape(&st.detail),
        opts = option_list(&st.versions, &st.config.version_bin),
        app = html_escape(&st.config.app_rel),
        entry = html_escape(&st.config.entry),
        start = start_or_stop,
        venv = venv,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sites::SiteRecord;

    fn sample() -> SiteRecord {
        SiteRecord {
            schema_version: 1,
            domain: "cpn-lab-test.example".into(),
            owner: "Admin".into(),
            docroot: "/tmp/cpn-manage-missing".into(),
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
            aliases: Vec::new(),
            staging_of: None,
        }
    }

    #[test]
    fn apps_tab_has_four_cards() {
        let html = tab_apps(&sample(), "Admin");
        assert!(html.contains("CMS Made Simple"));
        assert!(html.contains("Redis"));
        assert!(html.contains(">Node<") || html.contains("<span>Node</span>"));
        assert!(html.contains(">Python<") || html.contains("<span>Python</span>"));
        assert!(html.contains("tab=apps") || html.contains("/websites/apps"));
        assert!(!html.to_lowercase().contains("cyberpanel"));
    }
}
