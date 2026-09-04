//! Simple DNS zone file store under CPN data dir (PowerDNS optional later).

use crate::paths::join_data;
use std::fs;
use std::path::PathBuf;

pub fn dns_root() -> PathBuf {
    join_data("dns")
}

pub fn ensure_dns_root() -> Result<PathBuf, String> {
    let root = dns_root();
    fs::create_dir_all(&root).map_err(|e| format!("Cannot create DNS dir: {e}"))?;
    Ok(root)
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
    if name.contains("..") || name.starts_with('.') || name.ends_with('.') {
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
            }
        }
    }
    zones.sort();
    Ok(zones)
}

pub fn zone_path(name: &str) -> Result<PathBuf, String> {
    let zone = safe_zone_name(name)?;
    Ok(ensure_dns_root()?.join(format!("{zone}.zone")))
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
    fs::write(&path, content).map_err(|e| format!("Cannot write zone: {e}"))
}

pub fn delete_zone(name: &str) -> Result<(), String> {
    let path = zone_path(name)?;
    if path.is_file() {
        fs::remove_file(&path).map_err(|e| format!("Cannot delete zone: {e}"))?;
    }
    Ok(())
}

pub fn nameservers_path() -> PathBuf {
    join_data("dns/nameservers.json")
}

pub fn load_nameservers() -> Vec<String> {
    let path = nameservers_path();
    let Ok(raw) = fs::read_to_string(path) else {
        return vec![];
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save_nameservers(values: &[String]) -> Result<(), String> {
    ensure_dns_root()?;
    let path = nameservers_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Cannot create DNS dir: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(values)
        .map_err(|e| format!("Cannot encode nameservers: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("Cannot save nameservers: {e}"))
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
}
