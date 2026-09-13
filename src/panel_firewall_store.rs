//! Persisted CPN firewall manager state (rules, bans, trusted never-block IPs).

use crate::paths::default_data_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;
use std::str::FromStr;

const SCHEMA: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrustedSource {
    Server,
    FirstAdmin,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedIp {
    pub ip: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub protected: bool,
    pub source: TrustedSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirewallRule {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub port: String,
    #[serde(default = "default_any_source")]
    pub source: String,
}

fn default_any_source() -> String {
    "0.0.0.0/0".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BannedIp {
    pub ip: String,
    #[serde(default)]
    pub reason: String,
    pub banned_at: u64,
    pub expires_at: Option<u64>,
    #[serde(default = "default_active")]
    pub status: String,
}

fn default_active() -> String {
    "active".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FirewallManagerStore {
    #[serde(default = "schema_default")]
    pub schema_version: u32,
    #[serde(default)]
    pub rules: Vec<FirewallRule>,
    #[serde(default)]
    pub banned: Vec<BannedIp>,
    #[serde(default)]
    pub trusted: Vec<TrustedIp>,
    #[serde(default)]
    pub first_admin_ip: Option<String>,
    #[serde(default)]
    pub server_ip: Option<String>,
}

fn schema_default() -> u32 {
    SCHEMA
}

fn store_path() -> PathBuf {
    default_data_dir().join("firewall-manager.json")
}

pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn new_id() -> String {
    format!("r{}", now_unix())
        .chars()
        .chain(format!("{:x}", now_unix().wrapping_mul(31) % 0xffff).chars())
        .collect()
}

/// Normalize and validate a single IP or CIDR (IPv4 / IPv6).
pub fn validate_ip_or_cidr(raw: &str) -> Result<String, String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err("IP address is required".into());
    }
    if value.chars().count() > 64 {
        return Err("IP address is too long".into());
    }
    if value.contains(|c: char| !(c.is_ascii_hexdigit() || c == '.' || c == ':' || c == '/')) {
        return Err("IP address contains invalid characters".into());
    }
    if let Some((addr, prefix)) = value.split_once('/') {
        let ip = IpAddr::from_str(addr.trim()).map_err(|_| "Invalid IP address".to_string())?;
        let bits: u8 = prefix
            .trim()
            .parse()
            .map_err(|_| "Invalid CIDR prefix".to_string())?;
        let max = match ip {
            IpAddr::V4(_) => 32u8,
            IpAddr::V6(_) => 128u8,
        };
        if bits > max {
            return Err(format!("CIDR prefix must be 0-{max}"));
        }
        return Ok(format!("{}/{}", ip, bits));
    }
    let ip = IpAddr::from_str(value).map_err(|_| "Invalid IP address".to_string())?;
    Ok(ip.to_string())
}

pub fn validate_port(raw: &str) -> Result<String, String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err("Port is required".into());
    }
    if let Some((a, b)) = value.split_once('-') {
        let start: u16 = a
            .trim()
            .parse()
            .map_err(|_| "Invalid port range".to_string())?;
        let end: u16 = b
            .trim()
            .parse()
            .map_err(|_| "Invalid port range".to_string())?;
        if start == 0 || end == 0 || start > end {
            return Err("Invalid port range".into());
        }
        return Ok(format!("{start}-{end}"));
    }
    let port: u16 = value
        .parse()
        .map_err(|_| "Port must be 1-65535".to_string())?;
    if port == 0 {
        return Err("Port must be 1-65535".into());
    }
    Ok(port.to_string())
}

pub fn validate_protocol(raw: &str) -> Result<String, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "tcp" => Ok("tcp".into()),
        "udp" => Ok("udp".into()),
        _ => Err("Protocol must be tcp or udp".into()),
    }
}

fn ipv4_in_cidr(ip: Ipv4Addr, network: Ipv4Addr, prefix: u8) -> bool {
    if prefix == 0 {
        return true;
    }
    let mask = if prefix >= 32 {
        u32::MAX
    } else {
        !((1u32 << (32 - prefix)) - 1)
    };
    (u32::from(ip) & mask) == (u32::from(network) & mask)
}

