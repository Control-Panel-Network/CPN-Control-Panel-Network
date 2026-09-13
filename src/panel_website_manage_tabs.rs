//! Manage dashboard tab bodies (Domains, Logs, Config, Files, Plugins).
//! Overview: `panel_website_manage_overview`. SSL: `panel_website_manage_ssl`.

pub use crate::panel_website_manage_overview::tab_overview;
pub use crate::panel_website_manage_ssl::tab_ssl;

use crate::panel_ops_ftp::detect_ftp;
use crate::panel_ops_php::detect_php;
use crate::panel_website_logs::log_panel_html;
use crate::panel_website_manage_ui::{html_escape, section, tile};
use crate::service_detect::detect_web_server_label;
use crate::sites::{SiteRecord, list_sites, resolve_parent_domain};
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
        &format!("/websites/manage?domain={domain_q}&tab=alias"),
        "Domain Alias",
        "Add, list, and remove ServerAlias hostnames",
    ));
    tiles.push_str(&tile(
        &format!("/websites/manage?domain={domain_q}&tab=cron"),
        "Cron Jobs",
        "Per-site schedules jailed to the site home",
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
    if !site.aliases.is_empty() {
        list.push_str(
            r#"<p class="manage-muted" style="margin-top:12px;"><strong>Aliases:</strong> "#,
        );
        list.push_str(
            &site
                .aliases
                .iter()
                .map(|a| format!("<code>{}</code>", html_escape(a)))
                .collect::<Vec<_>>()
                .join(", "),
        );
        list.push_str(&format!(
            r#" · <a href="/websites/manage?domain={domain_q}&amp;tab=alias">Manage</a></p>"#
        ));
    }

    format!(
        "{tiles}{listed}",
        tiles = section("Domains", &tiles),
        listed = section("Registered under this site", &list),
    )
}

pub fn tab_logs(site: &SiteRecord) -> String {
    let _ = crate::panel_site_vhost_wire::ensure_site_vhost_logging(site);
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
        "{tiles}{php}{vhost}{rewrite}{ssh}{suspend_msg}",
        tiles = section("Configurations", &tiles),
        php = php_line,
        vhost = vhost_block,
        rewrite = rewrite_block,
        ssh = ssh,
        suspend_msg = crate::panel_hub_pages_site_messages::site_suspend_message_form(
            &site.domain,
            &site.owner_suspend_message,
        ),
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
    tiles.push_str(&tile(
        "/ftp/create",
        "Create SFTP Acct",
        "Jailed OpenSSH SFTP",
    ));
    tiles.push_str(&tile(
        "/ftp/delete",
        "Delete SFTP Acct",
        "Remove jailed user",
    ));
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
        &format!("/plugins?domain={domain_q}"),
        "Plugins",
        "Site plugins, Plugin Store, and host packages for this domain",
    ));
    tiles.push_str(&tile(
        &format!("/backups?scope=site&domain={domain_q}"),
        "Backups",
        "Selective backups for this site",
    ));
    tiles.push_str("</div>");
    section("Plugins", &tiles)
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
            internal_ip: None,
            owner_suspend_message: String::new(),
            suspended_by: None,
            php_version: None,
            aliases: Vec::new(),
        }
    }

    #[test]
    fn domains_has_tiles() {
        let html = tab_domains(&site());
        assert!(html.contains("Add Domains"));
        assert!(html.contains("Cron Jobs"));
        assert!(html.contains("tab=alias"));
        assert!(html.contains("tab=cron"));
        assert!(!html.to_lowercase().contains("scaffold"));
    }

    #[test]
    fn apps_tab_is_single_plugins_entry() {
        let html = tab_apps(&site());
        assert!(html.contains("<strong>Plugins</strong>"));
        assert!(html.contains("manage-section-title\">Plugins</h2>"));
        assert!(html.contains("/plugins?domain="));
        assert!(html.contains("/backups?scope=site"));
        assert!(!html.contains("Site Apps"));
        assert!(!html.contains("/apps?"));
        assert!(!html.to_lowercase().contains("cyberpanel"));
    }
}
