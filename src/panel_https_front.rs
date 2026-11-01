//! Panel HTTPS entrypoint: reverse-proxy hostname :443/:80 → local CPN listen port.
//!
//! Product model: the panel process stays plain HTTP on the listen port (default 2087).
//! Public HTTPS is terminated by OLS, nginx, or Caddy and proxied to loopback.
//! Never advertise `https://IP:2087` (TLS to a plain HTTP listener fails).

use crate::listen_port::{DEFAULT_PORT, load_preferred_listen_port};
use crate::litespeed_stack::{any_litespeed_installed, restart_litespeed};
use crate::panel_https_front_config::{
    ACME_WEBROOT, CADDY_SNIPPET, HTTPD_CONF, NGINX_CONF, OLS_VH_NAME, caddy_panel_proxy_snippet,
    ensure_ols_https_listener, insert_map_before_catchall, nginx_panel_proxy_conf,
    ols_panel_proxy_vhconf,
};
use crate::panel_network::{load_panel_hostname, save_panel_hostname, validate_panel_hostname};
use crate::panel_ops_certbot_install::ensure_certbot_on_path;
use crate::panel_ops_ssl_le::{certbot_available, run_certbot};
use crate::panel_public_url::save_panel_public_url;
use crate::website_preview::ssl_material_present;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelHttpsEngine {
    Openlitespeed,
    Nginx,
    Caddy,
}

#[derive(Debug, Clone)]
pub struct PanelHttpsReport {
    pub hostname: String,
    pub listen_port: u16,
    pub engine: Option<PanelHttpsEngine>,
    pub proxy_configured: bool,
    pub certificate_present: bool,
    pub public_url: Option<String>,
    pub notes: Vec<String>,
}

impl PanelHttpsReport {
    pub fn summary_line(&self) -> String {
        let engine = match self.engine {
            Some(PanelHttpsEngine::Openlitespeed) => "openlitespeed",
            Some(PanelHttpsEngine::Nginx) => "nginx",
            Some(PanelHttpsEngine::Caddy) => "caddy",
            None => "none",
        };
        format!(
            "panel_https hostname={} engine={} proxy={} cert={} public_url={}",
            self.hostname,
            engine,
            self.proxy_configured,
            self.certificate_present,
            self.public_url.as_deref().unwrap_or("")
        )
    }
}

pub fn detect_panel_https_engine() -> Option<PanelHttpsEngine> {
    if any_litespeed_installed() && Path::new(HTTPD_CONF).is_file() {
        return Some(PanelHttpsEngine::Openlitespeed);
    }
    if Path::new("/usr/sbin/nginx").is_file() || which_ok("nginx") {
        return Some(PanelHttpsEngine::Nginx);
    }
    if Path::new("/usr/bin/caddy").is_file() || which_ok("caddy") {
        return Some(PanelHttpsEngine::Caddy);
    }
    None
}

fn which_ok(bin: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {bin} >/dev/null 2>&1")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn resolve_listen_port(override_port: Option<u16>) -> u16 {
    override_port
        .filter(|p| *p > 0)
        .or_else(load_preferred_listen_port)
        .unwrap_or(DEFAULT_PORT)
}

pub fn panel_cert_paths(hostname: &str) -> (PathBuf, PathBuf) {
    let live = PathBuf::from(format!("/etc/letsencrypt/live/{hostname}"));
    let fullchain = live.join("fullchain.pem");
    let privkey = live.join("privkey.pem");
    if fullchain.is_file() && privkey.is_file() {
        return (fullchain, privkey);
    }
    let custom = PathBuf::from(format!("/var/lib/cpn/ssl/{hostname}"));
    (custom.join("fullchain.pem"), custom.join("privkey.pem"))
}

