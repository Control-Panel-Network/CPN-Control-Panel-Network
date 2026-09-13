//! Domain alias (ServerAlias / OLS map) management for websites.

use crate::account::now_unix;
use crate::litespeed_stack::{any_litespeed_installed, restart_litespeed};
use crate::panel_host_info::host_sidebar_info;
use crate::panel_ops_cloudflare::cloudflare_configured;
use crate::panel_ops_cloudflare_api::create_dns_record;
use crate::panel_session::session_secret;
use crate::sites::{
    SiteModify, SiteRecord, load_site, modify_site, normalize_domain, site_home_from_record,
};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

type HmacSha256 = Hmac<Sha256>;

const HTTPD_CONF: &str = "/usr/local/lsws/conf/httpd_config.conf";

fn hmac_hex(secret: &str, payload: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(payload.as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn verify_hmac_hex(expected: &str, provided: &str) -> bool {
    let a = expected.as_bytes();
    let b = provided.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (left, right) in a.iter().zip(b.iter()) {
        diff |= left ^ right;
    }
    diff == 0
}

/// CSRF token for site alias POST actions.
pub fn alias_csrf_token(username: &str) -> String {
    let secret = session_secret(None);
    let hour = now_unix() / 3600;
    let payload = format!("site-alias|{username}|{hour}");
    format!("{hour}.{}", hmac_hex(&secret, &payload))
}

pub fn verify_alias_csrf(username: &str, token: &str) -> bool {
    let secret = session_secret(None);
    let Some((hour_s, sig)) = token.split_once('.') else {
        return false;
    };
    let Ok(hour) = hour_s.parse::<u64>() else {
        return false;
    };
    let current = now_unix() / 3600;
    if hour + 2 < current || hour > current + 1 {
        return false;
    }
    let payload = format!("site-alias|{username}|{hour}");
    let expected = hmac_hex(&secret, &payload);
    verify_hmac_hex(&expected, sig)
}

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

fn cpn_alias_sidecar(domain: &str) -> PathBuf {
    crate::paths::join_data("vhosts").join(format!("{domain}.aliases"))
}

fn write_sidecar(site: &SiteRecord) -> Result<(), String> {
    let path = cpn_alias_sidecar(&site.domain);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create vhosts dir: {e}"))?;
    }
    let body = format!(
        "# CPN managed aliases for {}\n# Updated unix={}\ndomain={}\naliases={}\ndocroot={}\n",
        site.domain,
        now_unix(),
        site.domain,
        site.aliases.join(","),
        site.docroot
    );
    fs::write(&path, body).map_err(|e| format!("Could not write alias sidecar: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn hostnames_csv(site: &SiteRecord) -> String {
    let mut names = vec![site.domain.clone()];
    for alias in &site.aliases {
        if !names.iter().any(|n| n.eq_ignore_ascii_case(alias)) {
            names.push(alias.clone());
        }
    }
    names.join(", ")
}

fn ensure_ols_map(site: &SiteRecord) -> Result<String, String> {
    if !any_litespeed_installed() || !Path::new(HTTPD_CONF).is_file() {
        return Ok("LiteSpeed not present; registry + sidecar updated".into());
    }
    let vh_name = safe_vhost_name(&site.domain);
    let home = site_home_from_record(site);
    let vh_dir = PathBuf::from(format!("/usr/local/lsws/conf/vhosts/{vh_name}"));
    let _ = fs::create_dir_all(&vh_dir);
    let vhconf = vh_dir.join("vhconf.conf");
    if !vhconf.is_file() {
        let body = format!(
            "docRoot                   {}/\n\
             enableGzip                1\n\
             index  {{\n\
               useServer               0\n\
               indexFiles              index.html, index.php\n\
             }}\n\
             context / {{\n\
               allowBrowse             1\n\
               location                {}/\n\
               rewrite  {{\n\
                 enable                1\n\
                 RewriteFile           .htaccess\n\
               }}\n\
             }}\n",
            site.docroot, site.docroot
        );
        let _ = fs::write(&vhconf, body);
    }

    let mut conf = fs::read_to_string(HTTPD_CONF)
        .map_err(|e| format!("Could not read httpd_config.conf: {e}"))?;
    let mut updated = false;
    let vh_marker = format!("virtualHost {vh_name}");
    if !conf.contains(&vh_marker) {
        conf.push_str(&format!(
            "\nvirtualHost {vh_name} {{\n  vhRoot                  {home}/\n  configFile              $SERVER_ROOT/conf/vhosts/{vh_name}/vhconf.conf\n  allowSymbolLink         1\n  enableScript            1\n  restrained              1\n}}\n",
            home = home.display()
        ));
        updated = true;
    }

    let map_domains = hostnames_csv(site);
    let map_line = format!("  map                      {vh_name} {map_domains}\n");
    let mut map_touched = false;
    for listener in ["CPNHttp", "Default"] {
        let marker = format!("listener {listener}");
        let Some(start) = conf.find(&marker) else {
            continue;
        };
        let rest = &conf[start..];
        let Some(end_rel) = rest.find("\n}") else {
            continue;
        };
        let block_end = start + end_rel;
        let block = &conf[start..block_end];
        let map_prefix = format!("  map                      {vh_name} ");
        if let Some(pos) = block.find(&map_prefix) {
            let abs = start + pos;
            let line_end = conf[abs..]
                .find('\n')
                .map(|i| abs + i)
                .unwrap_or(conf.len());
            let existing = &conf[abs..line_end];
            if existing.trim() != map_line.trim() {
                conf.replace_range(abs..line_end, map_line.trim_end());
                updated = true;
            }
        } else if let Some(catch) = block.rfind("map ") {
            let abs = start + catch;
            conf.insert_str(abs, &map_line);
            updated = true;
        } else {
            conf.insert_str(block_end, &format!("\n{map_line}"));
            updated = true;
        }
        map_touched = true;
        break;
    }

    if updated {
        fs::write(HTTPD_CONF, &conf)
            .map_err(|e| format!("Could not update httpd_config.conf: {e}"))?;
        let _ = restart_litespeed();
        if map_touched {
            return Ok("OpenLiteSpeed ServerAlias map updated".into());
        }
        return Ok("OpenLiteSpeed vhost ready with aliases".into());
    }
    Ok("OpenLiteSpeed alias map already current".into())
}

fn ensure_apache_aliases(site: &SiteRecord) -> Result<(), String> {
    let candidates = [
        PathBuf::from(format!("/etc/httpd/conf.d/{}.conf", site.domain)),
        PathBuf::from(format!("/etc/apache2/sites-available/{}.conf", site.domain)),
    ];
    for path in candidates {
        if !path.is_file() {
            continue;
        }
        let raw = fs::read_to_string(&path).unwrap_or_default();
        let mut lines: Vec<String> = raw
            .lines()
            .filter(|l| !l.trim_start().starts_with("ServerAlias "))
            .map(|s| s.to_string())
            .collect();
        if !site.aliases.is_empty() {
            let insert_at = lines
                .iter()
                .position(|l| l.trim_start().starts_with("ServerName "))
                .map(|i| i + 1)
                .unwrap_or(0);
            lines.insert(
                insert_at,
                format!("    ServerAlias {}", site.aliases.join(" ")),
            );
        }
        let body = lines.join("\n") + "\n";
        fs::write(&path, body).map_err(|e| format!("Could not update {}: {e}", path.display()))?;
        let _ = Command::new("systemctl").args(["reload", "httpd"]).status();
        let _ = Command::new("systemctl").args(["reload", "apache2"]).status();
        return Ok(());
    }
    Ok(())
}

fn ensure_nginx_aliases(site: &SiteRecord) -> Result<(), String> {
    let path = PathBuf::from(format!("/etc/nginx/conf.d/cpn-{}.conf", site.domain));
    if !path.is_file() {
        return Ok(());
    }
    let raw = fs::read_to_string(&path).unwrap_or_default();
    let names = hostnames_csv(site);
    let mut out = String::new();
    for line in raw.lines() {
        if line.trim_start().starts_with("server_name ") {
            out.push_str(&format!("           server_name {names};\n"));
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    fs::write(&path, out).map_err(|e| format!("Could not update nginx conf: {e}"))?;
    let _ = Command::new("nginx").arg("-t").status();
    let _ = Command::new("systemctl").args(["reload", "nginx"]).status();
    Ok(())
}

fn apply_vhost(site: &SiteRecord) -> Result<String, String> {
    write_sidecar(site)?;
    let mut notes = Vec::new();
    match ensure_ols_map(site) {
        Ok(msg) => notes.push(msg),
        Err(err) => notes.push(format!("LiteSpeed: {err}")),
    }
    if let Err(err) = ensure_apache_aliases(site) {
        notes.push(format!("Apache: {err}"));
    }
    if let Err(err) = ensure_nginx_aliases(site) {
        notes.push(format!("nginx: {err}"));
    }
    Ok(notes.join("; "))
}

fn zone_candidates(hostname: &str) -> Vec<String> {
    let parts: Vec<&str> = hostname.split('.').collect();
    let mut out = Vec::new();
    if parts.len() >= 2 {
        out.push(hostname.to_string());
    }
    for i in 1..parts.len().saturating_sub(1) {
        out.push(parts[i..].join("."));
    }
    out
}

fn optional_cloudflare_dns(alias: &str, primary: &str, mode: &str) -> Result<String, String> {
    if !cloudflare_configured() {
        return Ok("Cloudflare not configured; skipped DNS".into());
    }
    let mode = mode.trim().to_ascii_lowercase();
    if mode == "none" || mode.is_empty() {
        return Ok("DNS create skipped".into());
    }
    let mut last_err = "No Cloudflare zone matched".to_string();
    for zone in zone_candidates(alias) {
        let result = if mode == "a" {
            let ip = host_sidebar_info().ip;
            if ip == "Unavailable" || ip.starts_with("127.") {
                return Err("Could not resolve a public A record target IP".into());
            }
            create_dns_record(&zone, "A", alias, &ip, 1, None, false)
        } else {
            create_dns_record(&zone, "CNAME", alias, primary, 1, None, false)
        };
        match result {
            Ok(msg) => return Ok(msg),
            Err(err) => last_err = err,
        }
    }
    Err(last_err)
}

/// Add a hostname alias for a site and apply vhost + optional Cloudflare DNS.
pub fn add_site_alias(
    domain_raw: &str,
    alias_raw: &str,
    dns_mode: &str,
) -> Result<(SiteRecord, String), String> {
    let domain = normalize_domain(domain_raw)?;
    let alias = normalize_domain(alias_raw)?;
    if alias.eq_ignore_ascii_case(&domain) {
        return Err("Alias cannot match the primary domain".into());
    }
    let site = load_site(&domain)?;
    if site.aliases.iter().any(|a| a.eq_ignore_ascii_case(&alias)) {
        return Err(format!("Alias `{alias}` is already listed"));
    }
    // Refuse if another site owns this FQDN as domain or alias.
    if let Ok(other) = load_site(&alias) {
        return Err(format!(
            "`{alias}` is already registered as site `{}`",
            other.domain
        ));
    }
    for other in crate::sites::list_sites().unwrap_or_default() {
        if other
            .aliases
            .iter()
            .any(|a| a.eq_ignore_ascii_case(&alias))
        {
            return Err(format!(
                "`{alias}` is already an alias on `{}`",
                other.domain
            ));
        }
    }
    let mut aliases = site.aliases.clone();
    aliases.push(alias.clone());
    aliases.sort();
    aliases.dedup();
    let updated = modify_site(
        &domain,
        SiteModify {
            aliases: Some(aliases),
            ..Default::default()
        },
    )?;
    let vhost_note = apply_vhost(&updated)?;
    let dns_note = optional_cloudflare_dns(&alias, &domain, dns_mode)
        .unwrap_or_else(|e| format!("Cloudflare DNS: {e}"));
    Ok((
        updated,
        format!("Added alias `{alias}`. {vhost_note}. {dns_note}"),
    ))
}

/// Remove a hostname alias and re-apply vhost maps.
pub fn remove_site_alias(domain_raw: &str, alias_raw: &str) -> Result<(SiteRecord, String), String> {
    let domain = normalize_domain(domain_raw)?;
    let alias = normalize_domain(alias_raw)?;
    let site = load_site(&domain)?;
    let before = site.aliases.len();
    let aliases: Vec<String> = site
        .aliases
        .into_iter()
        .filter(|a| !a.eq_ignore_ascii_case(&alias))
        .collect();
    if aliases.len() == before {
        return Err(format!("Alias `{alias}` was not listed"));
    }
    let updated = modify_site(
        &domain,
        SiteModify {
            aliases: Some(aliases),
            ..Default::default()
        },
    )?;
    let vhost_note = apply_vhost(&updated)?;
    Ok((
        updated,
        format!("Removed alias `{alias}`. {vhost_note}"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csrf_roundtrip() {
        let t = alias_csrf_token("Admin");
        assert!(verify_alias_csrf("Admin", &t));
        assert!(!verify_alias_csrf("other", &t));
    }

    #[test]
    fn hostnames_include_aliases() {
        let site = SiteRecord {
            schema_version: 4,
            domain: "example.com".into(),
            owner: "Admin".into(),
            docroot: "/home/example.com/public_html".into(),
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
            aliases: vec!["www.example.com".into(), "cdn.example.com".into()],
        };
        let csv = hostnames_csv(&site);
        assert!(csv.contains("example.com"));
        assert!(csv.contains("www.example.com"));
        assert!(csv.contains("cdn.example.com"));
    }
}
