//! Manage dashboard tab bodies (Overview, Domains, Logs, Config, SSL, Files, Apps).

use crate::panel_ops_db::list_databases;
use crate::panel_ops_ftp::detect_ftp;
use crate::panel_ops_php::detect_php;
use crate::panel_ops_ssl_le::ssl_status_for_domain;
use crate::panel_website_logs::log_panel_html;
use crate::panel_website_manage_ui::{html_escape, resource_card, section, ssl_status_card, tile};
use crate::panel_website_resources::{
    approx_dir_bytes, format_bytes, host_resource_snapshot, sparkline_svg,
};
use crate::service_detect::detect_web_server_label;
use crate::sites::{
    SiteRecord, is_legacy_docroot, list_sites, resolve_parent_domain, site_home_from_record,
};
use crate::website_preview::ssl_material_present;
use std::path::{Path, PathBuf};

fn child_sites(parent: &str) -> Vec<SiteRecord> {
    let Ok(all) = list_sites() else {
        return Vec::new();
    };
    all.into_iter()
        .filter(|site| {
            resolve_parent_domain(&site.domain)
                .ok()
                .flatten()
                .as_deref()
                == Some(parent)
        })
        .collect()
}

pub fn tab_overview(site: &SiteRecord) -> String {
    let disk_bytes = approx_dir_bytes(Path::new(&site.docroot), 8_000);
    let disk = disk_bytes
        .map(format_bytes)
        .unwrap_or_else(|| "Unavailable".into());
    // Soft quota hint: 10 GB visual only until Packages merge.
    let disk_pct = disk_bytes
        .map(|b| {
            let pct = ((b as f64 / (10.0 * 1024.0 * 1024.0 * 1024.0)) * 100.0) as u8;
            pct.min(100)
        })
        .unwrap_or(0);

    let db = list_databases();
    let db_count = if db.databases.is_empty() && !db.detail.contains("Listed via") {
        "n/a".into()
    } else {
        db.databases.len().to_string()
    };
    let ftp = detect_ftp();
    let ftp_label: String = if ftp.ready {
        "See FTP hub".to_string()
    } else {
        "0".to_string()
    };
    let snap = host_resource_snapshot();
    let cpu_label = snap
        .cpu_pct
        .map(|v| format!("{v:.0}%"))
        .unwrap_or_else(|| "n/a".into());
    let mem_label = snap
        .mem_pct
        .map(|v| format!("{v:.0}%"))
        .unwrap_or_else(|| "n/a".into());

    let mut cards = String::from(r#"<div class="manage-card-grid">"#);
    cards.push_str(&resource_card("Disk Usage", &disk, Some(disk_pct)));
    cards.push_str(&resource_card("Bandwidth", "Not metered", None));
    cards.push_str(&resource_card("Databases", &db_count, None));
    cards.push_str(&resource_card("FTP Accounts", &ftp_label, None));
    cards.push_str("</div>");

    let charts = format!(
        r#"<div class="manage-charts">
  <div class="manage-chart">
    <h3>CPU Usage</h3>
    <p>{detail} Current load hint: {cpu}</p>
    {cpu_svg}
  </div>
  <div class="manage-chart">
    <h3>Memory Usage</h3>
    <p>Host memory in use: {mem}</p>
    {mem_svg}
  </div>
</div>"#,
        detail = html_escape(&snap.detail),
        cpu = html_escape(&cpu_label),
        mem = html_escape(&mem_label),
        cpu_svg = sparkline_svg(snap.cpu_pct, "#3b82f6"),
        mem_svg = sparkline_svg(snap.mem_pct, "#12b76a"),
    );

    let home = site_home_from_record(site);
    let engine = site
        .engine
        .as_deref()
        .filter(|v| !v.is_empty())
        .unwrap_or("Not set");
    let stack = detect_web_server_label();
    let legacy = if is_legacy_docroot(&site.docroot) {
        r#"<p class="manage-muted">This site uses a legacy or custom document root. New sites use the domain home under <code>/home/</code>.</p>"#
    } else {
        ""
    };
    let meta = format!(
        r#"<p class="manage-muted">Owner: <strong>{owner}</strong> · Docroot: <code>{docroot}</code> · Home: <code>{home}</code> · Engine: {engine} · Stack: {stack}</p>{legacy}"#,
        owner = html_escape(&site.owner),
        docroot = html_escape(&site.docroot),
        home = html_escape(&home.display().to_string()),
        engine = html_escape(engine),
        stack = html_escape(&stack),
        legacy = legacy,
    );

    format!(
        "{cards}{ssl}{charts}{meta}",
        cards = cards,
        ssl = ssl_status_card(site),
        charts = charts,
        meta = meta,
    )
}

pub fn tab_domains(site: &SiteRecord) -> String {
    let domain_q = html_escape(&site.domain);
    let mut tiles = String::from(r#"<div class="manage-tile-grid">"#);
    tiles.push_str(&tile(
        "/websites",
        "Add Domains",
        "Create a site or subdomain from Websites",
    ));
    tiles.push_str(&tile(
        "/websites",
        "List Domains",
        "Open the Websites registry",
    ));
    tiles.push_str(&tile(
        &format!("/websites/manage?domain={domain_q}&tab=domains"),
        "Domain Alias",
        "Alias wiring ships with DNS hub (scaffold)",
    ));
    tiles.push_str(&tile(
        &format!("/websites/manage?domain={domain_q}&tab=domains#cron"),
        "Cron Jobs",
        "Per-site cron editor ships later (scaffold)",
    ));
    tiles.push_str("</div>");

    let parent = resolve_parent_domain(&site.domain)
        .ok()
        .flatten()
        .unwrap_or_default();
    let mut list = String::new();
    if parent.is_empty() {
        let children = child_sites(&site.domain);
        if children.is_empty() {
            list.push_str(
                r#"<p class="manage-muted">No subdomains registered under this domain yet.</p>"#,
            );
        } else {
            list.push_str(r#"<ul>"#);
            for child in &children {
                let st = if child.enabled { "Active" } else { "Suspended" };
                list.push_str(&format!(
                    r#"<li><a href="/websites/manage?domain={d}">{d}</a> · {st} · SSL {ssl}</li>"#,
                    d = html_escape(&child.domain),
                    st = st,
                    ssl = html_escape(child.ssl.provider.label()),
                ));
            }
            list.push_str("</ul>");
        }
    } else {
        list.push_str(&format!(
            r#"<p class="manage-muted">Parent domain: <a href="/websites/manage?domain={p}">{p}</a></p>"#,
            p = html_escape(&parent),
        ));
    }
    list.push_str(
        r#"<p id="cron" class="manage-muted" style="margin-top:16px;">Cron Jobs: schedule UI is not wired yet. Use system crontab on the host for now.</p>"#,
    );

    format!(
        "{tiles}{listed}",
        tiles = section("Domains", &tiles),
        listed = section("Registered under this site", &list),
    )
}

pub fn tab_logs(site: &SiteRecord) -> String {
    let mut tiles = String::from(r#"<div class="manage-tile-grid">"#);
    tiles.push_str(&tile(
        "#access",
        "Access Logs",
        "Tail allowlisted access logs",
    ));
    tiles.push_str(&tile("#error", "Error Logs", "Tail allowlisted error logs"));
    tiles.push_str("</div>");
    format!(
        "{tiles}<div id=\"access\">{access}</div><div id=\"error\">{error}</div>",
        tiles = section("Logs", &tiles),
        access = log_panel_html(site, "access"),
        error = log_panel_html(site, "error"),
    )
}

fn vhost_candidates(domain: &str) -> Vec<PathBuf> {
    vec![
        PathBuf::from(format!("/usr/local/lsws/conf/vhosts/{domain}/vhost.conf")),
        PathBuf::from(format!("/etc/httpd/conf.d/{domain}.conf")),
        PathBuf::from(format!("/etc/apache2/sites-available/{domain}.conf")),
        PathBuf::from(format!("/etc/nginx/conf.d/{domain}.conf")),
        PathBuf::from(format!("/etc/nginx/sites-available/{domain}")),
        PathBuf::from(format!("/var/lib/cpn/vhosts/{domain}.conf")),
    ]
}

fn first_readable(paths: &[PathBuf]) -> Option<(PathBuf, String)> {
    for path in paths {
        if path.is_file()
            && let Ok(raw) = std::fs::read(path)
        {
            let take = raw.len().min(24_000);
            let text = String::from_utf8_lossy(&raw[..take]).into_owned();
            return Some((path.clone(), text));
        }
    }
    None
}

pub fn tab_config(site: &SiteRecord) -> String {
    let stack = detect_web_server_label();
    let domain_q = html_escape(&site.domain);
    let mut tiles = String::from(r#"<div class="manage-tile-grid">"#);
    tiles.push_str(&tile(
        "/server/php/configs",
        "Web Server Manager",
        &format!("Detected stack: {stack}"),
    ));
    tiles.push_str(&tile(
        &format!("/websites/manage?domain={domain_q}&tab=config#vhost"),
        "vHost Conf",
        "Read-only preview with backup note",
    ));
    tiles.push_str(&tile(
        &format!("/websites/manage?domain={domain_q}&tab=config#rewrite"),
        "Rewrite Rules",
        "Show .htaccess when present",
    ));
    tiles.push_str(&tile(
        "/server/php/extensions",
        "Change PHP",
        "Installed PHP detection",
    ));
    tiles.push_str("</div>");

    let php = detect_php();
    let php_line = format!(
        r#"<p class="manage-muted">PHP: {} · {}</p>"#,
        html_escape(php.version.as_deref().unwrap_or("not detected")),
        html_escape(&php.detail),
    );

    let vhost_block = match first_readable(&vhost_candidates(&site.domain)) {
        Some((path, body)) => {
            let escaped = html_escape(&body);
            format!(
                r#"<div id="vhost"><h3>vHost Conf</h3>
<p class="manage-muted">Read-only from <code>{}</code>. Edits require admin permission and a backup (write UI ships next).</p>
<pre class="manage-log-pre">{escaped}</pre></div>"#,
                html_escape(&path.display().to_string()),
            )
        }
        None => r#"<div id="vhost"><h3>vHost Conf</h3>
<p class="manage-muted">No vhost file found yet for this domain. Files appear after panel recipes wire the web stack.</p></div>"#.into(),
    };

    let htaccess = Path::new(&site.docroot).join(".htaccess");
    let rewrite_block = if htaccess.is_file() {
        match std::fs::read_to_string(&htaccess) {
            Ok(body) => format!(
                r#"<div id="rewrite"><h3>Rewrite Rules</h3>
<p class="manage-muted">From <code>{}</code></p>
<pre class="manage-log-pre">{}</pre></div>"#,
                html_escape(&htaccess.display().to_string()),
                html_escape(&body.chars().take(12_000).collect::<String>()),
            ),
            Err(err) => format!(
                r#"<div id="rewrite"><h3>Rewrite Rules</h3><p class="manage-muted">{}</p></div>"#,
                html_escape(&err.to_string())
            ),
        }
    } else {
        r#"<div id="rewrite"><h3>Rewrite Rules</h3>
<p class="manage-muted">No <code>.htaccess</code> in the document root yet.</p></div>"#
            .into()
    };

    let ssh = format!(
        r#"<div><h3>SSH / SFTP</h3>
<p class="manage-muted">Connect with the hosting account that owns this site. Document root: <code>{doc}</code>. Credentials are never shown in the panel.</p></div>"#,
        doc = html_escape(&site.docroot),
    );

    format!(
        "{tiles}{php}{vhost}{rewrite}{ssh}",
        tiles = section("Configurations", &tiles),
        php = php_line,
        vhost = vhost_block,
        rewrite = rewrite_block,
        ssh = ssh,
    )
}

pub fn tab_ssl(site: &SiteRecord) -> String {
    let domain_q = html_escape(&site.domain);
    let row = ssl_status_for_domain(&site.domain);
    let has = ssl_material_present(&site.domain);
    let mut tiles = String::from(r#"<div class="manage-tile-grid">"#);
    tiles.push_str(&tile(
        &format!("/websites/manage?domain={domain_q}&tab=ssl#provider"),
        "SSL provider",
        &row.provider_label,
    ));
    tiles.push_str(&tile(
        "/security/ssl",
        "Manage SSL (all sites)",
        "Per-domain providers; no account-wide rewrite",
    ));
    tiles.push_str(&tile(
        &format!("/websites/manage?domain={domain_q}&tab=ssl#manual"),
        "Custom upload",
        "PEM cert + key (no auto-renew)",
    ));
    tiles.push_str("</div>");

    let mut opts = String::new();
    for p in crate::panel_ops_ssl_provider::SslProvider::all() {
        let sel = if p.as_str() == row.provider {
            " selected"
        } else {
            ""
        };
        opts.push_str(&format!(
            r#"<option value="{v}"{sel}>{l}</option>"#,
            v = p.as_str(),
            l = html_escape(p.label()),
            sel = sel,
        ));
    }
    let inc = if site.ssl.include_subdomains_on_cert {
        " checked"
    } else {
        ""
    };
    let provider_form = format!(
        r#"<div id="provider"><h3>Provider for {domain} only</h3>
<p class="manage-muted">Changing this domain does not rewrite siblings. Subdomains inherit parent provider only at creation time.</p>
<form method="post" action="/security/ssl/provider" class="stack-form" style="max-width:480px;">
  <input type="hidden" name="domain" value="{domain}">
  <input type="hidden" name="return" value="/websites/manage?domain={domain_q}&amp;tab=ssl">
  <label for="provider">SSL provider</label>
  <select id="provider" name="provider">{opts}</select>
  <label><input type="checkbox" name="include_subdomains" value="1"{inc}> Include matching subdomains on one SAN cert</label>
  <button type="submit" class="manage-btn primary">Save provider</button>
</form>
<p class="manage-muted">Status: {cert}. Shared owner: {shared}.</p>{err}</div>"#,
        domain = html_escape(&site.domain),
        domain_q = domain_q,
        opts = opts,
        inc = inc,
        cert = if has {
            "certificate material on disk"
        } else {
            "no certificate files found"
        },
        shared = html_escape(row.shared_cert_owner.as_deref().unwrap_or("-")),
        err = if row.last_error.is_empty() {
            String::new()
        } else {
            format!(
                r#"<p class="panel-notice error" role="status">{}</p>"#,
                html_escape(&row.last_error)
            )
        },
    );

    let issue = if row.auto_issue {
        format!(
            r#"<div id="issue"><h3>Issue / Renew</h3>
<form method="post" action="/security/ssl/issue">
  <input type="hidden" name="domain" value="{domain}">
  <input type="hidden" name="return" value="/websites/manage?domain={domain_q}&amp;tab=ssl">
  <button type="submit" class="manage-btn primary">Issue / Renew ({label})</button>
</form>
<p class="manage-muted">certbot available: {cb}. Honest errors on rate limits or missing public DNS.</p></div>"#,
            domain = html_escape(&site.domain),
            domain_q = domain_q,
            label = html_escape(&row.provider_label),
            cb = if row.certbot { "yes" } else { "no" },
        )
    } else {
        format!(
            r#"<div id="issue"><h3>Issue / Renew</h3>
<p class="manage-muted">Provider <strong>{}</strong> does not auto-issue. Choose Let's Encrypt, ZeroSSL, or Cloudflare CA, or upload Custom SSL.</p></div>"#,
            html_escape(&row.provider_label)
        )
    };

    let manual = format!(
        r#"<div id="manual"><h3>Custom SSL upload</h3>
<p class="manage-muted">Uploading sets this domain to Custom and leaves any shared SAN. Keys stored under <code>/var/lib/cpn/ssl/{domain}/</code> mode 600.</p>
<form method="post" action="/security/ssl/upload" class="stack-form" style="max-width:640px;">
  <input type="hidden" name="domain" value="{domain}">
  <input type="hidden" name="return" value="/websites/manage?domain={domain_q}&amp;tab=ssl">
  <label for="cert_pem">Certificate PEM (fullchain)</label>
  <textarea id="cert_pem" name="cert_pem" rows="6" style="width:100%;font:inherit;" required></textarea>
  <label for="key_pem">Private key PEM</label>
  <textarea id="key_pem" name="key_pem" rows="6" style="width:100%;font:inherit;" required></textarea>
  <button type="submit" class="manage-btn primary">Upload custom SSL</button>
</form></div>"#,
        domain = html_escape(&site.domain),
        domain_q = domain_q,
    );

    format!(
        "{tiles}{provider}{issue}{manual}",
        tiles = section("SSL", &tiles),
        provider = provider_form,
        issue = issue,
        manual = manual,
    )
}

pub fn tab_files(site: &SiteRecord) -> String {
    let domain_q = html_escape(&site.domain);
    let ftp = detect_ftp();
    let mut tiles = String::from(r#"<div class="manage-tile-grid">"#);
    tiles.push_str(&tile(
        "/server/files",
        "File Manager",
        "Browse allowlisted paths on this host",
    ));
    tiles.push_str(&tile(
        &format!("/websites/manage?domain={domain_q}&tab=files#basedir"),
        "open_basedir",
        "PHP open_basedir hint for this docroot",
    ));
    tiles.push_str(&tile("/ftp/create", "Create FTP Acct", "FTP hub scaffold"));
    tiles.push_str(&tile("/ftp/delete", "Delete FTP Acct", "FTP hub scaffold"));
    tiles.push_str("</div>");

    let basedir = format!(
        r#"<div id="basedir"><h3>open_basedir</h3>
<p class="manage-muted">Suggested scope for this site: <code>{docroot}</code> (and optional tmp). Apply via PHP configs when ready.</p>
<p class="manage-muted">FTP stack: {ftp}</p></div>"#,
        docroot = html_escape(&site.docroot),
        ftp = html_escape(&format!("{}: {}", ftp.stack, ftp.detail)),
    );

    format!(
        "{tiles}{basedir}",
        tiles = section("Files", &tiles),
        basedir = basedir,
    )
}

pub fn tab_apps(site: &SiteRecord) -> String {
    let domain_q = html_escape(&site.domain);
    let mut tiles = String::from(r#"<div class="manage-tile-grid">"#);
    tiles.push_str(&tile(
        &format!("/apps?domain={domain_q}"),
        "Site Apps",
        "Installers scoped to this domain",
    ));
    tiles.push_str(&tile(
        &format!("/plugins?domain={domain_q}"),
        "Plugins",
        "Site-scoped CPN plugins",
    ));
    tiles.push_str(&tile(
        &format!("/backups?scope=site&domain={domain_q}"),
        "Backups",
        "Selective backups for this site",
    ));
    tiles.push_str("</div>");
    section("Apps", &tiles)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site() -> SiteRecord {
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
        }
    }

    #[test]
    fn overview_has_resource_cards() {
        let html = tab_overview(&site());
        assert!(html.contains("Disk Usage"));
        assert!(html.contains("Bandwidth"));
        assert!(!html.to_lowercase().contains("email marketing"));
        assert!(!html.to_lowercase().contains("cyberpanel"));
    }

    #[test]
    fn domains_has_tiles() {
        let html = tab_domains(&site());
        assert!(html.contains("Add Domains"));
        assert!(html.contains("Cron Jobs"));
    }
}
