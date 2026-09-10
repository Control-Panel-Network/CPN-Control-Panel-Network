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

pub(crate) fn issue_acme(site: &SiteRecord, server_kind: &str) -> Result<String, String> {
    let domain = site.domain.clone();
    let coverage = effective_coverage(&site.ssl);
    let names = names_for_issue(site);
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
    let needs_dns =
        matches!(coverage, SslCoverageMode::Wildcard) || names.iter().any(|n| n.starts_with("*."));
    let dns_ready = cloudflare_configured() && cloudflare_dns_plugin_available();
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
        dns_ready
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
                owner.ssl.last_error.clear();
                let _ = persist_ssl_ok(&domain, owner.ssl);
            }
            let method = if use_dns { "DNS-01" } else { "webroot" };
            let coverage_label = coverage.label();
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
