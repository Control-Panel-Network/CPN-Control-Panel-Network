//! Panel section page HTML (websites, email, databases, backups).

use crate::http_helpers::smtp_status_public;
use crate::install_webmail_runtime::webmail_health_url;
use crate::panel_prefs::{load_panel_ui_prefs, set_show_document_roots};
use crate::service_detect::{detect_database, install_mariadb_server};
use crate::sites::{SiteRecord, list_sites};

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

fn site_action_buttons(site: &SiteRecord) -> String {
    let domain = html_escape(&site.domain);
    let suspend = if site.enabled {
        format!(
            r#"<form method="post" action="/websites/suspend" class="inline-form" onsubmit="return confirm('Suspend {domain}?');">
              <input type="hidden" name="domain" value="{domain}">
              <button type="submit" class="btn-warn" style="min-height:36px;padding:0 12px;border:0;border-radius:999px;background:#fffaeb;color:#b54708;font-weight:700;cursor:pointer;">Suspend</button>
            </form>"#,
            domain = domain,
        )
    } else {
        format!(
            r#"<form method="post" action="/websites/resume" class="inline-form">
              <input type="hidden" name="domain" value="{domain}">
              <button type="submit" class="btn-secondary" style="min-height:36px;padding:0 12px;border:0;border-radius:999px;background:#f2f4f7;color:#344054;font-weight:700;cursor:pointer;">Resume</button>
            </form>"#,
            domain = domain,
        )
    };
    format!(
        r#"<div class="site-actions" style="display:flex;flex-wrap:wrap;gap:6px;justify-content:flex-end;">
            <a class="btn-primary" style="min-height:36px;padding:0 14px;font-size:13px;" href="/websites/manage?domain={domain}">Manage</a>
            <a class="btn-secondary" style="min-height:36px;padding:0 12px;border-radius:999px;background:#f2f4f7;color:#344054;font-weight:700;display:inline-flex;align-items:center;font-size:13px;" href="/websites/manage?domain={domain}#settings">Settings</a>
            <a class="btn-secondary" style="min-height:36px;padding:0 12px;border-radius:999px;background:#f2f4f7;color:#344054;font-weight:700;display:inline-flex;align-items:center;font-size:13px;" href="/websites/manage?domain={domain}#docroot">File manager</a>
            {suspend}
            <form method="post" action="/websites/delete" class="inline-form" onsubmit="return confirm('Delete site {domain}? Document files under /home are kept.');">
              <input type="hidden" name="domain" value="{domain}">
              <button type="submit" class="btn-danger" style="min-height:36px;padding:0 12px;font-size:13px;">Delete</button>
            </form>
          </div>"#,
        domain = domain,
        suspend = suspend,
    )
}

fn site_rows(sites: &[SiteRecord], show_docroots: bool) -> String {
    if sites.is_empty() {
        return r#"<p class="empty-state">No sites yet. Create one below or use <code>cpn site create</code>.</p>
        <p class="muted">New sites store files under the domain home (for example <code>/home/example.com/public_html</code>).</p>"#
            .into();
    }
    let docroot_th = if show_docroots {
        "<th>Document root</th>"
    } else {
        ""
    };
    let mut rows = format!(
        r#"<div class="table-wrap"><table class="data-table">
      <thead><tr><th>Domain</th><th>Owner</th>{docroot_th}<th>Status</th><th>Actions</th></tr></thead><tbody>"#
    );
    for site in sites {
        let status = if site.enabled { "Active" } else { "Suspended" };
        let wired = if site.vhost_wired {
            "vhost wired"
        } else {
            "files ready"
        };
        let docroot_td = if show_docroots {
            let legacy = if crate::sites::is_legacy_docroot(&site.docroot) {
                r#"<div class="muted">Legacy path (still served from this location).</div>"#
            } else {
                ""
            };
            format!(
                r#"<td><details><summary>Show path</summary><code>{docroot}</code>{legacy}</details></td>"#,
                docroot = html_escape(&site.docroot),
                legacy = legacy,
            )
        } else {
            String::new()
        };
        rows.push_str(&format!(
            r#"<tr>
          <td><strong>{domain}</strong><div class="muted">{wired}</div></td>
          <td>{owner}</td>
          {docroot_td}
          <td>{status}</td>
          <td>{actions}</td>
        </tr>"#,
            domain = html_escape(&site.domain),
            owner = html_escape(&site.owner),
            docroot_td = docroot_td,
            status = status,
            actions = site_action_buttons(site),
        ));
    }
    rows.push_str("</tbody></table></div>");
    rows
}

