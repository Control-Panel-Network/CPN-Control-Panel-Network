//! Panel Apps page HTML (session-gated; domain/subdomain picker for site scope).

use crate::apps::{AppId, AppStateKind, AppStatus, list_apps};
use crate::apps_site::{bindings_for_domain, is_associable, is_site_scoped};
use crate::backups::is_subdomain_site;
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

fn site_options(sites: &[SiteRecord], selected: &str) -> String {
    let mut out = String::from(r#"<option value="">Host only (no site path)</option>"#);
    for site in sites {
        let kind = if is_subdomain_site(&site.domain) {
            "subdomain"
        } else {
            "domain"
        };
        let sel = if site.domain == selected {
            " selected"
        } else {
            ""
        };
        out.push_str(&format!(
            r#"<option value="{domain}"{sel}>{domain} ({kind})</option>"#,
            domain = html_escape(&site.domain),
            kind = kind,
            sel = sel,
        ));
    }
    out
}

fn domain_hidden(domain: &str) -> String {
    if domain.is_empty() {
        String::new()
    } else {
        format!(
            r#"<input type="hidden" name="domain" value="{d}">"#,
            d = html_escape(domain)
        )
    }
}

fn action_buttons(status: &AppStatus, domain: &str) -> String {
    let name = status.id.as_str();
    let label = status.id.label();
    let hidden = domain_hidden(domain);
    let scope_hint = if domain.is_empty() {
        "on this host"
    } else {
        "for the selected domain/subdomain"
    };
    match status.state {
        AppStateKind::NotInstalled => format!(
            r#"<form method="post" action="/apps/install" class="inline-form" onsubmit="return confirm('Install {label} {scope_hint}?');">
              <input type="hidden" name="name" value="{name}">
              {hidden}
              <button type="submit" class="btn-primary">Install</button>
            </form>"#,
            label = html_escape(label),
            scope_hint = scope_hint,
            name = html_escape(name),
            hidden = hidden,
        ),
        AppStateKind::Installed | AppStateKind::Running => format!(
            r#"<form method="post" action="/apps/reinstall" class="inline-form" onsubmit="return confirm('Reinstall {label}?');">
              <input type="hidden" name="name" value="{name}">
              {hidden}
              <button type="submit" class="btn-secondary" style="min-height:44px;padding:0 14px;border:0;border-radius:999px;background:#f2f4f7;color:#344054;font-weight:700;cursor:pointer;">Reinstall</button>
            </form>
            <form method="post" action="/apps/uninstall" class="inline-form" onsubmit="return confirm('Uninstall {label}?');">
              <input type="hidden" name="name" value="{name}">
              {hidden}
              <button type="submit" class="btn-danger">Uninstall</button>
            </form>"#,
            label = html_escape(label),
            name = html_escape(name),
            hidden = hidden,
        ),
    }
}

fn app_card(status: &AppStatus, domain: &str) -> String {
    let warn = status
        .warning
        .as_ref()
        .map(|w| {
            format!(
                r#"<p class="panel-notice error" style="margin-top:12px;">{msg}</p>"#,
                msg = html_escape(w)
            )
        })
        .unwrap_or_default();
    let xor_note = match status.id {
        AppId::Mariadb | AppId::Mysql => {
            r#"<p class="muted" style="margin-top:8px;">Hosts typically run MariaDB XOR MySQL. CPN refuses installing one while the other is present. Engines stay system-wide.</p>"#
        }
        _ => "",
    };
    let scope_note = if is_site_scoped(status.id) {
        "<p class=\"muted\" style=\"margin-top:8px;\">Site-scoped paths land under <code>/home/&lt;domain&gt;/apps/</code> (nested for subdomains) when a site is selected.</p>"
    } else if is_associable(status.id) {
        "<p class=\"muted\" style=\"margin-top:8px;\">System service on the host. Optional domain association is for ACL/display only.</p>"
    } else {
        ""
    };
    let binding_note = if !domain.is_empty() {
        let binds = bindings_for_domain(domain);
        let mine: Vec<_> = binds
            .iter()
            .filter(|b| b.app == status.id.as_str())
            .collect();
        if mine.is_empty() {
            String::new()
        } else {
            let paths: Vec<String> = mine
                .iter()
                .map(|b| {
                    if b.path.is_empty() {
                        format!("associated with {}", b.domain)
                    } else {
                        b.path.clone()
                    }
                })
                .collect();
            format!(
                r#"<p class="muted" style="margin-top:8px;">Binding: <code>{}</code></p>"#,
                html_escape(&paths.join("; "))
            )
        }
    } else {
        String::new()
    };
    format!(
        r#"<article class="section-card" style="margin-top:18px;">
        <h2>{label}</h2>
        <ul class="kv-list">
          <li><span>Status</span><strong>{state}</strong></li>
          <li><span>Id</span><strong><code>{id}</code></strong></li>
        </ul>
        <p>{detail}</p>
        {warn}
        {xor}
        {scope}
        {binding}
        <div style="display:flex;flex-wrap:wrap;gap:10px;margin-top:16px;">
          {actions}
        </div>
      </article>"#,
        label = html_escape(status.id.label()),
        state = html_escape(status.state.label()),
        id = html_escape(status.id.as_str()),
        detail = html_escape(&status.detail),
        warn = warn,
        xor = xor_note,
        scope = scope_note,
        binding = binding_note,
        actions = action_buttons(status, domain),
    )
}

pub struct AppsPageQuery<'a> {
    pub notice: Option<&'a str>,
    pub error: Option<&'a str>,
    pub domain: &'a str,
    pub sites: &'a [SiteRecord],
}

pub fn apps_main(q: AppsPageQuery<'_>) -> String {
    let domain = q.domain.trim();
    let apps = list_apps();
    let cards: String = apps.iter().map(|a| app_card(a, domain)).collect();
    let picker = if q.sites.is_empty() {
        r#"<p class="muted">No manageable sites yet. Create a website (or subdomain) you own to attach site-scoped app paths. Host engines can still be installed without a site.</p>"#.into()
    } else {
        format!(
            r#"<form method="get" action="/apps" class="stack-form" style="max-width:560px;">
          <label for="domain">Domain or subdomain</label>
          <select id="domain" name="domain" onchange="this.form.submit()">{opts}</select>
          <noscript><button type="submit" class="btn-secondary">Apply</button></noscript>
        </form>
        <p class="muted">Selected site paths use <code>/home/&lt;domain&gt;/...</code> (subdomains nest under the parent home).</p>"#,
            opts = site_options(q.sites, domain),
        )
    };
    format!(
        r#"{heading}
      {ok}
      {err}
      <article class="section-card">
        <h2>Host and site apps</h2>
        <p>MariaDB, MySQL, and RabbitMQ are host packages. phpMyAdmin and Email can also drop paths under the selected domain or subdomain home. Only sites you own or are granted appear below.</p>
        <p class="muted">CLI: <code>cpn app install --name phpmyadmin --domain example.com</code> or <code>--subdomain blog --domain example.com</code></p>
        {picker}
      </article>
      {cards}"#,
        heading = section_heading(
            "Apps",
            "Manage host apps and optional domain/subdomain associations.",
        ),
        ok = notice_block("ok", q.notice),
        err = notice_block("error", q.error),
        picker = picker,
        cards = cards,
    )
}
