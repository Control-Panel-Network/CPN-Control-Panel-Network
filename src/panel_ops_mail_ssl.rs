//! Mail hostname TLS: default Let's Encrypt for `mail.<apex>`, apply to Postfix/Dovecot.

use crate::panel_https_front_config::{
    ACME_WEBROOT, HTTPD_CONF, OLS_VH_NAME, insert_map_before_catchall,
};
use crate::panel_ops_certbot_install::ensure_certbot_on_path;
use crate::panel_ops_mail_onboarding::{
    load_mail_onboarding, preferred_mail_service_hostname, seed_mail_onboarding_hostname_if_empty,
};
use crate::panel_ops_ssl_le::{certbot_available, run_certbot};
use crate::panel_ops_ssl_provider::SslProvider;
use crate::paths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailSslConfig {
    pub schema_version: u32,
    /// Default is Let's Encrypt unless the operator picks Custom or None.
    pub provider: SslProvider,
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub custom_cert_path: String,
    #[serde(default)]
    pub custom_key_path: String,
    #[serde(default)]
    pub last_error: String,
    #[serde(default)]
    pub updated_at_unix: u64,
}

impl Default for MailSslConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            provider: SslProvider::LetsEncrypt,
            hostname: String::new(),
            custom_cert_path: String::new(),
            custom_key_path: String::new(),
            last_error: String::new(),
            updated_at_unix: 0,
        }
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|v| v.as_secs())
        .unwrap_or(0)
}

pub fn mail_ssl_config_path() -> PathBuf {
    paths::join_data("mail-ssl.json")
}

