//! Hub HTML for Email and Databases & FTP.

use crate::http_helpers::smtp_status_public;
use crate::install_webmail_runtime::webmail_health_url;
use crate::panel_hub_defs::{databases_hub_sections, email_hub_sections};
use crate::panel_hubs::{
    feature_shell, hub_tiles_grid, not_configured_body, section_heading, status_kv,
};
use crate::panel_ops_db::{create_database, drop_database, list_databases};
use crate::panel_ops_ftp::detect_ftp;
use crate::panel_ops_mail_extra::{
    CatchAll, MailForward, dkim_status, ensure_dkim_dir, load_catchall, load_forwards,
    mail_stack_note, save_catchall, save_forwards,
};
use crate::panel_sections::{databases_status_main, email_accounts_main};
use crate::postfix_fallback::postfix_is_ready;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn email_hub_main() -> String {
    let mut body = section_heading(
        "Email",
        "Mailboxes, forwarding, DKIM, and deliverability tools for this CPN host.",
    );
    for (title, tiles) in email_hub_sections() {
        body.push_str(&hub_tiles_grid(title, &tiles));
    }
    body
}

pub fn databases_ftp_hub_main() -> String {
    let mut body = section_heading(
        "Databases & FTP",
        "MariaDB databases, phpMyAdmin, and FTP accounts for hosted sites.",
    );
    for (title, tiles) in databases_hub_sections() {
        body.push_str(&hub_tiles_grid(title, &tiles));
    }
    body
}

pub fn email_accounts_page(
    selected_mail: Option<crate::model::MailSystem>,
    mail_client_ready: bool,
    mail_backend_ready: bool,
    notice: Option<&str>,
    error: Option<&str>,
) -> String {
    let inner = email_accounts_main(
        selected_mail,
        mail_client_ready,
        mail_backend_ready,
        notice,
        error,
    );
    format!(
        r#"{}{}"#,
        crate::panel_hubs::breadcrumb(&[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("Email Accounts", None),
        ]),
        inner
    )
}

pub fn email_create_redirect_hint() -> String {
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("Create Email", None),
        ],
        "Create Email",
        "Add a mailbox.",
        r#"<p>Use the create form on <a href="/email/accounts">Email Accounts</a>.</p>"#,
        None,
        None,
    )
}

pub fn email_forwarding_page(notice: Option<&str>, error: Option<&str>) -> String {
    let rows = load_forwards();
    let mut list = String::from("<ul>");
    if rows.is_empty() {
        list = "<p class=\"empty-state\">No forwards stored yet.</p>".into();
    } else {
        for row in &rows {
            list.push_str(&format!(
                "<li><code>{}</code> to <code>{}</code></li>",
                html_escape(&row.from),
                html_escape(&row.to)
            ));
        }
        list.push_str("</ul>");
    }
    let form = format!(
        r#"<p class="muted">{}</p>
        {list}
        <form method="post" action="/email/forwarding/save" class="stack-form" style="max-width:520px;margin-top:16px;">
          <label for="from">From</label>
          <input id="from" name="from" type="email" required>
          <label for="to">To</label>
          <input id="to" name="to" type="email" required>
          <button type="submit" class="btn-primary">Add forward</button>
        </form>"#,
        html_escape(&mail_stack_note()),
        list = list,
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("Forwarding", None),
        ],
        "Forwarding",
        "Forward to other addresses.",
        &form,
        notice,
        error,
    )
}

pub fn add_forward(from: &str, to: &str) -> Result<String, String> {
    let mut rows = load_forwards();
    rows.push(MailForward {
        from: from.trim().to_string(),
        to: to.trim().to_string(),
    });
    save_forwards(&rows)?;
    Ok("Forward saved".into())
}

