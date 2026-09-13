//! Inspect installed TLS certificates: notAfter, SANs, issuer, domain coverage.
//! Uses `openssl x509`. File presence alone is not validity.

use crate::panel_ops_ssl_provider::custom_cert_paths;
use crate::sites::{load_site, normalize_domain, resolve_parent_domain};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

pub const EXPIRING_SOON_DAYS: i64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SslValidityKind {
    None,
    Valid,
    ExpiringSoon,
    Expired,
    Mismatch,
    Invalid,
}

impl SslValidityKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Valid => "valid",
            Self::ExpiringSoon => "expiring",
            Self::Expired => "expired",
            Self::Mismatch => "mismatch",
            Self::Invalid => "invalid",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "No SSL",
            Self::Valid => "SSL Valid",
            Self::ExpiringSoon => "SSL Expiring soon",
            Self::Expired => "SSL Expired",
            Self::Mismatch => "SSL Mismatch",
            Self::Invalid => "SSL Invalid",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Valid => "Valid",
            Self::ExpiringSoon => "Expiring",
            Self::Expired => "Expired",
            Self::Mismatch => "Mismatch",
            Self::Invalid => "Invalid",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SslCertInsight {
    pub kind: SslValidityKind,
    pub cert_path: Option<String>,
    pub not_after_unix: Option<u64>,
    pub expires_display: Option<String>,
    pub days_remaining: Option<i64>,
    pub issuer: String,
    pub subject: String,
    pub sans: Vec<String>,
    pub covers_domain: bool,
    pub detail: String,
}

impl Default for SslCertInsight {
    fn default() -> Self {
        Self {
            kind: SslValidityKind::None,
            cert_path: None,
            not_after_unix: None,
            expires_display: None,
            days_remaining: None,
            issuer: String::new(),
            subject: String::new(),
            sans: Vec::new(),
            covers_domain: false,
            detail: "No certificate files found for this domain.".into(),
        }
    }
}

pub fn resolve_cert_path(domain: &str) -> Option<PathBuf> {
    let Ok(domain) = normalize_domain(domain) else {
        return None;
    };
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(site) = load_site(&domain) {
        if let Some(p) = site.ssl.custom_cert_path.as_ref() {
            let path = PathBuf::from(p);
            if path.is_file() {
                candidates.push(path);
            }
        }
        if let Some(owner) = site.ssl.shared_cert_owner.as_ref() {
            push_domain_cert_candidates(&mut candidates, owner);
        }
    }
    push_domain_cert_candidates(&mut candidates, &domain);
    if let Ok(Some(parent)) = resolve_parent_domain(&domain) {
        push_domain_cert_candidates(&mut candidates, &parent);
    }
    candidates.into_iter().find(|p| readable_file(p))
}

fn push_domain_cert_candidates(out: &mut Vec<PathBuf>, domain: &str) {
    let (custom_full, _) = custom_cert_paths(domain);
    out.push(custom_full);
    out.push(PathBuf::from(format!(
        "/etc/letsencrypt/live/{domain}/fullchain.pem"
    )));
    out.push(PathBuf::from(format!(
        "/etc/letsencrypt/live/{domain}/cert.pem"
    )));
    out.push(PathBuf::from(format!("/etc/ssl/cpn/{domain}.crt")));
    out.push(PathBuf::from(format!(
        "/etc/ssl/cpn/{domain}/fullchain.pem"
    )));
    out.push(PathBuf::from(format!(
        "/var/lib/cpn/ssl/{domain}/fullchain.pem"
    )));
}

fn readable_file(path: &Path) -> bool {
    path.is_file() && std::fs::File::open(path).is_ok()
}

pub fn inspect_domain_ssl(domain: &str) -> SslCertInsight {
    let Ok(domain) = normalize_domain(domain) else {
        return SslCertInsight {
            kind: SslValidityKind::Invalid,
            detail: "Invalid domain name.".into(),
            ..SslCertInsight::default()
        };
    };
    let Some(path) = resolve_cert_path(&domain) else {
        return SslCertInsight::default();
    };
    match parse_cert_pem(&path) {
        Ok(mut parsed) => {
            parsed.cert_path = Some(path.display().to_string());
            parsed.covers_domain = domain_covered(&domain, &parsed.sans, &parsed.subject);
            finalize_kind(&mut parsed);
            parsed
        }
        Err(err) => SslCertInsight {
            kind: SslValidityKind::Invalid,
            cert_path: Some(path.display().to_string()),
            detail: err,
            ..SslCertInsight::default()
        },
    }
}