pub fn load_mail_ssl_config() -> MailSslConfig {
    let Ok(raw) = fs::read_to_string(mail_ssl_config_path()) else {
        return MailSslConfig::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save_mail_ssl_config(mut cfg: MailSslConfig) -> Result<(), String> {
    cfg.schema_version = 1;
    cfg.updated_at_unix = now_unix();
    let dir = paths::default_data_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create data dir: {e}"))?;
    let json = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    let path = mail_ssl_config_path();
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut f = options
        .open(&path)
        .map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
    f.write_all(json.as_bytes())
        .map_err(|e| format!("Cannot save mail SSL config: {e}"))?;
    Ok(())
}

/// Resolve the mail service hostname used for client TLS and ACME.
pub fn resolve_mail_ssl_hostname() -> Option<String> {
    let mut cfg = load_mail_ssl_config();
    if !cfg.hostname.trim().is_empty() {
        return Some(cfg.hostname.trim().to_ascii_lowercase());
    }
    let onboarding = load_mail_onboarding();
    if !onboarding.hostname.trim().is_empty() {
        let host = onboarding.hostname.trim().to_ascii_lowercase();
        if host.starts_with("mail.") {
            return Some(host);
        }
        return Some(preferred_mail_service_hostname(&host));
    }
    let _ = seed_mail_onboarding_hostname_if_empty();
    let onboarding = load_mail_onboarding();
    if onboarding.hostname.trim().is_empty() {
        return None;
    }
    let host = preferred_mail_service_hostname(onboarding.hostname.trim());
    cfg.hostname = host.clone();
    let _ = save_mail_ssl_config(cfg);
    Some(host)
}

fn le_live_paths(hostname: &str) -> (PathBuf, PathBuf) {
    let base = PathBuf::from(format!("/etc/letsencrypt/live/{hostname}"));
    (base.join("fullchain.pem"), base.join("privkey.pem"))
}

pub fn mail_le_material_present(hostname: &str) -> bool {
    let (cert, key) = le_live_paths(hostname);
    cert.is_file() && key.is_file()
}

fn ensure_acme_webroot() -> Result<(), String> {
    let challenge = Path::new(ACME_WEBROOT).join(".well-known/acme-challenge");
    fs::create_dir_all(&challenge)
        .map_err(|e| format!("Could not create ACME webroot {}: {e}", challenge.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // `/var/lib/cpn` is often 0700; OLS (nobody) must traverse to the ACME webroot.
        if let Some(parent) = Path::new(ACME_WEBROOT).parent() {
            let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o711));
        }
        let _ = fs::set_permissions(ACME_WEBROOT, fs::Permissions::from_mode(0o755));
        let _ = fs::set_permissions(&challenge, fs::Permissions::from_mode(0o755));
        let _ = Command::new("chcon")
            .args(["-R", "-t", "httpd_sys_content_t", ACME_WEBROOT])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    Ok(())
}

/// Map `mail.<apex>` onto the CPNPanel ACME webroot for HTTP-01 on OpenLiteSpeed.
pub fn ensure_ols_mail_hostname_acme_map(hostname: &str) -> Result<bool, String> {
    let hostname = hostname.trim().to_ascii_lowercase();
    if hostname.is_empty() {
        return Ok(false);
    }
    let path = Path::new(HTTPD_CONF);
    if !path.is_file() {
        return Ok(false);
    }
    let raw =
        fs::read_to_string(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    let map_line = format!("    map                      {OLS_VH_NAME} {hostname}");
    if raw.contains(&format!("{OLS_VH_NAME} {hostname}")) {
        return Ok(false);
    }
    // Prefer the public HTTP listener (CPNHttp *:80). Default is often *:8088 only.
    let mut updated = raw.clone();
    let mut mapped = false;
    for listener in ["CPNHttp", "HTTP", "Default"] {
        if let Some(next) = insert_map_before_catchall(&updated, listener, &map_line) {
            updated = next;
            mapped = true;
            // Stop after mapping onto CPNHttp when present.
            if listener == "CPNHttp" {
                break;
            }
        }
    }
    if !mapped && !updated.contains(&format!("{OLS_VH_NAME} {hostname}")) {
        updated.push_str(&format!("\n{map_line}\n"));
    }
    if updated == raw {
        return Ok(false);
    }
    fs::write(path, &updated).map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
    let _ = Command::new("/usr/local/lsws/bin/lswsctrl")
        .arg("reload")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    Ok(true)
}

/// Point Postfix + Dovecot at the given certificate material.
pub fn apply_mail_tls_material(cert: &Path, key: &Path) -> Result<(), String> {
    if !cert.is_file() || !key.is_file() {
        return Err(format!(
            "Mail TLS material missing (cert={}, key={})",
            cert.display(),
            key.display()
        ));
    }
    let cert_s = cert.display().to_string();
    let key_s = key.display().to_string();

    let _ = Command::new("postconf")
        .args(["-e", &format!("smtpd_tls_cert_file={cert_s}")])
        .status();
    let _ = Command::new("postconf")
        .args(["-e", &format!("smtpd_tls_key_file={key_s}")])
        .status();
    let _ = Command::new("postfix").arg("reload").status();

    let ssl_conf = Path::new("/etc/dovecot/conf.d/10-ssl.conf");
    if ssl_conf.is_file() {
        let raw = fs::read_to_string(ssl_conf).unwrap_or_default();
        let mut lines: Vec<String> = Vec::new();
        let mut saw_cert = false;
        let mut saw_key = false;
        for line in raw.lines() {
            let t = line.trim_start();
            if t.starts_with("ssl_cert ") || t.starts_with("ssl_cert=") {
                lines.push(format!("ssl_cert = <{cert_s}"));
                saw_cert = true;
            } else if t.starts_with("ssl_key ") || t.starts_with("ssl_key=") {
                lines.push(format!("ssl_key = <{key_s}"));
                saw_key = true;
            } else {
                lines.push(line.to_string());
            }
        }
        if !saw_cert {
            lines.push(format!("ssl_cert = <{cert_s}"));
        }
        if !saw_key {
            lines.push(format!("ssl_key = <{key_s}"));
        }
        let body = if raw.ends_with('\n') {
            format!("{}\n", lines.join("\n"))
        } else {
            lines.join("\n")
        };
        fs::write(ssl_conf, body).map_err(|e| format!("Cannot update Dovecot SSL conf: {e}"))?;
    }
    let _ = Command::new("systemctl")
        .args(["reload", "dovecot"])
        .status();
    Ok(())
}

fn issue_lets_encrypt_for_mail_host(hostname: &str) -> Result<String, String> {
    ensure_certbot_on_path()?;
    if !certbot_available() {
        return Err("certbot is not available on PATH".into());
    }
    ensure_acme_webroot()?;
    let _ = ensure_ols_mail_hostname_acme_map(hostname);
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
        "--keep-until-expiring",
        "--cert-name",
        hostname,
    ];
    run_certbot(&args)?;
    let (cert, key) = le_live_paths(hostname);
    apply_mail_tls_material(&cert, &key)?;
    Ok(format!(
        "Let's Encrypt issued for `{hostname}` and applied to Postfix/Dovecot."
    ))
}

/// Ensure mail TLS according to persisted preference (default: Let's Encrypt).
pub fn ensure_mail_ssl_default() -> Result<String, String> {
    let mut cfg = load_mail_ssl_config();
    let Some(hostname) = resolve_mail_ssl_hostname() else {
        return Ok(
            "Mail SSL skipped: no mail hostname yet (set Server onboarding or create a site)."
                .into(),
        );
    };
    if cfg.hostname.trim().is_empty() {
        cfg.hostname = hostname.clone();
    }
    match cfg.provider {
        SslProvider::None => {
            cfg.last_error.clear();
            save_mail_ssl_config(cfg)?;
            Ok("Mail SSL provider is None; skipped issuance.".into())
        }
        SslProvider::Custom => {
            let cert = Path::new(cfg.custom_cert_path.trim());
            let key = Path::new(cfg.custom_key_path.trim());
            if cert.is_file() && key.is_file() {
                apply_mail_tls_material(cert, key)?;
                cfg.last_error.clear();
                save_mail_ssl_config(cfg)?;
                Ok("Custom mail SSL applied to Postfix/Dovecot.".into())
            } else {
                let err = "Custom mail SSL selected but cert/key paths are missing".to_string();
                cfg.last_error = err.clone();
                save_mail_ssl_config(cfg)?;
                Err(err)
            }
        }
        SslProvider::LetsEncrypt | SslProvider::ZeroSsl | SslProvider::CloudflareCa => {
            // Product default for Host email: always prefer Let's Encrypt for the mail hostname.
            cfg.provider = SslProvider::LetsEncrypt;
            if mail_le_material_present(&hostname) {
                let (cert, key) = le_live_paths(&hostname);
                apply_mail_tls_material(&cert, &key)?;
                cfg.last_error.clear();
                save_mail_ssl_config(cfg)?;
                return Ok(format!(
                    "Existing Let's Encrypt for `{hostname}` applied to Postfix/Dovecot."
                ));
            }
            match issue_lets_encrypt_for_mail_host(&hostname) {
                Ok(msg) => {
                    cfg.last_error.clear();
                    save_mail_ssl_config(cfg)?;
                    Ok(msg)
                }
                Err(err) => {
                    cfg.last_error = err.clone();
                    let _ = save_mail_ssl_config(cfg);
                    Err(err)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::panel_ops_mail_onboarding::{MailOnboarding, save_mail_onboarding};

    #[test]
    fn default_provider_is_lets_encrypt() {
        assert_eq!(MailSslConfig::default().provider, SslProvider::LetsEncrypt);
    }

    #[test]
    fn resolve_prefers_mail_prefix() {
        with_test_data_dir(|| {
            let mut onboarding = MailOnboarding::default();
            onboarding.hostname = "threebrickstudio.com".into();
            save_mail_onboarding(onboarding).unwrap();
            assert_eq!(
                resolve_mail_ssl_hostname().as_deref(),
                Some("mail.threebrickstudio.com")
            );
        });
    }
}
