//! HTTP routes for Root and Site File Manager.

use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_hub_http::{
    html_ok, login_redirect, redirect_notice, require_panel_user, urlencoding_simple,
};
use crate::panel_hub_pages_files::{root_files_page, site_files_page};
use crate::panel_ops_files::{
    MAX_UPLOAD_BYTES, check_rate_limit, copy_entries, create_file, delete_names, mkdir,
    move_entries, read_text, rename_entry, upload_bytes, verify_files_csrf, write_text,
};
use crate::panel_ops_files_archive::{compress_entries, extract_entry};
use crate::panel_pages::panel_shell;
use crate::site_acl::{SitePerm, require_manage_site};
use crate::sites::site_home_from_record;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
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

fn root_redirect(path: &str, notice: Option<&str>, error: Option<&str>) -> HttpResponse {
    let base = format!(
        "/server/files?path={}",
        urlencoding_simple(if path.trim().is_empty() { "/" } else { path })
    );
    redirect_notice(&base, notice, error)
}

fn site_redirect(
    domain: &str,
    path: &str,
    notice: Option<&str>,
    error: Option<&str>,
) -> HttpResponse {
    let base = format!(
        "/websites/files?domain={}&path={}",
        urlencoding_simple(domain),
        urlencoding_simple(if path.trim().is_empty() { "/" } else { path })
    );
    redirect_notice(&base, notice, error)
}

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

fn run_op(
    op: &str,
    path: &str,
    names: &[String],
    new_name: &str,
    dest: &str,
    archive_name: &str,
    content: &str,
    jail: &Path,
) -> Result<String, String> {
    match op {
        "mkdir" => mkdir(path, new_name, jail),
        "create" => create_file(path, new_name, jail),
        "delete" => delete_names(path, names, jail),
        "rename" => {
            let from = names.first().map(String::as_str).unwrap_or("");
            rename_entry(path, from, new_name, jail)
        }
        "copy" => copy_entries(path, names, dest, jail),
        "move" => move_entries(path, names, dest, jail),
        "write" => write_text(new_name, content, jail),
        "compress" => compress_entries(path, names, archive_name, jail),
        "extract" => {
            let archive = names.first().map(String::as_str).unwrap_or("");
            extract_entry(path, archive, jail)
        }
        _ => Err("Unknown file operation".into()),
    }
}

fn parse_op_form(
    form: &HashMap<String, String>,
) -> (String, String, String, String, String, String, Vec<String>) {
    let path = form
        .get("path")
        .map(String::as_str)
        .unwrap_or("/")
        .to_string();
    let op = form
        .get("op")
        .map(String::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let new_name = form
        .get("new_name")
        .map(String::as_str)
        .unwrap_or("")
        .to_string();
    let dest = form
        .get("dest")
        .map(String::as_str)
        .unwrap_or("")
        .to_string();
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
    (path, op, new_name, dest, archive_name, content, names)
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