pub fn email_catchall_page(notice: Option<&str>, error: Option<&str>) -> String {
    let rows = load_catchall();
    let mut list = if rows.is_empty() {
        "<p class=\"empty-state\">No catch-all rules stored yet.</p>".into()
    } else {
        let mut ul = String::from("<ul>");
        for row in &rows {
            ul.push_str(&format!(
                "<li><code>{}</code> to <code>{}</code></li>",
                html_escape(&row.domain),
                html_escape(&row.target)
            ));
        }
        ul.push_str("</ul>");
        ul
    };
    let _ = &mut list;
    let form = format!(
        r#"<p class="muted">{}</p>
        {list}
        <form method="post" action="/email/catchall/save" class="stack-form" style="max-width:520px;margin-top:16px;">
          <label for="domain">Domain</label>
          <input id="domain" name="domain" type="text" required placeholder="example.com">
          <label for="target">Target mailbox</label>
          <input id="target" name="target" type="email" required>
          <button type="submit" class="btn-primary">Add catch-all</button>
        </form>"#,
        html_escape(&mail_stack_note()),
        list = list,
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("Catch-All", None),
        ],
        "Catch-All",
        "Catch unrouted mail.",
        &form,
        notice,
        error,
    )
}

pub fn add_catchall(domain: &str, target: &str) -> Result<String, String> {
    let mut rows = load_catchall();
    rows.push(CatchAll {
        domain: domain.trim().to_string(),
        target: target.trim().to_string(),
    });
    save_catchall(&rows)?;
    Ok("Catch-all saved".into())
}

pub fn email_dkim_page() -> String {
    let (ready, detail) = dkim_status();
    let body = if ready {
        format!(
            "<p>{}</p><form method=\"post\" action=\"/email/dkim/ensure\"><button class=\"btn-primary\" type=\"submit\">Ensure DKIM directory</button></form>",
            html_escape(&detail)
        )
    } else {
        format!(
            "{}{}",
            not_configured_body(
                &detail,
                "Create the DKIM store directory when Postfix/OpenDKIM is ready."
            ),
            r#"<form method="post" action="/email/dkim/ensure" style="margin-top:12px;"><button class="btn-primary" type="submit">Create DKIM directory</button></form>"#
        )
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("DKIM Manager", None),
        ],
        "DKIM Manager",
        "Email signing keys.",
        &body,
        None,
        None,
    )
}

pub fn ensure_dkim() -> Result<String, String> {
    let dir = ensure_dkim_dir()?;
    Ok(format!("DKIM directory ready at {}", dir.display()))
}

pub fn email_webmail_page(
    selected_mail: Option<crate::model::MailSystem>,
    mail_client_ready: bool,
) -> String {
    let body = if mail_client_ready
        && matches!(
            selected_mail,
            Some(crate::model::MailSystem::Snappymail | crate::model::MailSystem::Roundcube)
        ) {
        format!(
            r#"<p><a class="btn-primary" href="{url}" target="_blank" rel="noopener noreferrer">Open webmail</a></p>
            <p class="muted">Health URL: <code>{url}</code></p>"#,
            url = html_escape(webmail_health_url()),
        )
    } else {
        not_configured_body(
            "Webmail client is not installed or not selected.",
            "Install SnappyMail or Roundcube from the installer mail stage.",
        )
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("Webmail", None),
        ],
        "Webmail",
        "Open webmail.",
        &body,
        None,
        None,
    )
}

pub fn email_delivery_page() -> String {
    let smtp = smtp_status_public();
    let mta = if postfix_is_ready() {
        "Postfix ready"
    } else {
        "Postfix not detected"
    };
    let smtp_line = if smtp.configured {
        format!(
            "{}:{} ({})",
            smtp.host.as_deref().unwrap_or("-"),
            smtp.port.unwrap_or(0),
            smtp.tls_mode.as_deref().unwrap_or("-"),
        )
    } else {
        "Not configured".into()
    };
    let kv = status_kv(&[("Local MTA", mta), ("Outbound SMTP", &smtp_line)]);
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("Email Delivery", None),
        ],
        "Email Delivery",
        "SMTP relay and domains.",
        &format!(
            "{kv}<p class=\"muted\">{}</p>",
            html_escape(&mail_stack_note())
        ),
        None,
        None,
    )
}

pub fn scaffold_feature(
    section: &str,
    section_href: &str,
    title: &str,
    subtitle: &str,
    detail: &str,
) -> String {
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            (section, Some(section_href)),
            (title, None),
        ],
        title,
        subtitle,
        &not_configured_body(
            detail,
            "This tile is scaffolded honestly until the backend ships.",
        ),
        None,
        None,
    )
}

