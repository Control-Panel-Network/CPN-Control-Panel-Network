//! HTTP routes for Root File Manager (`/server/files`, `/filemanager` aliases).

use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_hub_http::{
    html_ok, login_redirect, redirect_notice, require_panel_user, urlencoding_simple,
};
use crate::panel_hub_pages_files::files_page;
use crate::panel_ops_files::{
    check_rate_limit, copy_entries, create_file, delete_names, mkdir, move_entries, read_text,
    rename_entry, upload_bytes, verify_files_csrf, write_text, MAX_UPLOAD_BYTES,
};
use crate::panel_ops_files_archive::{compress_entries, extract_entry};
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use std::collections::HashMap;
use std::sync::Arc;

fn same_origin_ok(http: &HttpRequest) -> bool {
    let Some(origin) = http
        .headers()
        .get("origin")
        .or_else(|| http.headers().get("referer"))
        .and_then(|v| v.to_str().ok())
    else {
        return true;
    };
    let host = http.connection_info().host().to_string();
    origin.contains(&host)
}

fn files_redirect(path: &str, notice: Option<&str>, error: Option<&str>) -> HttpResponse {
    let base = format!(
        "/server/files?path={}",
        urlencoding_simple(if path.trim().is_empty() { "/" } else { path })
    );
    redirect_notice(&base, notice, error)
}

fn require_admin_user(state: &AppState, http: &HttpRequest) -> Result<String, HttpResponse> {
    let Some(user) = require_panel_user(state, http) else {
        return Err(login_redirect(http));
    };
    if !is_panel_admin(&user) {
        return Err(html_ok(panel_shell(
            &user,
            "server",
            "Root File Manager",
            &files_page(
                &user,
                "/",
                None,
                Some("Only the panel admin can use Root File Manager."),
                None,
                None,
            ),
        )));
    }
    Ok(user)
}

async fn render_files(user: &str, query: &HashMap<String, String>) -> HttpResponse {
    let path = query.get("path").map(String::as_str).unwrap_or("/");
    let notice = query.get("notice").map(String::as_str);
    let error = query.get("error").map(String::as_str);
    let edit = query.get("edit").map(String::as_str);
    let (edit_path, edit_content, err2) = if let Some(ep) = edit {
        match read_text(ep) {
            Ok(body) => (Some(ep.to_string()), Some(body), None),
            Err(e) => (None, None, Some(e)),
        }
    } else {
        (None, None, None)
    };
    let err = error.or(err2.as_deref());
    html_ok(panel_shell(
        user,
        "server",
        "Root File Manager",
        &files_page(
            user,
            path,
            notice,
            err,
            edit_path.as_deref(),
            edit_content.as_deref(),
        ),
    ))
}

#[get("/server/files")]
pub async fn server_files_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    match require_admin_user(&state, &http) {
        Ok(user) => render_files(&user, &query).await,
        Err(resp) => resp,
    }
}

#[get("/filemanager")]
pub async fn filemanager_alias(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    match require_admin_user(&state, &http) {
        Ok(user) => render_files(&user, &query).await,
        Err(resp) => resp,
    }
}

#[get("/server/filemanager")]
pub async fn server_filemanager_alias(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    match require_admin_user(&state, &http) {
        Ok(user) => render_files(&user, &query).await,
        Err(resp) => resp,
    }
}

#[post("/server/files/op")]
pub async fn server_files_op(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let user = match require_admin_user(&state, &http) {
        Ok(u) => u,
        Err(resp) => return resp,
    };
    if !same_origin_ok(&http) {
        return files_redirect("/", None, Some("Rejected cross-origin form post"));
    }
    let csrf = form.get("csrf").map(String::as_str).unwrap_or("");
    if !verify_files_csrf(&user, csrf) {
        return files_redirect("/", None, Some("Invalid or expired CSRF token"));
    }
    if let Err(e) = check_rate_limit(&user) {
        let path = form.get("path").map(String::as_str).unwrap_or("/");
        return files_redirect(path, None, Some(&e));
    }
    let path = form.get("path").map(String::as_str).unwrap_or("/").to_string();
    let op = form.get("op").map(String::as_str).unwrap_or("").trim().to_string();
    let new_name = form
        .get("new_name")
        .map(String::as_str)
        .unwrap_or("")
        .to_string();
    let dest = form.get("dest").map(String::as_str).unwrap_or("").to_string();
    let archive_name = form
        .get("archive_name")
        .map(String::as_str)
        .unwrap_or("")
        .to_string();
    let content = form.get("content").cloned().unwrap_or_default();
    let names: Vec<String> = form
        .get("names_csv")
        .map(|v| {
            v.lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let redirect_path = path.clone();

    let result = tokio::task::spawn_blocking(move || match op.as_str() {
        "mkdir" => mkdir(&path, &new_name),
        "create" => create_file(&path, &new_name),
        "delete" => delete_names(&path, &names),
        "rename" => {
            let from = names.first().map(String::as_str).unwrap_or("");
            rename_entry(&path, from, &new_name)
        }
        "copy" => copy_entries(&path, &names, &dest),
        "move" => move_entries(&path, &names, &dest),
        "write" => write_text(&new_name, &content),
        "compress" => compress_entries(&path, &names, &archive_name),
        "extract" => {
            let archive = names.first().map(String::as_str).unwrap_or("");
            extract_entry(&path, archive)
        }
        _ => Err("Unknown file operation".into()),
    })
    .await;

    match result {
        Ok(Ok(msg)) => files_redirect(&redirect_path, Some(&msg), None),
        Ok(Err(err)) => files_redirect(&redirect_path, None, Some(&err)),
        Err(err) => files_redirect(
            &redirect_path,
            None,
            Some(&format!("Operation failed: {err}")),
        ),
    }
}

#[post("/server/files/upload")]
pub async fn server_files_upload(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let user = match require_admin_user(&state, &http) {
        Ok(u) => u,
        Err(resp) => return resp,
    };
    if !same_origin_ok(&http) {
        return files_redirect("/", None, Some("Rejected cross-origin form post"));
    }
    let csrf = form.get("csrf").map(String::as_str).unwrap_or("");
    if !verify_files_csrf(&user, csrf) {
        return files_redirect("/", None, Some("Invalid or expired CSRF token"));
    }
    if let Err(e) = check_rate_limit(&user) {
        return files_redirect("/", None, Some(&e));
    }
    let path = form
        .get("path")
        .map(|s| s.as_str())
        .unwrap_or("/")
        .to_string();
    let filename = form
        .get("filename")
        .map(|s| s.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let b64 = form.get("file_b64").map(|s| s.as_str()).unwrap_or("");
    let cleaned = b64
        .split(',')
        .next_back()
        .unwrap_or(b64)
        .trim();
    let data = match B64.decode(cleaned) {
        Ok(d) => d,
        Err(_) => return files_redirect(&path, None, Some("Invalid upload encoding")),
    };
    if data.len() as u64 > MAX_UPLOAD_BYTES {
        return files_redirect(&path, None, Some("Upload exceeds size limit"));
    }
    let path_c = path.clone();
    let fname_c = filename.clone();
    match tokio::task::spawn_blocking(move || upload_bytes(&path_c, &fname_c, &data)).await {
        Ok(Ok(msg)) => files_redirect(&path, Some(&msg), None),
        Ok(Err(err)) => files_redirect(&path, None, Some(&err)),
        Err(err) => files_redirect(&path, None, Some(&format!("Upload task failed: {err}"))),
    }
}
