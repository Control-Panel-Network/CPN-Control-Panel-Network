//! Hub HTML for Email and Databases & FTP.

use crate::http_helpers::smtp_status_public;
use crate::panel_hub_defs::{databases_hub_sections, email_hub_sections};
use crate::panel_hubs::{
    feature_shell, hub_tiles_grid, not_configured_body, section_heading, status_kv,
};
use crate::panel_ops_db::{create_database, drop_database};
use crate::panel_ops_mail_extra::{dkim_status, load_catchall, load_forwards, mail_stack_note};
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
    let feats = crate::panel_feature_gate::InstalledOptionalFeatures::detect();
    let mut body = section_heading(
        "Email",
        "Mailboxes, forwarding, DKIM, and deliverability tools for this CPN host.",
    );
    for (title, tiles) in email_hub_sections() {
        let filtered = crate::panel_feature_gate::filter_hub_tiles(tiles, feats);
        if filtered.is_empty() {
            continue;
        }
        body.push_str(&hub_tiles_grid(title, &filtered));
    }
    body
}

pub fn databases_ftp_hub_main() -> String {
    let feats = crate::panel_feature_gate::InstalledOptionalFeatures::detect();
    let blurb = if feats.phpmyadmin {
        "MariaDB databases, phpMyAdmin, and FTP accounts for hosted sites."
    } else {
        "MariaDB databases and FTP accounts for hosted sites."
    };
    let mut body = section_heading("Databases & FTP", blurb);
    for (title, tiles) in databases_hub_sections() {
        let filtered = crate::panel_feature_gate::filter_hub_tiles(tiles, feats);
        if filtered.is_empty() {
            continue;
        }
        body.push_str(&hub_tiles_grid(title, &filtered));
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

pub fn email_create_page(notice: Option<&str>, error: Option<&str>) -> String {
    format!(
        r#"{}{}"#,
        crate::panel_hubs::breadcrumb(&[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("Create Email", None),
        ]),
        crate::panel_sections::email_create_main(notice, error),
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
    // Legacy helper: prefer `add_forward_for` from routes (owner + Postfix maps).
    crate::panel_ops_mail_extra::add_forward_for("system", from, to)
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
    crate::panel_ops_mail_extra::add_catchall_for("system", domain, target)
}

fn dkim_domain_table_html() -> String {
    let rows = crate::panel_ops_dkim_keys::list_dkim_domain_rows();
    if rows.is_empty() {
        return r#"<p class="muted">No domain folders yet. Run Ensure DKIM or create a website to generate keys.</p>"#.into();
    }
    let mut table = String::from(
        r#"<div class="table-wrap"><table class="data-table"><thead><tr><th>Domain</th><th>Key</th><th>DNS name</th><th>TXT preview</th></tr></thead><tbody>"#,
    );
    for row in &rows {
        let key_label = if row.has_key { "Present" } else { "Missing" };
        table.push_str(&format!(
            r#"<tr><td>{domain}</td><td>{key}</td><td><code>{dns}</code></td><td class="muted"><code>{txt}</code></td></tr>"#,
            domain = html_escape(&row.domain),
            key = key_label,
            dns = html_escape(&row.dns_name),
            txt = html_escape(&row.txt_preview),
        ));
    }
    table.push_str("</tbody></table></div>");
    table
}

pub fn email_dkim_page(notice: Option<&str>, error: Option<&str>) -> String {
    let (ready, detail) = dkim_status();
    let table = dkim_domain_table_html();
    let body = if ready {
        format!(
            r#"<p>{detail}</p>
        <h3 style="margin:16px 0 8px;font-size:15px;">Domain keys</h3>
        {table}
        <form method="post" action="/email/dkim/ensure" style="margin-top:16px;">
          <button class="btn-primary" type="submit">Ensure DKIM directory and keys</button>
        </form>
        <p class="muted">Ensure creates <code>/var/lib/cpn/dkim</code> if needed and generates selector <code>default</code> keys for each registered site domain.</p>"#,
            detail = html_escape(&detail),
            table = table,
        )
    } else {
        format!(
            r#"{not_cfg}
        <h3 style="margin:16px 0 8px;font-size:15px;">Domain keys</h3>
        {table}
        <form method="post" action="/email/dkim/ensure" style="margin-top:12px;">
          <button class="btn-primary" type="submit">Create DKIM directory and keys</button>
        </form>"#,
            not_cfg = not_configured_body(
                &detail,
                "Create the DKIM store and per-domain keys for DNS TXT records.",
            ),
            table = table,
        )
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("DKIM Manager", None),
        ],
        "DKIM Manager",
        "Email signing keys for your site domains.",
        &body,
        notice,
        error,
    )
}

pub fn ensure_dkim() -> Result<String, String> {
    crate::panel_ops_mail_extra::ensure_dkim_store_ready()?;
    let sites = crate::sites::list_sites().unwrap_or_default();
    let mut existing = 0usize;
    let mut generated = 0usize;
    let mut errors = Vec::new();
    for site in &sites {
        match crate::panel_ops_dkim_keys::ensure_dkim_for_domain(&site.domain) {
            Ok(msg) => {
                if msg.contains("already present") {
                    existing += 1;
                } else if msg.contains("Generated") {
                    generated += 1;
                }
            }
            Err(e) => errors.push(format!("{}: {e}", site.domain)),
        }
    }
    if !errors.is_empty() {
        return Err(errors.join("; "));
    }
    let folder_count = crate::panel_ops_dkim_keys::list_dkim_domain_rows().len();
    Ok(format!(
        "DKIM store is ready under /var/lib/cpn/dkim. {folder_count} domain folder(s) on disk. Checked {} registered site(s): {existing} already had keys, {generated} newly generated.",
        sites.len()
    ))
}

/// Deprecated wrapper: prefer `panel_hub_pages_webmail::email_webmail_page`.
pub fn email_webmail_page(
    _selected_mail: Option<crate::model::MailSystem>,
    _mail_client_ready: bool,
) -> String {
    crate::panel_hub_pages_webmail::email_webmail_page(None, None)
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
    crate::panel_hub_pages_db_manage::databases_all_page_for("", notice, error)
}

pub fn databases_create_page(notice: Option<&str>, error: Option<&str>) -> String {
    let form = r#"<form method="post" action="/databases/create" class="stack-form" style="max-width:420px;">
      <label for="name">Database name</label>
      <input id="name" name="name" type="text" required pattern="[A-Za-z0-9_]+" maxlength="64">
      <button type="submit" class="btn-primary">Create database</button>
    </form>
    <p class="muted">Uses local MariaDB client auth. Letters, digits, and underscore only.</p>"#;
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
    crate::panel_hub_pages_db_manage::databases_delete_page_for("", None, notice, error)
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

pub fn phpmyadmin_page(notice: Option<&str>, error: Option<&str>) -> String {
    let (installed, detail) = crate::apps_phpmyadmin_sso::phpmyadmin_open_status();
    let body = if installed {
        format!(
            r#"<p>{detail}</p>
      <p style="display:flex;flex-wrap:wrap;gap:10px;margin:16px 0;">
        <a class="btn-primary" href="/databases/phpmyadmin/open" target="_blank" rel="noopener noreferrer">Open phpMyAdmin (auto-login)</a>
        <a class="btn-secondary" href="/plugins?view=store&amp;category=Host">Host packages</a>
      </p>
      <p class="muted">Auto-login creates a short-lived MariaDB user and sign-on token (never shown). Panel admin Host open may use full grants. Site open with <code>?domain=</code> jails grants to databases registered for that domain only. CPN serves phpMyAdmin under <code>/phpmyadmin/</code> on the panel port (loopback backend <code>127.0.0.1:8081</code>).</p>"#,
            detail = html_escape(&detail),
        )
    } else {
        format!(
            r#"<p>{detail}</p>
      <p><a class="btn-primary" href="/plugins?view=store&amp;category=Host">Install via Host packages</a></p>
      <p class="muted">MariaDB is the default database engine. phpMyAdmin is the default companion UI when installed.</p>"#,
            detail = html_escape(&detail),
        )
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Databases & FTP", Some("/databases")),
            ("phpMyAdmin", None),
        ],
        "phpMyAdmin",
        "Open phpMyAdmin with optional auto-login.",
        &body,
        notice,
        error,
    )
}

pub use crate::panel_hub_pages_ftp::{
    ftp_accounts_page, ftp_create_page, ftp_delete_page, ftp_reset_page,
};
