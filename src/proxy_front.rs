//! Optional Nginx front / Proxy Manager path with unique internal IPs per site.
//!
//! First slice: installer flag + persisted feature config under `/var/lib/cpn/`,
//! per-site internal IP assignment, and Nginx reverse-proxy stub scaffolding.
//! Full Nginx Proxy Manager container install is deferred; OpenLiteSpeed can stay
//! as origin while Nginx fronts public traffic when this option is enabled.

use crate::paths;
use crate::sites::{SiteRecord, list_sites, load_site, normalize_domain};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::process::Command;

const SCHEMA_VERSION: u32 = 1;
/// Private pool for lab/panel-local unique IPs (not public WAN addresses).
const POOL_BASE: [u8; 4] = [10, 66, 0, 10];
const POOL_MAX_HOST: u8 = 250;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyFrontSettings {
    pub schema_version: u32,
    /// Operator opted into Nginx front + unique internal IPs.
    pub enabled: bool,
    /// Attempt to install nginx packages during server install when enabled.
    pub install_nginx: bool,
    /// Placeholder for future Nginx Proxy Manager (Docker) install.
    pub install_proxy_manager: bool,
    /// When true, OLS/origin is not expected to bind public :80/:443 alone.
    pub relax_origin_public_routing: bool,
    pub updated_at_unix: u64,
}

impl Default for ProxyFrontSettings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            enabled: false,
            install_nginx: true,
            install_proxy_manager: false,
            relax_origin_public_routing: true,
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

pub fn proxy_front_settings_path() -> PathBuf {
    paths::join_data("proxy_front.json")
}

pub fn nginx_stub_dir() -> PathBuf {
    paths::join_data("nginx-front")
}

