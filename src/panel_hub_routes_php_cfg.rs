//! Routes for `/server/php/configs` (LiteSpeed host default + Apache PHP).

use crate::installer::AppState;
use crate::panel_admin::is_panel_admin;
use crate::panel_hub_http::{
    authenticated_panel_shell, html_ok, login_redirect, redirect_notice, require_panel_user,
};
use crate::panel_hub_pages_php_cfg::php_configurations_page;
use crate::panel_ops_php_ext::verify_php_ext_csrf;
use crate::panel_ops_php_host::ensure_host_php_default;
use crate::panel_ops_php_ini::{
    BASIC_BOOL_KEYS, BASIC_VALUE_KEYS, PhpIniRuntime, apply_basic_settings,
    resolve_php_ini_target_for, restart_php_services, write_php_ini_with_backup,
};
use crate::php_apache_default::ensure_apache_php_default;
use actix_web::{HttpRequest, HttpResponse, post, web};
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
    true
}

fn redirect_cfg(
    php: &str,
    tab: &str,
    runtime: &str,
    notice: Option<&str>,
    error: Option<&str>,
) -> HttpResponse {
    let base = format!(
        "/server/php/configs?php={}&tab={}&runtime={}",
        crate::panel_hub_http::urlencoding_simple(php),
        crate::panel_hub_http::urlencoding_simple(tab),
        crate::panel_hub_http::urlencoding_simple(runtime)
    );
    redirect_notice(&base, notice, error)
}

fn form_runtime(form: &HashMap<String, String>) -> String {
    PhpIniRuntime::parse(form.get("runtime").map(String::as_str))
        .as_str()
        .to_string()
}

fn require_admin_csrf(
    http: &HttpRequest,
    user: &str,
    form: &HashMap<String, String>,
    deny: &str,
) -> Option<HttpResponse> {
    let runtime = form_runtime(form);
    if !is_panel_admin(user) {
        return Some(redirect_cfg("", "basic", &runtime, None, Some(deny)));
    }
    if !same_origin_ok(http) {
        return Some(redirect_cfg(
            "",
            "basic",
            &runtime,
            None,
            Some("Rejected cross-origin form post"),
        ));
    }
    let csrf = form.get("csrf").map(String::as_str).unwrap_or("");
    if !verify_php_ext_csrf(user, csrf) {
        return Some(redirect_cfg(
            "",
            "basic",
            &runtime,
            None,
            Some("Invalid or expired CSRF token"),
        ));
    }
    None
}

/// GET PHP Configurations page (paired with POST via web::resource in main).
pub async fn server_php_configs(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let php = query.get("php").map(String::as_str);
    let tab = query.get("tab").map(String::as_str).unwrap_or("basic");
    let runtime = query.get("runtime").map(String::as_str);
    html_ok(authenticated_panel_shell(
        &http,
        &user,
        "server",
        "PHP Configurations",
        &php_configurations_page(
            &user,
            php,
            tab,
            runtime,
            query.get("notice").map(String::as_str),
            query.get("error").map(String::as_str),
            is_panel_admin(&user),
        ),
    ))
}

/// Accidental GET (refresh, bookmark, automation) must never white-page 404.
pub async fn server_php_configs_set_default_get(
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let php = query.get("php").map(String::as_str).unwrap_or("");
    let tab = query.get("tab").map(String::as_str).unwrap_or("basic");
    let runtime = query
        .get("runtime")
        .map(String::as_str)
        .unwrap_or("litespeed");
    redirect_cfg(
        php,
        tab,
        runtime,
        Some("Use Set as host default from the PHP Configurations form (POST)."),
        None,
    )
}

/// POST `/server/php/configs` with `op=set-default` / `set-apache-default`.
pub async fn server_php_configs_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let op = form
        .get("op")
        .map(String::as_str)
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match op.as_str() {
        "set-default" | "set_default" => apply_configs_set_default(http, state, form).await,
        "set-apache-default" | "set_apache_default" => {
            apply_configs_set_apache_default(http, state, form).await
        }
        _ => {
            let php = form.get("php").map(String::as_str).unwrap_or("");
            let tab = form.get("tab").map(String::as_str).unwrap_or("basic");
            let runtime = form_runtime(&form);
            redirect_cfg(
                php,
                tab,
                &runtime,
                None,
                Some("Unknown PHP Configurations action. Use the page forms."),
            )
        }
    }
}

