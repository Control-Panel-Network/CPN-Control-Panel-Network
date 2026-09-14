//! Panel Host packages page: store-like card grid (search, categories, pagination).

use crate::apps::{AppStateKind, AppStatus, list_apps};
use crate::apps_site::{bindings_for_domain, is_associable, is_site_scoped};
use crate::backups::is_subdomain_site;
use crate::host_packages_catalog::{
    HostInstallStatus, filter_host_packages, format_host_dates, host_categories,
    host_package_is_featured, meta_for,
};
use crate::panel_plugins_spa::{
    list_mode_from_query, page_from_query, per_page_from_query, store_list_toolbar,
};
use crate::sites::SiteRecord;
use crate::uninstall_confirm::{host_uninstall_impacts, uninstall_form_attrs};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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

fn action_buttons(status: &AppStatus, domain: &str) -> String {
    let name = status.id.as_str();
    let label = status.id.label();
    let hidden = domain_hidden(domain);
    let meta = meta_for(status.id);
    let is_webmail = matches!(
        status.id,
        crate::apps::AppId::Snappymail
            | crate::apps::AppId::Tachyon
            | crate::apps::AppId::Roundcube
            | crate::apps::AppId::Nextsnapmail
    );
    let active = is_webmail && crate::apps_webmail::is_active_webmail(status.id);
    if matches!(meta.install_status, HostInstallStatus::Scaffold)
        && status.state == AppStateKind::NotInstalled
    {
        return format!(
            r#"<span class="plugin-badge" title="{title}">{badge}</span>
            <form method="post" action="/apps/install" class="inline-form" onsubmit="return confirm('{label}: {title}. Continue anyway?');">
              <input type="hidden" name="name" value="{name}">
              {hidden}
              <button type="submit" class="btn-secondary">Try install</button>
            </form>"#,
            title = html_escape(meta.description),
            badge = html_escape(meta.install_status.badge()),
            label = html_escape(label),
            name = html_escape(name),
            hidden = hidden,
        );
    }
    // NextSnapMail without Nextcloud: Install still runs the dependency chain.
    if status.id == crate::apps::AppId::Nextsnapmail
        && status.state == AppStateKind::NotInstalled
        && !crate::apps_nextcloud::nextcloud_present()
    {
        return format!(
            r#"<span class="plugin-badge">Needs Nextcloud</span>
            <form method="post" action="/apps/install" class="inline-form" onsubmit="return confirm('Install Nextcloud files under /opt/nextcloud, then NextSnapMail into apps/?');">
              <input type="hidden" name="name" value="nextsnapmail">
              {hidden}
              <button type="submit" class="btn-primary">Install Nextcloud + NextSnapMail</button>
            </form>
            <form method="post" action="/apps/install" class="inline-form" onsubmit="return confirm('Install Nextcloud files only under /opt/nextcloud?');">
              <input type="hidden" name="name" value="nextcloud">
              {hidden}
              <button type="submit" class="btn-secondary">Install Nextcloud first</button>
            </form>"#,
            hidden = hidden,
        );
    }
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
        AppStateKind::Installed | AppStateKind::Running => {
            let mut out = String::new();
            if is_webmail {
                if active {
                    out.push_str(r#"<span class="plugin-badge featured">Active</span>"#);
                } else {
                    out.push_str(&format!(
                        r#"<form method="post" action="/apps/activate" class="inline-form" onsubmit="return confirm('Set {label} as the active panel webmail? Mailboxes stay on Postfix/Dovecot.');">
              <input type="hidden" name="name" value="{name}">
              {hidden}
              <button type="submit" class="btn-primary">Set as active</button>
            </form>"#,
                        label = html_escape(label),
                        name = html_escape(name),
                        hidden = hidden,
                    ));
                }
            }
            if status.id.supports_service_control() {
                if status.state == AppStateKind::Installed {
                    out.push_str(&format!(
                        r#"<form method="post" action="/apps/start" class="inline-form" onsubmit="return confirm('Start {label}?');">
              <input type="hidden" name="name" value="{name}">
              {hidden}
              <button type="submit" class="btn-primary">Start</button>
            </form>"#,
                        label = html_escape(label),
                        name = html_escape(name),
                        hidden = hidden,
                    ));
                } else {
                    out.push_str(&format!(
                        r#"<form method="post" action="/apps/stop" class="inline-form" onsubmit="return confirm('Stop {label}?');">
              <input type="hidden" name="name" value="{name}">
              {hidden}
              <button type="submit" class="btn-secondary">Stop</button>
            </form>"#,
                        label = html_escape(label),
                        name = html_escape(name),
                        hidden = hidden,
                    ));
                }
            }
            out.push_str(&format!(
                r#"<form method="post" action="/apps/reinstall" class="inline-form" onsubmit="return confirm('Reinstall {label}?');">
              <input type="hidden" name="name" value="{name}">
              {hidden}
              <button type="submit" class="btn-secondary">Reinstall</button>
            </form>
            <form method="post" action="/apps/uninstall" {form_attrs}>
              <input type="hidden" name="name" value="{name}">
              {hidden}
              <input type="hidden" name="confirm" value="">
              <button type="submit" class="btn-danger">Uninstall</button>
            </form>"#,
                label = html_escape(label),
                name = html_escape(name),
                hidden = hidden,
                form_attrs = uninstall_form_attrs(label, &host_uninstall_impacts(status.id)),
            ));
            out
        }
    }
}