fn write_mode600(path: &Path, raw: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    file.write_all(raw.as_bytes())
        .map_err(|e| format!("Could not save {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn load_proxy_front() -> ProxyFrontSettings {
    let Ok(raw) = fs::read_to_string(proxy_front_settings_path()) else {
        return ProxyFrontSettings::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn proxy_front_enabled() -> bool {
    load_proxy_front().enabled
}

pub fn persist_proxy_front(settings: &ProxyFrontSettings) -> Result<(), String> {
    let mut next = settings.clone();
    next.schema_version = SCHEMA_VERSION;
    next.updated_at_unix = now_unix();
    let raw = serde_json::to_string_pretty(&next)
        .map_err(|e| format!("Could not serialize proxy front settings: {e}"))?;
    write_mode600(&proxy_front_settings_path(), &raw)
}

/// Persist installer checkbox selection (does not install packages by itself).
pub fn set_proxy_front_from_install(enabled: bool) -> Result<(), String> {
    let mut s = load_proxy_front();
    s.enabled = enabled;
    if enabled {
        s.install_nginx = true;
        s.relax_origin_public_routing = true;
        // NPM Docker path stays opt-in / future work unless explicitly set later.
        s.install_proxy_manager = false;
    }
    persist_proxy_front(&s)?;
    if enabled {
        ensure_nginx_stub_scaffold()?;
    }
    Ok(())
}

pub fn ensure_nginx_stub_scaffold() -> Result<String, String> {
    let dir = nginx_stub_dir();
    fs::create_dir_all(dir.join("conf.d"))
        .map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    let readme = dir.join("README.txt");
    if !readme.is_file() {
        let body = "CPN Nginx front stubs (unique internal IP path).\n\
Sites get 10.66.0.x addresses when proxy front is enabled.\n\
Full Nginx Proxy Manager container install is a follow-up.\n\
OpenLiteSpeed/origin may serve backends on loopback while Nginx fronts public ports.\n";
        fs::write(&readme, body).map_err(|e| format!("Could not write README: {e}"))?;
    }
    Ok(format!("Nginx front stub dir ready at {}", dir.display()))
}

fn parse_v4(ip: &str) -> Option<Ipv4Addr> {
    ip.parse::<Ipv4Addr>().ok()
}

fn next_free_internal_ip(used: &[Ipv4Addr]) -> Result<Ipv4Addr, String> {
    for host in POOL_BASE[3]..=POOL_MAX_HOST {
        let candidate = Ipv4Addr::new(POOL_BASE[0], POOL_BASE[1], POOL_BASE[2], host);
        if !used.contains(&candidate) {
            return Ok(candidate);
        }
    }
    Err("Internal IP pool exhausted (10.66.0.10-250)".into())
}

/// Assign or return existing unique internal IP for a domain when proxy front is on.
pub fn ensure_site_internal_ip(domain_raw: &str) -> Result<Option<String>, String> {
    if !proxy_front_enabled() {
        return Ok(None);
    }
    let domain = normalize_domain(domain_raw)?;
    let mut site = load_site(&domain)?;
    if let Some(existing) = site
        .internal_ip
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        return Ok(Some(existing.to_string()));
    }
    let used: Vec<Ipv4Addr> = list_sites()?
        .into_iter()
        .filter_map(|s| s.internal_ip.as_deref().and_then(parse_v4))
        .collect();
    let ip = next_free_internal_ip(&used)?;
    site.internal_ip = Some(ip.to_string());
    crate::sites::update_site_internal_ip(&domain, Some(ip.to_string()))?;
    write_site_nginx_stub(&site)?;
    Ok(Some(ip.to_string()))
}

fn write_site_nginx_stub(site: &SiteRecord) -> Result<(), String> {
    let Some(ip) = site
        .internal_ip
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    else {
        return Ok(());
    };
    let dir = nginx_stub_dir().join("conf.d");
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    let path = dir.join(format!("{}.conf", site.domain.replace('/', "_")));
    let body = format!(
        "# CPN stub for {domain} -> internal {ip}\n\
# Backend origin (OpenLiteSpeed or local) typically listens on 127.0.0.1.\n\
# Replace with live Nginx/NPM upstream wiring in a later release.\n\
# server {{\n\
#   listen {ip}:80;\n\
#   server_name {domain};\n\
#   location / {{ proxy_pass http://127.0.0.1:8088; }}\n\
# }}\n",
        domain = site.domain,
        ip = ip
    );
    fs::write(&path, body).map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    Ok(())
}

/// Best-effort package install for nginx when proxy front is enabled (Linux).
pub fn maybe_install_nginx_packages(log: &mut dyn FnMut(String)) -> Result<(), String> {
    let settings = load_proxy_front();
    if !settings.enabled || !settings.install_nginx {
        return Ok(());
    }
    ensure_nginx_stub_scaffold()?;
    #[cfg(not(unix))]
    {
        log("Proxy front enabled, but nginx package install is Linux-only.".into());
        return Ok(());
    }
    #[cfg(unix)]
    {
        log("Proxy front: ensuring nginx packages (best effort)...".into());
        let dnf = Command::new("dnf")
            .args(["install", "-y", "nginx"])
            .status();
        if dnf.map(|s| s.success()).unwrap_or(false) {
            log("Proxy front: nginx package installed or already present.".into());
            // Stubs only in this slice: do not start nginx while OLS/origin still owns :80.
            let _ = Command::new("systemctl")
                .args(["disable", "--now", "nginx"])
                .status();
            log(
                "Proxy front: nginx left stopped (avoid :80 conflict until front is wired).".into(),
            );
            return Ok(());
        }
        let apt = Command::new("apt-get")
            .args(["install", "-y", "nginx"])
            .status();
        if apt.map(|s| s.success()).unwrap_or(false) {
            log("Proxy front: nginx package installed or already present.".into());
            let _ = Command::new("systemctl")
                .args(["disable", "--now", "nginx"])
                .status();
            log(
                "Proxy front: nginx left stopped (avoid :80 conflict until front is wired).".into(),
            );
            return Ok(());
        }
        log(
            "Proxy front: could not auto-install nginx; stubs remain under /var/lib/cpn/nginx-front/."
                .into(),
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_skips_used_addresses() {
        let used = vec![Ipv4Addr::new(10, 66, 0, 10), Ipv4Addr::new(10, 66, 0, 11)];
        assert_eq!(
            next_free_internal_ip(&used).unwrap(),
            Ipv4Addr::new(10, 66, 0, 12)
        );
    }
}
