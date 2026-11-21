//! DNS and ACME readiness checks for panel hostname Let's Encrypt (HTTP-01).

use crate::panel_https_front_config::ACME_WEBROOT;
use crate::panel_ops_certbot_install::ensure_certbot_on_path;
use crate::panel_ops_ssl_le::{certbot_available, run_certbot};
use crate::website_preview::ssl_material_present;
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, TcpStream, ToSocketAddrs};
use std::process::Command;
use std::thread;
use std::time::Duration;

const CERTBOT_RETRIES: u32 = 3;
const CERTBOT_RETRY_DELAY_SECS: u64 = 4;

/// Collect non-loopback IPv4 addresses on this host (best effort).
pub fn local_ipv4_addresses() -> Vec<Ipv4Addr> {
    let mut out = HashSet::new();
    if let Ok(output) = Command::new("hostname").arg("-I").output()
        && output.status.success()
    {
        let text = String::from_utf8_lossy(&output.stdout);
        for token in text.split_whitespace() {
            if let Ok(ip) = token.parse::<Ipv4Addr>()
                && !ip.is_loopback()
            {
                out.insert(ip);
            }
        }
    }
    if out.is_empty() {
        let ip = crate::panel_host_info::host_sidebar_info().ip;
        if let Ok(parsed) = ip.trim().parse::<Ipv4Addr>()
            && !parsed.is_loopback()
        {
            out.insert(parsed);
        }
    }
    out.into_iter().collect()
}

fn local_ipv6_addresses() -> Vec<Ipv6Addr> {
    let mut out = HashSet::new();
    if let Ok(output) = Command::new("hostname").arg("-I").output()
        && output.status.success()
    {
        let text = String::from_utf8_lossy(&output.stdout);
        for token in text.split_whitespace() {
            if token.contains(':')
                && let Ok(ip) = token.parse::<Ipv6Addr>()
                && !ip.is_loopback()
            {
                out.insert(ip);
            }
        }
    }
    out.into_iter().collect()
}

/// True when an A or AAAA record for `hostname` points at an address on this host.
pub fn panel_hostname_resolves_here(hostname: &str) -> Result<(), String> {
    let locals_v4: HashSet<Ipv4Addr> = local_ipv4_addresses().into_iter().collect();
    let locals_v6: HashSet<Ipv6Addr> = local_ipv6_addresses().into_iter().collect();
    if locals_v4.is_empty() && locals_v6.is_empty() {
        return Err(
            "Could not determine this host's public IP; fix DNS manually then run cpn network enable-https"
                .into(),
        );
    }

    let mut resolved_v4 = Vec::new();
    let mut resolved_v6 = Vec::new();
    let probe_port = 443_u16;
    let target = format!("{hostname}:{probe_port}");
    let addrs = target
        .to_socket_addrs()
        .map_err(|error| format!("DNS lookup failed for {hostname}: {error}"))?;
    for addr in addrs {
        match addr.ip() {
            IpAddr::V4(v4) => resolved_v4.push(v4),
            IpAddr::V6(v6) => resolved_v6.push(v6),
        }
    }
    if resolved_v4.is_empty() && resolved_v6.is_empty() {
        return Err(format!(
            "No A/AAAA records found for {hostname}. Point DNS at this server, then run: cpn network enable-https"
        ));
    }
    if resolved_v4.iter().any(|ip| locals_v4.contains(ip))
        || resolved_v6.iter().any(|ip| locals_v6.contains(ip))
    {
        return Ok(());
    }
    let mut seen = Vec::new();
    for ip in resolved_v4 {
        seen.push(ip.to_string());
    }
    for ip in resolved_v6 {
        seen.push(ip.to_string());
    }
    let mut local_addrs: Vec<String> = locals_v4.iter().map(|ip| ip.to_string()).collect();
    local_addrs.extend(locals_v6.iter().map(|ip| ip.to_string()));
    Err(format!(
        "DNS for {hostname} points to {} but this host has {}. Fix A/AAAA, then run: cpn network enable-https",
        seen.join(", "),
        if local_addrs.is_empty() {
            "no matching addresses".into()
        } else {
            local_addrs.join(", ")
        }
    ))
}

