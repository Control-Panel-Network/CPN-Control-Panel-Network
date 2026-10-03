//! Public SSL badge: origin cert files vs Cloudflare edge TLS.
//!
//! The websites list badge is origin material (and live origin HTTPS), not
//! Cloudflare orange-cloud TLS by itself. CF SSL is shown only when the
//! hostname is orange-cloud proxied.

use crate::panel_ops_cloudflare::cloudflare_configured;
use crate::panel_ops_cloudflare_api::{CfDnsRecord, list_dns_records};
use crate::panel_ops_ssl_inspect::{SslCertInsight, SslValidityKind, inspect_domain_ssl};
use crate::panel_ops_ssl_provider::SslProvider;
use crate::sites::SiteRecord;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const CF_DNS_CACHE_TTL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicSslKind {
    OriginValid,
    OriginExpiring,
    OriginExpired,
    OriginMismatch,
    OriginInvalid,
    CloudflareEdge,
    None,
}

impl PublicSslKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OriginValid => "valid",
            Self::OriginExpiring => "expiring",
            Self::OriginExpired => "expired",
            Self::OriginMismatch => "mismatch",
            Self::OriginInvalid => "invalid",
            Self::CloudflareEdge => "cf",
            Self::None => "none",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::OriginValid => "Valid",
            Self::OriginExpiring => "Expiring",
            Self::OriginExpired => "Expired",
            Self::OriginMismatch => "Mismatch",
            Self::OriginInvalid => "Invalid",
            Self::CloudflareEdge => "CF SSL",
            Self::None => "NONE",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::OriginValid => "Origin SSL valid",
            Self::OriginExpiring => "Origin SSL expiring soon",
            Self::OriginExpired => "Origin SSL expired",
            Self::OriginMismatch => "Origin SSL mismatch",
            Self::OriginInvalid => "Origin SSL invalid",
            Self::CloudflareEdge => "Cloudflare SSL",
            Self::None => "No SSL",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PublicSslView {
    pub kind: PublicSslKind,
    pub origin: SslCertInsight,
    pub zone_linked: bool,
    pub proxied: bool,
    pub title: String,
    pub detail: String,
}

#[derive(Clone)]
struct DnsCacheEntry {
    at: Instant,
    records: Result<Vec<CfDnsRecord>, String>,
}

fn dns_cache() -> &'static Mutex<HashMap<String, DnsCacheEntry>> {
    static CACHE: OnceLock<Mutex<HashMap<String, DnsCacheEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn names_equal(a: &str, b: &str) -> bool {
    a.trim_end_matches('.')
        .eq_ignore_ascii_case(b.trim_end_matches('.'))
}

fn cached_dns_records(domain: &str) -> Result<Vec<CfDnsRecord>, String> {
    let key = domain.trim().to_ascii_lowercase();
    if let Ok(guard) = dns_cache().lock()
        && let Some(entry) = guard.get(&key)
        && entry.at.elapsed() < CF_DNS_CACHE_TTL
    {
        return entry.records.clone();
    }
    let records = list_dns_records(&key);
    if let Ok(mut guard) = dns_cache().lock() {
        guard.insert(
            key,
            DnsCacheEntry {
                at: Instant::now(),
                records: records.clone(),
            },
        );
    }
    records
}

/// Cloudflare zone + orange-cloud proxy for this FQDN. Never infers proxy from
/// "Cloudflare is configured" alone.
pub fn cloudflare_hostname_tls(domain: &str) -> (bool, bool) {
    if !cloudflare_configured() {
        return (false, false);
    }
    match cached_dns_records(domain) {
        Ok(recs) => {
            let proxied = recs.iter().any(|r| {
                names_equal(&r.name, domain)
                    && r.proxied
                    && matches!(
                        r.record_type.to_ascii_uppercase().as_str(),
                        "A" | "AAAA" | "CNAME"
                    )
            });
            (true, proxied)
        }
        Err(_) => (false, false),
    }
}