fn ensure_acme_webroot() -> Result<(), String> {
    let challenge = PathBuf::from(ACME_WEBROOT).join(".well-known/acme-challenge");
    fs::create_dir_all(&challenge)
        .map_err(|e| format!("Could not create ACME webroot {}: {e}", challenge.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(ACME_WEBROOT, fs::Permissions::from_mode(0o755));
        let _ = fs::set_permissions(&challenge, fs::Permissions::from_mode(0o755));
    }
    Ok(())
}

fn write_file(path: &Path, body: &str) -> Result<bool, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
    }
    let prev = fs::read_to_string(path).unwrap_or_default();
    if prev == body {
        return Ok(false);
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o644);
    }
    let mut file = options
        .open(path)
        .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    file.write_all(body.as_bytes())
        .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    Ok(true)
}

fn configure_ols(
    hostname: &str,
    listen_port: u16,
    notes: &mut Vec<String>,
) -> Result<bool, String> {
    ensure_acme_webroot()?;
    let vh_dir = PathBuf::from(format!("/usr/local/lsws/conf/vhosts/{OLS_VH_NAME}"));
    fs::create_dir_all(&vh_dir).map_err(|e| format!("OLS vhost dir: {e}"))?;
    let vhconf = vh_dir.join("vhconf.conf");
    let body = ols_panel_proxy_vhconf(listen_port, ACME_WEBROOT);
    let mut changed = write_file(&vhconf, &body)?;

    let mut conf =
        fs::read_to_string(HTTPD_CONF).map_err(|e| format!("Could not read {HTTPD_CONF}: {e}"))?;
    let vh_block = format!(
        "\nvirtualHost {OLS_VH_NAME} {{\n\
           vhRoot                  {ACME_WEBROOT}/\n\
           configFile              $SERVER_ROOT/conf/vhosts/{OLS_VH_NAME}/vhconf.conf\n\
           allowSymbolLink         1\n\
           enableScript            1\n\
           restrained              1\n\
         }}\n"
    );
    if !conf.contains(&format!("virtualHost {OLS_VH_NAME}")) {
        conf.push_str(&vh_block);
        changed = true;
    }

    let map_line = format!("  map                      {OLS_VH_NAME} {hostname}\n");
    for listener in ["CPNHttp", "Default"] {
        if let Some(updated) = insert_map_before_catchall(&conf, listener, &map_line) {
            conf = updated;
            changed = true;
            break;
        }
    }

    let (fullchain, privkey) = panel_cert_paths(hostname);
    if fullchain.is_file() && privkey.is_file() {
        let next = ensure_ols_https_listener(
            &conf,
            hostname,
            &fullchain.display().to_string(),
            &privkey.display().to_string(),
        );
        if next != conf {
            conf = next;
            changed = true;
        }
    } else {
        notes.push(
            "TLS certificate not present yet; HTTP :80 proxy is configured. Issue Let's Encrypt next."
                .into(),
        );
    }

    if changed {
        write_file(Path::new(HTTPD_CONF), &conf)?;
        let _ = restart_litespeed();
        notes.push("OpenLiteSpeed panel reverse proxy updated; LSWS restarted".into());
    } else {
        notes.push("OpenLiteSpeed panel reverse proxy already current".into());
    }
    Ok(true)
}

