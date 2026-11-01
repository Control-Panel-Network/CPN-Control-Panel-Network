//! HTTP endpoint for signed remote notification broadcasts.

use crate::panel_action_log;
use crate::panel_notifications_remote::{
    load_remote_push_config, process_remote_broadcast, RemoteBroadcastRequest,
};
use actix_web::{HttpRequest, HttpResponse, post, web};

fn json_body(status: u16, value: serde_json::Value) -> HttpResponse {
    let body = serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{\"ok\":false}".into());
    let mut response = match status {
        200 => HttpResponse::Ok(),
        400 => HttpResponse::BadRequest(),
        401 => HttpResponse::Unauthorized(),
        403 => HttpResponse::Forbidden(),
        404 => HttpResponse::NotFound(),
        429 => HttpResponse::TooManyRequests(),
        _ => HttpResponse::InternalServerError(),
    };
    response
        .content_type("application/json; charset=utf-8")
        .body(body)
}

fn header_str<'a>(req: &'a HttpRequest, name: &str) -> &'a str {
    req.headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
}

fn peer_ip(req: &HttpRequest) -> String {
    if let Some(fwd) = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
    {
        if let Some(first) = fwd.split(',').next() {
            let trimmed = first.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    req.connection_info()
        .peer_addr()
        .unwrap_or("unknown")
        .to_string()
}

fn status_for_error(message: &str) -> u16 {
    let lower = message.to_ascii_lowercase();
    if lower.contains("disabled") || lower.contains("not configured") {
        404
    } else if lower.contains("signature")
        || lower.contains("timestamp")
        || lower.contains("nonce")
        || lower.contains("replay")
    {
        401
    } else if lower.contains("ip not allowed") {
        403
    } else if lower.contains("rate limit") {
        429
    } else {
        400
    }
}

/// Signed broadcast from News Targeted api. No panel session; HMAC + replay protection required.
#[post("/api/nt/notifications/broadcast")]
pub async fn panel_notifications_remote_broadcast(
    http: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    let cfg = load_remote_push_config();
    let timestamp = header_str(&http, "X-CPN-Timestamp");
    let nonce = header_str(&http, "X-CPN-Nonce");
    let signature = header_str(&http, "X-CPN-Signature");
    let peer = peer_ip(&http);

    let parsed: RemoteBroadcastRequest = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => {
            return json_body(
                400,
                serde_json::json!({ "ok": false, "error": "invalid JSON body" }),
            );
        }
    };

    let operator = if parsed.operator.trim().is_empty() {
        "nt-remote".to_string()
    } else {
        parsed.operator.trim().to_string()
    };
    match process_remote_broadcast(&cfg, &peer, timestamp, nonce, signature, &body, parsed) {
        Ok(result) => {
            let target = format!("delivered={}", result.delivered);
            let message = format!(
                "remote broadcast delivered={} failed={}",
                result.delivered, result.failed
            );
            let _ = panel_action_log::record(
                &operator,
                "panel.notifications.remote",
                &target,
                result.ok,
                &message,
            );
            json_body(
                200,
                serde_json::json!({
                    "ok": result.ok,
                    "delivered": result.delivered,
                    "failed": result.failed,
                    "skipped": result.skipped,
                    "errors": result.errors,
                }),
            )
        }
        Err(err) => {
            let status = status_for_error(&err);
            let _ = panel_action_log::record(
                &operator,
                "panel.notifications.remote",
                "denied",
                false,
                &err,
            );
            json_body(
                status,
                serde_json::json!({ "ok": false, "error": err }),
            )
        }
    }
}