fn finalize_kind(insight: &mut SslCertInsight) {
    if !insight.covers_domain {
        insight.kind = SslValidityKind::Mismatch;
        insight.detail = format!(
            "Certificate on disk does not cover this hostname. SANs: {}.",
            if insight.sans.is_empty() {
                "(none)".into()
            } else {
                insight.sans.join(", ")
            }
        );
        return;
    }
    let Some(days) = insight.days_remaining else {
        insight.kind = SslValidityKind::Invalid;
        insight.detail = "Could not determine certificate expiry.".into();
        return;
    };
    if days < 0 {
        insight.kind = SslValidityKind::Expired;
        insight.detail = format!(
            "Certificate expired on {}.",
            insight.expires_display.as_deref().unwrap_or("unknown")
        );
        return;
    }
    if days <= EXPIRING_SOON_DAYS {
        insight.kind = SslValidityKind::ExpiringSoon;
        insight.detail = format!(
            "Certificate expires on {} ({} days left).",
            insight.expires_display.as_deref().unwrap_or("unknown"),
            days
        );
        return;
    }
    insight.kind = SslValidityKind::Valid;
    insight.detail = format!(
        "Certificate is valid through {}.",
        insight.expires_display.as_deref().unwrap_or("unknown")
    );
}

#[derive(Debug, Default)]
struct RawCertParse {
    enddate_line: String,
    issuer: String,
    subject: String,
    san_blob: String,
}

