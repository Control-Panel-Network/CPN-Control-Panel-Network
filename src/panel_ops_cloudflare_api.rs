//! Cloudflare DNS API client (curl). Prefer API Token (Bearer). Never log tokens.

use crate::panel_ops_cloudflare::{
    CloudflareAuthType, CloudflareSettings, load_cloudflare, normalize_record_type,
};
use crate::panel_ops_dns::{list_zones, read_zone};
use serde::Deserialize;
use serde_json::{Value, json};
use std::process::{Command, Stdio};

const CF_API: &str = "https://api.cloudflare.com/client/v4";

#[derive(Debug, Clone)]
pub struct CfDnsRecord {
    pub id: String,
    pub name: String,
    pub record_type: String,
    pub content: String,
    pub ttl: u32,
    pub priority: Option<u16>,
    pub proxied: bool,
}

#[derive(Debug, Deserialize)]
struct CfEnvelope {
    success: bool,
    errors: Option<Vec<CfError>>,
    result: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct CfError {
    message: Option<String>,
}

fn auth_headers(settings: &CloudflareSettings) -> Result<Vec<String>, String> {
    let token = settings.api_token.trim();
    if token.is_empty() {
        return Err("Cloudflare API token is not configured".into());
    }
    let mut headers = vec!["Content-Type: application/json".to_string()];
    match settings.auth_type {
        CloudflareAuthType::ApiToken => {
            headers.push(format!("Authorization: Bearer {token}"));
        }
        CloudflareAuthType::GlobalKey => {
            let email = settings.email.trim();
            if email.is_empty() {
                return Err("Cloudflare email is required for Global API Key auth".into());
            }
            headers.push(format!("X-Auth-Email: {email}"));
            headers.push(format!("X-Auth-Key: {token}"));
        }
    }
    Ok(headers)
}

fn curl_json(method: &str, url: &str, body: Option<&str>) -> Result<Value, String> {
    let settings = load_cloudflare();
    let headers = auth_headers(&settings)?;
    let mut cmd = Command::new("curl");
    cmd.args([
        "--fail-with-body",
        "--silent",
        "--show-error",
        "--max-time",
        "60",
        "-X",
        method,
        url,
    ]);
    for h in &headers {
        cmd.args(["-H", h]);
    }
    if let Some(b) = body {
        cmd.args(["--data-binary", b]);
    }
    let output = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Could not call Cloudflare API (curl missing?): {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if stdout.trim().is_empty() && !output.status.success() {
        return Err(format!(
            "Cloudflare API request failed: {}",
            stderr.trim().chars().take(200).collect::<String>()
        ));
    }
    let env: CfEnvelope = serde_json::from_str(&stdout).map_err(|e| {
        format!(
            "Cloudflare API returned non-JSON ({}): {}",
            e,
            stdout.chars().take(120).collect::<String>()
        )
    })?;
    if !env.success {
        let msg = env
            .errors
            .unwrap_or_default()
            .into_iter()
            .filter_map(|e| e.message)
            .collect::<Vec<_>>()
            .join("; ");
        return Err(if msg.is_empty() {
            "Cloudflare API reported failure".into()
        } else {
            format!("Cloudflare API: {msg}")
        });
    }
    Ok(env.result.unwrap_or(Value::Null))
}

fn parse_record(v: &Value) -> Option<CfDnsRecord> {
    Some(CfDnsRecord {
        id: v.get("id")?.as_str()?.to_string(),
        name: v.get("name")?.as_str()?.to_string(),
        record_type: v.get("type")?.as_str()?.to_string(),
        content: v.get("content")?.as_str().unwrap_or("").to_string(),
        ttl: v.get("ttl")?.as_u64().unwrap_or(1) as u32,
        priority: v.get("priority").and_then(|p| p.as_u64()).map(|p| p as u16),
        proxied: v.get("proxied").and_then(|p| p.as_bool()).unwrap_or(false),
    })
}

pub fn resolve_zone_id(domain: &str) -> Result<String, String> {
    let domain = domain.trim().to_ascii_lowercase();
    if domain.is_empty() {
        return Err("Domain is required".into());
    }
    let url = format!(
        "{CF_API}/zones?name={}&status=active",
        urlencoding_simple(&domain)
    );
    let result = curl_json("GET", &url, None)?;
    let arr = result
        .as_array()
        .ok_or_else(|| "Unexpected Cloudflare zone list".to_string())?;
    if arr.is_empty() {
        return Err(format!(
            "No active Cloudflare zone found for `{domain}`. Confirm the zone exists and the token can read it."
        ));
    }
    arr[0]
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Cloudflare zone response missing id".into())
}

fn urlencoding_simple(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}

pub fn list_dns_records(domain: &str) -> Result<Vec<CfDnsRecord>, String> {
    let zone_id = resolve_zone_id(domain)?;
    let mut page = 1u32;
    let mut out = Vec::new();
    loop {
        let url = format!("{CF_API}/zones/{zone_id}/dns_records?per_page=100&page={page}");
        let result = curl_json("GET", &url, None)?;
        let arr = result
            .as_array()
            .ok_or_else(|| "Unexpected Cloudflare DNS list".to_string())?;
        if arr.is_empty() {
            break;
        }
        for item in arr {
            if let Some(r) = parse_record(item) {
                out.push(r);
            }
        }
        if arr.len() < 100 {
            break;
        }
        page += 1;
        if page > 50 {
            break;
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name).then(a.record_type.cmp(&b.record_type)));
    Ok(out)
}

pub fn create_dns_record(
    domain: &str,
    record_type: &str,
    name: &str,
    content: &str,
    ttl: u32,
    priority: Option<u16>,
    proxied: bool,
) -> Result<String, String> {
    let rtype = normalize_record_type(record_type)?;
    let zone_id = resolve_zone_id(domain)?;
    let name = name.trim();
    let content = content.trim();
    if name.is_empty() || content.is_empty() {
        return Err("Name and value are required".into());
    }
    let ttl = if ttl == 0 { 1 } else { ttl };
    let mut body = json!({
        "type": rtype,
        "name": name,
        "content": content,
        "ttl": ttl,
    });
    if matches!(rtype.as_str(), "A" | "AAAA" | "CNAME") {
        body["proxied"] = json!(proxied);
    }
    if let Some(p) = priority {
        if matches!(rtype.as_str(), "MX" | "SRV") {
            body["priority"] = json!(p);
        }
    }
    let payload = serde_json::to_string(&body).map_err(|e| e.to_string())?;
    let url = format!("{CF_API}/zones/{zone_id}/dns_records");
    let _ = curl_json("POST", &url, Some(&payload))?;
    Ok(format!("Added {rtype} record `{name}`"))
}

pub fn delete_dns_record(domain: &str, record_id: &str) -> Result<String, String> {
    let record_id = record_id.trim();
    if record_id.is_empty()
        || !record_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err("Invalid record id".into());
    }
    let zone_id = resolve_zone_id(domain)?;
    let url = format!("{CF_API}/zones/{zone_id}/dns_records/{record_id}");
    let _ = curl_json("DELETE", &url, None)?;
    Ok("DNS record deleted".into())
}

pub fn set_proxy(domain: &str, record_id: &str, proxied: bool) -> Result<String, String> {
    let record_id = record_id.trim();
    if record_id.is_empty() {
        return Err("Invalid record id".into());
    }
    let zone_id = resolve_zone_id(domain)?;
    let url = format!("{CF_API}/zones/{zone_id}/dns_records/{record_id}");
    let get = curl_json("GET", &url, None)?;
    let rtype = get
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if !matches!(rtype.as_str(), "A" | "AAAA" | "CNAME") {
        return Err(format!("Proxy is not supported for {rtype} records"));
    }
    let mut body = get.clone();
    body["proxied"] = json!(proxied);
    // PATCH with essential fields only
    let patch = json!({
        "type": rtype,
        "name": get.get("name").and_then(|v| v.as_str()).unwrap_or(""),
        "content": get.get("content").and_then(|v| v.as_str()).unwrap_or(""),
        "ttl": get.get("ttl").and_then(|v| v.as_u64()).unwrap_or(1),
        "proxied": proxied,
    });
    let payload = serde_json::to_string(&patch).map_err(|e| e.to_string())?;
    let _ = curl_json("PATCH", &url, Some(&payload))?;
    Ok(if proxied {
        "Proxy enabled".into()
    } else {
        "Proxy disabled (DNS only)".into()
    })
}

/// Push local CPN zone file A/AAAA/CNAME/MX/TXT lines into Cloudflare when sync is enabled.
pub fn sync_local_zone_to_cloudflare(domain: &str) -> Result<String, String> {
    let settings = load_cloudflare();
    if !settings.sync_local {
        return Err("Sync local records to Cloudflare is disabled in API Settings".into());
    }
    let domain = domain.trim().to_ascii_lowercase();
    let content = match read_zone(&domain) {
        Ok(c) => c,
        Err(_) => {
            // Try listing: if no local zone, still refresh remote view success message
            let zones = list_zones().unwrap_or_default();
            if !zones.iter().any(|z| z == &domain) {
                return Err(format!(
                    "No local zone file for `{domain}` under CPN DNS data. Create one under Server > DNS Zones, or manage records only in Cloudflare."
                ));
            }
            return Err("Could not read local zone file".into());
        }
    };
    let mut created = 0u32;
    let mut skipped = 0u32;
    let mut errors = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        // Accept: name [ttl] IN TYPE value...
        if parts.len() < 4 {
            skipped += 1;
            continue;
        }
        let name = parts[0].trim_end_matches('.');
        let (rtype, value_idx) = if parts[1].eq_ignore_ascii_case("IN") {
            (parts[2], 3)
        } else if parts.len() >= 5 && parts[2].eq_ignore_ascii_case("IN") {
            (parts[3], 4)
        } else {
            skipped += 1;
            continue;
        };
        let Ok(rtype) = normalize_record_type(rtype) else {
            skipped += 1;
            continue;
        };
        if !matches!(rtype.as_str(), "A" | "AAAA" | "CNAME" | "MX" | "TXT" | "NS") {
            skipped += 1;
            continue;
        }
        let mut priority = None;
        let content_val = if rtype == "MX" && parts.len() > value_idx + 1 {
            priority = parts[value_idx].parse().ok();
            parts[value_idx + 1..]
                .join(" ")
                .trim_matches('"')
                .to_string()
        } else {
            parts[value_idx..].join(" ").trim_matches('"').to_string()
        };
        match create_dns_record(&domain, &rtype, name, &content_val, 1, priority, false) {
            Ok(_) => created += 1,
            Err(e) => {
                // Duplicate often means already present
                if e.to_ascii_lowercase().contains("already exists") {
                    skipped += 1;
                } else {
                    errors.push(e);
                }
            }
        }
    }
    if !errors.is_empty() {
        return Err(format!(
            "Synced {created} record(s); {skipped} skipped; errors: {}",
            errors.into_iter().take(3).collect::<Vec<_>>().join(" | ")
        ));
    }
    Ok(format!(
        "Sync complete: {created} created/updated attempts, {skipped} skipped for `{domain}`"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_domain_keeps_dots() {
        assert_eq!(urlencoding_simple("a.b-c.example.com"), "a.b-c.example.com");
    }
}
