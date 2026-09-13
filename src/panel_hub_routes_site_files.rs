//! HTTP routes for Site File Manager (jailed to site home).

use crate::installer::AppState;
use crate::panel_hub_http::{html_ok, login_redirect, require_panel_user, urlencoding_simple};
use crate::panel_hub_pages_files::site_files_page;
use crate::panel_hub_routes_files_common::{
    parse_op_form, run_op, same_origin_ok, site_redirect,
};
use crate::panel_ops_files::{
    MAX_UPLOAD_BYTES, check_rate_limit, read_text, upload_bytes, verify_files_csrf,
};
use crate::panel_pages::panel_shell;
use crate::site_acl::{SitePerm, require_manage_site};
use crate::sites::site_home_from_record;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use std::collections::HashMap;
use std::sync::Arc;

async fn render_site_files_page(
    http: &HttpRequest,
    state: &AppState,
    query: &HashMap<String, String>,
) -> HttpResponse {
    let Some(user) = require_panel_user(state, http) else {
        return login_redirect(http);
    };
    let domain = query.get("domain").map(String::as_str).unwrap_or("");
    let site = match require_manage_site(&user, domain, SitePerm::Enable) {
        Ok(s) => s,
        Err(err) => {
            return HttpResponse::SeeOther()
                .append_header((
                    "Location",
                    format!("/websites?error={}", urlencoding_simple(&err)),
                ))
                .finish();
        }
    };
    let jail = site_home_from_record(&site);
    let home = jail.display().to_string();
    let path = query
        .get("path")
        .map(String::as_str)
        .filter(|p| !p.trim().is_empty())
        .unwrap_or(home.as_str());
    let notice = query.get("notice").map(String::as_str);
    let error = query.get("error").map(String::as_str);
    let edit = query.get("edit").map(String::as_str);
    let (edit_path, edit_content, err2) = if let Some(ep) = edit {
        match read_text(ep, &jail) {
            Ok(body) => (Some(ep.to_string()), Some(body), None),
            Err(e) => (None, None, Some(e)),
        }
    } else {
        (None, None, None)
    };
    let err = error.or(err2.as_deref());
    html_ok(panel_shell(
        &user,
        "websites",
        &format!("File Manager: {}", site.domain),
        &site_files_page(
            &user,
            &site.domain,
            &jail,
            path,
            notice,
            err,
            edit_path.as_deref(),
            edit_content.as_deref(),
        ),
    ))
}

#[get("/websites/files")]
pub async fn site_files_page_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    render_site_files_page(&http, &state, &query).await
}

#[get("/filemanager/site")]
pub async fn site_filemanager_alias(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    render_site_files_page(&http, &state, &query).await
}

#[post("/websites/files/op")]
pub async fn site_files_op(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let domain = form
        .get("domain")
        .map(String::as_str)
        .unwrap_or("")
        .to_string();
    let site = match require_manage_site(&user, &domain, SitePerm::Enable) {
        Ok(s) => s,
        Err(err) => return site_redirect(&domain, "/", None, Some(&err)),
    };
    if !same_origin_ok(&http) {
        return site_redirect(&domain, "/", None, Some("Rejected cross-origin form post"));
    }
    let csrf = form.get("csrf").map(String::as_str).unwrap_or("");
    if !verify_files_csrf(&user, csrf) {
        return site_redirect(&domain, "/", None, Some("Invalid or expired CSRF token"));
    }
    if let Err(e) = check_rate_limit(&user) {
        let path = form.get("path").map(String::as_str).unwrap_or("/");
        return site_redirect(&domain, path, None, Some(&e));
    }
    let jail = site_home_from_record(&site);
    let home = jail.display().to_string();
    let (path, op, new_name, dest, archive_name, content, names) = parse_op_form(&form);
    let path = if path.trim().is_empty() {
        home.clone()
    } else {
        path
    };
    let redirect_path = path.clone();
    let domain_c = site.domain.clone();
    let jail_c = jail.clone();
    let result = tokio::task::spawn_blocking(move || {
        run_op(
            &op,
            &path,
            &names,
            &new_name,
            &dest,
            &archive_name,
            &content,
            &jail_c,
        )
    })
    .await;
    match result {
        Ok(Ok(msg)) => site_redirect(&domain_c, &redirect_path, Some(&msg), None),
        Ok(Err(err)) => site_redirect(&domain_c, &redirect_path, None, Some(&err)),
        Err(err) => site_redirect(
            &domain_c,
            &redirect_path,
            None,
            Some(&format!("Operation failed: {err}")),
        ),
    }
}

#[post("/websites/files/upload")]
pub async fn site_files_upload(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let domain = form
        .get("domain")
        .map(String::as_str)
        .unwrap_or("")
        .to_string();
    let site = match require_manage_site(&user, &domain, SitePerm::Enable) {
        Ok(s) => s,
        Err(err) => return site_redirect(&domain, "/", None, Some(&err)),
    };
    if !same_origin_ok(&http) {
        return site_redirect(&domain, "/", None, Some("Rejected cross-origin form post"));
    }
    let csrf = form.get("csrf").map(String::as_str).unwrap_or("");
    if !verify_files_csrf(&user, csrf) {
        return site_redirect(&domain, "/", None, Some("Invalid or expired CSRF token"));
    }
    if let Err(e) = check_rate_limit(&user) {
        return site_redirect(&domain, "/", None, Some(&e));
    }
    let jail = site_home_from_record(&site);
    let home = jail.display().to_string();
    let path = form
        .get("path")
        .map(|s| s.as_str())
        .filter(|p| !p.trim().is_empty())
        .unwrap_or(home.as_str())
        .to_string();
    let filename = form
        .get("filename")
        .map(|s| s.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let b64 = form.get("file_b64").map(|s| s.as_str()).unwrap_or("");
    let cleaned = b64.split(',').next_back().unwrap_or(b64).trim();
    let data = match B64.decode(cleaned) {
        Ok(d) => d,
        Err(_) => return site_redirect(&site.domain, &path, None, Some("Invalid upload encoding")),
    };
    if data.len() as u64 > MAX_UPLOAD_BYTES {
        return site_redirect(&site.domain, &path, None, Some("Upload exceeds size limit"));
    }
    let path_c = path.clone();
    let fname_c = filename.clone();
    let jail_c = jail.clone();
    let domain_c = site.domain.clone();
    match tokio::task::spawn_blocking(move || upload_bytes(&path_c, &fname_c, &data, &jail_c)).await
    {
        Ok(Ok(msg)) => site_redirect(&domain_c, &path, Some(&msg), None),
        Ok(Err(err)) => site_redirect(&domain_c, &path, None, Some(&err)),
        Err(err) => site_redirect(
            &domain_c,
            &path,
            None,
            Some(&format!("Upload task failed: {err}")),
        ),
    }
}