/// HTTP port is reachable locally (OLS/nginx/Caddy should listen after proxy setup).
pub fn acme_http_port_ready() -> bool {
    let Ok(addr) = "127.0.0.1:80".parse() else {
        return false;
    };
    TcpStream::connect_timeout(&addr, Duration::from_secs(2)).is_ok()
}

fn ensure_acme_webroot_dir() -> Result<(), String> {
    let challenge = std::path::PathBuf::from(ACME_WEBROOT).join(".well-known/acme-challenge");
    std::fs::create_dir_all(&challenge)
        .map_err(|e| format!("Could not create ACME webroot {}: {e}", challenge.display()))?;
    Ok(())
}

/// Issue panel LE cert when DNS points here; retries when forced.
pub fn issue_panel_le_certificate(
    hostname: &str,
    notes: &mut Vec<String>,
    force_when_dns_ready: bool,
) -> Result<bool, String> {
    if ssl_material_present(hostname) {
        notes.push("Certificate material already present for panel hostname".into());
        return Ok(true);
    }
    if let Err(err) = ensure_certbot_on_path() {
        if force_when_dns_ready {
            return Err(format!(
                "certbot is required but could not be installed: {err}. Run cpn network enable-https after fixing package install"
            ));
        }
        notes.push(format!("certbot install skipped: {err}"));
        return Ok(false);
    }
    if !certbot_available() {
        if force_when_dns_ready {
            return Err(
                "certbot is not available. Install certbot, then run: cpn network enable-https"
                    .into(),
            );
        }
        notes.push("certbot not available; configure DNS and run cpn network enable-https".into());
        return Ok(false);
    }

    let dns_here = panel_hostname_resolves_here(hostname);
    if dns_here.is_err() {
        let msg = dns_here.err().unwrap_or_default();
        notes.push(msg);
        notes.push(
            "HTTP reverse proxy is configured; issue TLS when DNS points here: cpn network enable-https"
                .into(),
        );
        return Ok(false);
    }
    notes.push(format!(
        "DNS for {hostname} points at this host; issuing Let's Encrypt (HTTP-01)"
    ));

    if !acme_http_port_ready() {
        if force_when_dns_ready {
            return Err(
                "TCP 80 is not listening locally after proxy setup. Check OpenLiteSpeed/nginx, open firewall port 80, then run: cpn network enable-https".into(),
            );
        }
        notes.push(
            "Port 80 is not listening yet; deferred Let's Encrypt. Open TCP 80 and retry.".into(),
        );
        return Ok(false);
    }

    ensure_acme_webroot_dir()?;
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

    let mut last_err = String::new();
    for attempt in 1..=CERTBOT_RETRIES {
        match run_certbot(&args) {
            Ok(_) => {
                notes.push(format!(
                    "Let's Encrypt certificate issued for {hostname} (attempt {attempt}/{CERTBOT_RETRIES})"
                ));
                return Ok(true);
            }
            Err(err) => {
                last_err = err;
                if attempt < CERTBOT_RETRIES {
                    notes.push(format!(
                        "Let's Encrypt attempt {attempt}/{CERTBOT_RETRIES} failed: {last_err}; retrying in {CERTBOT_RETRY_DELAY_SECS}s"
                    ));
                    thread::sleep(Duration::from_secs(CERTBOT_RETRY_DELAY_SECS));
                }
            }
        }
    }

    if force_when_dns_ready {
        return Err(format!(
            "Let's Encrypt failed after {CERTBOT_RETRIES} attempts: {last_err}. HTTP login still works; retry: cpn network enable-https"
        ));
    }
    notes.push(format!(
        "Let's Encrypt issue failed: {last_err}. Retry: cpn network enable-https"
    ));
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_ipv4_skips_loopback_tokens() {
        let list = local_ipv4_addresses();
        for ip in list {
            assert!(!ip.is_loopback());
        }
    }

    #[test]
    fn acme_http_probe_is_callable() {
        let _ = acme_http_port_ready();
    }
}