fn ipv6_in_cidr(ip: Ipv6Addr, network: Ipv6Addr, prefix: u8) -> bool {
    if prefix == 0 {
        return true;
    }
    let ip_bytes = ip.octets();
    let net_bytes = network.octets();
    let full = (prefix / 8) as usize;
    let rem = prefix % 8;
    if ip_bytes[..full] != net_bytes[..full] {
        return false;
    }
    if rem == 0 {
        return true;
    }
    let mask = !((1u8 << (8 - rem)) - 1);
    (ip_bytes[full] & mask) == (net_bytes[full] & mask)
}

/// True when `candidate` (IP or CIDR host) is covered by `entry` (IP or CIDR).
pub fn ip_matches_entry(candidate: &str, entry: &str) -> bool {
    let Ok(cand) = validate_ip_or_cidr(candidate) else {
        return false;
    };
    let Ok(ent) = validate_ip_or_cidr(entry) else {
        return false;
    };
    if cand.eq_ignore_ascii_case(&ent) {
        return true;
    }
    let cand_host = cand.split('/').next().unwrap_or(&cand);
    let Ok(cand_ip) = IpAddr::from_str(cand_host) else {
        return false;
    };
    if let Some((net_s, pref_s)) = ent.split_once('/') {
        let Ok(net) = IpAddr::from_str(net_s) else {
            return false;
        };
        let Ok(pref) = pref_s.parse::<u8>() else {
            return false;
        };
        return match (cand_ip, net) {
            (IpAddr::V4(c), IpAddr::V4(n)) => ipv4_in_cidr(c, n, pref),
            (IpAddr::V6(c), IpAddr::V6(n)) => ipv6_in_cidr(c, n, pref),
            _ => false,
        };
    }
    false
}

pub fn load_store() -> FirewallManagerStore {
    let path = store_path();
    match fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => FirewallManagerStore {
            schema_version: SCHEMA,
            ..Default::default()
        },
    }
}

