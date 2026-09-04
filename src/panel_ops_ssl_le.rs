//! Per-domain SSL issue / renew / upload for CPN.
//!
//! Respects each site's `ssl.provider`. Does **not** force Let's Encrypt onto
//! domains set to None or Custom. Shared SAN certs only include children that
//! share the same auto provider and when the owner opts in.

use crate::panel_ops_cloudflare::cloudflare_configured;
use crate::panel_ops_ssl_provider::{
    SiteSslSettings, SslProvider, custom_cert_paths, san_member_domains, ssl_material_dir,
};
use crate::paths;
use crate::sites::{SiteModify, SiteRecord, list_sites, load_site, modify_site, normalize_domain};
use crate::website_preview::ssl_material_present;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct SslStatusRow {
    pub domain: String,
    pub provider: String,
    pub provider_label: String,
    pub has_cert: bool,
    pub certbot: bool,
    pub auto_issue: bool,
    pub include_subdomains_on_cert: bool,
    pub shared_cert_owner: Option<String>,
    pub last_error: String,
    pub needs_issue: bool,
}

pub fn certbot_available() -> bool {
    Command::new("certbot")
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn cloudflare_dns_plugin_available() -> bool {
    Path::new("/usr/bin/certbot-dns-cloudflare").exists()
        || Command::new("certbot")
            .args(["plugins"])
            .output()
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .to_ascii_lowercase()
                    .contains("dns-cloudflare")
            })
            .unwrap_or(false)
}

fn child_provider_pairs(parent: &str) -> Vec<(String, SslProvider)> {
    list_sites()
        .unwrap_or_default()
        .into_iter()
        .filter(|s| {
            crate::sites::resolve_parent_domain(&s.domain)
                .ok()
                .flatten()
                .as_deref()
                == Some(parent)
        })
        .map(|s| (s.domain, s.ssl.provider))
        .collect()
}

pub fn ssl_status_for_domain(domain: &str) -> SslStatusRow {
    let site = load_site(domain).ok();
    let ssl = site.as_ref().map(|s| s.ssl.clone()).unwrap_or_default();
    let has = ssl_material_present(domain)
        || ssl
            .custom_cert_path
            .as_ref()
            .map(|p| Path::new(p).is_file())
            .unwrap_or(false);
    let certbot = certbot_available();
    let needs_issue = ssl.provider.supports_auto_issue() && !has;
    SslStatusRow {
        domain: domain.to_string(),
        provider: ssl.provider.as_str().to_string(),
        provider_label: ssl.provider.label().to_string(),
        has_cert: has,
        certbot,
        auto_issue: ssl.provider.supports_auto_issue(),
        include_subdomains_on_cert: ssl.include_subdomains_on_cert,
        shared_cert_owner: ssl.shared_cert_owner,
        last_error: ssl.last_error,
        needs_issue,
    }
}

pub fn ssl_status_all_sites() -> Vec<SslStatusRow> {
    list_sites()
        .unwrap_or_default()
        .into_iter()
        .map(|s| ssl_status_for_domain(&s.domain))
        .collect()
}

fn persist_ssl_error(domain: &str, err: &str) -> Result<(), String> {
    let site = load_site(domain)?;
    let mut ssl = site.ssl;
    ssl.last_error = err.to_string();
    modify_site(
        domain,
        SiteModify {
            ssl: Some(ssl),
            ..SiteModify::default()
        },
    )?;
    Ok(())
}

fn persist_ssl_ok(domain: &str, ssl: SiteSslSettings) -> Result<(), String> {
    modify_site(
        domain,
        SiteModify {
            ssl: Some(ssl),
            ..SiteModify::default()
        },
    )?;
    Ok(())
}

pub fn set_domain_provider(domain: &str, provider: SslProvider) -> Result<String, String> {
    let domain = normalize_domain(domain)?;
    let mut site = load_site(&domain)?;
    let prev = site.ssl.provider;
    site.ssl.provider = provider;
    if provider.is_custom() || provider.is_none() {
        site.ssl.shared_cert_owner = None;
        site.ssl.include_subdomains_on_cert = false;
    }
    site.ssl.last_error.clear();
    persist_ssl_ok(&domain, site.ssl)?;
    Ok(format!(
        "`{domain}` SSL provider: {} -> {} (siblings unchanged)",
        prev.label(),
        provider.label()
    ))
}