pub fn websites_main(notice: Option<&str>, error: Option<&str>) -> String {
    let sites = list_sites().unwrap_or_default();
    let prefs = load_panel_ui_prefs();
    let show = prefs.show_document_roots;
    let toggle_label = if show {
        "Hide document roots"
    } else {
        "Show document roots"
    };
    let toggle_value = if show { "0" } else { "1" };
    format!(
        r#"{heading}
      {ok}
      {err}
      <article class="section-card">
        <h2>Sites ({count})</h2>
        <p class="muted">Each site has a Manage page with overview, quick links, and suspend/delete. Document roots live under the domain home. Vhost wiring is applied later by panel recipes.</p>
        <form method="post" action="/websites/prefs" class="inline-form" style="margin:12px 0;">
          <input type="hidden" name="show_document_roots" value="{toggle_value}">
          <button type="submit" class="btn-secondary" style="min-height:40px;padding:0 14px;border:0;border-radius:999px;background:#f2f4f7;color:#344054;font-weight:700;cursor:pointer;">{toggle_label}</button>
        </form>
        {rows}
      </article>
      <article class="section-card" style="margin-top:22px;">
        <h2>Add site</h2>
        <p>Creates the site home and document root. Subdomains require the parent domain first. Optional custom docroot must be an absolute path.</p>
        <form method="post" action="/websites/create" class="stack-form">
          <label for="domain">Domain</label>
          <input id="domain" name="domain" type="text" required placeholder="example.com" autocomplete="off">
          <label for="owner">Owner</label>
          <input id="owner" name="owner" type="text" required placeholder="admin" autocomplete="username">
          <label for="docroot">Docroot (optional)</label>
          <input id="docroot" name="docroot" type="text" placeholder="/home/example.com/public_html">
          <button type="submit" class="btn-primary">Create site</button>
        </form>
      </article>"#,
        heading = section_heading(
            "Websites",
            "Manage website files under /home for each domain.",
        ),
        ok = notice_block("ok", notice),
        err = notice_block("error", error),
        count = sites.len(),
        toggle_value = toggle_value,
        toggle_label = toggle_label,
        rows = site_rows(&sites, show),
    )
}

pub fn set_websites_docroot_pref(show: bool) -> Result<(), String> {
    set_show_document_roots(show)?;
    Ok(())
}

