//! ACME / Cloudflare CA issue paths for CPN SSL.

use crate::panel_ops_cloudflare::cloudflare_configured;
use crate::panel_ops_ssl_le::{
    cloudflare_dns_plugin_available, effective_coverage, load_zerossl_eab, names_for_issue,
    persist_ssl_error, persist_ssl_ok, run_certbot, write_cloudflare_ini,
};
use crate::panel_ops_ssl_provider::{SslCoverageMode, SslProvider};
use crate::sites::{SiteRecord, load_site, normalize_domain};
use std::path::Path;

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

/// Let's Encrypt on the origin as backup when Cloudflare is public TLS.
/// Does not treat Cloudflare edge TLS as a local origin certificate.
pub fn issue_origin_backup(domain: &str) -> Result<String, String> {
    let domain = normalize_domain(domain)?;
    match crate::panel_ops_certbot_install::ensure_certbot_on_path() {
        Ok(_) => {}
        Err(e) => {
            let _ = persist_ssl_error(&domain, &e);
            return Err(e);
        }
    }
    let mut site = load_site(&domain)?;
    if site.ssl.provider == SslProvider::Custom {
        let has_custom = site
            .ssl
            .custom_cert_path
            .as_ref()
            .map(|p| Path::new(p).is_file())
            .unwrap_or(false);
        if has_custom {
            return Err(format!(
                "`{domain}` already has Custom SSL on the origin. Keep that cert, or switch provider before issuing Let's Encrypt."
            ));
        }
        site.ssl.provider = SslProvider::LetsEncrypt;
    }
    if site.ssl.provider == SslProvider::None {
        site.ssl.provider = SslProvider::LetsEncrypt;
    }
    site.ssl.install_origin_cert = true;
    site.ssl.last_error.clear();
    persist_ssl_ok(&domain, site.ssl.clone())?;
    let site = load_site(&domain)?;
    let dns_ready = cloudflare_configured() && cloudflare_dns_plugin_available();
    let coverage = effective_coverage(&site.ssl);
    let needs_wildcard = matches!(coverage, SslCoverageMode::Wildcard);
    // Apex/SAN prefer HTTP-01. Wildcard uses DNS-01 only when Cloudflare DNS-01 is ready.
    let force_http01 = !(needs_wildcard && dns_ready);
    let msg = issue_acme_inner(&site, "letsencrypt", force_http01)?;
    Ok(format!(
        "Origin Let's Encrypt backup for `{domain}`. {msg} If Cloudflare proxy stops, origin HTTPS still uses this certificate."
    ))
}

pub(crate) fn issue_acme(site: &SiteRecord, server_kind: &str) -> Result<String, String> {
    let coverage = effective_coverage(&site.ssl);
    let needs_wildcard = matches!(coverage, SslCoverageMode::Wildcard);
    let dns_ready = cloudflare_configured() && cloudflare_dns_plugin_available();
    let force_http01 = !(needs_wildcard && dns_ready);
    issue_acme_inner(site, server_kind, force_http01)
}

pub(crate) fn http01_names(site: &SiteRecord) -> Vec<String> {
    let mut names = names_for_issue(site);
    names.retain(|n| !n.starts_with("*."));
    if names.is_empty() {
        names.push(site.domain.clone());
    }
    names
}