fn host_card(status: &AppStatus, domain: &str, all: &[AppStatus]) -> String {
    let meta = meta_for(status.id);
    let featured = host_package_is_featured(status.id, all);
    let featured_badge = if featured {
        r#"<span class="plugin-badge featured">Featured</span>"#
    } else {
        ""
    };
    let default_badge = if status.id == crate::apps::AppId::Tachyon {
        r#"<span class="plugin-badge featured">Default</span>"#
    } else {
        ""
    };
    let active_badge = if matches!(
        status.id,
        crate::apps::AppId::Snappymail
            | crate::apps::AppId::Tachyon
            | crate::apps::AppId::Roundcube
            | crate::apps::AppId::Nextsnapmail
    ) && crate::apps_webmail::is_active_webmail(status.id)
    {
        r#"<span class="plugin-badge featured">Active</span>"#
    } else {
        ""
    };
    let status_badge = format!(
        r#"<span class="plugin-badge">{}</span>"#,
        html_escape(meta.install_status.badge())
    );
    let pricing = if meta.pricing.eq_ignore_ascii_case("paid") {
        r#"<span class="plugin-badge paid">Paid</span>"#
    } else {
        r#"<span class="plugin-badge free">Free</span>"#
    };
    let warn = status
        .warning
        .as_ref()
        .map(|w| {
            format!(
                r#"<p class="panel-notice error" style="margin:8px 0 0;">{msg}</p>"#,
                msg = html_escape(w)
            )
        })
        .unwrap_or_default();
    let dates = format_host_dates(&meta);
    let dates_html = if dates.is_empty() {
        String::new()
    } else {
        format!(r#"<p class="plugin-dates">{}</p>"#, html_escape(&dates))
    };
    let binding_note = if !domain.is_empty() && is_associable(status.id) {
        let binds = bindings_for_domain(domain);
        let mine: Vec<_> = binds
            .iter()
            .filter(|b| b.app == status.id.as_str())
            .collect();
        if mine.is_empty() {
            String::new()
        } else {
            r#"<p class="plugin-meta">Bound to selected site</p>"#.to_string()
        }
    } else if is_site_scoped(status.id) {
        r#"<p class="plugin-meta">May drop paths under the selected site home.</p>"#.into()
    } else {
        String::new()
    };
    format!(
        r#"<article class="plugin-card">
          <h3>{label}</h3>
          <div class="plugin-badges">
            <span class="plugin-badge cat">{cat}</span>
            <span class="plugin-badge">v{ver}</span>
            {pricing}
            {featured}
            {default_badge}
            {active_badge}
            {status_badge}
          </div>
          <p class="plugin-desc">{desc}</p>
          <p class="plugin-meta">Status: {state} · Id: <code>{id}</code></p>
          <p class="plugin-meta">{detail}</p>
          {dates}
          {binding}
          {warn}
          <div class="plugin-actions">{actions}</div>
        </article>"#,
        label = html_escape(status.id.label()),
        cat = html_escape(meta.category),
        ver = html_escape(meta.version),
        pricing = pricing,
        featured = featured_badge,
        default_badge = default_badge,
        active_badge = active_badge,
        status_badge = status_badge,
        desc = html_escape(meta.description),
        state = html_escape(status.state.label()),
        id = html_escape(status.id.as_str()),
        detail = html_escape(&status.detail),
        dates = dates_html,
        binding = binding_note,
        warn = warn,
        actions = action_buttons(status, domain),
    )
}

fn category_pills(
    apps: &[AppStatus],
    active: &str,
    domain: &str,
    mode: &str,
    per_page: usize,
) -> String {
    let cats = host_categories(apps);
    let mut domain_q = format!("&amp;domain={}", urlencoding_simple(domain));
    domain_q.push_str(&format!(
        "&amp;mode={}&amp;per_page={}",
        urlencoding_simple(mode),
        per_page
    ));
    let mut out = String::from(r#"<div class="category-pills">"#);
    out.push_str(&format!(
        r#"<a class="{cls}" href="/plugins?view=host{domain_q}">All categories</a>"#,
        cls = if active.is_empty() || active.eq_ignore_ascii_case("all") {
            "active"
        } else {
            ""
        },
        domain_q = domain_q,
    ));
    out.push_str(&format!(
        r#"<a class="{cls}" href="/plugins?view=host&amp;category=Featured{domain_q}">Featured</a>"#,
        cls = if active.eq_ignore_ascii_case("featured") {
            "active"
        } else {
            ""
        },
        domain_q = domain_q,
    ));
    out.push_str(&format!(
        r#"<a class="{cls}" href="/plugins?view=host&amp;category=Paid{domain_q}">Paid</a>"#,
        cls = if active.eq_ignore_ascii_case("paid") {
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
            r#"<a class="{cls}" href="/plugins?view=host&amp;category={enc}{domain_q}">{label}</a>"#,
            cls = cls,
            enc = urlencoding_simple(cat),
            domain_q = domain_q,
            label = html_escape(cat),
        ));
    }
    out.push_str("</div>");
    out
}

pub struct AppsPageQuery<'a> {
    pub notice: Option<&'a str>,
    pub error: Option<&'a str>,
    pub domain: &'a str,
    pub sites: &'a [SiteRecord],
    pub q: &'a str,
    pub category: &'a str,
    pub mode: &'a str,
    pub page: usize,
    pub per_page: usize,
}

pub fn apps_main(q: AppsPageQuery<'_>) -> String {
    let domain = q.domain.trim();
    let apps = list_apps();
    let mode = list_mode_from_query(q.mode);
    let per_page = per_page_from_query(&q.per_page.to_string());
    let filtered = filter_host_packages(&apps, q.q, q.category);
    let total = filtered.len();
    let (page_items, page, total_pages, toolbar_per) = if mode == "scroll" {
        (filtered.as_slice(), 1usize, 1usize, total.max(1))
    } else {
        let total_pages = total.div_ceil(per_page).max(1);
        let page = page_from_query(&q.page.to_string()).min(total_pages);
        let start = (page - 1) * per_page;
        let end = (start + per_page).min(total);
        (
            if start < end {
                &filtered[start..end]
            } else {
                &filtered[..0]
            },
            page,
            total_pages,
            per_page.max(4),
        )
    };
    let mut cards = String::from(r#"<div class="plugin-grid">"#);
    for status in page_items {
        cards.push_str(&host_card(status, domain, &apps));
    }
    cards.push_str("</div>");
    if total == 0 {
        cards = r#"<p class="empty-state">No host packages match this search.</p>"#.into();
    }
    let scroll_cls = if mode == "scroll" {
        "plugin-grid-scroll is-scroll"
    } else {
        "plugin-grid-scroll"
    };
    let picker = if q.sites.is_empty() {
        r#"<p class="muted">No manageable sites yet. Host engines and webmail can still be installed without a site.</p>"#.into()
    } else {
        format!(
            r#"<form method="get" action="/plugins" class="domain-picker">
          <input type="hidden" name="view" value="host">
          <div>
            <label for="domain"><strong>Domain or subdomain</strong></label><br>
            <select id="domain" name="domain" onchange="this.form.submit()">{opts}</select>
          </div>
          <noscript><button type="submit" class="btn-secondary">Apply</button></noscript>
        </form>
        <p class="muted">Selected site paths use <code>/home/&lt;domain&gt;/...</code> (subdomains nest under the parent home).</p>"#,
            opts = site_options(q.sites, domain),
        )
    };
    format!(
        r#"{ok}
      {err}
      <article class="section-card" style="margin-bottom:14px;">
        <h2>Domain scope</h2>
        <p>MariaDB, PostgreSQL, and RabbitMQ are host packages. phpMyAdmin, Email, and webmail clients (default: Tachyon; also SnappyMail, Roundcube, NextSnapMail, SOGo) appear as store-style cards below. Install a client, then use <strong>Set as active</strong> to switch the panel proxy without orphaning mailboxes. CLI: <code>cpn app install --name tachyon</code> · <code>cpn app activate --name roundcube</code></p>
        {picker}
      </article>
      <p class="plugin-count">{count} host packages</p>
      <form method="get" action="/plugins" class="plugin-search-row">
        <input type="hidden" name="view" value="host">
        <input type="hidden" name="domain" value="{domain}">
        <input type="hidden" name="mode" value="{mode}">
        <input type="hidden" name="per_page" value="{per_page}">
        <label for="hq">Search</label>
        <input class="plugin-search" id="hq" name="q" type="search" value="{q}" placeholder="Search host packages by name or description...">
        <button type="submit" class="btn-primary">Search</button>
      </form>
      {pills}
      {toolbar}
      <div class="{scroll_cls}">{cards}</div>"#,
        ok = notice_block("ok", q.notice),
        err = notice_block("error", q.error),
        picker = picker,
        count = total,
        domain = html_escape(domain),
        mode = html_escape(mode),
        per_page = toolbar_per,
        q = html_escape(q.q),
        pills = category_pills(&apps, q.category, domain, mode, toolbar_per),
        toolbar = store_list_toolbar(mode, toolbar_per, page, total_pages, total),
        scroll_cls = scroll_cls,
        cards = cards,
    )
}

#[cfg(test)]
mod tests {
    use crate::apps::AppId;

    #[test]
    fn parse_known_webmail_ids() {
        assert_eq!(AppId::parse("tachyon").unwrap(), AppId::Tachyon);
        assert_eq!(AppId::parse("roundcube").unwrap(), AppId::Roundcube);
        assert_eq!(AppId::parse("nextsnapmail").unwrap(), AppId::Nextsnapmail);
        assert_eq!(AppId::parse("sogo").unwrap(), AppId::Sogo);
    }
}
