//! Website hostname DNS on Cloudflare: proxied A (+ www CNAME) on create, remove on delete.

use crate::panel_host_info::host_sidebar_info;
use crate::panel_ops_cloudflare::cloudflare_configured;
use crate::panel_ops_cloudflare_api::{
    CfDnsRecord, create_dns_record, delete_dns_record, list_dns_records, update_dns_record,
};
use crate::sites::normalize_domain;

/// Resolve a public A target for site DNS (sidebar host IPv4).
pub fn site_dns_target_ip() -> Result<String, String> {
    let ip = host_sidebar_info().ip;
    let ip = ip.trim();
    if ip.is_empty() || ip.eq_ignore_ascii_case("Unavailable") {
        return Err("Could not resolve a host IPv4 for Cloudflare A records".into());
    }
    if ip.starts_with("127.") {
        return Err("Host IPv4 is loopback; refusing Cloudflare A record".into());
    }
    Ok(ip.to_string())
}

fn names_equal(a: &str, b: &str) -> bool {
    a.trim_end_matches('.')
        .eq_ignore_ascii_case(b.trim_end_matches('.'))
}

fn find_record<'a>(records: &'a [CfDnsRecord], name: &str, rtype: &str) -> Option<&'a CfDnsRecord> {
    records
        .iter()
        .find(|r| r.record_type.eq_ignore_ascii_case(rtype) && names_equal(&r.name, name))
}

fn upsert_proxied_a(zone_domain: &str, hostname: &str, ip: &str) -> Result<String, String> {
    let existing = list_dns_records(zone_domain)?;
    if let Some(found) = find_record(&existing, hostname, "A") {
        if found.content == ip && found.proxied {
            return Ok(format!("A `{hostname}` already points to {ip} (proxied)"));
        }
        let msg = update_dns_record(zone_domain, &found.id, hostname, ip, 1, None, true)?;
        return Ok(format!("updated A `{hostname}`: {msg}"));
    }
    // Do not replace an existing CNAME for the same name; operators may point elsewhere.
    if find_record(&existing, hostname, "CNAME").is_some() {
        return Err(format!(
            "CNAME already exists for `{hostname}`; leave it unchanged (not replacing with A)"
        ));
    }
    let msg = create_dns_record(zone_domain, "A", hostname, ip, 1, None, true)?;
    Ok(format!("added A `{hostname}`: {msg}"))
}

fn upsert_www_cname(zone_domain: &str, hostname: &str) -> Result<String, String> {
    let www = format!("www.{hostname}");
    let existing = list_dns_records(zone_domain)?;
    if let Some(found) = find_record(&existing, &www, "CNAME") {
        let target = found.content.trim_end_matches('.').to_ascii_lowercase();
        let want = hostname.trim_end_matches('.').to_ascii_lowercase();
        if target == want && found.proxied {
            return Ok(format!(
                "CNAME `{www}` already points to `{hostname}` (proxied)"
            ));
        }
        let msg = update_dns_record(zone_domain, &found.id, &www, hostname, 1, None, true)?;
        return Ok(format!("updated CNAME `{www}`: {msg}"));
    }
    if find_record(&existing, &www, "A").is_some() || find_record(&existing, &www, "AAAA").is_some()
    {
        return Ok(format!(
            "skipped www CNAME: A/AAAA already exists for `{www}`"
        ));
    }
    let msg = create_dns_record(zone_domain, "CNAME", &www, hostname, 1, None, true)?;
    Ok(format!("added CNAME `{www}`: {msg}"))
}

/// When Cloudflare is connected, upsert proxied A for the site FQDN and www CNAME.
/// Idempotent. Never deletes unrelated records.
pub fn ensure_site_cloudflare_dns(domain_raw: &str) -> Result<String, String> {
    if !cloudflare_configured() {
        return Ok("Cloudflare not connected; skipped site DNS".into());
    }
    let domain = normalize_domain(domain_raw)?;
    let ip = site_dns_target_ip()?;
    // Zone resolve walks parents (subdomain FQDN -> apex zone).
    let mut messages = Vec::new();
    messages.push(upsert_proxied_a(&domain, &domain, &ip)?);
    // Mirror existing CPN lab zones (www.test, www.cmstest): always offer www.<fqdn>.
    if !domain.starts_with("www.") {
        match upsert_www_cname(&domain, &domain) {
            Ok(m) => messages.push(m),
            Err(e) => messages.push(format!("www CNAME note: {e}")),
        }
    }
    Ok(format!(
        "Cloudflare site DNS for `{domain}`: {}",
        messages.join("; ")
    ))
}

fn delete_matching(
    zone_domain: &str,
    records: &[CfDnsRecord],
    name: &str,
    types: &[&str],
) -> Vec<String> {
    let mut out = Vec::new();
    for rtype in types {
        if let Some(found) = find_record(records, name, rtype) {
            match delete_dns_record(zone_domain, &found.id) {
                Ok(msg) => out.push(format!("removed {rtype} `{name}`: {msg}")),
                Err(e) => {
                    let lower = e.to_ascii_lowercase();
                    if lower.contains("not found") || lower.contains("81044") {
                        out.push(format!("{rtype} `{name}` already gone"));
                    } else {
                        out.push(format!("remove {rtype} `{name}` note: {e}"));
                    }
                }
            }
        }
    }
    out
}

/// Remove CPN-managed website records for this FQDN (A/AAAA and www A/AAAA/CNAME).
/// Idempotent when records are already absent. Does not touch TXT/MX/mail.
pub fn remove_site_cloudflare_dns(domain_raw: &str) -> Result<String, String> {
    if !cloudflare_configured() {
        return Ok("Cloudflare not connected; skipped site DNS remove".into());
    }
    let domain = normalize_domain(domain_raw)?;
    let records = match list_dns_records(&domain) {
        Ok(r) => r,
        Err(e) => {
            let lower = e.to_ascii_lowercase();
            if lower.contains("no active cloudflare zone") {
                return Ok(format!(
                    "No Cloudflare zone for `{domain}`; nothing to remove"
                ));
            }
            return Err(e);
        }
    };
    let mut messages = Vec::new();
    messages.extend(delete_matching(&domain, &records, &domain, &["A", "AAAA"]));
    if !domain.starts_with("www.") {
        let www = format!("www.{domain}");
        messages.extend(delete_matching(
            &domain,
            &records,
            &www,
            &["A", "AAAA", "CNAME"],
        ));
    }
    if messages.is_empty() {
        Ok(format!(
            "Cloudflare site DNS for `{domain}`: no matching A/AAAA/www records"
        ))
    } else {
        Ok(format!(
            "Cloudflare site DNS remove for `{domain}`: {}",
            messages.join("; ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_equal_trims_dot() {
        assert!(names_equal("test2.example.com.", "test2.example.com"));
        assert!(!names_equal("a.example.com", "b.example.com"));
    }
}
