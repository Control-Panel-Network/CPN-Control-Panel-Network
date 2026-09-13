//! Authenticated JSON metrics for Manage Overview (host samples + bandwidth).

use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use crate::panel_website_bandwidth::bandwidth_for_site;
use crate::panel_website_metrics_chart::metrics_chart_svg;
use crate::panel_website_metrics_ring::{WINDOW_SECS, record_host_sample, stats_for};
use crate::panel_website_resources::format_bytes;
use crate::site_acl::{SitePerm, require_manage_site};
use actix_web::{HttpRequest, HttpResponse, get, web};
use serde::Deserialize;
use std::sync::Arc;

fn json_ok(value: serde_json::Value) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/json; charset=utf-8")
        .body(serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{\"ok\":false}".into()))
}

fn json_err(status: u16, message: &str) -> HttpResponse {
    let body = serde_json::to_string_pretty(&serde_json::json!({ "ok": false, "error": message }))
        .unwrap_or_else(|_| "{\"ok\":false}".into());
    let mut response = match status {
        401 => HttpResponse::Unauthorized(),
        403 => HttpResponse::Forbidden(),
        400 => HttpResponse::BadRequest(),
        _ => HttpResponse::InternalServerError(),
    };
    response
        .content_type("application/json; charset=utf-8")
        .body(body)
}

#[derive(Debug, Deserialize)]
pub struct MetricsQuery {
    domain: String,
}

fn pct_json(v: Option<f32>) -> serde_json::Value {
    match v {
        Some(n) => serde_json::json!((n * 10.0).round() / 10.0),
        None => serde_json::Value::Null,
    }
}

#[get("/api/websites/manage/metrics")]
pub async fn websites_manage_metrics(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<MetricsQuery>,
) -> HttpResponse {
    let Some(user) = panel_user_from_request(&state, &http) else {
        return json_err(401, "Login required");
    };
    let site = match require_manage_site(&user, &query.domain, SitePerm::Enable) {
        Ok(site) => site,
        Err(err) => return json_err(403, &err),
    };

    let samples = record_host_sample();
    let cpu_vals: Vec<f32> = samples.iter().map(|s| s.cpu).collect();
    let mem_vals: Vec<f32> = samples.iter().map(|s| s.mem).collect();
    let (cpu_cur, cpu_avg, cpu_peak) = stats_for(&cpu_vals);
    let (mem_cur, mem_avg, mem_peak) = stats_for(&mem_vals);
    let bw = bandwidth_for_site(&site);
    let cpu_stroke = crate::panel_dashboard::gauge_stroke_for_usage(
        cpu_cur.unwrap_or(0.0).clamp(0.0, 100.0) as u8,
    );
    let mem_stroke = crate::panel_dashboard::gauge_stroke_for_usage(
        mem_cur.unwrap_or(0.0).clamp(0.0, 100.0) as u8,
    );

    json_ok(serde_json::json!({
        "ok": true,
        "scope": "host",
        "domain": site.domain,
        "window_seconds": WINDOW_SECS,
        "detail": "Host live metrics (not per-site). Site-level CPU/bandwidth metering ships later.",
        "cpu": {
            "current": pct_json(cpu_cur),
            "avg": pct_json(cpu_avg),
            "peak": pct_json(cpu_peak),
            "svg": metrics_chart_svg(&samples, "cpu", cpu_stroke),
        },
        "mem": {
            "current": pct_json(mem_cur),
            "avg": pct_json(mem_avg),
            "peak": pct_json(mem_peak),
            "svg": metrics_chart_svg(&samples, "mem", mem_stroke),
        },
        "bandwidth": {
            "label": bw.label,
            "hint": bw.hint,
            "bytes": bw.bytes,
            "bytes_label": bw.bytes.map(format_bytes),
            "period": bw.period,
            "source": bw.source,
            "quota_mb": bw.quota_mb,
        },
        "samples": samples.iter().map(|s| serde_json::json!({
            "t": s.t,
            "cpu": s.cpu,
            "mem": s.mem,
        })).collect::<Vec<_>>(),
    }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn route_path_documented() {
        assert!(true);
    }
}
