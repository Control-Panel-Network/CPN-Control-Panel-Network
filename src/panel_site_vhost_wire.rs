//! Best-effort OLS/LSE and nginx vhost wiring so site access/error logs are written.

use crate::litespeed_stack::{any_litespeed_installed, restart_litespeed};
use crate::panel_website_logs::{ensure_site_log_files, site_access_log_path, site_error_log_path};
use crate::sites::{SiteModify, SiteRecord, modify_site, site_home_from_record};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const HTTPD_CONF: &str = "/usr/local/lsws/conf/httpd_config.conf";

fn safe_vhost_name(domain: &str) -> String {
    domain
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn ols_vhconf(docroot: &str, access_log: &str, error_log: &str) -> String {
    format!(
        "docRoot                   {docroot}/\n\
         enableGzip                1\n\
         index  {{\n\
           useServer               0\n\
           indexFiles              index.html, index.php\n\
         }}\n\
         errorlog {error_log} {{\n\
           logLevel                ERROR\n\
           rollingSize             10M\n\
           useServer               0\n\
         }}\n\
         accessLog {access_log} {{\n\
           rollingSize             10M\n\
           keepDays                30\n\
           compressArchive         0\n\
           logReferer              1\n\
           logUserAgent            1\n\
           useServer               0\n\
         }}\n\
         context / {{\n\
           allowBrowse             1\n\
           location                {docroot}/\n\
           rewrite  {{\n\
             enable                1\n\
             RewriteFile           .htaccess\n\
           }}\n\
         }}\n"
    )
}

fn insert_map_before_catchall(conf: &str, listener: &str, map_line: &str) -> Option<String> {
    if conf.contains(map_line.trim()) {
        return None;
    }
    let marker = format!("listener {listener}");
    let Some(start) = conf.find(&marker) else {
        return None;
    };
    let rest = &conf[start..];
    let Some(end_rel) = rest.find("\n}") else {
        return None;
    };
    let block_end = start + end_rel;
    let before = &conf[..block_end];
    let after = &conf[block_end..];
    // Prefer inserting before a catch-all map … *
    if let Some(catch) = before.rfind("map ") {
        let catch_slice = &before[catch..];
        if catch_slice.contains('*') {
            let mut out = String::with_capacity(conf.len() + map_line.len() + 2);
            out.push_str(&before[..catch]);
            out.push_str(map_line);
            if !map_line.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&before[catch..]);
            out.push_str(after);
            return Some(out);
        }
    }
    let mut out = String::with_capacity(conf.len() + map_line.len() + 2);
    out.push_str(before);
    out.push('\n');
    out.push_str(map_line.trim_end());
    out.push('\n');
    out.push_str(after);
    Some(out)
}

fn ensure_ols_site_vhost(site: &SiteRecord) -> Result<bool, String> {
    if !any_litespeed_installed() || !Path::new(HTTPD_CONF).is_file() {
        return Ok(false);
    }
    let home = site_home_from_record(site);
    let access = site_access_log_path(site);
    let error = site_error_log_path(site);
    let vh_name = safe_vhost_name(&site.domain);
    let vh_dir = PathBuf::from(format!("/usr/local/lsws/conf/vhosts/{vh_name}"));
    fs::create_dir_all(&vh_dir).map_err(|e| format!("Could not create OLS vhost dir: {e}"))?;
    let vhconf = vh_dir.join("vhconf.conf");
    let body = ols_vhconf(
        &site.docroot,
        &access.display().to_string(),
        &error.display().to_string(),
    );
    let mut changed = false;
    let prev = fs::read_to_string(&vhconf).unwrap_or_default();
    if prev != body {
        fs::write(&vhconf, body).map_err(|e| format!("Could not write vhconf: {e}"))?;
        changed = true;
    }

    let mut conf = fs::read_to_string(HTTPD_CONF)
        .map_err(|e| format!("Could not read httpd_config.conf: {e}"))?;
    let vh_block = format!(
        "\nvirtualHost {vh_name} {{\n  vhRoot                  {home}/\n  configFile              $SERVER_ROOT/conf/vhosts/{vh_name}/vhconf.conf\n  allowSymbolLink         1\n  enableScript            1\n  restrained              1\n}}\n",
        home = home.display()
    );
    if !conf.contains(&format!("virtualHost {vh_name}")) {
        conf.push_str(&vh_block);
        changed = true;
    }

    let map_line = format!(
        "  map                      {vh_name} {domain}\n",
        domain = site.domain
    );
    for listener in ["CPNHttp", "Default"] {
        if let Some(updated) = insert_map_before_catchall(&conf, listener, &map_line) {
            conf = updated;
            changed = true;
            break;
        }
    }

    if changed {
        fs::write(HTTPD_CONF, conf)
            .map_err(|e| format!("Could not update httpd_config.conf: {e}"))?;
        let _ = restart_litespeed();
    }
    Ok(changed)
}