pub fn set_include_subdomains(domain: &str, include: bool) -> Result<String, String> {
    let domain = normalize_domain(domain)?;
    let mut site = load_site(&domain)?;
    site.ssl.include_subdomains_on_cert = include;
    persist_ssl_ok(&domain, site.ssl)?;
    Ok(format!(
        "`{domain}` include-subdomains-on-cert set to {include}"
    ))
}

fn write_cloudflare_ini() -> Result<std::path::PathBuf, String> {
    let settings = crate::panel_ops_cloudflare::load_cloudflare();
    if settings.api_token.trim().is_empty() {
        return Err(
            "Cloudflare API token required (configure under Cloudflare DNS API Settings)".into(),
        );
    }
    let dir = paths::join_data("ssl");
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create SSL dir: {e}"))?;
    let path = dir.join("cloudflare.ini");
    let body = format!("dns_cloudflare_api_token = {}\n", settings.api_token.trim());
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true).mode(0o600);
        use std::io::Write;
        let mut f = options
            .open(&path)
            .map_err(|e| format!("Cannot write cloudflare.ini: {e}"))?;
        f.write_all(body.as_bytes())
            .map_err(|e| format!("Cannot write cloudflare.ini: {e}"))?;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        fs::write(&path, body).map_err(|e| format!("Cannot write cloudflare.ini: {e}"))?;
    }
    Ok(path)
}

fn zerossl_eab_path() -> std::path::PathBuf {
    paths::join_data("ssl/zerossl-eab.json")
}

/// Optional ZeroSSL EAB credentials (kid + hmac) in secure prefs; never commit secrets.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Default)]
pub struct ZeroSslEab {
    pub kid: String,
    pub hmac_key: String,
}

pub fn load_zerossl_eab() -> Option<ZeroSslEab> {
    let raw = fs::read_to_string(zerossl_eab_path()).ok()?;
    serde_json::from_str(&raw).ok()
}

fn run_certbot(args: &[&str]) -> Result<String, String> {
    if !certbot_available() {
        return Err("certbot was not found on PATH. Install ACME certbot, then retry.".into());
    }
    let output = Command::new("certbot")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run certbot: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");
    if !output.status.success() {
        let hint = if combined.to_ascii_lowercase().contains("rate limit")
            || combined.to_ascii_lowercase().contains("too many")
        {
            " ACME rate limit likely hit; wait and retry."
        } else {
            ""
        };
        let short: String = combined.chars().take(400).collect();
        return Err(format!("certbot failed.{hint} {short}"));
    }
    Ok(combined)
}

fn names_for_issue(site: &SiteRecord) -> Vec<String> {
    let kids = child_provider_pairs(&site.domain);
    san_member_domains(
        &site.domain,
        site.ssl.provider,
        site.ssl.include_subdomains_on_cert,
        &kids,
    )
}

/// Issue or renew according to the domain's own provider setting.
pub fn issue_or_renew(domain: &str) -> Result<String, String> {
    let domain = normalize_domain(domain)?;
    let site = load_site(&domain)?;
    match site.ssl.provider {
        SslProvider::None => Err(format!(
            "`{domain}` SSL provider is None: CPN will not issue, install, or auto-renew"
        )),
        SslProvider::Custom => Err(format!(
            "`{domain}` uses Custom SSL. Upload cert/key (no auto-renew) or switch provider first"
        )),
        SslProvider::LetsEncrypt => issue_acme(&site, "letsencrypt"),
        SslProvider::ZeroSsl => issue_acme(&site, "zerossl"),
        SslProvider::CloudflareCa => issue_cloudflare_ca(&site),
    }
}

