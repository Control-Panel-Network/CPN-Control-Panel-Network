//! Per-domain SSL provider model for CPN.
//!
//! Providers are stored on each site/subdomain independently. There is no
//! account-level switch that rewrites all domains. Subdomains may inherit the
//! parent provider as an **initial** value only.

use crate::paths;
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SslProvider {
    #[default]
    LetsEncrypt,
    ZeroSsl,
    CloudflareCa,
    Custom,
    None,
}

impl SslProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LetsEncrypt => "letsencrypt",
            Self::ZeroSsl => "zerossl",
            Self::CloudflareCa => "cloudflare_ca",
            Self::Custom => "custom",
            Self::None => "none",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::LetsEncrypt => "Let's Encrypt",
            Self::ZeroSsl => "ZeroSSL",
            Self::CloudflareCa => "Cloudflare CA",
            Self::Custom => "Custom SSL",
            Self::None => "None",
        }
    }

    /// Parse CLI/UI values. Unknown values return Err (do not silently coerce).
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "letsencrypt" | "lets-encrypt" | "le" | "acme" => Ok(Self::LetsEncrypt),
            "zerossl" | "zero-ssl" | "zero_ssl" => Ok(Self::ZeroSsl),
            "cloudflare_ca" | "cloudflare-ca" | "cloudflare" | "cf_ca" | "origin_ca" => {
                Ok(Self::CloudflareCa)
            }
            "custom" | "manual" | "uploaded" => Ok(Self::Custom),
            "none" | "off" | "disabled" => Ok(Self::None),
            other => Err(format!(
                "Unknown SSL provider '{other}' (use letsencrypt|zerossl|cloudflare_ca|custom|none)"
            )),
        }
    }

    pub fn all() -> &'static [SslProvider] {
        &[
            Self::LetsEncrypt,
            Self::ZeroSsl,
            Self::CloudflareCa,
            Self::Custom,
            Self::None,
        ]
    }

    /// Providers that support automatic issue/renew from the panel.
    pub fn supports_auto_issue(self) -> bool {
        matches!(self, Self::LetsEncrypt | Self::ZeroSsl | Self::CloudflareCa)
    }

    /// True when this domain should never have certs issued or renewed by CPN.
    pub fn is_none(self) -> bool {
        matches!(self, Self::None)
    }

    pub fn is_custom(self) -> bool {
        matches!(self, Self::Custom)
    }
}

/// Per-domain SSL settings embedded on `SiteRecord` (schema >= 2).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SiteSslSettings {
    pub provider: SslProvider,
    /// When true on an apex (or cert owner), issue one SAN cert covering listed children
    /// that share the same provider and are not Custom/None.
    #[serde(default)]
    pub include_subdomains_on_cert: bool,
    /// If this domain is covered by another domain's shared cert, that owner FQDN.
    #[serde(default)]
    pub shared_cert_owner: Option<String>,
    #[serde(default)]
    pub last_issue_unix: u64,
    #[serde(default)]
    pub last_error: String,
    /// Relative or absolute path to uploaded fullchain (Custom only).
    #[serde(default)]
    pub custom_cert_path: Option<String>,
    /// Relative or absolute path to uploaded private key (Custom only). Mode 600.
    #[serde(default)]
    pub custom_key_path: Option<String>,
    /// When Cloudflare proxy is on and provider is LE / Cloudflare CA, install origin material.
    #[serde(default)]
    pub install_origin_cert: bool,
}

impl Default for SiteSslSettings {
    fn default() -> Self {
        Self {
            provider: SslProvider::LetsEncrypt,
            include_subdomains_on_cert: false,
            shared_cert_owner: None,
            last_issue_unix: 0,
            last_error: String::new(),
            custom_cert_path: None,
            custom_key_path: None,
            install_origin_cert: true,
        }
    }
}

impl SiteSslSettings {
    pub fn with_provider(provider: SslProvider) -> Self {
        Self {
            provider,
            ..Self::default()
        }
    }
}

/// Installer / panel default for **newly created** sites only (not an account-wide rewrite).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SslDefaults {
    pub schema_version: u32,
    pub default_provider: SslProvider,
    pub updated_at_unix: u64,
}

