//! Panel section page HTML (websites, email, databases, backups).

use crate::account::data_dir;
use crate::http_helpers::smtp_status_public;
use crate::install_webmail_runtime::webmail_health_url;
use crate::sites::{SiteRecord, list_sites};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn backups_dir() -> PathBuf {
    data_dir().join("backups")
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

fn site_rows(sites: &[SiteRecord]) -> String {
    if sites.is_empty() {
        return r#"<p class="empty-state">No sites yet. Create one below or use <code>cpn site create</code>.</p>
        <p class="muted">Files live under <code>/home/&lt;domain&gt;/public_html</code>. Subdomains nest under the parent home (for example <code>/home/example.com/blog.example.com/public_html</code>).</p>"#
            .into();
    }
    let mut rows = String::from(
        r#"<div class="table-wrap"><table class="data-table">
      <thead><tr><th>Domain</th><th>Owner</th><th>Document root</th><th>Status</th><th></th></tr></thead><tbody>"#,
    );
    for site in sites {
        let status = if site.enabled { "Enabled" } else { "Disabled" };
        let wired = if site.vhost_wired {
            "vhost wired"
        } else {
            "record only"
        };
        let legacy = if crate::sites::is_legacy_docroot(&site.docroot) {
            r#"<div class="muted">Legacy path (still served from this location). New sites use /home/&lt;domain&gt;/public_html.</div>"#
        } else {
            ""
        };
        rows.push_str(&format!(
            r#"<tr>
          <td><strong>{domain}</strong><div class="muted">{wired}</div></td>
          <td>{owner}</td>
          <td><code>{docroot}</code>{legacy}</td>
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
            docroot = html_escape(&site.docroot),
            legacy = legacy,
            status = status,
            wired = wired,
        ));
    }
    rows.push_str("</tbody></table></div>");
    rows
}

pub fn websites_main(notice: Option<&str>, error: Option<&str>) -> String {
    let sites = list_sites().unwrap_or_default();
    format!(
        r#"{heading}
      {ok}
      {err}
      <article class="section-card">
        <h2>Sites ({count})</h2>
        <p class="muted">Website files live under <code>/home/&lt;domain&gt;/</code> (docroot <code>public_html</code>). A small registry JSON under <code>/var/lib/cpn/sites/</code> points at each docroot. Vhost wiring is applied later by panel recipes.</p>
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
            "Manage website files under /home and site registry records.",
        ),
        ok = notice_block("ok", notice),
        err = notice_block("error", error),
        count = sites.len(),
        rows = site_rows(&sites),
    )
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

fn service_active(names: &[&str]) -> Option<&'static str> {
    for name in names {
        let output = Command::new("systemctl")
            .args(["is-active", "--quiet", name])
            .status();
        if let Ok(status) = output
            && status.success()
        {
            return Some(match *name {
                "mariadb" => "MariaDB (active)",
                "mysql" => "MySQL (active)",
                "mysqld" => "mysqld (active)",
                _ => "Database service (active)",
            });
        }
    }
    None
}

fn port_3306_open() -> bool {
    std::net::TcpStream::connect_timeout(
        &"127.0.0.1:3306"
            .parse()
            .unwrap_or_else(|_| std::net::SocketAddr::from(([127, 0, 0, 1], 3306))),
        std::time::Duration::from_millis(250),
    )
    .is_ok()
}