pub(crate) fn issue_acme_inner(
    site: &SiteRecord,
    server_kind: &str,
    force_http01: bool,
) -> Result<String, String> {
    let domain = site.domain.clone();
    let coverage = effective_coverage(&site.ssl);
    let names = if force_http01 {
        http01_names(site)
    } else {
        names_for_issue(site)
    };
    let mut args: Vec<String> = vec![
        "certonly".into(),
        "--non-interactive".into(),
        "--agree-tos".into(),
        "--register-unsafely-without-email".into(),
    ];
    if server_kind == "zerossl" {
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
    let needs_dns = !force_http01
        && (matches!(coverage, SslCoverageMode::Wildcard)
            || names.iter().any(|n| n.starts_with("*.")));
    let use_dns = if needs_dns {
        if !cloudflare_configured() {
            return Err(
                "Wildcard certificates require DNS-01. Configure a Cloudflare API token under Cloudflare DNS > API Settings, then retry."
                    .into(),
            );
        }
        if !cloudflare_dns_plugin_available() {
            return Err(
                "Wildcard certificates require DNS-01, but certbot-dns-cloudflare is not installed. Install the plugin, or switch coverage to SAN for HTTP-01 webroot."
                    .into(),
            );
        }
        true
    } else {
        false
    };
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
                if n.starts_with("*.") {
                    continue;
                }
                if let Ok(mut s) = load_site(n) {
                    s.ssl.last_issue_unix = now;
                    s.ssl.last_origin_retry_unix = now;
                    s.ssl.last_error.clear();
                    if n != &domain {
                        s.ssl.shared_cert_owner = Some(domain.clone());
                    } else {
                        s.ssl.shared_cert_owner = None;
                    }
                    let _ = persist_ssl_ok(n, s.ssl);
                }
            }
            if let Ok(mut owner) = load_site(&domain) {
                owner.ssl.last_issue_unix = now;
                owner.ssl.last_origin_retry_unix = now;
                owner.ssl.last_error.clear();
                let _ = persist_ssl_ok(&domain, owner.ssl);
            }
            let method = if use_dns { "DNS-01" } else { "HTTP-01" };
            let coverage_label = if force_http01 {
                "SAN"
            } else {
                coverage.label()
            };
            let origin = if cloudflare_configured() && site.ssl.install_origin_cert {
                " Origin cert kept for Cloudflare proxy when enabled."
            } else {
                ""
            };
            Ok(format!(
                "{} {} certificate issued for {} via {method}.{origin}",
                if server_kind == "zerossl" {
                    "ZeroSSL"
                } else {
                    "Let's Encrypt"
                },
                coverage_label,
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
    let settings = crate::panel_ops_cloudflare::load_cloudflare();
    if settings.api_token.trim().is_empty() {
        return Err("Cloudflare API token is empty".into());
    }
    if cloudflare_dns_plugin_available() {
        let msg = issue_acme_inner(site, "letsencrypt", false)?;
        return Ok(format!(
            "Cloudflare CA path: installed origin-compatible cert via ACME. {msg} Note: dedicated Origin CA API issuance can be added when CSR upload is wired."
        ));
    }
    match issue_acme_inner(site, "letsencrypt", true) {
        Ok(msg) => Ok(format!(
            "Cloudflare CA path: installed origin-compatible cert via HTTP-01. {msg}"
        )),
        Err(e) => {
            let err = format!(
                "Cloudflare CA: HTTP-01 origin issue failed ({e}). Install certbot-dns-cloudflare for DNS-01, or upload an Origin CA cert as Custom SSL."
            );
            let _ = persist_ssl_error(&domain, &err);
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::http01_names;
    use crate::sites::SiteRecord;

    fn site(domain: &str) -> SiteRecord {
        SiteRecord {
            schema_version: 5,
            domain: domain.into(),
            owner: "Admin".into(),
            docroot: "/tmp/public_html".into(),
            enabled: true,
            engine: None,
            notes: String::new(),
            created_at_unix: 0,
            updated_at_unix: 0,
            vhost_wired: false,
            ssl: Default::default(),
            internal_ip: None,
            owner_suspend_message: String::new(),
            suspended_by: None,
            php_version: None,
            aliases: Vec::new(),
            staging_of: None,
        }
    }

    #[test]
    fn http01_strips_wildcard_names() {
        crate::account::with_test_data_dir(|| {
            let names = http01_names(&site("example.com"));
            assert!(names.iter().all(|n| !n.starts_with("*.")), "{names:?}");
            assert!(names.iter().any(|n| n == "example.com"), "{names:?}");
        });
    }
}