impl Default for SslDefaults {
    fn default() -> Self {
        Self {
            schema_version: 1,
            default_provider: SslProvider::LetsEncrypt,
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

pub fn ssl_defaults_path() -> PathBuf {
    paths::join_data("ssl-defaults.json")
}

pub fn load_ssl_defaults() -> SslDefaults {
    let Ok(raw) = fs::read_to_string(ssl_defaults_path()) else {
        return SslDefaults::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save_ssl_defaults(provider: SslProvider) -> Result<(), String> {
    let dir = paths::default_data_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create data dir: {e}"))?;
    let body = SslDefaults {
        schema_version: 1,
        default_provider: provider,
        updated_at_unix: now_unix(),
    };
    let json = serde_json::to_string_pretty(&body).map_err(|e| e.to_string())?;
    let path = ssl_defaults_path();
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
        .map_err(|e| format!("Cannot save SSL defaults: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Material paths for a domain under `/var/lib/cpn/ssl/<domain>/`.
pub fn ssl_material_dir(domain: &str) -> PathBuf {
    paths::join_data(format!("ssl/{domain}"))
}

pub fn custom_cert_paths(domain: &str) -> (PathBuf, PathBuf) {
    let dir = ssl_material_dir(domain);
    (dir.join("fullchain.pem"), dir.join("privkey.pem"))
}

/// Resolve initial provider for a new site: explicit override, else parent (subdomain), else defaults.
pub fn initial_provider_for_new_site(
    explicit: Option<SslProvider>,
    parent_provider: Option<SslProvider>,
) -> SslProvider {
    if let Some(p) = explicit {
        return p;
    }
    if let Some(p) = parent_provider {
        return p;
    }
    load_ssl_defaults().default_provider
}

/// Domains that can share one SAN cert with `owner` (same provider, not Custom/None, opt-in).
pub fn san_member_domains(
    owner_domain: &str,
    owner_provider: SslProvider,
    include_subdomains: bool,
    children: &[(String, SslProvider)],
) -> Vec<String> {
    let mut names = vec![owner_domain.to_string()];
    if !include_subdomains || !owner_provider.supports_auto_issue() {
        return names;
    }
    for (child, prov) in children {
        if *prov == owner_provider {
            names.push(child.clone());
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn parse_all_providers() {
        assert_eq!(
            SslProvider::parse("letsencrypt").unwrap(),
            SslProvider::LetsEncrypt
        );
        assert_eq!(SslProvider::parse("zerossl").unwrap(), SslProvider::ZeroSsl);
        assert_eq!(
            SslProvider::parse("cloudflare_ca").unwrap(),
            SslProvider::CloudflareCa
        );
        assert_eq!(SslProvider::parse("custom").unwrap(), SslProvider::Custom);
        assert_eq!(SslProvider::parse("none").unwrap(), SslProvider::None);
        assert!(SslProvider::parse("bogus").is_err());
    }

    #[test]
    fn none_and_custom_skip_auto() {
        assert!(!SslProvider::None.supports_auto_issue());
        assert!(!SslProvider::Custom.supports_auto_issue());
        assert!(SslProvider::LetsEncrypt.supports_auto_issue());
        assert!(SslProvider::ZeroSsl.supports_auto_issue());
        assert!(SslProvider::CloudflareCa.supports_auto_issue());
    }

    #[test]
    fn initial_provider_isolation() {
        with_test_data_dir(|| {
            save_ssl_defaults(SslProvider::ZeroSsl).unwrap();
            assert_eq!(
                initial_provider_for_new_site(None, None),
                SslProvider::ZeroSsl
            );
            assert_eq!(
                initial_provider_for_new_site(Some(SslProvider::None), Some(SslProvider::ZeroSsl)),
                SslProvider::None
            );
            assert_eq!(
                initial_provider_for_new_site(None, Some(SslProvider::Custom)),
                SslProvider::Custom
            );
        });
    }

    #[test]
    fn san_excludes_diverged_children() {
        let kids = vec![
            ("a.example.com".into(), SslProvider::LetsEncrypt),
            ("b.example.com".into(), SslProvider::Custom),
            ("c.example.com".into(), SslProvider::None),
            ("d.example.com".into(), SslProvider::LetsEncrypt),
        ];
        let names = san_member_domains("example.com", SslProvider::LetsEncrypt, true, &kids);
        assert_eq!(
            names,
            vec![
                "example.com".to_string(),
                "a.example.com".to_string(),
                "d.example.com".to_string()
            ]
        );
        let solo = san_member_domains("example.com", SslProvider::LetsEncrypt, false, &kids);
        assert_eq!(solo, vec!["example.com".to_string()]);
    }

    #[test]
    fn changing_child_leaves_shared_cert() {
        // Documented behavior: Custom/None children are not SAN members.
        let kids = vec![("blog.example.com".into(), SslProvider::Custom)];
        let names = san_member_domains("example.com", SslProvider::LetsEncrypt, true, &kids);
        assert_eq!(names, vec!["example.com".to_string()]);
    }
}
