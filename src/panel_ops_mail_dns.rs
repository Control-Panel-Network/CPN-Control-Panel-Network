//! SPF / DKIM / DMARC DNS helpers for local zones and Cloudflare.

use crate::panel_host_info::host_sidebar_info;
use crate::panel_ops_cloudflare::cloudflare_configured;
use crate::panel_ops_dkim_keys::{
    dkim_dns_name, ensure_dkim_for_domain, read_dkim_txt_value, selector,
};
use crate::panel_ops_dns::{read_zone, write_zone};
use crate::panel_ops_email_auth::{DnsRecordPlan, push_dns_plans_to_cloudflare};
use crate::sites::normalize_domain;

pub fn mail_auth_dns_plans(
    domain: &str,
    dkim_txt: &str,
    server_ip: Option<&str>,
) -> Vec<DnsRecordPlan> {
    let domain = domain.trim().trim_end_matches('.').to_ascii_lowercase();
    let spf = "v=spf1 a mx ~all".to_string();
    let dmarc = format!("v=DMARC1; p=none; rua=mailto:dmarc@{domain}");
    let mut plans = vec![
        DnsRecordPlan {
            record_type: "TXT".into(),
            name: domain.clone(),
            content: spf,
            ttl: 3600,
            note: "SPF: allow this host A/MX to send mail.".into(),
        },
        DnsRecordPlan {
            record_type: "TXT".into(),
            name: format!("{sel}._domainkey.{domain}", sel = selector()),
            content: dkim_txt.to_string(),
            ttl: 3600,
            note: "DKIM public key (selector default).".into(),
        },
        DnsRecordPlan {
            record_type: "TXT".into(),
            name: format!("_dmarc.{domain}"),
            content: dmarc,
            ttl: 3600,
            note: "DMARC policy (p=none starter; tighten later).".into(),
        },
        DnsRecordPlan {
            record_type: "MX".into(),
            name: domain.clone(),
            content: format!("mail.{domain}"),
            ttl: 3600,
            note: "MX to mail.<domain> (priority 10 when pushed to Cloudflare).".into(),
        },
    ];
    if let Some(ip) = server_ip
        .map(str::trim)
        .filter(|v| !v.is_empty() && *v != "Unavailable")
    {
        plans.push(DnsRecordPlan {
            record_type: "A".into(),
            name: format!("mail.{domain}"),
            content: ip.to_string(),
            ttl: 3600,
            note: "A for mail host (local mail).".into(),
        });
        plans.push(DnsRecordPlan {
            record_type: "A".into(),
            name: domain.clone(),
            content: ip.to_string(),
            ttl: 3600,
            note: "A for apex (web + SPF a mechanism).".into(),
        });
    }
    plans
}

fn zone_line_for_plan(plan: &DnsRecordPlan) -> String {
    match plan.record_type.to_ascii_uppercase().as_str() {
        "MX" => format!(
            "{}. IN MX 10 {}\n",
            plan.name.trim_end_matches('.'),
            plan.content.trim_end_matches('.')
        ),
        "TXT" => {
            let escaped = plan.content.replace('\\', "\\\\").replace('"', "\\\"");
            format!(
                "{}. IN TXT \"{escaped}\"\n",
                plan.name.trim_end_matches('.')
            )
        }
        other => format!(
            "{}. IN {other} {}\n",
            plan.name.trim_end_matches('.'),
            plan.content
        ),
    }
}