pub fn select_public_kind(
    origin: SslValidityKind,
    _zone_linked: bool,
    proxied: bool,
) -> PublicSslKind {
    match origin {
        SslValidityKind::Valid => PublicSslKind::OriginValid,
        SslValidityKind::ExpiringSoon => PublicSslKind::OriginExpiring,
        SslValidityKind::Expired => PublicSslKind::OriginExpired,
        SslValidityKind::Mismatch => PublicSslKind::OriginMismatch,
        SslValidityKind::Invalid => PublicSslKind::OriginInvalid,
        SslValidityKind::None => {
            if proxied {
                PublicSslKind::CloudflareEdge
            } else {
                PublicSslKind::None
            }
        }
    }
}

fn title_and_detail(
    kind: PublicSslKind,
    origin: &SslCertInsight,
    zone_linked: bool,
    proxied: bool,
) -> (String, String) {
    match kind {
        PublicSslKind::OriginValid | PublicSslKind::OriginExpiring => {
            let exp = origin.expires_display.as_deref().unwrap_or("unknown");
            (
                format!(
                    "Origin certificate files (Let's Encrypt or custom) on this server. Expires {exp}."
                ),
                origin.detail.clone(),
            )
        }
        PublicSslKind::OriginExpired
        | PublicSslKind::OriginMismatch
        | PublicSslKind::OriginInvalid => (
            origin.detail.clone(),
            format!(
                "{} This badge is origin TLS on the site vhost, not Cloudflare edge TLS.",
                origin.detail
            ),
        ),
        PublicSslKind::CloudflareEdge => (
            "Cloudflare SSL: orange-cloud proxy is on for this hostname. Visitors get HTTPS at Cloudflare. Origin has no local certificate files.".into(),
            "Public HTTPS is Cloudflare edge TLS. Origin vhost has no Let's Encrypt or custom cert files. Issue origin Let's Encrypt as backup so HTTPS still works if Cloudflare proxy stops.".into(),
        ),
        PublicSslKind::None => {
            let extra = if zone_linked && !proxied {
                " A Cloudflare zone is linked, but proxy is DNS-only (grey cloud), so Cloudflare is not providing public TLS for this hostname."
            } else {
                " Cloudflare orange-cloud proxy was not detected for this hostname."
            };
            (
                format!(
                    "No SSL: no origin certificate files on this server, and Cloudflare edge TLS does not apply.{extra}"
                ),
                format!(
                    "Badge is origin cert files (and live HTTPS on the site vhost), not Cloudflare orange-cloud TLS.{} Issue Let's Encrypt on the origin for local HTTPS.",
                    extra
                ),
            )
        }
    }
}

pub fn inspect_public_ssl(domain: &str) -> PublicSslView {
    let origin = inspect_domain_ssl(domain);
    let (zone_linked, proxied) = cloudflare_hostname_tls(domain);
    let kind = select_public_kind(origin.kind, zone_linked, proxied);
    let (title, detail) = title_and_detail(kind, &origin, zone_linked, proxied);
    PublicSslView {
        kind,
        origin,
        zone_linked,
        proxied,
        title,
        detail,
    }
}

pub fn offers_origin_backup(site: &SiteRecord, view: &PublicSslView) -> bool {
    if site.ssl.provider == SslProvider::Custom
        && site
            .ssl
            .custom_cert_path
            .as_ref()
            .map(|p| std::path::Path::new(p).is_file())
            .unwrap_or(false)
    {
        return false;
    }
    !matches!(
        view.kind,
        PublicSslKind::OriginValid | PublicSslKind::OriginExpiring
    )
}

fn html_escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}