fn parse_cert_pem(path: &Path) -> Result<SslCertInsight, String> {
    let output = Command::new("openssl")
        .args([
            "x509",
            "-in",
            &path.display().to_string(),
            "-noout",
            "-enddate",
            "-issuer",
            "-subject",
            "-ext",
            "subjectAltName",
        ])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("openssl failed to start: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "openssl x509 failed: {}",
            err.trim().chars().take(240).collect::<String>()
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut raw = RawCertParse::default();
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("notAfter=") {
            raw.enddate_line = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("issuer=") {
            raw.issuer = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("subject=") {
            raw.subject = rest.trim().to_string();
        } else if line.contains("DNS:") || line.starts_with("DNS:") {
            raw.san_blob.push_str(line);
            raw.san_blob.push(' ');
        }
    }
    if raw.enddate_line.is_empty() {
        return Err("openssl did not return notAfter".into());
    }
    let not_after_unix = parse_openssl_utc(&raw.enddate_line)?;
    let expires_display = format_dd_mm_yyyy(not_after_unix);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days_remaining = ((not_after_unix as i64) - (now as i64)) / 86_400;
    let sans = parse_san_names(&raw.san_blob);
    let issuer_short = shorten_dn(&raw.issuer);
    Ok(SslCertInsight {
        kind: SslValidityKind::Invalid,
        cert_path: None,
        not_after_unix: Some(not_after_unix),
        expires_display: Some(expires_display),
        days_remaining: Some(days_remaining),
        issuer: issuer_short,
        subject: raw.subject,
        sans,
        covers_domain: false,
        detail: String::new(),
    })
}

fn parse_san_names(blob: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in blob.split([',', ' ', '\t', '\n']) {
        let part = part.trim();
        if let Some(dns) = part.strip_prefix("DNS:") {
            let name = dns.trim().trim_matches('"').to_ascii_lowercase();
            if !name.is_empty() && !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out
}

fn cn_from_subject(subject: &str) -> Option<String> {
    for part in subject.split([',', '/']) {
        let part = part.trim();
        if let Some(cn) = part
            .strip_prefix("CN=")
            .or_else(|| part.strip_prefix("CN ="))
        {
            let cn = cn.trim().to_ascii_lowercase();
            if !cn.is_empty() {
                return Some(cn);
            }
        }
    }
    None
}

pub fn domain_covered(domain: &str, sans: &[String], subject: &str) -> bool {
    let host = domain.trim().trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() {
        return false;
    }
    let mut names = sans.to_vec();
    if let Some(cn) = cn_from_subject(subject)
        && !names.contains(&cn)
    {
        names.push(cn);
    }
    for name in names {
        if name == host {
            return true;
        }
        if let Some(suffix) = name.strip_prefix("*.") {
            // One-label wildcard only: *.example.com matches a.example.com, not a.b.example.com.
            if host.ends_with(suffix)
                && host.len() > suffix.len()
                && host.as_bytes().get(host.len() - suffix.len() - 1) == Some(&b'.')
            {
                let left = &host[..host.len() - suffix.len() - 1];
                if !left.is_empty() && !left.contains('.') {
                    return true;
                }
            }
        }
    }
    false
}

fn shorten_dn(dn: &str) -> String {
    if let Some(o) = dn.split(',').find_map(|p| {
        let p = p.trim();
        p.strip_prefix("O=")
            .or_else(|| p.strip_prefix("O ="))
            .map(str::trim)
    }) && !o.is_empty()
    {
        return o.to_string();
    }
    if let Some(cn) = cn_from_subject(dn) {
        return cn;
    }
    dn.chars().take(80).collect()
}

fn parse_openssl_utc(s: &str) -> Result<u64, String> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() < 4 {
        return Err(format!("Unrecognized notAfter format: {s}"));
    }
    let mon = match parts[0] {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        other => return Err(format!("Unknown month in notAfter: {other}")),
    };
    let day: u32 = parts[1]
        .parse()
        .map_err(|_| format!("Bad day in notAfter: {}", parts[1]))?;
    let time_parts: Vec<&str> = parts[2].split(':').collect();
    if time_parts.len() != 3 {
        return Err(format!("Bad time in notAfter: {}", parts[2]));
    }
    let hour: u32 = time_parts[0].parse().map_err(|_| "bad hour".to_string())?;
    let min: u32 = time_parts[1]
        .parse()
        .map_err(|_| "bad minute".to_string())?;
    let sec: u32 = time_parts[2]
        .parse()
        .map_err(|_| "bad second".to_string())?;
    let year: i32 = parts[3]
        .parse()
        .map_err(|_| format!("Bad year in notAfter: {}", parts[3]))?;
    days_from_civil(year, mon, day)
        .map(|days| {
            let secs = days * 86_400 + (hour as i64) * 3600 + (min as i64) * 60 + (sec as i64);
            secs as u64
        })
        .ok_or_else(|| format!("Invalid calendar date in notAfter: {s}"))
}

fn days_from_civil(year: i32, month: u32, day: u32) -> Option<i64> {
    if !(1..=12).contains(&month) || day == 0 || day > 31 {
        return None;
    }
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let m = month as i32;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + day as i32 - 1;
    if doy < 0 {
        return None;
    }
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy as u32;
    Some((era as i64) * 146_097 + doe as i64 - 719_468)
}

pub fn format_dd_mm_yyyy(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let (y, m, d) = civil_from_days(days);
    format!("{d:02}/{m:02}/{y}")
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}

pub fn ssl_status_badge_html(insight: &SslCertInsight) -> String {
    let kind = insight.kind.as_str();
    let label = insight.kind.label();
    let title = match (insight.expires_display.as_deref(), insight.issuer.as_str()) {
        (Some(exp), iss) if !iss.is_empty() => format!("Expires: {exp}. Issuer: {iss}"),
        (Some(exp), _) => format!("Expires: {exp}"),
        _ => insight.detail.clone(),
    };
    let title_esc = title
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;");
    let lock = concat!(
        r#"<svg class="ssl-lock" viewBox="0 0 16 16" width="12" height="12" aria-hidden="true">"#,
        r#"<path fill="currentColor" d="M4 7V5a4 4 0 0 1 8 0v2h1a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V8a1 1 0 0 1 1-1h1zm2 0h4V5a2 2 0 1 0-4 0v2z"/></svg>"#
    );
    format!(
        r#"<span class="manage-badge ssl-badge ssl-{kind}" title="{title}">{lock} {label}</span>"#,
        kind = kind,
        title = title_esc,
        lock = lock,
        label = label,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_openssl_enddate_to_norwegian_date() {
        let unix = parse_openssl_utc("Dec 12 19:43:09 2026 GMT").expect("parse");
        assert_eq!(format_dd_mm_yyyy(unix), "12/12/2026");
    }

    #[test]
    fn wildcard_covers_one_label_only() {
        let sans = vec![
            "*.test2.newstargeted.com".into(),
            "test2.newstargeted.com".into(),
        ];
        assert!(domain_covered("test2.newstargeted.com", &sans, ""));
        assert!(domain_covered("www.test2.newstargeted.com", &sans, ""));
        assert!(!domain_covered("a.b.test2.newstargeted.com", &sans, ""));
        assert!(!domain_covered("other.example.com", &sans, ""));
    }

    #[test]
    fn cn_fallback_covers_domain() {
        assert!(domain_covered("example.com", &[], "CN=example.com, O=Test"));
    }

    #[test]
    fn no_cyberpanel_in_labels() {
        for kind in [
            SslValidityKind::None,
            SslValidityKind::Valid,
            SslValidityKind::ExpiringSoon,
            SslValidityKind::Expired,
            SslValidityKind::Mismatch,
            SslValidityKind::Invalid,
        ] {
            let blob = format!("{} {}", kind.label(), kind.short_label()).to_lowercase();
            assert!(!blob.contains("cyberpanel"));
        }
    }
}
