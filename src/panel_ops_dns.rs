//! DNS zone store under the CPN data directory (file-backed JSON + zone text).

use crate::panel_ops_dns_zonefile::{parse_zone_file, serialize_zone_file, validate_record};
use crate::panel_session::session_secret;
use crate::paths::join_data;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

pub use crate::panel_ops_dns_ns::{
    NsHost, add_ns_host, delete_ns_host, load_default_nameservers, load_nameservers, load_ns_hosts,
    save_default_nameservers, validate_hostname, validate_ipv4, validate_ipv6,
};
pub use crate::panel_ops_dns_zonefile::{ALLOWED_TYPES, DnsRecord};

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn hmac_hex(secret: &str, payload: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(payload.as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// CSRF token for DNS admin POSTs (bound to user + hour bucket).
pub fn dns_csrf_token(username: &str) -> String {
    let secret = session_secret(None);
    let hour = now_unix() / 3600;
    let payload = format!("dns|{username}|{hour}");
    format!("{hour}.{}", hmac_hex(&secret, &payload))
}

pub fn verify_dns_csrf(username: &str, token: &str) -> bool {
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
    let payload = format!("dns|{username}|{hour}");
    hmac_hex(&secret, &payload) == sig
}

pub fn dns_root() -> PathBuf {
    join_data("dns")
}

pub fn ensure_dns_root() -> Result<PathBuf, String> {
    let root = dns_root();
    fs::create_dir_all(&root).map_err(|e| format!("Cannot create DNS dir: {e}"))?;
    Ok(root)
}

/// Normalize a domain for zone create (strip scheme, www, trailing dots).
pub fn normalize_domain_input(raw: &str) -> Result<String, String> {
    let mut name = raw.trim().to_ascii_lowercase();
    for prefix in ["https://", "http://"] {
        if let Some(rest) = name.strip_prefix(prefix) {
            name = rest.to_string();
        }
    }
    if let Some((host, _)) = name.split_once('/') {
        name = host.to_string();
    }
    while name.ends_with('.') {
        name.pop();
    }
    if let Some(rest) = name.strip_prefix("www.") {
        name = rest.to_string();
    }
    safe_zone_name(&name)
}

fn safe_zone_name(name: &str) -> Result<String, String> {
    let name = name.trim().to_ascii_lowercase();
    if name.is_empty() || name.len() > 253 {
        return Err("Invalid zone name".into());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    {
        return Err("Zone name may only contain letters, digits, dots, and hyphens".into());
    }
    if name.contains("..") || name.starts_with('.') || name.ends_with('.') || name.starts_with('-')
    {
        return Err("Invalid zone name".into());
    }
    Ok(name)
}

pub fn list_zones() -> Result<Vec<String>, String> {
    let root = ensure_dns_root()?;
    let mut zones = Vec::new();
    if let Ok(rd) = fs::read_dir(root) {
        for ent in rd.flatten() {
            let name = ent.file_name().to_string_lossy().to_string();
            if let Some(zone) = name.strip_suffix(".zone") {
                zones.push(zone.to_string());
            } else if let Some(zone) = name.strip_suffix(".json") {
                if zone != "nameservers"
                    && zone != "ns-hosts"
                    && zone != "default-nameservers"
                    && !zones.iter().any(|z| z == zone)
                {
                    zones.push(zone.to_string());
                }
            }
        }
    }
    zones.sort();
    zones.dedup();
    Ok(zones)
}

pub fn zone_path(name: &str) -> Result<PathBuf, String> {
    let zone = safe_zone_name(name)?;
    Ok(ensure_dns_root()?.join(format!("{zone}.zone")))
}

fn zone_json_path(name: &str) -> Result<PathBuf, String> {
    let zone = safe_zone_name(name)?;
    Ok(ensure_dns_root()?.join(format!("{zone}.json")))
}

pub fn read_zone(name: &str) -> Result<String, String> {
    let path = zone_path(name)?;
    fs::read_to_string(&path).map_err(|e| format!("Cannot read zone: {e}"))
}

pub fn write_zone(name: &str, content: &str) -> Result<(), String> {
    let path = zone_path(name)?;
    if content.len() > 256 * 1024 {
        return Err("Zone file too large".into());
    }
    let records = parse_zone_file(name, content)?;
    fs::write(&path, content).map_err(|e| format!("Cannot write zone: {e}"))?;
    save_zone_records_json(name, &records)?;
    Ok(())
}

pub fn delete_zone(name: &str) -> Result<(), String> {
    let path = zone_path(name)?;
    if path.is_file() {
        fs::remove_file(&path).map_err(|e| format!("Cannot delete zone: {e}"))?;
    }
    let json = zone_json_path(name)?;
    if json.is_file() {
        let _ = fs::remove_file(&json);
    }
    Ok(())
}

pub fn load_zone_records(name: &str) -> Result<Vec<DnsRecord>, String> {
    let zone = safe_zone_name(name)?;
    let json_path = zone_json_path(&zone)?;
    if json_path.is_file() {
        let raw =
            fs::read_to_string(&json_path).map_err(|e| format!("Cannot read records: {e}"))?;
        let records: Vec<DnsRecord> = serde_json::from_str(&raw)
            .map_err(|e| format!("Cannot parse zone records JSON: {e}"))?;
        return Ok(records);
    }
    let zone_file = zone_path(&zone)?;
    if zone_file.is_file() {
        let content =
            fs::read_to_string(&zone_file).map_err(|e| format!("Cannot read zone: {e}"))?;
        return parse_zone_file(&zone, &content);
    }
    Err("Zone not found".into())
}

fn save_zone_records_json(name: &str, records: &[DnsRecord]) -> Result<(), String> {
    let zone = safe_zone_name(name)?;
    ensure_dns_root()?;
    let json_path = zone_json_path(&zone)?;
    let raw =
        serde_json::to_string_pretty(records).map_err(|e| format!("Cannot encode records: {e}"))?;
    fs::write(&json_path, raw).map_err(|e| format!("Cannot save records: {e}"))
}

pub fn save_zone_records(name: &str, records: &[DnsRecord]) -> Result<(), String> {
    let zone = safe_zone_name(name)?;
    for rec in records {
        validate_record(rec)?;
    }
    save_zone_records_json(&zone, records)?;
    let zone_body = serialize_zone_file(&zone, records);
    fs::write(zone_path(&zone)?, zone_body).map_err(|e| format!("Cannot write zone: {e}"))?;
    Ok(())
}

fn new_record_id() -> String {
    format!("r{:x}{:04x}", now_unix(), (now_unix() % 65535) as u16)
}

/// Create a new zone seeded with SOA, default NS, and optional apex A.
pub fn create_zone(domain_raw: &str, host_ipv4: Option<&str>) -> Result<String, String> {
    let zone = normalize_domain_input(domain_raw)?;
    if zone_path(&zone)?.is_file() || zone_json_path(&zone)?.is_file() {
        return Err(format!("Zone {zone} already exists"));
    }
    let defaults = load_default_nameservers();
    if defaults.is_empty() {
        return Err(
            "Configure Default Nameservers before creating a zone (Server > Default Nameservers)"
                .into(),
        );
    }
    let serial = {
        let t = now_unix();
        let day = ((t / 86400) as u32).saturating_mul(100);
        day + ((t % 86400) / 100) as u32
    };
    let primary = defaults[0].trim_end_matches('.');
    let mut records = Vec::new();
    records.push(DnsRecord {
        id: new_record_id(),
        name: "@".into(),
        rtype: "SOA".into(),
        ttl: 86400,
        priority: None,
        weight: None,
        port: None,
        content: format!("{primary}. hostmaster.{zone}. {serial} 3600 600 1209600 3600"),
    });
    for ns in &defaults {
        let ns_fqdn = ns.trim_end_matches('.');
        records.push(DnsRecord {
            id: new_record_id(),
            name: "@".into(),
            rtype: "NS".into(),
            ttl: 86400,
            priority: None,
            weight: None,
            port: None,
            content: format!("{ns_fqdn}."),
        });
    }
    if let Some(ip) = host_ipv4.filter(|v| !v.is_empty() && *v != "Unavailable") {
        if validate_ipv4(ip).is_ok() {
            records.push(DnsRecord {
                id: new_record_id(),
                name: "@".into(),
                rtype: "A".into(),
                ttl: 3600,
                priority: None,
                weight: None,
                port: None,
                content: ip.to_string(),
            });
        }
    }
    let hosts = load_ns_hosts();
    for ns in &defaults {
        let host = ns.trim_end_matches('.').to_ascii_lowercase();
        let in_zone = host == zone || host.ends_with(&format!(".{zone}"));
        if !in_zone {
            continue;
        }
        if let Some(h) = hosts
            .iter()
            .find(|h| h.hostname.eq_ignore_ascii_case(&host))
        {
            let label = relative_ns_label(&zone, &host);
            if let Some(ref v4) = h.ipv4 {
                records.push(DnsRecord {
                    id: new_record_id(),
                    name: label.clone(),
                    rtype: "A".into(),
                    ttl: 3600,
                    priority: None,
                    weight: None,
                    port: None,
                    content: v4.clone(),
                });
            }
            if let Some(ref v6) = h.ipv6 {
                records.push(DnsRecord {
                    id: new_record_id(),
                    name: label,
                    rtype: "AAAA".into(),
                    ttl: 3600,
                    priority: None,
                    weight: None,
                    port: None,
                    content: v6.clone(),
                });
            }
        }
    }
    save_zone_records(&zone, &records)?;
    Ok(zone)
}

fn relative_ns_label(zone: &str, ns_host: &str) -> String {
    let host = ns_host.trim_end_matches('.').to_ascii_lowercase();
    if let Some(rest) = host.strip_suffix(&format!(".{zone}")) {
        return rest.to_string();
    }
    if host == zone {
        return "@".into();
    }
    host
}

pub fn add_zone_record(zone: &str, mut rec: DnsRecord) -> Result<(), String> {
    validate_record(&rec)?;
    let mut records = load_zone_records(zone)?;
    if rec.id.trim().is_empty() {
        rec.id = new_record_id();
    }
    records.push(rec);
    save_zone_records(zone, &records)
}

pub fn update_zone_record(zone: &str, rec: DnsRecord) -> Result<(), String> {
    validate_record(&rec)?;
    let mut records = load_zone_records(zone)?;
    let Some(idx) = records.iter().position(|r| r.id == rec.id) else {
        return Err("Record not found".into());
    };
    records[idx] = rec;
    save_zone_records(zone, &records)
}

pub fn delete_zone_record(zone: &str, id: &str) -> Result<(), String> {
    let mut records = load_zone_records(zone)?;
    let before = records.len();
    records.retain(|r| r.id != id);
    if records.len() == before {
        return Err("Record not found".into());
    }
    save_zone_records(zone, &records)
}

pub fn nameservers_path() -> PathBuf {
    join_data("dns/nameservers.json")
}

pub fn save_nameservers(values: &[String]) -> Result<(), String> {
    save_default_nameservers(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn zone_crud_roundtrip() {
        with_test_data_dir(|| {
            write_zone("example.com", "example.com. IN A 127.0.0.1\n").unwrap();
            assert_eq!(list_zones().unwrap(), vec!["example.com".to_string()]);
            assert!(read_zone("example.com").unwrap().contains("127.0.0.1"));
            delete_zone("example.com").unwrap();
            assert!(list_zones().unwrap().is_empty());
        });
    }

    #[test]
    fn rejects_bad_zone_name() {
        assert!(safe_zone_name("../etc").is_err());
        assert!(safe_zone_name("bad name").is_err());
    }

    #[test]
    fn normalize_strips_www_and_scheme() {
        assert_eq!(
            normalize_domain_input("https://www.Example.com/path").unwrap(),
            "example.com"
        );
    }

    #[test]
    fn create_zone_seeds_soa_and_ns() {
        with_test_data_dir(|| {
            save_default_nameservers(&["ns1.example.com".into(), "ns2.example.com".into()])
                .unwrap();
            save_ns_hosts(&[NsHost {
                hostname: "ns1.example.com".into(),
                ipv4: Some("203.0.113.10".into()),
                ipv6: None,
            }])
            .unwrap();
            let zone = create_zone("example.com", Some("203.0.113.50")).unwrap();
            assert_eq!(zone, "example.com");
            let recs = load_zone_records("example.com").unwrap();
            assert!(recs.iter().any(|r| r.rtype == "SOA"));
            assert!(recs.iter().filter(|r| r.rtype == "NS").count() >= 2);
            assert!(
                recs.iter()
                    .any(|r| r.rtype == "A" && r.content == "203.0.113.50")
            );
        });
    }

    #[test]
    fn csrf_roundtrip() {
        unsafe {
            std::env::set_var("CPN_PANEL_SESSION_SECRET", "dns-csrf-test-secret-fixed");
        }
        let t = dns_csrf_token("admin");
        assert!(verify_dns_csrf("admin", &t));
        assert!(!verify_dns_csrf("other", &t));
        unsafe {
            std::env::remove_var("CPN_PANEL_SESSION_SECRET");
        }
    }
}
