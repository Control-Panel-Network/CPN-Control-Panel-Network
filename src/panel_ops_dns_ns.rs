//! Nameserver host inventory and default NS assignment for new zones.

use crate::paths::join_data;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NsHost {
    pub hostname: String,
    #[serde(default)]
    pub ipv4: Option<String>,
    #[serde(default)]
    pub ipv6: Option<String>,
}

fn ensure_dns_dir() -> Result<PathBuf, String> {
    let root = join_data("dns");
    fs::create_dir_all(&root).map_err(|e| format!("Cannot create DNS dir: {e}"))?;
    Ok(root)
}

fn ns_hosts_path() -> PathBuf {
    join_data("dns/ns-hosts.json")
}

fn default_ns_path() -> PathBuf {
    join_data("dns/default-nameservers.json")
}

fn legacy_nameservers_path() -> PathBuf {
    join_data("dns/nameservers.json")
}

pub fn validate_hostname(raw: &str) -> Result<String, String> {
    let name = raw.trim().trim_end_matches('.').to_ascii_lowercase();
    if name.is_empty() || name.len() > 253 {
        return Err("Invalid hostname".into());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    {
        return Err("Hostname may only contain letters, digits, dots, and hyphens".into());
    }
    if name.contains("..") || name.starts_with('.') || name.starts_with('-') {
        return Err("Invalid hostname".into());
    }
    Ok(name)
}

pub fn validate_ipv4(raw: &str) -> Result<String, String> {
    let s = raw.trim();
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 {
        return Err("Invalid IPv4 address".into());
    }
    for p in &parts {
        let _: u8 = p
            .parse()
            .map_err(|_| "Invalid IPv4 address".to_string())?;
        if p.len() > 1 && p.starts_with('0') {
            return Err("Invalid IPv4 address".into());
        }
    }
    Ok(s.to_string())
}

pub fn validate_ipv6(raw: &str) -> Result<String, String> {
    let s = raw.trim();
    if s.is_empty() || s.len() > 45 || !s.contains(':') {
        return Err("Invalid IPv6 address".into());
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.')
    {
        return Err("Invalid IPv6 address".into());
    }
    Ok(s.to_string())
}

pub fn load_nameservers() -> Vec<String> {
    let path = legacy_nameservers_path();
    let Ok(raw) = fs::read_to_string(path) else {
        return vec![];
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn write_legacy_nameservers(values: &[String]) -> Result<(), String> {
    ensure_dns_dir()?;
    let raw = serde_json::to_string_pretty(values)
        .map_err(|e| format!("Cannot encode nameservers: {e}"))?;
    fs::write(legacy_nameservers_path(), raw).map_err(|e| format!("Cannot save nameservers: {e}"))
}

pub fn load_ns_hosts() -> Vec<NsHost> {
    let path = ns_hosts_path();
    if let Ok(raw) = fs::read_to_string(&path) {
        if let Ok(hosts) = serde_json::from_str::<Vec<NsHost>>(&raw) {
            return hosts;
        }
    }
    load_nameservers()
        .into_iter()
        .filter_map(|h| validate_hostname(&h).ok())
        .map(|hostname| NsHost {
            hostname,
            ipv4: None,
            ipv6: None,
        })
        .collect()
}

pub fn save_ns_hosts(hosts: &[NsHost]) -> Result<(), String> {
    ensure_dns_dir()?;
    for h in hosts {
        validate_hostname(&h.hostname)?;
        if let Some(ref v4) = h.ipv4 {
            validate_ipv4(v4)?;
        }
        if let Some(ref v6) = h.ipv6 {
            validate_ipv6(v6)?;
        }
    }
    let raw =
        serde_json::to_string_pretty(hosts).map_err(|e| format!("Cannot encode NS hosts: {e}"))?;
    fs::write(ns_hosts_path(), raw).map_err(|e| format!("Cannot save NS hosts: {e}"))
}

pub fn add_ns_host(hostname: &str, ipv4: Option<&str>, ipv6: Option<&str>) -> Result<String, String> {
    let hostname = validate_hostname(hostname)?;
    let ipv4 = match ipv4.map(str::trim).filter(|s| !s.is_empty()) {
        Some(v) => Some(validate_ipv4(v)?),
        None => None,
    };
    let ipv6 = match ipv6.map(str::trim).filter(|s| !s.is_empty()) {
        Some(v) => Some(validate_ipv6(v)?),
        None => None,
    };
    if ipv4.is_none() && ipv6.is_none() {
        return Err("Provide at least one glue A or AAAA address".into());
    }
    let mut hosts = load_ns_hosts();
    if hosts
        .iter()
        .any(|h| h.hostname.eq_ignore_ascii_case(&hostname))
    {
        return Err(format!("Nameserver {hostname} already exists"));
    }
    hosts.push(NsHost {
        hostname: hostname.clone(),
        ipv4,
        ipv6,
    });
    hosts.sort_by(|a, b| a.hostname.cmp(&b.hostname));
    save_ns_hosts(&hosts)?;
    Ok(hostname)
}

pub fn delete_ns_host(hostname: &str) -> Result<(), String> {
    let hostname = validate_hostname(hostname)?;
    let mut hosts = load_ns_hosts();
    let before = hosts.len();
    hosts.retain(|h| !h.hostname.eq_ignore_ascii_case(&hostname));
    if hosts.len() == before {
        return Err("Nameserver not found".into());
    }
    save_ns_hosts(&hosts)?;
    let mut defaults = load_default_nameservers();
    defaults.retain(|d| !d.eq_ignore_ascii_case(&hostname));
    let _ = save_default_nameservers(&defaults);
    Ok(())
}

pub fn load_default_nameservers() -> Vec<String> {
    let path = default_ns_path();
    if let Ok(raw) = fs::read_to_string(&path) {
        if let Ok(list) = serde_json::from_str::<Vec<String>>(&raw) {
            return list;
        }
    }
    load_nameservers()
}

pub fn save_default_nameservers(values: &[String]) -> Result<(), String> {
    ensure_dns_dir()?;
    let mut clean = Vec::new();
    for v in values {
        let h = validate_hostname(v)?;
        if !clean.iter().any(|x: &String| x.eq_ignore_ascii_case(&h)) {
            clean.push(h);
        }
    }
    let raw = serde_json::to_string_pretty(&clean)
        .map_err(|e| format!("Cannot encode default nameservers: {e}"))?;
    fs::write(default_ns_path(), raw)
        .map_err(|e| format!("Cannot save default nameservers: {e}"))?;
    write_legacy_nameservers(&clean)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn ns_host_roundtrip() {
        with_test_data_dir(|| {
            add_ns_host("ns1.example.com", Some("203.0.113.10"), None).unwrap();
            let hosts = load_ns_hosts();
            assert_eq!(hosts.len(), 1);
            assert_eq!(hosts[0].ipv4.as_deref(), Some("203.0.113.10"));
            delete_ns_host("ns1.example.com").unwrap();
            assert!(load_ns_hosts().is_empty());
        });
    }
}