pub fn ssl_public_badge_html(view: &PublicSslView) -> String {
    let title = html_escape_attr(&view.title);
    let label = view.kind.label();
    let kind = view.kind.as_str();
    let lock = concat!(
        r#"<svg class="ssl-lock" viewBox="0 0 16 16" width="12" height="12" aria-hidden="true">"#,
        r#"<path fill="currentColor" d="M4 7V5a4 4 0 0 1 8 0v2h1a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V8a1 1 0 0 1 1-1h1zm2 0h4V5a2 2 0 1 0-4 0v2z"/></svg>"#
    );
    format!(
        r#"<span class="manage-badge ssl-badge ssl-{kind}" title="{title}">{lock} {label}</span>"#,
        kind = kind,
        title = title,
        lock = lock,
        label = html_escape_attr(label),
    )
}

pub fn ssl_list_badge_html(view: &PublicSslView) -> String {
    let tip = html_escape_attr(&view.title);
    let short = html_escape_attr(view.kind.short_label());
    let (bg, fg, class) = match view.kind {
        PublicSslKind::OriginValid => ("rgba(18,183,106,.18)", "#6ce9a6", ""),
        PublicSslKind::OriginExpiring => ("rgba(247,144,9,.2)", "#fdb022", ""),
        PublicSslKind::OriginExpired
        | PublicSslKind::OriginInvalid
        | PublicSslKind::OriginMismatch => ("rgba(240,68,56,.18)", "#f97066", ""),
        PublicSslKind::CloudflareEdge => ("rgba(59,130,246,.2)", "#93c5fd", " cf"),
        PublicSslKind::None => ("rgba(152,162,179,.16)", "#98a2b3", " off"),
    };
    format!(
        r#"<span class="site-ssl-badge{class}" title="{tip}" style="background:{bg};color:{fg};">{short}</span>"#,
        class = class,
        tip = tip,
        short = short,
        bg = bg,
        fg = fg,
    )
}

pub fn origin_backup_form(domain: &str, return_to: &str, button: &str) -> String {
    let d = html_escape_attr(domain);
    let ret = html_escape_attr(return_to);
    let btn = html_escape_attr(button);
    format!(
        r#"<form method="post" action="/security/ssl/origin-backup" class="inline-form">
  <input type="hidden" name="domain" value="{d}">
  <input type="hidden" name="return" value="{ret}">
  <button type="submit" class="btn-secondary" style="min-height:36px;padding:0 12px;border:0;border-radius:999px;background:#d1e9ff;color:#175cd3;font-weight:700;cursor:pointer;" title="Install Let's Encrypt on this origin even when Cloudflare is public TLS. If Cloudflare proxy stops, origin HTTPS still works.">{btn}</button>
</form>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxied_without_origin_is_cf_ssl_not_valid() {
        assert_eq!(
            select_public_kind(SslValidityKind::None, true, true),
            PublicSslKind::CloudflareEdge
        );
        assert_ne!(
            select_public_kind(SslValidityKind::None, true, true),
            PublicSslKind::OriginValid
        );
    }

    #[test]
    fn zone_linked_dns_only_is_none_not_cf() {
        assert_eq!(
            select_public_kind(SslValidityKind::None, true, false),
            PublicSslKind::None
        );
    }

    #[test]
    fn origin_cert_wins_over_cloudflare() {
        assert_eq!(
            select_public_kind(SslValidityKind::Valid, true, true),
            PublicSslKind::OriginValid
        );
    }

    #[test]
    fn labels_are_clear_and_product_safe() {
        for kind in [
            PublicSslKind::None,
            PublicSslKind::CloudflareEdge,
            PublicSslKind::OriginValid,
        ] {
            let blob = format!("{} {}", kind.label(), kind.short_label()).to_lowercase();
            assert!(!blob.contains("cyberpanel"));
            assert!(!blob.contains("insecure"));
        }
        assert_eq!(PublicSslKind::None.short_label(), "NONE");
        assert_eq!(PublicSslKind::CloudflareEdge.short_label(), "CF SSL");
    }
}
