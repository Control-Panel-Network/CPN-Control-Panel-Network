//! Optional external panel base URL for emails and browser links (NAT labs, reverse proxies).

use crate::account::data_dir;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

fn public_url_path() -> PathBuf {
    data_dir().join("panel_public_url")
}

fn write_mode_600(path: &PathBuf, contents: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Could not create data directory {}: {error}",
                parent.display()
            )
        })?;
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
        .map_err(|error| format!("Could not write {}: {error}", path.display()))?;
    file.write_all(contents)
        .map_err(|error| format!("Could not save {}: {error}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn normalize_base(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

/// Validate a full panel base URL (scheme + host, optional port; no path required).
pub fn validate_panel_public_url(raw: &str) -> Result<String, String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err("Panel public URL cannot be empty".into());
    }
    if value.len() > 512 {
        return Err("Panel public URL is too long (max 512)".into());
    }
    if value.contains(char::is_whitespace) || value.contains('<') || value.contains('>') {
        return Err("Panel public URL contains invalid characters".into());
    }
    let lower = value.to_ascii_lowercase();
    let rest = if let Some(r) = lower.strip_prefix("https://") {
        r
    } else if let Some(r) = lower.strip_prefix("http://") {
        r
    } else {
        return Err(
            "Panel public URL must start with http:// or https:// (example: http://127.0.0.1:2089)"
                .into(),
        );
    };
    if rest.is_empty() || rest.starts_with('/') {
        return Err("Panel public URL needs a host after the scheme".into());
    }
    if rest.contains('/') || rest.contains('?') || rest.contains('#') {
        return Err(
            "Panel public URL must be scheme + host[:port] only (no path, query, or fragment)"
                .into(),
        );
    }
    // Preserve original scheme casing as lowercase; keep host as typed (IPv6 etc.).
    let scheme = if lower.starts_with("https://") {
        "https"
    } else {
        "http"
    };
    let host_port = value
        .split_once("://")
        .map(|(_, hp)| hp.trim().trim_end_matches('/'))
        .unwrap_or("")
        .to_string();
    if host_port.is_empty() {
        return Err("Panel public URL needs a host after the scheme".into());
    }
    Ok(format!("{scheme}://{host_port}"))
}

pub fn load_panel_public_url() -> Option<String> {
    let raw = fs::read_to_string(public_url_path()).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    validate_panel_public_url(trimmed).ok()
}

pub fn save_panel_public_url(url: &str) -> Result<(), String> {
    let url = validate_panel_public_url(url)?;
    write_mode_600(&public_url_path(), format!("{url}\n").as_bytes())
}

pub fn clear_panel_public_url() -> Result<(), String> {
    let path = public_url_path();
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|error| format!("Could not clear panel public URL: {error}"))?;
    }
    Ok(())
}

/// Loopback listen URL for the guest process (not the host NAT mapped port).
pub fn local_listen_base_url(listen_port: u16) -> String {
    if listen_port == 443 {
        "https://127.0.0.1".into()
    } else if listen_port == 80 {
        "http://127.0.0.1".into()
    } else {
        format!("http://127.0.0.1:{listen_port}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelEmailBases {
    pub primary: String,
    pub alternates: Vec<String>,
}

/// Resolve email link bases: external public URL, then hostname HTTPS, then loopback.
/// When primary differs from hostname HTTPS and/or loopback, those are listed as alternates.
pub fn panel_email_bases(listen_port: u16, host_hint: Option<&str>) -> PanelEmailBases {
    let primary = crate::panel_network::public_base_url(listen_port, host_hint);
    let primary_norm = normalize_base(&primary);
    let mut alternates = Vec::new();

    if let Some(hostname) = crate::panel_network::load_panel_hostname() {
        let https = format!("https://{hostname}");
        if normalize_base(&https) != primary_norm {
            alternates.push(https);
        }
    }

    let local = local_listen_base_url(listen_port);
    if normalize_base(&local) != primary_norm
        && !alternates
            .iter()
            .any(|existing| normalize_base(existing) == normalize_base(&local))
    {
        alternates.push(local);
    }

    PanelEmailBases {
        primary,
        alternates,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::panel_network::{clear_panel_hostname, save_panel_hostname};

    #[test]
    fn validates_public_url() {
        assert_eq!(
            validate_panel_public_url("http://127.0.0.1:2089/").unwrap(),
            "http://127.0.0.1:2089"
        );
        assert_eq!(
            validate_panel_public_url("HTTPS://panel.example.com").unwrap(),
            "https://panel.example.com"
        );
        assert!(validate_panel_public_url("panel.example.com").is_err());
        assert!(validate_panel_public_url("http://panel.example.com/login").is_err());
        assert!(validate_panel_public_url("ftp://127.0.0.1:2089").is_err());
    }

    #[test]
    fn email_bases_prefer_public_url_and_list_alternates() {
        with_test_data_dir(|| {
            save_panel_hostname("test2.newstargeted.com").unwrap();
            save_panel_public_url("http://127.0.0.1:2089").unwrap();
            let bases = panel_email_bases(2087, Some("10.0.2.15"));
            assert_eq!(bases.primary, "http://127.0.0.1:2089");
            assert!(
                bases
                    .alternates
                    .iter()
                    .any(|u| u == "https://test2.newstargeted.com")
            );
            assert!(
                bases
                    .alternates
                    .iter()
                    .any(|u| u == "http://127.0.0.1:2087")
            );
            clear_panel_public_url().unwrap();
            clear_panel_hostname().unwrap();
        });
    }

    #[test]
    fn email_bases_without_public_url_use_hostname_primary() {
        with_test_data_dir(|| {
            save_panel_hostname("panel.example.com").unwrap();
            let bases = panel_email_bases(2087, None);
            assert_eq!(bases.primary, "https://panel.example.com");
            assert_eq!(bases.alternates, vec!["http://127.0.0.1:2087".to_string()]);
            clear_panel_hostname().unwrap();
        });
    }
}
