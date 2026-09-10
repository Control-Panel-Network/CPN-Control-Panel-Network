//! Cloudflare connection verify + accessible zone listing (no secrets in results).

use crate::panel_ops_cloudflare::{CloudflareAuthType, load_cloudflare, record_cloudflare_verify};
use crate::panel_ops_cloudflare_api::curl_json;

const CF_API: &str = "https://api.cloudflare.com/client/v4";

/// Safe result of Test connection (no secrets).
#[derive(Debug, Clone)]
pub struct CloudflareVerifyResult {
    pub ok: bool,
    pub token_status: String,
    pub zone_count: u32,
    pub zone_names: Vec<String>,
    pub message: String,
}

/// List zones the configured token can read (paginated, capped).
pub fn list_accessible_zones(max_zones: u32) -> Result<Vec<String>, String> {
    let mut page = 1u32;
    let mut out = Vec::new();
    let cap = max_zones.clamp(1, 200);
    loop {
        let url = format!("{CF_API}/zones?per_page=50&page={page}&status=active");
        let result = curl_json("GET", &url, None)?;
        let arr = result
            .as_array()
            .ok_or_else(|| "Unexpected Cloudflare zone list".to_string())?;
        if arr.is_empty() {
            break;
        }
        for item in arr {
            if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                let n = name.trim().to_ascii_lowercase();
                if !n.is_empty() && !out.iter().any(|x| x == &n) {
                    out.push(n);
                }
            }
            if out.len() as u32 >= cap {
                out.sort();
                return Ok(out);
            }
        }
        if arr.len() < 50 {
            break;
        }
        page += 1;
        if page > 20 {
            break;
        }
    }
    out.sort();
    Ok(out)
}

/// Verify token with Cloudflare and count accessible zones. Persists last status.
pub fn verify_cloudflare_connection() -> Result<CloudflareVerifyResult, String> {
    let settings = load_cloudflare();
    if settings.api_token.trim().is_empty() {
        return Err("Cloudflare API token is not configured".into());
    }

    let token_status = match settings.auth_type {
        CloudflareAuthType::ApiToken => {
            let url = format!("{CF_API}/user/tokens/verify");
            match curl_json("GET", &url, None) {
                Ok(result) => {
                    let status = result
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("active");
                    if status != "active" {
                        let msg = format!("Token status: {status}");
                        let _ = record_cloudflare_verify(false, &msg, None);
                        return Ok(CloudflareVerifyResult {
                            ok: false,
                            token_status: status.to_string(),
                            zone_count: 0,
                            zone_names: vec![],
                            message: msg,
                        });
                    }
                    status.to_string()
                }
                Err(err) => {
                    let msg = format!("Token verify failed: {err}");
                    let _ = record_cloudflare_verify(false, &msg, None);
                    return Ok(CloudflareVerifyResult {
                        ok: false,
                        token_status: "invalid".into(),
                        zone_count: 0,
                        zone_names: vec![],
                        message: msg,
                    });
                }
            }
        }
        CloudflareAuthType::GlobalKey => {
            let url = format!("{CF_API}/user");
            match curl_json("GET", &url, None) {
                Ok(_) => "valid".into(),
                Err(err) => {
                    let msg = format!("Global API Key check failed: {err}");
                    let _ = record_cloudflare_verify(false, &msg, None);
                    return Ok(CloudflareVerifyResult {
                        ok: false,
                        token_status: "invalid".into(),
                        zone_count: 0,
                        zone_names: vec![],
                        message: msg,
                    });
                }
            }
        }
    };

    match list_accessible_zones(100) {
        Ok(zones) => {
            let count = zones.len() as u32;
            let preview: Vec<String> = zones.iter().take(8).cloned().collect();
            let preview_txt = if preview.is_empty() {
                "none".to_string()
            } else {
                preview.join(", ")
            };
            let msg = format!(
                "Connection OK. Token: {token_status}. Zones accessible: {count} ({preview_txt})"
            );
            let _ = record_cloudflare_verify(true, &msg, Some(count));
            Ok(CloudflareVerifyResult {
                ok: true,
                token_status,
                zone_count: count,
                zone_names: zones,
                message: msg,
            })
        }
        Err(err) => {
            let msg = format!("Token looks {token_status}, but listing zones failed: {err}");
            let _ = record_cloudflare_verify(false, &msg, None);
            Ok(CloudflareVerifyResult {
                ok: false,
                token_status,
                zone_count: 0,
                zone_names: vec![],
                message: msg,
            })
        }
    }
}
