//! Extra panel hostnames (aliases) for the same install.
//!
//! Primary public URL stays `panel_public_url` / `panel_hostname` (emails, MOTD,
//! phone-home). Aliases are additional Host/Origin entries so reverse-proxy
//! hostnames can reach the same panel without being treated as foreign sites.
//!
//! WebAuthn passkeys remain bound to the RP ID of the hostname used at enroll
//! time. Unrelated domains (for example `panel.example.com` vs a Contabo
//! `*.contaboserver.net` name) cannot share one passkey; enroll per hostname
//! or use one primary HTTPS hostname for passkeys. TOTP works on every alias
//! that hits this install (session cookies stay host-scoped).

use crate::account::data_dir;
use crate::panel_network::validate_panel_hostname;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use url::Url;

const MAX_ALIASES: usize = 16;
const ALIASES_FILE: &str = "panel_host_aliases.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AliasStore {
    #[serde(default)]
    hosts: Vec<String>,
}

fn aliases_path() -> PathBuf {
    data_dir().join(ALIASES_FILE)
}

fn write_mode_600(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Could not create {}: {err}", parent.display()))?;
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
        .map_err(|err| format!("Could not write {}: {err}", path.display()))?;
    file.write_all(bytes)
        .map_err(|err| format!("Could not save {}: {err}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn load_store() -> AliasStore {
    let Ok(raw) = fs::read_to_string(aliases_path()) else {
        return AliasStore::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn save_store(store: &AliasStore) -> Result<(), String> {
    let json = serde_json::to_string_pretty(store)
        .map_err(|err| format!("Could not serialize host aliases: {err}"))?;
    write_mode_600(&aliases_path(), format!("{json}\n").as_bytes())
}

/// Sorted unique validated alias hostnames (no scheme/port).
pub fn load_panel_host_aliases() -> Vec<String> {
    let mut hosts: Vec<String> = load_store()
        .hosts
        .into_iter()
        .filter_map(|h| validate_panel_hostname(&h).ok())
        .collect();
    hosts.sort();
    hosts.dedup();
    hosts.into_iter().take(MAX_ALIASES).collect()
}

pub fn add_panel_host_alias(raw: &str) -> Result<String, String> {
    let host = validate_panel_hostname(raw)?;
    if let Some(primary) = crate::panel_network::load_panel_hostname()
        && primary.eq_ignore_ascii_case(&host)
    {
        return Err(
            "That hostname is already the primary panel_hostname; aliases are for extra names only"
                .into(),
        );
    }
    let mut store = load_store();
    let mut seen: std::collections::BTreeSet<String> = store
        .hosts
        .iter()
        .filter_map(|h| validate_panel_hostname(h).ok())
        .collect();
    if seen.contains(&host) {
        return Ok(host);
    }
    if seen.len() >= MAX_ALIASES {
        return Err(format!(
            "At most {MAX_ALIASES} panel host aliases are allowed"
        ));
    }
    seen.insert(host.clone());
    store.hosts = seen.into_iter().collect();
    save_store(&store)?;
    Ok(host)
}

pub fn remove_panel_host_alias(raw: &str) -> Result<bool, String> {
    let host = validate_panel_hostname(raw)?;
    let mut store = load_store();
    let before = store.hosts.len();
    store.hosts.retain(|h| {
        validate_panel_hostname(h)
            .map(|v| !v.eq_ignore_ascii_case(&host))
            .unwrap_or(true)
    });
    if store.hosts.len() == before {
        return Ok(false);
    }
    save_store(&store)?;
    Ok(true)
}

pub fn clear_panel_host_aliases() -> Result<(), String> {
    let path = aliases_path();
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|err| format!("Could not clear panel host aliases: {err}"))?;
    }
    Ok(())
}

/// HTTPS origins for WebAuthn allowlist extras (alias hostnames on 443).
pub fn alias_https_origins() -> Vec<Url> {
    load_panel_host_aliases()
        .into_iter()
        .filter_map(|host| Url::parse(&format!("https://{host}/")).ok())
        .collect()
}

/// Authorities (`host` or `host:port`) for CSRF / Origin allowlist merges.
pub fn alias_authorities_for_allowlist(default_port: u16) -> Vec<String> {
    let mut out = Vec::new();
    for host in load_panel_host_aliases() {
        // TLS front on 443 usually omits the port in Host / Origin.
        out.push(host.clone());
        out.push(format!("{host}:443"));
        if default_port != 443 && default_port != 80 {
            out.push(format!("{host}:{default_port}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::panel_network::{clear_panel_hostname, save_panel_hostname};

    #[test]
    fn aliases_add_list_remove() {
        with_test_data_dir(|| {
            assert!(load_panel_host_aliases().is_empty());
            add_panel_host_alias("backup.example.com").unwrap();
            add_panel_host_alias("BACKUP.example.com").unwrap();
            let list = load_panel_host_aliases();
            assert_eq!(list, vec!["backup.example.com".to_string()]);
            assert!(remove_panel_host_alias("backup.example.com").unwrap());
            assert!(!remove_panel_host_alias("backup.example.com").unwrap());
            assert!(load_panel_host_aliases().is_empty());
        });
    }

    #[test]
    fn rejects_primary_hostname_as_alias() {
        with_test_data_dir(|| {
            save_panel_hostname("panel.example.com").unwrap();
            let err = add_panel_host_alias("panel.example.com").unwrap_err();
            assert!(err.contains("primary"), "{err}");
            clear_panel_hostname().unwrap();
        });
    }

    #[test]
    fn alias_origins_are_https() {
        with_test_data_dir(|| {
            add_panel_host_alias("vmi.example.net").unwrap();
            let origins = alias_https_origins();
            assert_eq!(origins.len(), 1);
            assert_eq!(origins[0].as_str(), "https://vmi.example.net/");
        });
    }
}