fn ensure_nginx_site_logs(site: &SiteRecord) -> Result<bool, String> {
    let nginx_bin = Path::new("/usr/sbin/nginx");
    if !nginx_bin.is_file() {
        return Ok(false);
    }
    // Only write a snippet when nginx is the site engine or no LiteSpeed is present.
    let engine = site.engine.as_deref().unwrap_or("");
    if any_litespeed_installed() && engine != "nginx" {
        return Ok(false);
    }
    let access = site_access_log_path(site);
    let error = site_error_log_path(site);
    let conf_path = PathBuf::from(format!("/etc/nginx/conf.d/cpn-{}.conf", site.domain));
    let body = format!(
        "# CPN managed site logging for {domain}\n\
         server {{\n\
           listen 80;\n\
           server_name {domain};\n\
           root {docroot};\n\
           access_log {access};\n\
           error_log {error};\n\
           location / {{\n\
             try_files $uri $uri/ /index.html;\n\
           }}\n\
         }}\n",
        domain = site.domain,
        docroot = site.docroot,
        access = access.display(),
        error = error.display(),
    );
    let prev = fs::read_to_string(&conf_path).unwrap_or_default();
    if prev == body {
        return Ok(false);
    }
    if let Some(parent) = conf_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("nginx conf.d: {e}"))?;
    }
    fs::write(&conf_path, body).map_err(|e| format!("Could not write nginx site conf: {e}"))?;
    let _ = Command::new("nginx").arg("-t").status();
    let _ = Command::new("systemctl").args(["reload", "nginx"]).status();
    Ok(true)
}

/// Ensure log files exist and the active web stack writes access/error logs for this site.
pub fn ensure_site_vhost_logging(site: &SiteRecord) -> Result<String, String> {
    ensure_site_log_files(site)?;
    let mut notes = Vec::new();
    match ensure_ols_site_vhost(site) {
        Ok(true) => notes.push("OpenLiteSpeed/LiteSpeed vhost log paths updated".to_string()),
        Ok(false) => {}
        Err(err) => notes.push(format!("LiteSpeed wire skipped: {err}")),
    }
    match ensure_nginx_site_logs(site) {
        Ok(true) => notes.push("nginx site log paths updated".to_string()),
        Ok(false) => {}
        Err(err) => notes.push(format!("nginx wire skipped: {err}")),
    }
    if !site.vhost_wired
        && (Path::new(&format!(
            "/usr/local/lsws/conf/vhosts/{}/vhconf.conf",
            safe_vhost_name(&site.domain)
        ))
        .is_file()
            || Path::new(&format!("/etc/nginx/conf.d/cpn-{}.conf", site.domain)).is_file())
    {
        let _ = modify_site(
            &site.domain,
            SiteModify {
                vhost_wired: Some(true),
                ..Default::default()
            },
        );
    }
    if notes.is_empty() {
        Ok("Site log files ready".into())
    } else {
        Ok(notes.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_name_keeps_fqdn() {
        assert_eq!(
            safe_vhost_name("test2.newstargeted.com"),
            "test2.newstargeted.com"
        );
        assert_eq!(safe_vhost_name("bad name!"), "bad_name_");
    }

    #[test]
    fn insert_map_before_star() {
        let conf = r#"listener CPNHttp {
  address                 *:80
  secure                  0
  map                     CPN *
}
"#;
        let updated = insert_map_before_catchall(
            conf,
            "CPNHttp",
            "  map                      test2.newstargeted.com test2.newstargeted.com\n",
        )
        .expect("insert");
        assert!(updated.contains("test2.newstargeted.com"));
        let star = updated.find("map                     CPN *").unwrap();
        let domain = updated.find("test2.newstargeted.com").unwrap();
        assert!(domain < star);
    }
}
