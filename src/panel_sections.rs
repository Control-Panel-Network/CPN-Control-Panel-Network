//! Panel section page HTML (websites, email, databases, backups).

use crate::panel_prefs::{set_remote_site_previews, set_show_document_roots};
use crate::service_detect::{detect_database, install_mariadb_server};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn email_mail_stack_notice_html() -> String {
    let stack = crate::apps_email::detect_mail_stack_public();
    if stack.postfix_running && stack.dovecot_running && stack.imap_listening {
        return String::new();
    }
    format!(
        r#"<p class="panel-notice error" role="alert">{detail} <a href="/plugins?view=store&amp;target=host&amp;q=email">Open Email host package</a> and use Start, or Install if missing.</p>"#,
        detail = html_escape(&stack.detail),
    )
}

/// Shared create-mailbox form. Prefer [`crate::panel_hub_pages_email_create`].
pub fn email_mailbox_create_form_html(username: &str, return_path: &str) -> String {
    crate::panel_hub_pages_email_create::email_mailbox_create_form_html(username, return_path)
}

pub fn email_create_main(username: &str, notice: Option<&str>, error: Option<&str>) -> String {
    crate::panel_hub_pages_email_create::email_create_main(username, notice, error)
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

pub fn set_websites_docroot_pref(show: bool) -> Result<(), String> {
    set_show_document_roots(show)?;
    Ok(())
}

/// Toggle the Microlink screenshot fallback for Site preview thumbnails.
pub fn set_websites_remote_preview_pref(enabled: bool) -> Result<(), String> {
    set_remote_site_previews(enabled)?;
    Ok(())
}

pub fn email_accounts_main(
    username: &str,
    selected_mail: Option<crate::model::MailSystem>,
    mail_client_ready: bool,
    mail_backend_ready: bool,
    notice: Option<&str>,
    error: Option<&str>,
) -> String {
    crate::panel_hub_pages_email_accounts::email_accounts_main(
        username,
        selected_mail,
        mail_client_ready,
        mail_backend_ready,
        notice,
        error,
    )
}

pub fn databases_status_main(notice: Option<&str>, error: Option<&str>) -> String {
    let status = detect_database();
    let pma = crate::apps::detect_app(crate::apps::AppId::Phpmyadmin);
    let install_form = if status.service_label == "Not detected" && !status.listening_3306 {
        r#"<form method="post" action="/databases/install-mariadb" class="stack-form" style="margin-top:18px;" onsubmit="return confirm('Install MariaDB server packages on this host now?');">
          <button type="submit" class="btn-primary">Install MariaDB</button>
        </form>
        <p class="muted">Fresh installs default to MariaDB + phpMyAdmin. This button installs MariaDB only if the host was set up without them. Requires root / panel host privileges.</p>"#
            .into()
    } else {
        String::new()
    };
    let pma_url = crate::apps_phpmyadmin::phpmyadmin_health_url();
    format!(
        r#"{heading}
      {ok}
      {err}
      <article class="section-card">
        <h2>Database status</h2>
        <ul class="kv-list">
          <li><span>Detected service</span><strong>{title}</strong></li>
          <li><span>TCP 127.0.0.1:3306</span><strong>{port}</strong></li>
          <li><span>phpMyAdmin</span><strong>{pma_state}</strong></li>
        </ul>
        <p>{detail}</p>
        <p class="muted">phpMyAdmin: {pma_detail} Local URL when wired: <code>{pma_url}</code>.</p>
        {install}
        <p class="muted">Default stack is MariaDB plus phpMyAdmin. CPN does not install Oracle MySQL as a host package.</p>
        <p class="muted">Use <a href="/databases/all">All Databases</a> to change passwords or delete with typed-name confirmation. Registry entries below still count toward package quotas.</p>
        <h3 style="margin:18px 0 8px;font-size:15px;">Register database (quota)</h3>
        <form method="post" action="/databases/create" class="stack-form" style="display:grid;gap:10px;max-width:420px;">
          <label>Name <input name="name" required maxlength="64" placeholder="app_db"></label>
          <label>Owner <input name="owner" maxlength="128" placeholder="defaults to signed-in user"></label>
          <label>Domain <input name="domain" maxlength="253" placeholder="example.com"></label>
          <button type="submit" class="btn-primary">Register database</button>
        </form>
        <h3 style="margin:22px 0 8px;font-size:15px;">Register FTP account (quota)</h3>
        <form method="post" action="/databases/ftp-create" class="stack-form" style="display:grid;gap:10px;max-width:420px;">
          <label>FTP username <input name="username" required maxlength="64" placeholder="siteftp"></label>
          <label>Owner <input name="owner" maxlength="128" placeholder="defaults to signed-in user"></label>
          <label>Domain <input name="domain" maxlength="253" placeholder="example.com"></label>
          <button type="submit" class="btn-primary">Register FTP account</button>
        </form>
      </article>"#,
        heading = section_heading(
            "MariaDB Manager",
            "Honest detection of local MariaDB and phpMyAdmin. No credentials are stored here.",
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
        pma_state = html_escape(pma.state.as_str()),
        pma_detail = html_escape(&pma.detail),
        pma_url = html_escape(pma_url),
        install = install_form,
    )
}

pub fn run_mariadb_install() -> Result<String, String> {
    install_mariadb_server()
}
