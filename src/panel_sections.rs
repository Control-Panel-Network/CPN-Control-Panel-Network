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

fn site_rows(sites: &[SiteRecord], show_docroots: bool) -> String {
    if sites.is_empty() {
        return r#"<p class="empty-state">No sites yet. Create one below or use <code>cpn site create</code>.</p>
        <p class="muted">Files live under <code>/home/&lt;domain&gt;/public_html</code>. Subdomains nest under the parent home (for example <code>/home/example.com/blog.example.com/public_html</code>).</p>"#
            .into();
    }
    let docroot_th = if show_docroots {
        "<th>Document root</th>"
    } else {
        ""
    };
    let mut rows = format!(
        r#"<div class="table-wrap"><table class="data-table">
      <thead><tr><th>Domain</th><th>Owner</th>{docroot_th}<th>Status</th><th></th></tr></thead><tbody>"#
    );
    for site in sites {
        let status = if site.enabled { "Enabled" } else { "Disabled" };
        let wired = if site.vhost_wired {
            "vhost wired"
        } else {
            "files ready"
        };
        let docroot_td = if show_docroots {
            let legacy = if crate::sites::is_legacy_docroot(&site.docroot) {
                r#"<div class="muted">Legacy path (still served from this location). New sites use /home/&lt;domain&gt;/public_html.</div>"#
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
          <td>
            <form method="post" action="/websites/delete" class="inline-form" onsubmit="return confirm('Delete site {domain}? Document files under /home are kept.');">
              <input type="hidden" name="domain" value="{domain}">
              <button type="submit" class="btn-danger">Delete</button>
            </form>
          </td>
        </tr>"#,
            domain = html_escape(&site.domain),
            owner = html_escape(&site.owner),
            docroot_td = docroot_td,
            status = status,
            wired = wired,
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
        <p class="muted">Website files live under <code>/home/&lt;domain&gt;/public_html</code> (subdomains nest under the parent home). Internal site records are for the panel only; operators work with the files under <code>/home/</code>. Vhost wiring is applied later by panel recipes.</p>
        <form method="post" action="/websites/prefs" class="inline-form" style="margin:12px 0;">
          <input type="hidden" name="show_document_roots" value="{toggle_value}">
          <button type="submit" class="btn-secondary" style="min-height:40px;padding:0 14px;border:0;border-radius:999px;background:#f2f4f7;color:#344054;font-weight:700;cursor:pointer;">{toggle_label}</button>
        </form>
        {rows}
      </article>
      <article class="section-card" style="margin-top:22px;">
        <h2>Add site</h2>
        <p>Creates <code>/home/&lt;domain&gt;/public_html</code> (or nests a subdomain under the parent home). Subdomains require the parent domain site first. Optional custom docroot must be an absolute path.</p>
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
) -> String {
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
    let smtp_line = if smtp.configured {
        format!(
            "{}:{} ({}) from {}",
            smtp.host.as_deref().unwrap_or("-"),
            smtp.port.unwrap_or(0),
            smtp.tls_mode.as_deref().unwrap_or("-"),
            smtp.from_address.as_deref().unwrap_or("-"),
        )
    } else {
        "Outbound SMTP is not configured".into()
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
    format!(
        r#"{heading}
      <article class="section-card">
        <h2>Mail stack</h2>
        <ul class="kv-list">
          <li><span>Selected client</span><strong>{mail}</strong></li>
          <li><span>Webmail client</span><strong>{client}</strong></li>
          <li><span>IMAP/SMTP backend</span><strong>{backend}</strong></li>
          <li><span>Outbound SMTP</span><strong>{smtp}</strong></li>
        </ul>
        {webmail}
      </article>"#,
        heading = section_heading(
            "Email",
            "Mail client status and outbound SMTP summary (no secrets).",
        ),
        mail = html_escape(mail),
        client = client_ready,
        backend = backend_ready,
        smtp = html_escape(&smtp_line),
        webmail = webmail,
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