pub fn databases_all_page(notice: Option<&str>, error: Option<&str>) -> String {
    let status = list_databases();
    let list = if status.databases.is_empty() {
        format!(
            "<p class=\"empty-state\">{}</p>",
            html_escape(&status.detail)
        )
    } else {
        let mut ul = String::from("<ul>");
        for db in &status.databases {
            ul.push_str(&format!("<li><code>{}</code></li>", html_escape(db)));
        }
        ul.push_str("</ul>");
        format!(
            "<p class=\"muted\">{}</p>{ul}",
            html_escape(&status.detail),
            ul = ul
        )
    };
    let kv = status_kv(&[
        ("Engine", &status.engine_label),
        ("TCP 3306", if status.listening { "Open" } else { "Closed" }),
    ]);
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Databases & FTP", Some("/databases")),
            ("All Databases", None),
        ],
        "All Databases",
        "View databases (MariaDB first).",
        &format!("{kv}{list}"),
        notice,
        error,
    )
}

pub fn databases_create_page(notice: Option<&str>, error: Option<&str>) -> String {
    let form = r#"<form method="post" action="/databases/create" class="stack-form" style="max-width:420px;">
      <label for="name">Database name</label>
      <input id="name" name="name" type="text" required pattern="[A-Za-z0-9_]+" maxlength="64">
      <button type="submit" class="btn-primary">Create database</button>
    </form>
    <p class="muted">Uses local MariaDB/MySQL client auth. Letters, digits, and underscore only.</p>"#;
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Databases & FTP", Some("/databases")),
            ("Create Database", None),
        ],
        "Create Database",
        "Add a database.",
        form,
        notice,
        error,
    )
}

pub fn databases_delete_page(notice: Option<&str>, error: Option<&str>) -> String {
    let form = r#"<form method="post" action="/databases/delete" class="stack-form" style="max-width:420px;" onsubmit="return confirm('Drop this database permanently?');">
      <label for="name">Database name</label>
      <input id="name" name="name" type="text" required pattern="[A-Za-z0-9_]+" maxlength="64">
      <button type="submit" class="btn-danger">Delete database</button>
    </form>
    <p class="muted">Refuses system schemas (mysql, sys, information_schema, performance_schema).</p>"#;
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Databases & FTP", Some("/databases")),
            ("Delete Database", None),
        ],
        "Delete Database",
        "Remove a database.",
        form,
        notice,
        error,
    )
}

pub fn run_create_database(name: &str) -> Result<String, String> {
    create_database(name)
}

pub fn run_drop_database(name: &str) -> Result<String, String> {
    drop_database(name)
}

pub fn databases_manager_page(notice: Option<&str>, error: Option<&str>) -> String {
    let inner = databases_status_main(notice, error);
    format!(
        r#"{}{}"#,
        crate::panel_hubs::breadcrumb(&[
            ("Dashboard", Some("/dashboard")),
            ("Databases & FTP", Some("/databases")),
            ("MariaDB Manager", None),
        ]),
        inner
    )
}

pub fn phpmyadmin_page() -> String {
    let body = r#"<p>phpMyAdmin is installed per site via Apps when available.</p>
      <p><a class="btn-primary" href="/apps">Open Apps</a></p>
      <p class="muted">MariaDB is the default database engine. phpMyAdmin is the default companion UI when installed.</p>"#;
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Databases & FTP", Some("/databases")),
            ("phpMyAdmin", None),
        ],
        "phpMyAdmin",
        "Open phpMyAdmin.",
        body,
        None,
        None,
    )
}

pub fn ftp_accounts_page() -> String {
    let status = detect_ftp();
    let kv = status_kv(&[("Stack", &status.stack)]);
    let extra = if status.ready {
        format!("<p>{}</p>", html_escape(&status.detail))
    } else {
        not_configured_body(
            &status.detail,
            "Install Pure-FTPd or vsftpd, then return here.",
        )
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Databases & FTP", Some("/databases")),
            ("FTP Accounts", None),
        ],
        "FTP Accounts",
        "View FTP users.",
        &format!("{kv}{extra}"),
        None,
        None,
    )
}