fn issue_acme(site: &SiteRecord, server_kind: &str) -> Result<String, String> {
    let domain = site.domain.clone();
    let names = names_for_issue(site);
    let mut args: Vec<String> = vec![
        "certonly".into(),
        "--non-interactive".into(),
        "--agree-tos".into(),
        "--register-unsafely-without-email".into(),
    ];
    if server_kind == "zerossl" {
        // Prefer ZeroSSL ACME directory when EAB is configured; otherwise honest error.
        let Some(eab) = load_zerossl_eab() else {
            return Err(
                "ZeroSSL requires EAB credentials in /var/lib/cpn/ssl/zerossl-eab.json (kid + hmac_key). Not stored in the repo."
                    .into(),
            );
        };
        if eab.kid.trim().is_empty() || eab.hmac_key.trim().is_empty() {
            return Err("ZeroSSL EAB kid/hmac_key are empty".into());
        }
        args.push("--server".into());
        args.push("https://acme.zerossl.com/v2/DV90".into());
        args.push("--eab-kid".into());
        args.push(eab.kid.trim().to_string());
        args.push("--eab-hmac-key".into());
        args.push(eab.hmac_key.trim().to_string());
    }
    let use_dns = cloudflare_configured() && cloudflare_dns_plugin_available();
    if use_dns {
        let ini = write_cloudflare_ini()?;
        args.push("--dns-cloudflare".into());
        args.push("--dns-cloudflare-credentials".into());
        args.push(ini.display().to_string());
    } else {
        if !Path::new(&site.docroot).is_dir() {
            return Err(format!(
                "Docroot `{}` missing; cannot use webroot. Configure Cloudflare DNS-01 or create the docroot.",
                site.docroot
            ));
        }
        args.push("--webroot".into());
        args.push("-w".into());
        args.push(site.docroot.clone());
    }
    for n in &names {
        args.push("-d".into());
        args.push(n.clone());
    }
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    match run_certbot(&arg_refs) {
        Ok(_) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|v| v.as_secs())
                .unwrap_or(0);
            for n in &names {
                if let Ok(mut s) = load_site(n) {
                    s.ssl.last_issue_unix = now;
                    s.ssl.last_error.clear();
                    if n != &domain {
                        s.ssl.shared_cert_owner = Some(domain.clone());
                    } else {
                        s.ssl.shared_cert_owner = None;
                    }
                    let _ = persist_ssl_ok(n, s.ssl);
                }
            }
            let method = if use_dns { "DNS-01" } else { "webroot" };
            let origin = if cloudflare_configured() && site.ssl.install_origin_cert {
                " Origin cert kept for Cloudflare proxy when enabled."
            } else {
                ""
            };
            Ok(format!(
                "{} certificate issued for {} via {method}.{origin}",
                if server_kind == "zerossl" {
                    "ZeroSSL"
                } else {
                    "Let's Encrypt"
                },
                names.join(", ")
            ))
        }
        Err(e) => {
            let _ = persist_ssl_error(&domain, &e);
            Err(e)
        }
    }
}

fn issue_cloudflare_ca(site: &SiteRecord) -> Result<String, String> {
    let domain = site.domain.clone();
    if !cloudflare_configured() {
        return Err("Cloudflare CA requires an API token under Cloudflare DNS API Settings".into());
    }
    // Origin CA via API is a follow-on; for now require certbot DNS-01 against LE is NOT used.
    // Honest path: attempt Cloudflare Origin CA CSR flow is not fully wired; report clearly.
    let settings = crate::panel_ops_cloudflare::load_cloudflare();
    if settings.api_token.trim().is_empty() {
        return Err("Cloudflare API token is empty".into());
    }
    // Prefer DNS-01 ACME when plugin present (Cloudflare can still terminate edge TLS;
    // origin material from ACME serves as installable origin cert).
    if cloudflare_dns_plugin_available() {
        let msg = issue_acme(site, "letsencrypt")?;
        return Ok(format!(
            "Cloudflare CA path: installed origin-compatible cert via DNS-01. {msg} Note: dedicated Cloudflare Origin CA API issuance can be added when CSR upload is wired."
        ));
    }
    let err = "Cloudflare CA: install certbot-dns-cloudflare or upload a Cloudflare Origin CA cert as Custom SSL. Token is present but Origin CA auto-issue is not fully wired yet.".to_string();
    let _ = persist_ssl_error(&domain, &err);
    Err(err)
}