async fn apply_configs_set_default(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_admin_csrf(
        &http,
        &user,
        &form,
        "Only the panel admin can set the host PHP default",
    ) {
        return resp;
    }
    let php = form
        .get("php")
        .map(String::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let tab = form
        .get("tab")
        .map(String::as_str)
        .unwrap_or("basic")
        .to_string();
    let runtime = form_runtime(&form);
    match tokio::task::spawn_blocking({
        let php = php.clone();
        move || ensure_host_php_default(Some(&php))
    })
    .await
    {
        Ok(Ok(msg)) => redirect_cfg(&php, &tab, &runtime, Some(&msg), None),
        Ok(Err(err)) => redirect_cfg(&php, &tab, &runtime, None, Some(&err)),
        Err(err) => redirect_cfg(
            &php,
            &tab,
            &runtime,
            None,
            Some(&format!("Set-default task failed: {err}")),
        ),
    }
}

async fn apply_configs_set_apache_default(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_admin_csrf(
        &http,
        &user,
        &form,
        "Only the panel admin can set the Apache PHP default",
    ) {
        return resp;
    }
    let php = form
        .get("php")
        .map(String::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let tab = form
        .get("tab")
        .map(String::as_str)
        .unwrap_or("basic")
        .to_string();
    match tokio::task::spawn_blocking({
        let php = php.clone();
        move || ensure_apache_php_default(Some(&php))
    })
    .await
    {
        Ok(Ok(msg)) => redirect_cfg(&php, &tab, "apache", Some(&msg), None),
        Ok(Err(err)) => redirect_cfg(&php, &tab, "apache", None, Some(&err)),
        Err(err) => redirect_cfg(
            &php,
            &tab,
            "apache",
            None,
            Some(&format!("Set Apache PHP default task failed: {err}")),
        ),
    }
}

/// Legacy POST `/server/php/configs/set-default` (same apply as op=set-default).
pub async fn server_php_configs_set_default(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    apply_configs_set_default(http, state, form).await
}

#[post("/server/php/configs/save-basic")]
pub async fn server_php_configs_save_basic(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_admin_csrf(
        &http,
        &user,
        &form,
        "Only the panel admin can edit PHP configurations",
    ) {
        return resp;
    }
    let php = form
        .get("php")
        .map(String::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let runtime = form_runtime(&form);
    let runtime_kind = PhpIniRuntime::parse(Some(&runtime));
    let bools: Vec<(String, bool)> = BASIC_BOOL_KEYS
        .iter()
        .map(|k| {
            (
                (*k).to_string(),
                form.get(*k)
                    .map(|v| v == "1" || v.eq_ignore_ascii_case("on"))
                    .unwrap_or(false),
            )
        })
        .collect();
    let values: Vec<(String, String)> = BASIC_VALUE_KEYS
        .iter()
        .filter_map(|k| {
            form.get(*k)
                .map(|v| ((*k).to_string(), v.trim().to_string()))
        })
        .collect();
    match tokio::task::spawn_blocking({
        let php = php.clone();
        move || {
            let target = resolve_php_ini_target_for(&php, runtime_kind)?;
            let backup = apply_basic_settings(&target.ini_path, &bools, &values)?;
            Ok::<_, String>(format!(
                "Saved basic settings for PHP {} ({}). Backup: {}",
                php,
                runtime_kind.as_str(),
                backup.display()
            ))
        }
    })
    .await
    {
        Ok(Ok(msg)) => redirect_cfg(&php, "basic", &runtime, Some(&msg), None),
        Ok(Err(err)) => redirect_cfg(&php, "basic", &runtime, None, Some(&err)),
        Err(err) => redirect_cfg(
            &php,
            "basic",
            &runtime,
            None,
            Some(&format!("Save task failed: {err}")),
        ),
    }
}

#[post("/server/php/configs/save-advanced")]
pub async fn server_php_configs_save_advanced(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) = require_admin_csrf(
        &http,
        &user,
        &form,
        "Only the panel admin can edit PHP configurations",
    ) {
        return resp;
    }
    let php = form
        .get("php")
        .map(String::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let runtime = form_runtime(&form);
    let runtime_kind = PhpIniRuntime::parse(Some(&runtime));
    let ini = form.get("ini").cloned().unwrap_or_default();
    if ini.len() > 2_000_000 {
        return redirect_cfg(
            &php,
            "advanced",
            &runtime,
            None,
            Some("php.ini payload is too large"),
        );
    }
    match tokio::task::spawn_blocking({
        let php = php.clone();
        move || {
            let target = resolve_php_ini_target_for(&php, runtime_kind)?;
            let backup = write_php_ini_with_backup(&target.ini_path, &ini)?;
            Ok::<_, String>(format!(
                "Saved php.ini for PHP {} ({}). Backup: {}",
                php,
                runtime_kind.as_str(),
                backup.display()
            ))
        }
    })
    .await
    {
        Ok(Ok(msg)) => redirect_cfg(&php, "advanced", &runtime, Some(&msg), None),
        Ok(Err(err)) => redirect_cfg(&php, "advanced", &runtime, None, Some(&err)),
        Err(err) => redirect_cfg(
            &php,
            "advanced",
            &runtime,
            None,
            Some(&format!("Save task failed: {err}")),
        ),
    }
}

#[post("/server/php/configs/restart")]
pub async fn server_php_configs_restart(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    if let Some(resp) =
        require_admin_csrf(&http, &user, &form, "Only the panel admin can restart PHP")
    {
        return resp;
    }
    let php = form
        .get("php")
        .map(String::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let runtime = form_runtime(&form);
    let runtime_kind = PhpIniRuntime::parse(Some(&runtime));
    let tab = if form.contains_key("ini") {
        "advanced"
    } else {
        "basic"
    };
    match tokio::task::spawn_blocking({
        let php = php.clone();
        move || {
            let target = resolve_php_ini_target_for(&php, runtime_kind)?;
            restart_php_services(target.family)
        }
    })
    .await
    {
        Ok(Ok(msg)) => redirect_cfg(&php, tab, &runtime, Some(&msg), None),
        Ok(Err(err)) => redirect_cfg(&php, tab, &runtime, None, Some(&err)),
        Err(err) => redirect_cfg(
            &php,
            tab,
            &runtime,
            None,
            Some(&format!("Restart task failed: {err}")),
        ),
    }
}