pub fn email_main(
    selected_mail: Option<crate::model::MailSystem>,
    mail_client_ready: bool,
    mail_backend_ready: bool,
    notice: Option<&str>,
    error: Option<&str>,
) -> String {
    use crate::mail_accounts::{MailSmtpMode, list_accounts_public};
    use crate::postfix_fallback::postfix_is_ready;

    let mail = selected_mail
        .map(|value| value.label())
        .unwrap_or("Not selected");
    let client_ready = if mail_client_ready {
        "Ready"
    } else {
        "Not installed"
    };
    let backend_ready = if mail_backend_ready {
        "Ready"
    } else {
        "Not verified"
    };
    let smtp = smtp_status_public();
    let postfix_ready = postfix_is_ready();
    let smtp_line = if smtp.configured {
        format!(
            "{}:{} ({}) from {}",
            smtp.host.as_deref().unwrap_or("-"),
            smtp.port.unwrap_or(0),
            smtp.tls_mode.as_deref().unwrap_or("-"),
            smtp.from_address.as_deref().unwrap_or("-"),
        )
    } else if postfix_ready {
        "Using local Postfix fallback (127.0.0.1)".into()
    } else {
        "Outbound SMTP is not configured".into()
    };
    let mta_line = if postfix_ready {
        "Postfix running (default local MTA)"
    } else {
        "Postfix not detected"
    };
    let webmail = if mail_client_ready
        && matches!(
            selected_mail,
            Some(crate::model::MailSystem::Snappymail | crate::model::MailSystem::Roundcube)
        ) {
        format!(
            r#"<p><a class="btn-primary" href="{url}" target="_blank" rel="noopener noreferrer">Open webmail</a></p>
        <p class="muted">Local health URL: <code>{url}</code></p>"#,
            url = html_escape(webmail_health_url()),
        )
    } else if matches!(selected_mail, Some(crate::model::MailSystem::Thunderbird)) {
        "<p class=\"muted\">Thunderbird is a desktop client only. No local webmail URL is provisioned.</p>"
            .into()
    } else {
        "<p class=\"muted\">Install a webmail stack from the installer mail stage to enable a local webmail link.</p>"
            .into()
    };

    let accounts = list_accounts_public();
    let mut rows = String::new();
    if accounts.is_empty() {
        rows.push_str(r#"<p class="muted">No mailboxes yet. Enabled accounts require valid external SMTP or a running Postfix local binding.</p>"#);
    } else {
        rows.push_str(r#"<table class="data-table" style="width:100%;border-collapse:collapse;margin-top:12px;"><thead><tr><th align="left">Address</th><th align="left">SMTP</th><th align="left">Valid</th><th align="left">State</th><th></th></tr></thead><tbody>"#);
        for acct in &accounts {
            let valid = if acct.smtp_valid { "Valid" } else { "Invalid" };
            let err = acct
                .smtp_error
                .as_ref()
                .map(|e| format!(r#" <span class="muted">({})</span>"#, html_escape(e)))
                .unwrap_or_default();
            let mode = match acct.smtp_mode {
                MailSmtpMode::External => "external",
                MailSmtpMode::PostfixLocal => "postfix_local",
            };
            let toggle = if acct.enabled {
                format!(
                    r#"<form method="post" action="/email/accounts/disable" class="inline-form" style="display:inline;"><input type="hidden" name="id" value="{id}"><button type="submit" class="btn-secondary">Disable</button></form>"#,
                    id = html_escape(&acct.id),
                )
            } else {
                format!(
                    r#"<form method="post" action="/email/accounts/enable" class="inline-form" style="display:inline;" onsubmit="return confirm('Enable only if SMTP is valid.');"><input type="hidden" name="id" value="{id}"><button type="submit" class="btn-primary">Enable</button></form>"#,
                    id = html_escape(&acct.id),
                )
            };
            rows.push_str(&format!(
                r#"<tr><td>{addr}<br><span class="muted">{domain}</span></td><td>{mode}: {summary}</td><td><strong>{valid}</strong>{err}</td><td>{state}</td><td>{toggle}</td></tr>"#,
                addr = html_escape(&acct.address),
                domain = html_escape(if acct.domain.is_empty() {
                    "-"
                } else {
                    &acct.domain
                }),
                mode = mode,
                summary = html_escape(&acct.smtp_summary),
                valid = valid,
                err = err,
                state = if acct.enabled { "Enabled" } else { "Disabled" },
                toggle = toggle,
            ));
        }
        rows.push_str("</tbody></table>");
    }

    let create_form = r#"
      <form method="post" action="/email/accounts/create" class="stack-form" style="max-width:560px;margin-top:16px;">
        <label for="address">Mailbox address</label>
        <input id="address" name="address" type="email" required placeholder="user@example.com">
        <label for="domain">Site FQDN (optional)</label>
        <input id="domain" name="domain" type="text" placeholder="example.com or blog.example.com">
        <label for="smtp_mode">SMTP mode</label>
        <select id="smtp_mode" name="smtp_mode">
          <option value="postfix_local">Local Postfix (default)</option>
          <option value="external">External SMTP</option>
        </select>
        <label for="smtp_host">External host</label>
        <input id="smtp_host" name="smtp_host" type="text" placeholder="smtp.example.com">
        <label for="smtp_port">Port</label>
        <input id="smtp_port" name="smtp_port" type="number" value="587">
        <label for="smtp_tls">Encryption</label>
        <select id="smtp_tls" name="smtp_tls">
          <option value="starttls">STARTTLS</option>
          <option value="tls">TLS</option>
          <option value="none">None</option>
        </select>
        <label for="smtp_username">SMTP username</label>
        <input id="smtp_username" name="smtp_username" type="text" autocomplete="off">
        <label for="smtp_password">SMTP password</label>
        <input id="smtp_password" name="smtp_password" type="password" autocomplete="new-password">
        <label><input type="checkbox" name="enabled" value="1" checked> Enable now (requires valid SMTP)</label>
        <button type="submit" class="btn-primary">Create mailbox</button>
      </form>"#;

    format!(
        r#"{heading}
      {ok}
      {err}
      <article class="section-card">
        <h2>Mail stack</h2>
        <ul class="kv-list">
          <li><span>Selected client</span><strong>{mail}</strong></li>
          <li><span>Webmail client</span><strong>{client}</strong></li>
          <li><span>IMAP/SMTP backend</span><strong>{backend}</strong></li>
          <li><span>Local MTA</span><strong>{mta}</strong></li>
          <li><span>Outbound SMTP</span><strong>{smtp}</strong></li>
        </ul>
        <p class="muted">If no external SMTP was set during install, Postfix is the default local MTA. Switching to external SMTP later does not remove Postfix unless you uninstall Email.</p>
        {webmail}
      </article>
      <article class="section-card" style="margin-top:18px;">
        <h2>Mailboxes</h2>
        <p>Enabled accounts must have complete external SMTP or a verified Postfix local binding.</p>
        {rows}
        {create}
      </article>"#,
        heading = section_heading(
            "Email",
            "Mail stack, Postfix default MTA, and per-mailbox SMTP validity.",
        ),
        ok = notice_block("ok", notice),
        err = notice_block("error", error),
        mail = html_escape(mail),
        client = client_ready,
        backend = backend_ready,
        mta = mta_line,
        smtp = html_escape(&smtp_line),
        webmail = webmail,
        rows = rows,
        create = create_form,
    )
}

pub fn databases_main(notice: Option<&str>, error: Option<&str>) -> String {
    let status = detect_database();
    let install_form = if status.service_label == "Not detected" && !status.listening_3306 {
        r#"<form method="post" action="/databases/install-mariadb" class="stack-form" style="margin-top:18px;" onsubmit="return confirm('Install MariaDB server packages on this host now?');">
          <button type="submit" class="btn-primary">Install MariaDB</button>
        </form>
        <p class="muted">Runs a package install via dnf or apt (mariadb-server), then enables the service. Requires root / panel host privileges.</p>"#
            .into()
    } else {
        String::new()
    };
    format!(
        r#"{heading}
      {ok}
      {err}
      <article class="section-card">
        <h2>Database status</h2>
        <ul class="kv-list">
          <li><span>Detected service</span><strong>{title}</strong></li>
          <li><span>TCP 127.0.0.1:3306</span><strong>{port}</strong></li>
        </ul>
        <p>{detail}</p>
        {install}
        <p class="muted">Next steps: install MariaDB, create databases with your preferred tooling, then wire panel DB management in a later release.</p>
      </article>"#,
        heading = section_heading(
            "Databases",
            "Honest detection of local MariaDB/MySQL. No credentials are stored here.",
        ),
        ok = notice_block("ok", notice),
        err = notice_block("error", error),
        title = html_escape(&status.service_label),
        port = if status.listening_3306 {
            "Open"
        } else {
            "Closed"
        },
        detail = html_escape(&status.detail),
        install = install_form,
    )
}

pub fn run_mariadb_install() -> Result<String, String> {
    install_mariadb_server()
}