pub fn upload_custom_ssl(domain: &str, cert_pem: &str, key_pem: &str) -> Result<String, String> {
    let domain = normalize_domain(domain)?;
    let mut site = load_site(&domain)?;
    if cert_pem.trim().is_empty() || key_pem.trim().is_empty() {
        return Err("Certificate and private key PEM are required".into());
    }
    if !cert_pem.contains("BEGIN CERTIFICATE") || !key_pem.contains("BEGIN") {
        return Err("PEM appears invalid (missing BEGIN markers)".into());
    }
    let dir = ssl_material_dir(&domain);
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create SSL dir: {e}"))?;
    let (cert_path, key_path) = custom_cert_paths(&domain);
    fs::write(&cert_path, cert_pem.as_bytes()).map_err(|e| format!("Cannot write cert: {e}"))?;
    fs::write(&key_path, key_pem.as_bytes()).map_err(|e| format!("Cannot write key: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600));
        let _ = fs::set_permissions(&cert_path, fs::Permissions::from_mode(0o644));
    }
    site.ssl.provider = SslProvider::Custom;
    site.ssl.custom_cert_path = Some(cert_path.display().to_string());
    site.ssl.custom_key_path = Some(key_path.display().to_string());
    site.ssl.shared_cert_owner = None;
    site.ssl.include_subdomains_on_cert = false;
    site.ssl.last_error.clear();
    persist_ssl_ok(&domain, site.ssl)?;
    Ok(format!(
        "Custom SSL stored for `{domain}` (no auto-renew). Key mode 600 under {}",
        dir.display()
    ))
}

pub fn renew_auto_providers() -> Result<String, String> {
    if !certbot_available() {
        return Err("certbot was not found on PATH; cannot renew".into());
    }
    // certbot renew only touches certs it manages; domains on None/Custom are untouched.
    let output = Command::new("certbot")
        .args(["renew", "--non-interactive"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run certbot renew: {e}"))?;
    if !output.status.success() {
        let combined = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let short: String = combined.chars().take(400).collect();
        return Err(format!("certbot renew failed: {short}"));
    }
    Ok("certbot renew completed (None/Custom domains were not forced)".into())
}

pub fn issue_all_needing_auto() -> Result<String, String> {
    let rows = ssl_status_all_sites();
    if rows.is_empty() {
        return Err("No sites registered in CPN".into());
    }
    let mut ok = 0u32;
    let mut skip = 0u32;
    let mut fail = Vec::new();
    for row in rows {
        if !row.auto_issue {
            skip += 1;
            continue;
        }
        if row.has_cert {
            skip += 1;
            continue;
        }
        if row.shared_cert_owner.is_some() {
            skip += 1;
            continue;
        }
        match issue_or_renew(&row.domain) {
            Ok(_) => ok += 1,
            Err(e) => fail.push(format!("{}: {e}", row.domain)),
        }
    }
    if ok == 0 && !fail.is_empty() {
        return Err(format!(
            "No certificates issued. Failures: {}",
            fail.into_iter().take(5).collect::<Vec<_>>().join(" | ")
        ));
    }
    let fail_n = fail.len();
    Ok(format!(
        "Bulk SSL: {ok} issued, {skip} skipped (None/Custom/has-cert/shared), {fail_n} failed{}",
        if fail.is_empty() {
            String::new()
        } else {
            format!(
                ". Errors: {}",
                fail.into_iter().take(3).collect::<Vec<_>>().join(" | ")
            )
        }
    ))
}

// Compatibility aliases used by earlier Cloudflare DNS + LE routes/UI.
pub fn issue_lets_encrypt(domain: &str) -> Result<String, String> {
    issue_or_renew(domain)
}
pub fn issue_le_for_all_without_custom() -> Result<String, String> {
    issue_all_needing_auto()
}
pub fn renew_lets_encrypt_all() -> Result<String, String> {
    renew_auto_providers()
}
pub fn set_custom_ssl(domain: &str) -> Result<String, String> {
    set_domain_provider(domain, SslProvider::Custom)
}
pub fn restore_lets_encrypt(domain: &str) -> Result<String, String> {
    set_domain_provider(domain, SslProvider::LetsEncrypt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::sites::create_site_with_ssl;

    #[test]
    fn none_refuses_issue() {
        with_test_data_dir(|| {
            create_site_with_ssl(
                "cpn-lab-test.example",
                "Admin",
                None,
                None,
                None,
                Some(SslProvider::None),
            )
            .unwrap();
            let err = issue_or_renew("cpn-lab-test.example").unwrap_err();
            assert!(err.contains("None"));
        });
    }

    #[test]
    fn sibling_providers_isolated() {
        with_test_data_dir(|| {
            create_site_with_ssl(
                "example.com",
                "Admin",
                None,
                None,
                None,
                Some(SslProvider::LetsEncrypt),
            )
            .unwrap();
            create_site_with_ssl(
                "a.example.com",
                "Admin",
                None,
                None,
                None,
                Some(SslProvider::None),
            )
            .unwrap();
            create_site_with_ssl(
                "b.example.com",
                "Admin",
                None,
                None,
                None,
                None, // inherit LE from parent as initial only
            )
            .unwrap();
            assert_eq!(
                load_site("a.example.com").unwrap().ssl.provider,
                SslProvider::None
            );
            assert_eq!(
                load_site("b.example.com").unwrap().ssl.provider,
                SslProvider::LetsEncrypt
            );
            set_domain_provider("b.example.com", SslProvider::Custom).unwrap();
            assert_eq!(
                load_site("example.com").unwrap().ssl.provider,
                SslProvider::LetsEncrypt
            );
            assert_eq!(
                load_site("b.example.com").unwrap().ssl.provider,
                SslProvider::Custom
            );
        });
    }
}
