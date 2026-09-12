//! MTA-STS and BIMI helpers: recommended DNS, policy files, optional Cloudflare push.

use crate::account::data_dir;
use crate::panel_ops_cloudflare::cloudflare_configured;
use crate::panel_ops_cloudflare_api::{create_dns_record, list_dns_records, update_dns_record};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsRecordPlan {
    pub record_type: String,
    pub name: String,
    pub content: String,
    pub ttl: u32,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtaStsSettings {
    pub domain: String,
    /// none | testing | enforce
    pub mode: String,
    pub max_age: u32,
    pub mx: Vec<String>,
    pub enabled: bool,
}

impl Default for MtaStsSettings {
    fn default() -> Self {
        Self {
            domain: String::new(),
            mode: "testing".into(),
            max_age: 86400,
            mx: Vec::new(),
            enabled: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BimiSettings {
    pub domain: String,
    pub logo_svg_url: String,
    /// Optional Authority URL / VMC (PEM or HTTPS URL). Empty when not used.
    pub authority_url: String,
    pub enabled: bool,
}

fn auth_root() -> PathBuf {
    data_dir().join("email-auth")
}

fn mta_sts_path(domain: &str) -> PathBuf {
    auth_root()
        .join(sanitize_domain(domain))
        .join("mta-sts.json")
}

fn bimi_path(domain: &str) -> PathBuf {
    auth_root().join(sanitize_domain(domain)).join("bimi.json")
}

fn policy_txt_path(domain: &str) -> PathBuf {
    auth_root()
        .join(sanitize_domain(domain))
        .join("mta-sts.txt")
}

fn sanitize_domain(raw: &str) -> String {
    raw.trim()
        .trim_end_matches('.')
        .to_ascii_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

pub fn validate_domain(raw: &str) -> Result<String, String> {
    let d = sanitize_domain(raw);
    if d.is_empty() || !d.contains('.') || d.len() > 253 {
        return Err("Enter a valid domain (example: mail.example.com or example.com)".into());
    }
    if d.starts_with('.') || d.ends_with('.') || d.contains("..") {
        return Err("Domain format is invalid".into());
    }
    Ok(d)
}

fn normalize_mode(raw: &str) -> Result<String, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "none" => Ok("none".into()),
        "testing" => Ok("testing".into()),
        "enforce" => Ok("enforce".into()),
        _ => Err("MTA-STS mode must be none, testing, or enforce".into()),
    }
}

pub fn load_mta_sts(domain: &str) -> MtaStsSettings {
    let Ok(domain) = validate_domain(domain) else {
        return MtaStsSettings::default();
    };
    let path = mta_sts_path(&domain);
    if !path.is_file() {
        return MtaStsSettings {
            mx: vec![format!("mail.{domain}")],
            domain,
            ..Default::default()
        };
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_else(|| MtaStsSettings {
            domain,
            ..Default::default()
        })
}

pub fn save_mta_sts(settings: &MtaStsSettings) -> Result<(), String> {
    let domain = validate_domain(&settings.domain)?;
    let mode = normalize_mode(&settings.mode)?;
    let mut next = settings.clone();
    next.domain = domain.clone();
    next.mode = mode;
    next.max_age = next.max_age.clamp(60, 31536000);
    next.mx = next
        .mx
        .iter()
        .map(|m| m.trim().trim_end_matches('.').to_ascii_lowercase())
        .filter(|m| !m.is_empty())
        .collect();
    if next.mx.is_empty() {
        return Err("Add at least one MX hostname for the MTA-STS policy".into());
    }
    let dir = auth_root().join(sanitize_domain(&domain));
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create email-auth dir: {e}"))?;
    let raw = serde_json::to_string_pretty(&next)
        .map_err(|e| format!("Could not serialize MTA-STS settings: {e}"))?;
    fs::write(mta_sts_path(&domain), raw)
        .map_err(|e| format!("Could not write MTA-STS settings: {e}"))?;
    let policy = render_mta_sts_policy(&next);
    fs::write(policy_txt_path(&domain), policy)
        .map_err(|e| format!("Could not write mta-sts.txt: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(mta_sts_path(&domain), fs::Permissions::from_mode(0o600));
        let _ = fs::set_permissions(policy_txt_path(&domain), fs::Permissions::from_mode(0o644));
    }
    Ok(())
}

pub fn render_mta_sts_policy(settings: &MtaStsSettings) -> String {
    let mut out = String::from("version: STSv1\n");
    out.push_str(&format!("mode: {}\n", settings.mode));
    for mx in &settings.mx {
        out.push_str(&format!("mx: {mx}\n"));
    }
    out.push_str(&format!("max_age: {}\n", settings.max_age));
    out
}

pub fn mta_sts_dns_records(settings: &MtaStsSettings) -> Vec<DnsRecordPlan> {
    let domain = settings.domain.trim_end_matches('.');
    let id = format!("v=STSv1; id={}", chrono_like_id());
    vec![
        DnsRecordPlan {
            record_type: "TXT".into(),
            name: format!("_mta-sts.{domain}"),
            content: id,
            ttl: 3600,
            note: "MTA-STS discovery TXT. Update id when the policy file changes."
                .into(),
        },
        DnsRecordPlan {
            record_type: "A".into(),
            name: format!("mta-sts.{domain}"),
            content: "203.0.113.10".into(),
            ttl: 3600,
            note: "Replace with your policy host IPv4 (or use CNAME to a host that serves HTTPS /.well-known/mta-sts.txt). Cloudflare push skips this placeholder A record; create the host manually."
                .into(),
        },
    ]
}

pub fn load_bimi(domain: &str) -> BimiSettings {
    let Ok(domain) = validate_domain(domain) else {
        return BimiSettings::default();
    };
    let path = bimi_path(&domain);
    if !path.is_file() {
        return BimiSettings {
            domain,
            ..Default::default()
        };
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_else(|| BimiSettings {
            domain,
            ..Default::default()
        })
}

pub fn save_bimi(settings: &BimiSettings) -> Result<(), String> {
    let domain = validate_domain(&settings.domain)?;
    let logo = settings.logo_svg_url.trim().to_string();
    if settings.enabled && logo.is_empty() {
        return Err("BIMI requires an HTTPS SVG logo URL".into());
    }
    if !logo.is_empty() && !logo.to_ascii_lowercase().starts_with("https://") {
        return Err("BIMI logo URL must use https://".into());
    }
    let mut next = settings.clone();
    next.domain = domain.clone();
    next.logo_svg_url = logo;
    next.authority_url = settings.authority_url.trim().to_string();
    let dir = auth_root().join(sanitize_domain(&domain));
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create email-auth dir: {e}"))?;
    let raw = serde_json::to_string_pretty(&next)
        .map_err(|e| format!("Could not serialize BIMI settings: {e}"))?;
    fs::write(bimi_path(&domain), raw)
        .map_err(|e| format!("Could not write BIMI settings: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(bimi_path(&domain), fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn bimi_dns_records(settings: &BimiSettings) -> Vec<DnsRecordPlan> {
    let domain = settings.domain.trim_end_matches('.');
    let mut content = format!("v=BIMI1; l={};", settings.logo_svg_url.trim());
    if !settings.authority_url.trim().is_empty() {
        content.push_str(&format!(" a={};", settings.authority_url.trim()));
    }
    vec![DnsRecordPlan {
        record_type: "TXT".into(),
        name: format!("default._bimi.{domain}"),
        content,
        ttl: 3600,
        note: "BIMI TXT. Many inbox providers need a Validating Mark Certificate (VMC) for logos; Cloudflare mainly helps as DNS. SnappyMail/Roundcube usually do not show BIMI logos like Gmail."
            .into(),
    }]
}

pub fn policy_file_path_display(domain: &str) -> String {
    policy_txt_path(domain).display().to_string()
}

fn chrono_like_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

/// Add-only / upsert TXT or CNAME by name+type. Never deletes unrelated records.
pub fn push_dns_plans_to_cloudflare(
    domain: &str,
    plans: &[DnsRecordPlan],
) -> Result<String, String> {
    if !cloudflare_configured() {
        return Err(
            "Cloudflare API token is not configured. Open Cloudflare DNS > API Settings, or copy the records manually."
                .into(),
        );
    }
    let existing = list_dns_records(domain)?;
    let mut messages = Vec::new();
    for plan in plans {
        // Skip RFC documentation placeholder A records (must be set by the operator).
        if plan.record_type.eq_ignore_ascii_case("A") && plan.content.starts_with("203.0.113.") {
            messages.push(format!(
                "skipped placeholder A `{}` (set a real IP or CNAME for the policy host)",
                plan.name
            ));
            continue;
        }
        let name = plan.name.trim_end_matches('.').to_ascii_lowercase();
        let rtype = plan.record_type.to_ascii_uppercase();
        let proxied = false; // MTA-STS / BIMI must not be orange-cloud proxied as TXT; CNAME for mta-sts host is DNS-only.
        if let Some(found) = existing.iter().find(|r| {
            r.record_type.eq_ignore_ascii_case(&rtype)
                && r.name.trim_end_matches('.').eq_ignore_ascii_case(&name)
        }) {
            let msg = update_dns_record(
                domain,
                &found.id,
                &plan.name,
                &plan.content,
                plan.ttl,
                None,
                proxied,
            )?;
            messages.push(format!("updated: {msg}"));
        } else {
            let msg = create_dns_record(
                domain,
                &plan.record_type,
                &plan.name,
                &plan.content,
                plan.ttl,
                None,
                proxied,
            )?;
            messages.push(format!("added: {msg}"));
        }
    }
    Ok(messages.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn mta_sts_policy_and_dns() {
        with_test_data_dir(|| {
            let s = MtaStsSettings {
                domain: "example.com".into(),
                mode: "testing".into(),
                mx: vec!["mail.example.com".into()],
                enabled: true,
                ..MtaStsSettings::default()
            };
            save_mta_sts(&s).unwrap();
            let policy = render_mta_sts_policy(&load_mta_sts("example.com"));
            assert!(policy.contains("version: STSv1"));
            assert!(policy.contains("mode: testing"));
            assert!(policy.contains("mx: mail.example.com"));
            let dns = mta_sts_dns_records(&load_mta_sts("example.com"));
            assert!(dns.iter().any(|r| r.name.starts_with("_mta-sts.")));
            assert!(dns.iter().any(|r| r.name.starts_with("mta-sts.")));
        });
    }

    #[test]
    fn bimi_requires_https_logo() {
        with_test_data_dir(|| {
            let mut s = BimiSettings {
                domain: "example.com".into(),
                enabled: true,
                logo_svg_url: "http://example.com/logo.svg".into(),
                ..BimiSettings::default()
            };
            assert!(save_bimi(&s).is_err());
            s.logo_svg_url = "https://example.com/logo.svg".into();
            save_bimi(&s).unwrap();
            let dns = bimi_dns_records(&load_bimi("example.com"));
            assert_eq!(dns[0].name, "default._bimi.example.com");
            assert!(dns[0].content.contains("v=BIMI1"));
        });
    }
}
