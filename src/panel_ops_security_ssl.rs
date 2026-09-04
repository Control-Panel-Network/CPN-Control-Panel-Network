//! ModSecurity, malware scan, and SSL probes for the Security hub.

use crate::panel_network::load_panel_hostname;
use crate::panel_ops_security::{cmd_stdout, systemctl_active, which_exists};
use crate::sites::list_sites;
use crate::website_preview::ssl_material_present;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ModsecStatus {
    pub detected: bool,
    pub engine: String,
    pub detail: String,
    pub rule_paths: Vec<String>,
}

fn path_if_exists(p: &str) -> Option<String> {
    if Path::new(p).exists() {
        Some(p.to_string())
    } else {
        None
    }
}

pub fn modsec_status() -> ModsecStatus {
    let mut hits: Vec<String> = Vec::new();
    for p in [
        "/etc/httpd/conf.d/mod_security.conf",
        "/etc/httpd/conf.modules.d/10-mod_security.conf",
        "/etc/nginx/modsec/main.conf",
        "/etc/nginx/modsecurity.conf",
        "/etc/modsecurity/modsecurity.conf",
        "/usr/lib64/httpd/modules/mod_security2.so",
        "/usr/lib/nginx/modules/ngx_http_modsecurity_module.so",
    ] {
        if let Some(h) = path_if_exists(p) {
            hits.push(h);
        }
    }
    let mut rule_paths = Vec::new();
    for p in [
        "/etc/modsecurity/crs",
        "/usr/share/modsecurity-crs",
        "/etc/httpd/modsecurity.d/owasp-crs",
        "/etc/nginx/owasp-crs",
        "/usr/local/modsecurity-crs",
    ] {
        if Path::new(p).is_dir() {
            rule_paths.push(p.to_string());
        }
    }
    if hits.is_empty() && rule_paths.is_empty() {
        return ModsecStatus {
            detected: false,
            engine: "none".into(),
            detail: "ModSecurity / WAF modules were not detected. Install httpd/nginx ModSecurity packages to enable.".into(),
            rule_paths,
        };
    }
    let engine = if hits.iter().any(|h| h.contains("nginx")) {
        "nginx-modsecurity"
    } else if hits
        .iter()
        .any(|h| h.contains("httpd") || h.contains("mod_security"))
    {
        "httpd-modsecurity"
    } else {
        "modsecurity"
    };
    ModsecStatus {
        detected: true,
        engine: engine.into(),
        detail: format!("Detected paths:\n{}", hits.join("\n")),
        rule_paths,
    }
}

pub fn list_modsec_rule_files(limit: usize) -> Vec<String> {
    let roots = [
        "/etc/modsecurity",
        "/usr/share/modsecurity-crs",
        "/etc/httpd/modsecurity.d",
        "/etc/nginx/modsec",
        "/etc/nginx/owasp-crs",
    ];
    let mut files = Vec::new();
    for root in roots {
        let path = Path::new(root);
        if !path.is_dir() {
            continue;
        }
        let Ok(walk) = fs::read_dir(path) else {
            continue;
        };
        for entry in walk.flatten() {
            let p = entry.path();
            if p.is_file() {
                let name = p.display().to_string();
                if name.ends_with(".conf") || name.ends_with(".rules") {
                    files.push(name);
                    if files.len() >= limit {
                        return files;
                    }
                }
            }
        }
    }
    files
}

#[derive(Debug, Clone)]
pub struct MalwareScanStatus {
    pub engine: String,
    pub installed: bool,
    pub detail: String,
}

pub fn malware_scan_status() -> MalwareScanStatus {
    if which_exists("clamscan") || which_exists("clamdscan") || which_exists("clamd") {
        let active = systemctl_active("clamd");
        let version = cmd_stdout("clamscan", &["--version"])
            .or_else(|| cmd_stdout("clamdscan", &["--version"]))
            .unwrap_or_else(|| "ClamAV detected".into());
        return MalwareScanStatus {
            engine: "clamav".into(),
            installed: true,
            detail: format!("{version}; clamd active={active}"),
        };
    }
    MalwareScanStatus {
        engine: "none".into(),
        installed: false,
        detail: "No ClamAV tools found. CPN Malware scan is scaffolded until ClamAV (or another CPN scanner) is installed.".into(),
    }
}

#[derive(Debug, Clone)]
pub struct SiteSslRow {
    pub domain: String,
    pub has_cert: bool,
}

pub fn site_ssl_rows() -> Vec<SiteSslRow> {
    list_sites()
        .unwrap_or_default()
        .into_iter()
        .map(|s| SiteSslRow {
            has_cert: ssl_material_present(&s.domain),
            domain: s.domain,
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct HostnameSslStatus {
    pub hostname: Option<String>,
    pub has_cert: bool,
    pub certbot: bool,
    pub detail: String,
}

pub fn hostname_ssl_status() -> HostnameSslStatus {
    let hostname = load_panel_hostname();
    let certbot = which_exists("certbot");
    let has_cert = hostname
        .as_ref()
        .map(|h| ssl_material_present(h))
        .unwrap_or(false);
    let detail = match &hostname {
        Some(h) if has_cert => format!("Certificate material found for panel hostname `{h}`."),
        Some(h) => format!(
            "Panel hostname `{h}` is set; no certificate material detected yet.{}",
            if certbot {
                " certbot is available on this host."
            } else {
                " Install certbot to issue a hostname certificate."
            }
        ),
        None => "No panel hostname configured. Set one under Settings / network before issuing Hostname SSL.".into(),
    };
    HostnameSslStatus {
        hostname,
        has_cert,
        certbot,
        detail,
    }
}

#[derive(Debug, Clone)]
pub struct MailSslStatus {
    pub has_cert: bool,
    pub paths_checked: Vec<String>,
    pub detail: String,
}

pub fn mail_ssl_status() -> MailSslStatus {
    let candidates = [
        "/etc/letsencrypt/live/mail/fullchain.pem",
        "/etc/postfix/ssl/cert.pem",
        "/etc/pki/dovecot/certs/dovecot.pem",
        "/etc/dovecot/private/dovecot.pem",
        "/etc/ssl/cpn/mail/fullchain.pem",
        "/var/lib/cpn/ssl/mail/fullchain.pem",
    ];
    let mut found = Vec::new();
    for p in candidates {
        if Path::new(p).is_file() {
            found.push(p.to_string());
        }
    }
    if let Some(host) = load_panel_hostname() {
        let mail_host = format!("mail.{host}");
        let p = format!("/etc/letsencrypt/live/{mail_host}/fullchain.pem");
        if Path::new(&p).is_file() {
            found.push(p);
        }
        let p2 = format!("/etc/letsencrypt/live/{host}/fullchain.pem");
        if Path::new(&p2).is_file() && !found.contains(&p2) {
            found.push(p2);
        }
    }
    let has_cert = !found.is_empty();
    let detail = if has_cert {
        format!(
            "Mail-related certificate files found:\n{}",
            found.join("\n")
        )
    } else {
        "No mail server certificate files detected in common Postfix/Dovecot/Let's Encrypt paths."
            .into()
    };
    MailSslStatus {
        has_cert,
        paths_checked: candidates.iter().map(|s| (*s).to_string()).collect(),
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malware_status_never_imunify() {
        let st = malware_scan_status();
        assert!(!st.detail.contains("Imunify"));
        assert!(!st.engine.contains("Imunify"));
    }
}
