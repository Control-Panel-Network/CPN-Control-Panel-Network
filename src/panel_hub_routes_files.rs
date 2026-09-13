//! HTTP routes for Root File Manager (`/server/files`, `/filemanager` aliases).

use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_hub_http::{html_ok, login_redirect, require_panel_user};
use crate::panel_hub_pages_files::root_files_page;
use crate::panel_hub_routes_files_common::{parse_op_form, root_redirect, run_op, same_origin_ok};
use crate::panel_ops_files::{
    MAX_UPLOAD_BYTES, check_rate_limit, read_text, upload_bytes, verify_files_csrf,
};
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn require_admin_user(state: &AppState, http: &HttpRequest) -> Result<String, Box<HttpResponse>> {
    let Some(user) = require_panel_user(state, http) else {
        return Err(Box::new(login_redirect(http)));
    };
    if !is_panel_admin(&user) {
        return Err(Box::new(html_ok(panel_shell(
            &user,
            "root-files",
            "Root File Manager",
            &root_files_page(
                &user,
                "/",
                None,
                Some("Only the panel admin can use Root File Manager."),
                None,
                None,
            ),
        ))));
    }
    Ok(user)
}

async fn render_root_files(user: &str, query: &HashMap<String, String>) -> HttpResponse {
    let path = query.get("path").map(String::as_str).unwrap_or("/");
    let notice = query.get("notice").map(String::as_str);
    let error = query.get("error").map(String::as_str);
    let edit = query.get("edit").map(String::as_str);
    let jail = Path::new("/");
    let (edit_path, edit_content, err2) = if let Some(ep) = edit {
        match read_text(ep, jail) {
            Ok(body) => (Some(ep.to_string()), Some(body), None),
            Err(e) => (None, None, Some(e)),
        }
    } else {
        (None, None, None)
    };
    let err = error.or(err2.as_deref());
    html_ok(panel_shell(
        user,
        "root-files",
        "Root File Manager",
        &root_files_page(
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
        Ok(user) => render_root_files(&user, &query).await,
        Err(resp) => *resp,
    }
}

#[get("/filemanager")]
pub async fn filemanager_alias(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    match require_admin_user(&state, &http) {
        Ok(user) => render_root_files(&user, &query).await,
        Err(resp) => *resp,
    }
}

#[get("/server/filemanager")]
pub async fn server_filemanager_alias(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    match require_admin_user(&state, &http) {
        Ok(user) => render_root_files(&user, &query).await,
        Err(resp) => *resp,
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
        Err(resp) => return *resp,
    };
    if !same_origin_ok(&http) {
        return root_redirect("/", None, Some("Rejected cross-origin form post"));
    }
    let csrf = form.get("csrf").map(String::as_str).unwrap_or("");
    if !verify_files_csrf(&user, csrf) {
        return root_redirect("/", None, Some("Invalid or expired CSRF token"));
    }
    if let Err(e) = check_rate_limit(&user) {
        let path = form.get("path").map(String::as_str).unwrap_or("/");
        return root_redirect(path, None, Some(&e));
    }
    let (path, op, new_name, dest, archive_name, content, names) = parse_op_form(&form);
    let redirect_path = path.clone();
    let jail = PathBuf::from("/");
    let result = tokio::task::spawn_blocking(move || {
        run_op(
            &op,
            &path,
            &names,
            &new_name,
            &dest,
            &archive_name,
            &content,
            &jail,
        )
    })
    .await;

    match result {
        Ok(Ok(msg)) => root_redirect(&redirect_path, Some(&msg), None),
        Ok(Err(err)) => root_redirect(&redirect_path, None, Some(&err)),
        Err(err) => root_redirect(
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
        Err(resp) => return *resp,
    };
    if !same_origin_ok(&http) {
        return root_redirect("/", None, Some("Rejected cross-origin form post"));
    }
    let csrf = form.get("csrf").map(String::as_str).unwrap_or("");
    if !verify_files_csrf(&user, csrf) {
        return root_redirect("/", None, Some("Invalid or expired CSRF token"));
    }
    if let Err(e) = check_rate_limit(&user) {
        return root_redirect("/", None, Some(&e));
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
    let cleaned = b64.split(',').next_back().unwrap_or(b64).trim();
    let data = match B64.decode(cleaned) {
        Ok(d) => d,
        Err(_) => return root_redirect(&path, None, Some("Invalid upload encoding")),
    };
    if data.len() as u64 > MAX_UPLOAD_BYTES {
        return root_redirect(&path, None, Some("Upload exceeds size limit"));
    }
    let path_c = path.clone();
    let fname_c = filename.clone();
    let jail = PathBuf::from("/");
    match tokio::task::spawn_blocking(move || upload_bytes(&path_c, &fname_c, &data, &jail)).await {
        Ok(Ok(msg)) => root_redirect(&path, Some(&msg), None),
        Ok(Err(err)) => root_redirect(&path, None, Some(&err)),
        Err(err) => root_redirect(&path, None, Some(&format!("Upload task failed: {err}"))),
    }
}
