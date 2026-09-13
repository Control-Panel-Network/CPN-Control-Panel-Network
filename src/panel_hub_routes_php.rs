//! Routes for `/server/php/extensions` manager.

use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_hub_http::{html_ok, login_redirect, redirect_notice, require_panel_user};
use crate::panel_hub_pages_php_ext::php_extensions_page;
use crate::panel_ops_php_ext::{install_extension, uninstall_extension, verify_php_ext_csrf};
use crate::panel_ops_php_host::ensure_host_php_default;
use crate::panel_pages::panel_shell;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::collections::HashMap;
use std::sync::Arc;

fn same_origin_ok(http: &HttpRequest) -> bool {
    let host = http
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if host.is_empty() {
        return true;
    }
    if let Some(origin) = http.headers().get("origin").and_then(|v| v.to_str().ok()) {
        return origin.contains(host);
    }
    if let Some(referer) = http.headers().get("referer").and_then(|v| v.to_str().ok()) {
        return referer.contains(host);
    }
    // Browsers may omit Origin/Referer on same-site navigations; session + CSRF still apply.
    true
}

fn redirect_ext(php: &str, q: &str, notice: Option<&str>, error: Option<&str>) -> HttpResponse {
    let mut base = format!(
        "/server/php/extensions?php={}&load=1",
        crate::panel_hub_http::urlencoding_simple(php)
    );
    if !q.is_empty() {
        base.push_str("&q=");
        base.push_str(&crate::panel_hub_http::urlencoding_simple(q));
    }
    redirect_notice(&base, notice, error)
}

#[get("/server/php/extensions")]
pub async fn server_php_extensions(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let php = query.get("php").map(String::as_str);
    let search = query.get("q").map(String::as_str);
    let loaded = query
        .get("load")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    html_ok(panel_shell(
        &user,
        "server",
        "PHP Extensions",
        &php_extensions_page(
            &user,
            php,
            search,
            loaded,
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
            is_panel_admin(&user),
        ),
    ))
}

#[post("/server/php/extensions/install")]
pub async fn server_php_extensions_install(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return redirect_ext("", "", None, Some("Only the panel admin can install extensions"));
    }
    if !same_origin_ok(&http) {
        return redirect_ext("", "", None, Some("Rejected cross-origin form post"));
    }
    let csrf = form.get("csrf").map(String::as_str).unwrap_or("");
    if !verify_php_ext_csrf(&user, csrf) {
        return redirect_ext("", "", None, Some("Invalid or expired CSRF token"));
    }
    let php = form.get("php").map(String::as_str).unwrap_or("").trim();
    let package = form.get("package").map(String::as_str).unwrap_or("").trim();
    let q = form.get("q").map(String::as_str).unwrap_or("").trim();
    match tokio::task::spawn_blocking({
        let php = php.to_string();
        let package = package.to_string();
        move || install_extension(&php, &package)
    })
    .await
    {
        Ok(Ok(msg)) => redirect_ext(php, q, Some(&msg), None),
        Ok(Err(err)) => redirect_ext(php, q, None, Some(&err)),
        Err(err) => redirect_ext(php, q, None, Some(&format!("Install task failed: {err}"))),
    }
}

#[post("/server/php/extensions/uninstall")]
pub async fn server_php_extensions_uninstall(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return redirect_ext(
            "",
            "",
            None,
            Some("Only the panel admin can uninstall extensions"),
        );
    }
    if !same_origin_ok(&http) {
        return redirect_ext("", "", None, Some("Rejected cross-origin form post"));
    }
    let csrf = form.get("csrf").map(String::as_str).unwrap_or("");
    if !verify_php_ext_csrf(&user, csrf) {
        return redirect_ext("", "", None, Some("Invalid or expired CSRF token"));
    }
    let php = form.get("php").map(String::as_str).unwrap_or("").trim();
    let package = form.get("package").map(String::as_str).unwrap_or("").trim();
    let q = form.get("q").map(String::as_str).unwrap_or("").trim();
    match tokio::task::spawn_blocking({
        let php = php.to_string();
        let package = package.to_string();
        move || uninstall_extension(&php, &package)
    })
    .await
    {
        Ok(Ok(msg)) => redirect_ext(php, q, Some(&msg), None),
        Ok(Err(err)) => redirect_ext(php, q, None, Some(&err)),
        Err(err) => redirect_ext(php, q, None, Some(&format!("Uninstall task failed: {err}"))),
    }
}

#[post("/server/php/extensions/set-default")]
pub async fn server_php_extensions_set_default(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return redirect_ext("", "", None, Some("Only the panel admin can set the host PHP default"));
    }
    if !same_origin_ok(&http) {
        return redirect_ext("", "", None, Some("Rejected cross-origin form post"));
    }
    let csrf = form.get("csrf").map(String::as_str).unwrap_or("");
    if !verify_php_ext_csrf(&user, csrf) {
        return redirect_ext("", "", None, Some("Invalid or expired CSRF token"));
    }
    let php = form.get("php").map(String::as_str).unwrap_or("").trim().to_string();
    match tokio::task::spawn_blocking({
        let php = php.clone();
        move || ensure_host_php_default(Some(&php))
    })
    .await
    {
        Ok(Ok(msg)) => redirect_ext(&php, "", Some(&msg), None),
        Ok(Err(err)) => redirect_ext(&php, "", None, Some(&err)),
        Err(err) => redirect_ext(&php, "", None, Some(&format!("Set-default task failed: {err}"))),
    }
}