pub fn save_store(store: &FirewallManagerStore) -> Result<(), String> {
    let path = store_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Cannot create data dir: {e}"))?;
    }
    let mut body = store.clone();
    body.schema_version = SCHEMA;
    let raw = serde_json::to_string_pretty(&body)
        .map_err(|e| format!("Cannot encode firewall store: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("Cannot write firewall store: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn upsert_trusted(store: &mut FirewallManagerStore, ip: &str, label: &str, source: TrustedSource) {
    let Ok(norm) = validate_ip_or_cidr(ip) else {
        return;
    };
    if let Some(existing) = store.trusted.iter_mut().find(|t| t.ip == norm) {
        existing.protected = true;
        if existing.label.is_empty() {
            existing.label = label.to_string();
        }
        if matches!(source, TrustedSource::Server | TrustedSource::FirstAdmin) {
            existing.source = source;
        }
        return;
    }
    store.trusted.push(TrustedIp {
        ip: norm,
        label: label.to_string(),
        protected: true,
        source,
    });
}

/// Seed server IP and first-admin IP into the never-block list.
pub fn ensure_protected_seeds(
    server_ip: Option<&str>,
    login_ip: Option<&str>,
) -> FirewallManagerStore {
    let mut store = load_store();
    let mut changed = false;

    if let Some(sip) = server_ip
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "Unavailable")
    {
        if store.server_ip.as_deref() != Some(sip) {
            store.server_ip = Some(sip.to_string());
            changed = true;
        }
        let before = store.trusted.len();
        upsert_trusted(
            &mut store,
            sip,
            "Server IP (protected)",
            TrustedSource::Server,
        );
        if store.trusted.len() != before
            || store
                .trusted
                .iter()
                .any(|t| t.ip == *sip || ip_matches_entry(sip, &t.ip))
        {
            changed = true;
        }
    }

    if store.first_admin_ip.is_none() {
        if let Some(lip) = login_ip.map(str::trim).filter(|s| !s.is_empty())
            && let Ok(norm) = validate_ip_or_cidr(lip)
        {
            // Still trust loopback so local lab operators are never blocked.
            store.first_admin_ip = Some(norm.clone());
            upsert_trusted(
                &mut store,
                &norm,
                "First admin IP (protected)",
                TrustedSource::FirstAdmin,
            );
            changed = true;
        }
    } else if let Some(ref fip) = store.first_admin_ip.clone() {
        let before = store.trusted.len();
        upsert_trusted(
            &mut store,
            fip,
            "First admin IP (protected)",
            TrustedSource::FirstAdmin,
        );
        if store.trusted.len() != before {
            changed = true;
        }
    }

    if changed {
        let _ = save_store(&store);
    }
    store
}

/// Record peer IP from a successful first-admin login when missing.
pub fn record_admin_login_ip(username: &str, peer_ip: Option<&str>) {
    if !crate::panel_admin::is_panel_admin(username) {
        return;
    }
    let Some(raw) = peer_ip.map(str::trim).filter(|s| !s.is_empty()) else {
        return;
    };
    let Ok(norm) = validate_ip_or_cidr(raw) else {
        return;
    };
    let mut store = load_store();
    if store.first_admin_ip.is_some() {
        // Keep first admin IP sticky; still ensure it stays trusted.
        if let Some(ref fip) = store.first_admin_ip.clone() {
            upsert_trusted(
                &mut store,
                fip,
                "First admin IP (protected)",
                TrustedSource::FirstAdmin,
            );
            let _ = save_store(&store);
        }
        return;
    }
    store.first_admin_ip = Some(norm.clone());
    upsert_trusted(
        &mut store,
        &norm,
        "First admin IP (protected)",
        TrustedSource::FirstAdmin,
    );
    let _ = save_store(&store);
}

pub fn is_protected_ip(store: &FirewallManagerStore, ip: &str) -> bool {
    let Ok(norm) = validate_ip_or_cidr(ip) else {
        return false;
    };
    if let Some(ref sip) = store.server_ip
        && (ip_matches_entry(&norm, sip) || ip_matches_entry(sip, &norm))
    {
        return true;
    }
    if let Some(ref fip) = store.first_admin_ip
        && (ip_matches_entry(&norm, fip) || ip_matches_entry(fip, &norm))
    {
        return true;
    }
    store
        .trusted
        .iter()
        .any(|t| ip_matches_entry(&norm, &t.ip) || ip_matches_entry(&t.ip, &norm))
}

pub fn add_rule(
    name: &str,
    protocol: &str,
    port: &str,
    source: &str,
) -> Result<FirewallManagerStore, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 64 {
        return Err("Rule name must be 1-64 characters".into());
    }
    if name.chars().any(|c| c.is_control()) {
        return Err("Rule name cannot include control characters".into());
    }
    let protocol = validate_protocol(protocol)?;
    let port = validate_port(port)?;
    let source = if source.trim().is_empty() {
        "0.0.0.0/0".into()
    } else {
        validate_ip_or_cidr(source)?
    };
    let mut store = load_store();
    store.rules.push(FirewallRule {
        id: new_id(),
        name: name.to_string(),
        protocol,
        port,
        source,
    });
    save_store(&store)?;
    Ok(store)
}

pub fn remove_rule(id: &str) -> Result<FirewallManagerStore, String> {
    let mut store = load_store();
    let before = store.rules.len();
    store.rules.retain(|r| r.id != id);
    if store.rules.len() == before {
        return Err("Rule not found".into());
    }
    save_store(&store)?;
    Ok(store)
}