pub fn databases_main() -> String {
    let service = service_active(&["mariadb", "mysql", "mysqld"]);
    let listening = port_3306_open();
    let (title, detail) = match (service, listening) {
        (Some(label), true) => (
            label.to_string(),
            "A database daemon is active and accepting connections on 127.0.0.1:3306.".to_string(),
        ),
        (Some(label), false) => (
            label.to_string(),
            "The service unit is active, but TCP :3306 did not accept a quick probe.".to_string(),
        ),
        (None, true) => (
            "Listener on :3306".into(),
            "Something accepts connections on 127.0.0.1:3306, but no MariaDB/MySQL systemd unit was active."
                .into(),
        ),
        (None, false) => (
            "Not detected".into(),
            "CPN does not provision database instances yet. Install MariaDB/MySQL on the host, then re-open this page."
                .into(),
        ),
    };
    format!(
        r#"{heading}
      <article class="section-card">
        <h2>Database status</h2>
        <ul class="kv-list">
          <li><span>Detected service</span><strong>{title}</strong></li>
          <li><span>TCP 127.0.0.1:3306</span><strong>{port}</strong></li>
        </ul>
        <p>{detail}</p>
        <p class="muted">Next steps: install MariaDB, create databases with your preferred tooling, then wire panel DB management in a later release.</p>
      </article>"#,
        heading = section_heading(
            "Databases",
            "Honest detection of local MariaDB/MySQL. No credentials are stored here.",
        ),
        title = html_escape(&title),
        port = if listening { "Open" } else { "Closed" },
        detail = html_escape(&detail),
    )
}

fn list_backup_files() -> Vec<(String, u64)> {
    let dir = backups_dir();
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            continue;
        }
        let size = fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
        files.push((name, size));
    }
    files.sort_by(|a, b| b.0.cmp(&a.0));
    files
}

fn backup_rows(files: &[(String, u64)]) -> String {
    if files.is_empty() {
        return r#"<p class="empty-state">No backups yet. Run a panel-data backup below.</p>"#
            .into();
    }
    let mut rows = String::from(
        r#"<div class="table-wrap"><table class="data-table">
      <thead><tr><th>File</th><th>Size</th></tr></thead><tbody>"#,
    );
    for (name, size) in files {
        rows.push_str(&format!(
            r#"<tr><td><code>{name}</code></td><td>{size} bytes</td></tr>"#,
            name = html_escape(name),
            size = size,
        ));
    }
    rows.push_str("</tbody></table></div>");
    rows
}

pub fn backups_main(notice: Option<&str>, error: Option<&str>) -> String {
    let files = list_backup_files();
    format!(
        r#"{heading}
      {ok}
      {err}
      <article class="section-card">
        <h2>Backup archive</h2>
        <p>Stores copies under <code>/var/lib/cpn/backups/</code>. Includes panel bootstrap, accounts, sites, and preferences. SMTP secrets are included when present; treat archives as sensitive.</p>
        {rows}
        <form method="post" action="/backups/run" class="stack-form" style="margin-top:18px;">
          <button type="submit" class="btn-primary">Run panel-data backup</button>
        </form>
      </article>"#,
        heading = section_heading("Backups", "Panel data archives for this host.",),
        ok = notice_block("ok", notice),
        err = notice_block("error", error),
        rows = backup_rows(&files),
    )
}

pub fn create_panel_backup() -> Result<String, String> {
    let dir = backups_dir();
    fs::create_dir_all(&dir).map_err(|error| format!("Could not create backups dir: {error}"))?;
    let stamp = crate::account::now_unix();
    let name = format!("panel-{stamp}.tar.gz");
    let dest = dir.join(&name);
    let data = data_dir();
    let mut paths = Vec::new();
    for rel in [
        "panel-bootstrap.json",
        "accounts",
        "sites",
        "smtp.json",
        "panel-session.secret",
        "panel-hostname.json",
        "listen-port.json",
        "install-manifest.json",
    ] {
        if data.join(rel).exists() {
            paths.push(rel.to_string());
        }
    }
    if paths.is_empty() {
        return Err("No panel data files found to archive".into());
    }
    let status = Command::new("tar")
        .arg("-czf")
        .arg(&dest)
        .arg("-C")
        .arg(&data)
        .args(&paths)
        .status()
        .map_err(|error| format!("Could not start tar: {error}"))?;
    if !status.success() {
        return Err("tar backup failed".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dest, fs::Permissions::from_mode(0o600));
    }
    Ok(name)
}