fn configure_nginx(
    hostname: &str,
    listen_port: u16,
    notes: &mut Vec<String>,
) -> Result<bool, String> {
    ensure_acme_webroot()?;
    let (fullchain, privkey) = panel_cert_paths(hostname);
    let cert = if fullchain.is_file() && privkey.is_file() {
        Some((
            fullchain.display().to_string(),
            privkey.display().to_string(),
        ))
    } else {
        None
    };
    let cert_refs = cert.as_ref().map(|(c, k)| (c.as_str(), k.as_str()));
    let body = nginx_panel_proxy_conf(hostname, listen_port, ACME_WEBROOT, cert_refs);
    let changed = write_file(Path::new(NGINX_CONF), &body)?;
    if changed {
        let test = Command::new("nginx").arg("-t").output();
        if let Ok(out) = test
            && !out.status.success()
        {
            return Err(format!(
                "nginx -t failed after writing {NGINX_CONF}: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        let _ = Command::new("systemctl").args(["reload", "nginx"]).status();
        notes.push("nginx panel reverse proxy reloaded".into());
    } else {
        notes.push("nginx panel reverse proxy already current".into());
    }
    if cert.is_none() {
        notes.push(
            "TLS certificate not present yet; HTTP :80 proxy is configured. Issue Let's Encrypt next."
                .into(),
        );
    }
    Ok(true)
}

fn configure_caddy(
    hostname: &str,
    listen_port: u16,
    notes: &mut Vec<String>,
) -> Result<bool, String> {
    let body = caddy_panel_proxy_snippet(hostname, listen_port);
    let changed = write_file(Path::new(CADDY_SNIPPET), &body)?;
    let main = Path::new("/etc/caddy/Caddyfile");
    if main.is_file() {
        let current = fs::read_to_string(main).unwrap_or_default();
        let import_line = "import /etc/caddy/Caddyfile.d/*.caddy";
        if !current.contains("Caddyfile.d") {
            let mut next = current;
            if !next.ends_with('\n') {
                next.push('\n');
            }
            next.push_str(import_line);
            next.push('\n');
            let _ = write_file(main, &next);
            notes.push("Added Caddyfile.d import to /etc/caddy/Caddyfile".into());
        }
    }
    if changed {
        let _ = Command::new("systemctl").args(["reload", "caddy"]).status();
        notes.push(
            "Caddy panel reverse proxy reloaded (Caddy issues TLS when DNS is public)".into(),
        );
    } else {
        notes.push("Caddy panel reverse proxy already current".into());
    }
    Ok(true)
}

fn issue_panel_certificate(hostname: &str, notes: &mut Vec<String>) -> Result<bool, String> {
    if ssl_material_present(hostname) {
        notes.push("Certificate material already present for panel hostname".into());
        return Ok(true);
    }
    if let Err(err) = ensure_certbot_on_path() {
        notes.push(format!("certbot install skipped: {err}"));
        return Ok(false);
    }
    if !certbot_available() {
        notes.push("certbot not available; configure DNS and retry Enable HTTPS".into());
        return Ok(false);
    }
    ensure_acme_webroot()?;
    let args = [
        "certonly",
        "--non-interactive",
        "--agree-tos",
        "--register-unsafely-without-email",
        "--webroot",
        "-w",
        ACME_WEBROOT,
        "-d",
        hostname,
    ];
    match run_certbot(&args) {
        Ok(_) => {
            notes.push(format!(
                "Let's Encrypt certificate issued for {hostname} (HTTP-01 webroot)"
            ));
            Ok(true)
        }
        Err(err) => {
            notes.push(format!(
                "Let's Encrypt issue deferred: {err}. Point DNS A/AAAA for {hostname} at this server, open TCP 80, then run: cpn network enable-https"
            ));
            Ok(false)
        }
    }
}

fn persist_public_https_url(hostname: &str, has_cert: bool, notes: &mut Vec<String>) {
    let url = if has_cert {
        format!("https://{hostname}")
    } else {
        format!("http://{hostname}")
    };
    match save_panel_public_url(&url) {
        Ok(()) => notes.push(format!("panel_public_url set to {url}")),
        Err(err) => notes.push(format!("Could not save panel_public_url: {err}")),
    }
}

/// Configure reverse proxy (+ optional LE) for the panel hostname.
///
/// When the proxy is configured, defaults to **Domain-only** (loopback bind) unless
/// `keep_ip_backup` is true (owner wants `http://IP:port` reachable as emergency backup).
pub fn enable_panel_https(
    hostname_raw: Option<&str>,
    listen_port: Option<u16>,
    issue_certificate: bool,
) -> Result<PanelHttpsReport, String> {
    enable_panel_https_with(hostname_raw, listen_port, issue_certificate, false)
}

pub fn enable_panel_https_with(
    hostname_raw: Option<&str>,
    listen_port: Option<u16>,
    issue_certificate: bool,
    keep_ip_backup: bool,
) -> Result<PanelHttpsReport, String> {
    let hostname = match hostname_raw.map(str::trim).filter(|v| !v.is_empty()) {
        Some(raw) => validate_panel_hostname(raw)?,
        None => load_panel_hostname().ok_or_else(|| {
            "No panel hostname set. Pass a hostname or run: cpn network set-hostname --hostname panel.example.com".to_string()
        })?,
    };
    save_panel_hostname(&hostname)?;
    let listen_port = resolve_listen_port(listen_port);
    let mut notes = Vec::new();
    notes.push(format!(
        "Panel process stays HTTP on 127.0.0.1:{listen_port}; public HTTPS uses the web server on 443"
    ));
    notes.push(
        "Do not open https://SERVER_IP:2087 (plain HTTP listen port; browsers show ERR_SSL_PROTOCOL_ERROR)"
            .into(),
    );

    let engine = detect_panel_https_engine();
    let mut proxy_configured = false;
    match engine {
        Some(PanelHttpsEngine::Openlitespeed) => {
            proxy_configured = configure_ols(&hostname, listen_port, &mut notes)?;
        }
        Some(PanelHttpsEngine::Nginx) => {
            proxy_configured = configure_nginx(&hostname, listen_port, &mut notes)?;
        }
        Some(PanelHttpsEngine::Caddy) => {
            proxy_configured = configure_caddy(&hostname, listen_port, &mut notes)?;
        }
        None => {
            notes.push(
                "No OpenLiteSpeed, nginx, or Caddy detected. Install a web stack first, then retry Enable HTTPS."
                    .into(),
            );
        }
    }

    let mut certificate_present = ssl_material_present(&hostname);
    if issue_certificate && !certificate_present {
        if matches!(engine, Some(PanelHttpsEngine::Caddy)) {
            notes.push(
                "Caddy manages certificates automatically when the hostname has public DNS".into(),
            );
            certificate_present = true;
        } else if proxy_configured {
            let _ = issue_panel_certificate(&hostname, &mut notes)?;
            certificate_present = ssl_material_present(&hostname);
            if certificate_present {
                match engine {
                    Some(PanelHttpsEngine::Openlitespeed) => {
                        let _ = configure_ols(&hostname, listen_port, &mut notes)?;
                    }
                    Some(PanelHttpsEngine::Nginx) => {
                        let _ = configure_nginx(&hostname, listen_port, &mut notes)?;
                    }
                    _ => {}
                }
            }
        }
    }

    if proxy_configured {
        persist_public_https_url(
            &hostname,
            certificate_present || matches!(engine, Some(PanelHttpsEngine::Caddy)),
            &mut notes,
        );
        notes.extend(crate::panel_access_mode::apply_domain_only_after_https_proxy(keep_ip_backup));
    }

    let public_url = crate::panel_public_url::load_panel_public_url();
    Ok(PanelHttpsReport {
        hostname,
        listen_port,
        engine,
        proxy_configured,
        certificate_present,
        public_url,
        notes,
    })
}

/// Best-effort hook for install finish / set-hostname.
pub fn enable_panel_https_best_effort(hostname: &str, listen_port: u16) -> PanelHttpsReport {
    match enable_panel_https_with(Some(hostname), Some(listen_port), true, false) {
        Ok(report) => report,
        Err(err) => PanelHttpsReport {
            hostname: hostname.to_string(),
            listen_port,
            engine: detect_panel_https_engine(),
            proxy_configured: false,
            certificate_present: false,
            public_url: None,
            notes: vec![format!("Panel HTTPS setup skipped: {err}")],
        },
    }
}