pub fn add_ban(
    ip: &str,
    reason: &str,
    duration_secs: Option<u64>,
) -> Result<FirewallManagerStore, String> {
    let ip = validate_ip_or_cidr(ip)?;
    if ip.contains('/') {
        // Allow CIDR bans only for /32 or /128 (single host) or explicit networks up to /24 IPv4.
        if let Some((_, pref)) = ip.split_once('/') {
            let bits: u8 = pref.parse().unwrap_or(0);
            if bits < 24 && ip.contains('.') {
                return Err("IPv4 ban CIDR must be /24 or narrower".into());
            }
        }
    }
    let mut store = load_store();
    if is_protected_ip(&store, &ip) {
        return Err(
            "Refused: that address is on the trusted (never block) list or is the server/first-admin IP"
                .into(),
        );
    }
    let reason = reason.trim().chars().take(200).collect::<String>();
    let now = now_unix();
    store.banned.retain(|b| b.ip != ip);
    store.banned.push(BannedIp {
        ip,
        reason,
        banned_at: now,
        expires_at: duration_secs.map(|d| now.saturating_add(d)),
        status: "active".into(),
    });
    save_store(&store)?;
    Ok(store)
}

pub fn remove_ban(ip: &str) -> Result<FirewallManagerStore, String> {
    let ip = validate_ip_or_cidr(ip)?;
    let mut store = load_store();
    let before = store.banned.len();
    store.banned.retain(|b| b.ip != ip);
    if store.banned.len() == before {
        return Err("Banned IP not found".into());
    }
    save_store(&store)?;
    Ok(store)
}

pub fn add_trusted_manual(ip: &str, label: &str) -> Result<FirewallManagerStore, String> {
    let ip = validate_ip_or_cidr(ip)?;
    let label = label.trim().chars().take(64).collect::<String>();
    let mut store = load_store();
    upsert_trusted(
        &mut store,
        &ip,
        if label.is_empty() {
            "Trusted IP"
        } else {
            &label
        },
        TrustedSource::Manual,
    );
    // Manual adds are protected from ban as well.
    if let Some(t) = store.trusted.iter_mut().find(|t| t.ip == ip) {
        t.protected = true;
        if !label.is_empty() {
            t.label = label;
        }
    }
    save_store(&store)?;
    Ok(store)
}

pub fn remove_trusted(ip: &str) -> Result<FirewallManagerStore, String> {
    let ip = validate_ip_or_cidr(ip)?;
    let mut store = load_store();
    let Some(entry) = store.trusted.iter().find(|t| t.ip == ip) else {
        return Err("Trusted IP not found".into());
    };
    if matches!(
        entry.source,
        TrustedSource::Server | TrustedSource::FirstAdmin
    ) {
        return Err("Cannot remove protected server or first-admin IP".into());
    }
    store.trusted.retain(|t| t.ip != ip);
    save_store(&store)?;
    Ok(store)
}

pub fn purge_expired_bans(store: &mut FirewallManagerStore) -> Vec<String> {
    let now = now_unix();
    let mut removed = Vec::new();
    store.banned.retain(|b| {
        if let Some(exp) = b.expires_at
            && exp <= now
        {
            removed.push(b.ip.clone());
            return false;
        }
        true
    });
    removed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn validates_ip_and_cidr() {
        assert!(validate_ip_or_cidr("192.0.2.10").is_ok());
        assert!(validate_ip_or_cidr("192.0.2.0/24").is_ok());
        assert!(validate_ip_or_cidr("not-an-ip").is_err());
        assert!(validate_ip_or_cidr("192.0.2.0/99").is_err());
        assert!(validate_ip_or_cidr("1; rm -rf /").is_err());
    }

    #[test]
    fn protected_ban_fails_closed() {
        with_test_data_dir(|| {
            let mut store = FirewallManagerStore::default();
            store.server_ip = Some("203.0.113.10".into());
            upsert_trusted(&mut store, "203.0.113.10", "Server", TrustedSource::Server);
            save_store(&store).unwrap();
            let err = add_ban("203.0.113.10", "test", None).unwrap_err();
            assert!(err.contains("Refused"));
        });
    }

    #[test]
    fn cidr_match_works() {
        assert!(ip_matches_entry("192.0.2.55", "192.0.2.0/24"));
        assert!(!ip_matches_entry("198.51.100.1", "192.0.2.0/24"));
    }
}