fn upsert_lines(existing: &str, new_lines: &[String]) -> String {
    let mut kept: Vec<String> = existing
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    for line in new_lines {
        let name = line
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        let rtype = line
            .split_whitespace()
            .find(|p| {
                matches!(
                    p.to_ascii_uppercase().as_str(),
                    "A" | "AAAA" | "CNAME" | "MX" | "TXT" | "NS"
                )
            })
            .unwrap_or("")
            .to_ascii_uppercase();
        kept.retain(|old| {
            let oname = old
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            let otype = old
                .split_whitespace()
                .find(|p| {
                    matches!(
                        p.to_ascii_uppercase().as_str(),
                        "A" | "AAAA" | "CNAME" | "MX" | "TXT" | "NS"
                    )
                })
                .unwrap_or("")
                .to_ascii_uppercase();
            !(oname == name && otype == rtype)
        });
        kept.push(line.trim_end().to_string());
    }
    let mut out = kept.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

pub fn upsert_mail_dns_local(domain: &str) -> Result<String, String> {
    let domain = normalize_domain(domain)?;
    let _ = ensure_dkim_for_domain(&domain)?;
    let dkim_txt = read_dkim_txt_value(&domain)?;
    let ip = host_sidebar_info().ip;
    let ip_opt = if ip == "Unavailable" {
        None
    } else {
        Some(ip.as_str())
    };
    let plans = mail_auth_dns_plans(&domain, &dkim_txt, ip_opt);
    let lines: Vec<String> = plans.iter().map(zone_line_for_plan).collect();
    let existing = read_zone(&domain).unwrap_or_default();
    let merged = upsert_lines(&existing, &lines);
    write_zone(&domain, &merged)?;
    let _ = dkim_dns_name(&domain);
    Ok(format!(
        "Local DNS zone `{domain}` updated with SPF, DKIM, DMARC (and MX/A when IP known)."
    ))
}

pub fn upsert_mail_dns_cloudflare(domain: &str) -> Result<String, String> {
    let domain = normalize_domain(domain)?;
    if !cloudflare_configured() {
        return Ok("Cloudflare not configured; skipped remote SPF/DKIM/DMARC push.".into());
    }
    let _ = ensure_dkim_for_domain(&domain)?;
    let dkim_txt = read_dkim_txt_value(&domain)?;
    let ip = host_sidebar_info().ip;
    let ip_opt = if ip == "Unavailable" {
        None
    } else {
        Some(ip.as_str())
    };
    let mut plans = mail_auth_dns_plans(&domain, &dkim_txt, ip_opt);
    let mx_plans: Vec<DnsRecordPlan> = plans
        .iter()
        .filter(|p| p.record_type.eq_ignore_ascii_case("MX"))
        .cloned()
        .collect();
    plans.retain(|p| !p.record_type.eq_ignore_ascii_case("MX"));
    let mut messages = Vec::new();
    if !plans.is_empty() {
        messages.push(push_dns_plans_to_cloudflare(&domain, &plans)?);
    }
    for mx in mx_plans {
        match crate::panel_ops_cloudflare_api::create_dns_record(
            &domain,
            "MX",
            &mx.name,
            &mx.content,
            mx.ttl,
            Some(10),
            false,
        ) {
            Ok(m) => messages.push(m),
            Err(e) if e.to_ascii_lowercase().contains("already exists") => {
                messages.push(format!("MX already present for {}", mx.name));
            }
            Err(e) => messages.push(format!("MX push note: {e}")),
        }
    }
    Ok(format!(
        "Cloudflare mail DNS for `{domain}`: {}",
        messages.join("; ")
    ))
}

pub fn provision_mail_dns(domain: &str) -> Result<String, String> {
    let local = upsert_mail_dns_local(domain)?;
    let cf = upsert_mail_dns_cloudflare(domain).unwrap_or_else(|e| format!("Cloudflare: {e}"));
    Ok(format!("{local} {cf}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn plans_include_spf_dkim_dmarc() {
        let plans =
            mail_auth_dns_plans("example.com", "v=DKIM1; k=rsa; p=ABC", Some("203.0.113.10"));
        assert!(plans.iter().any(|p| p.content.starts_with("v=spf1")));
        assert!(plans.iter().any(|p| p.name.contains("_domainkey")));
        assert!(plans.iter().any(|p| p.name.starts_with("_dmarc")));
    }

    #[test]
    fn upsert_local_zone_merges() {
        with_test_data_dir(|| {
            let merged = upsert_lines(
                "example.com. IN A 1.2.3.4\n",
                &["example.com. IN TXT \"v=spf1 a mx ~all\"\n".into()],
            );
            assert!(merged.contains("IN A"));
            assert!(merged.contains("v=spf1"));
        });
    }
}
