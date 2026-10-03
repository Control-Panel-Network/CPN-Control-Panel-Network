//! JSON directory listing and versioned File Manager assets.

use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_hub_http::{login_redirect, require_panel_user};
use crate::panel_hub_pages_files_assets::{fm_asset_version, fm_css_body, fm_js_body};
use crate::panel_ops_path::{
    LIST_TIME_BUDGET, MAX_LIST_ENTRIES, list_dir_bounded, resolve_under_jail,
};
use crate::site_acl::{SitePerm, require_manage_site};
use crate::sites::site_home_from_record;
use actix_web::{HttpRequest, HttpResponse, route, web};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Bound JSON listing so Actix never sits until `408 Request Timeout`.
const LIST_HTTP_BUDGET: Duration = Duration::from_secs(8);

#[derive(Serialize)]
struct FileListResponse {
    ok: bool,
    path: String,
    entries: Vec<FileListEntry>,
    truncated: bool,
    timed_out: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct FileListEntry {
    basename: String,
    label: String,
    is_dir: bool,
    size: u64,
    mtime: String,
    mode: String,
}

fn json_list_ok(body: &FileListResponse) -> HttpResponse {
    let payload = serde_json::to_string_pretty(body).unwrap_or_else(|_| {
        r#"{"ok":false,"path":"","entries":[],"truncated":false,"timed_out":false,"error":"encode"}"#
            .into()
    });
    HttpResponse::Ok()
        .content_type("application/json; charset=utf-8")
        .body(payload)
}

fn json_list_fail(status: u16, error: &str, timed_out: bool) -> HttpResponse {
    let body = FileListResponse {
        ok: false,
        path: String::new(),
        entries: Vec::new(),
        truncated: true,
        timed_out,
        error: Some(error.to_string()),
    };
    let payload = serde_json::to_string_pretty(&body).unwrap_or_else(|_| {
        format!(
            r#"{{"ok":false,"path":"","entries":[],"truncated":true,"timed_out":{timed_out},"error":"encode"}}"#
        )
    });
    let mut b = match status {
        403 => HttpResponse::Forbidden(),
        503 => HttpResponse::ServiceUnavailable(),
        _ => HttpResponse::BadRequest(),
    };
    if status == 503 {
        b.insert_header(("Retry-After", "5"));
    }
    b.content_type("application/json; charset=utf-8")
        .body(payload)
}

async fn list_under_jail(path_q: String, jail: PathBuf) -> HttpResponse {
    let result = tokio::time::timeout(
        LIST_HTTP_BUDGET,
        tokio::task::spawn_blocking(move || {
            let resolved = resolve_under_jail(&path_q, &jail)?;
            let listing = list_dir_bounded(&resolved, MAX_LIST_ENTRIES, LIST_TIME_BUDGET)?;
            let entries = listing
                .entries
                .into_iter()
                .map(|e| FileListEntry {
                    basename: e.basename,
                    label: e.label,
                    is_dir: e.is_dir,
                    size: e.size,
                    mtime: e.mtime_label,
                    mode: e.mode_label,
                })
                .collect();
            Ok::<FileListResponse, String>(FileListResponse {
                ok: true,
                path: resolved.display().to_string(),
                entries,
                truncated: listing.truncated,
                timed_out: listing.timed_out,
                error: None,
            })
        }),
    )
    .await;
    match result {
        Ok(Ok(Ok(body))) => json_list_ok(&body),
        Ok(Ok(Err(err))) => json_list_fail(400, &err, false),
        Ok(Err(_)) => json_list_fail(503, "Listing task failed. Reload to try again.", false),
        Err(_) => json_list_fail(
            503,
            "Directory listing timed out. Open a narrower path (do not list huge restore trees at once).",
            true,
        ),
    }
}

#[route("/server/files/list", method = "GET", method = "HEAD")]
pub async fn server_files_list(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return json_list_fail(
            403,
            "Only the panel admin can use Root File Manager.",
            false,
        );
    }
    let path = query
        .get("path")
        .cloned()
        .unwrap_or_else(|| "/".to_string());
    list_under_jail(path, PathBuf::from("/")).await
}

#[route("/websites/files/list", method = "GET", method = "HEAD")]
pub async fn site_files_list(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let domain = query.get("domain").map(String::as_str).unwrap_or("");
    let site = match require_manage_site(&user, domain, SitePerm::Enable) {
        Ok(s) => s,
        Err(err) => return json_list_fail(403, &err, false),
    };
    let jail = site_home_from_record(&site);
    let home = jail.display().to_string();
    let path = query
        .get("path")
        .cloned()
        .filter(|p| !p.trim().is_empty())
        .unwrap_or(home);
    list_under_jail(path, jail).await
}

fn asset_response(body: &'static str, ctype: &str) -> HttpResponse {
    HttpResponse::Ok()
        .content_type(ctype)
        .insert_header((
            "Cache-Control",
            "public, max-age=86400, stale-while-revalidate=60",
        ))
        .insert_header(("X-CPN-FM-Asset", fm_asset_version()))
        .body(body)
}

fn require_panel_or_login(state: &AppState, http: &HttpRequest) -> Result<(), Box<HttpResponse>> {
    if require_panel_user(state, http).is_none() {
        return Err(Box::new(login_redirect(http)));
    }
    Ok(())
}

#[route("/server/files/assets/fm.css", method = "GET", method = "HEAD")]
pub async fn server_files_asset_css(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    if let Err(resp) = require_panel_or_login(&state, &http) {
        return *resp;
    }
    asset_response(fm_css_body(), "text/css; charset=utf-8")
}

#[route("/server/files/assets/fm.js", method = "GET", method = "HEAD")]
pub async fn server_files_asset_js(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    if let Err(resp) = require_panel_or_login(&state, &http) {
        return *resp;
    }
    asset_response(fm_js_body(), "application/javascript; charset=utf-8")
}

#[cfg(test)]
mod tests {
    use super::LIST_HTTP_BUDGET;
    use std::time::Duration;

    #[test]
    fn list_budget_is_below_actix_client_timeout() {
        assert!(LIST_HTTP_BUDGET < Duration::from_secs(30));
        assert!(LIST_HTTP_BUDGET >= Duration::from_secs(4));
    }
}
