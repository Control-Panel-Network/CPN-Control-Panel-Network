//! Authenticated `/wordpress` panel routes.

use crate::installer::AppState;
use crate::panel_hub_http::{
    html_blocking, html_ok, login_redirect, redirect_notice, require_panel_user, urlencoding_simple,
};
use crate::panel_pages::panel_shell;
use crate::panel_wordpress_ui::{
    WordpressInstallDraft, wordpress_install_page, wordpress_list_page, wordpress_manage_page,
    wordpress_subsites_install_page, wordpress_subsites_list_page,
};
use crate::wordpress_install::{
    UploadedPluginZip, WordpressInstallRequest, install_wordpress, parse_plugin_sources,
    store_uploaded_plugin_zips,
};
use crate::wordpress_manage::{
    activate_theme, all_sites_snapshot, delete_wordpress, install_plugin, refresh_wordpress_site,
    set_debugging, set_maintenance, set_password_protection, set_search_indexing,
};
use crate::wordpress_scan::scan_wordpress_sites;
use crate::wordpress_wpcli::{detect_wp_cli, ensure_wp_cli, sanitize_wp_cli_error};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use std::sync::Arc;

const WP_INSTALL_DRAFT_COOKIE: &str = "cpn_wp_install_draft";

fn wp_redirect(base: &str, notice: Option<&str>, error: Option<&str>) -> HttpResponse {
    redirect_notice(base, notice, error)
}

fn parse_bool_flag(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn encode_install_draft(draft: &WordpressInstallDraft) -> String {
    let json = serde_json::to_vec(draft).unwrap_or_default();
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json)
}

fn decode_install_draft(raw: &str) -> Option<WordpressInstallDraft> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(raw.trim())
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn read_install_draft(http: &HttpRequest) -> Option<WordpressInstallDraft> {
    let cookie_header = http.headers().get("cookie")?.to_str().ok()?;
    for part in cookie_header.split(';') {
        let part = part.trim();
        let Some((name, value)) = part.split_once('=') else {
            continue;
        };
        if name == WP_INSTALL_DRAFT_COOKIE {
            return decode_install_draft(value);
        }
    }
    None
}

fn cookie_has_install_draft(http: &HttpRequest) -> bool {
    http.headers()
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .map(|c| {
            c.split(';')
                .any(|p| p.trim().starts_with(&format!("{WP_INSTALL_DRAFT_COOKIE}=")))
        })
        .unwrap_or(false)
}

fn clear_install_draft_header() -> String {
    format!("{WP_INSTALL_DRAFT_COOKIE}=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax")
}

fn set_install_draft_header(draft: &WordpressInstallDraft) -> String {
    format!(
        "{WP_INSTALL_DRAFT_COOKIE}={}; Path=/; Max-Age=120; HttpOnly; SameSite=Lax",
        encode_install_draft(draft)
    )
}

fn draft_from_form(form: &WordpressInstallForm) -> WordpressInstallDraft {
    WordpressInstallDraft {
        domain: form.domain.clone(),
        title: form.title.clone(),
        site_url: form.site_url.clone(),
        admin_user: form.admin_user.clone(),
        admin_password: form.admin_password.clone(),
        admin_email: form.admin_email.clone(),
        plugin_sources: form.plugin_sources.clone(),
        create_site_if_missing: parse_bool_flag(&form.create_site_if_missing),
    }
}

fn wp_redirect_with_draft(
    base: &str,
    notice: Option<&str>,
    error: Option<&str>,
    draft: Option<&WordpressInstallDraft>,
) -> HttpResponse {
    let mut loc = base.to_string();
    let mut first = !base.contains('?');
    if let Some(n) = notice {
        loc.push(if first { '?' } else { '&' });
        first = false;
        loc.push_str("notice=");
        loc.push_str(&urlencoding_simple(n));
    }
    if let Some(e) = error {
        loc.push(if first { '?' } else { '&' });
        loc.push_str("error=");
        loc.push_str(&urlencoding_simple(e));
    }
    let mut builder = HttpResponse::SeeOther();
    builder.append_header(("Location", loc));
    if let Some(draft) = draft {
        builder.append_header(("Set-Cookie", set_install_draft_header(draft)));
    }
    builder.finish()
}

fn html_ok_install(http: &HttpRequest, body: String) -> HttpResponse {
    let mut builder = HttpResponse::Ok();
    builder.content_type("text/html; charset=utf-8");
    if cookie_has_install_draft(http) {
        builder.append_header(("Set-Cookie", clear_install_draft_header()));
    }
    builder.body(body)
}

#[get("/wordpress")]
pub async fn wordpress_list_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let notice = query.get("notice").cloned();
    let error = query.get("error").cloned();
    let q = query.get("q").cloned();
    html_blocking(move || {
        let wp_cli = detect_wp_cli();
        panel_shell(
            &user,
            "wordpress",
            "WordPress",
            &wordpress_list_page(notice.as_deref(), error.as_deref(), &wp_cli, q.as_deref()),
        )
    })
    .await
}

#[get("/wordpress/subsites")]
pub async fn wordpress_subsites_list_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let notice = query.get("notice").cloned();
    let error = query.get("error").cloned();
    let q = query.get("q").cloned();
    html_blocking(move || {
        let wp_cli = detect_wp_cli();
        panel_shell(
            &user,
            "wordpress-subsites",
            "WordPress Sub-sites",
            &wordpress_subsites_list_page(
                notice.as_deref(),
                error.as_deref(),
                &wp_cli,
                q.as_deref(),
            ),
        )
    })
    .await
}

#[get("/wordpress/install")]
pub async fn wordpress_install_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let draft = read_install_draft(&http);
    html_ok_install(
        &http,
        panel_shell(
            &user,
            "wordpress",
            "Install WordPress",
            &wordpress_install_page(
                query.get("notice").map(String::as_str),
                query.get("error").map(String::as_str),
                &user,
                draft.as_ref(),
            ),
        ),
    )
}

#[get("/wordpress/subsites/install")]
pub async fn wordpress_subsites_install_get(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let draft = read_install_draft(&http);
    html_ok_install(
        &http,
        panel_shell(
            &user,
            "wordpress-subsites",
            "Install WordPress Sub-site",
            &wordpress_subsites_install_page(
                query.get("notice").map(String::as_str),
                query.get("error").map(String::as_str),
                &user,
                draft.as_ref(),
            ),
        ),
    )
}

#[derive(Debug, Default, Clone, serde::Deserialize)]
pub struct WordpressInstallForm {
    #[serde(default)]
    domain: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    site_url: String,
    #[serde(default)]
    admin_user: String,
    #[serde(default)]
    admin_password: String,
    #[serde(default)]
    admin_email: String,
    #[serde(default)]
    plugin_sources: String,
    /// JSON array of { "name": "...", "data": "<base64>" } plugin ZIP uploads.
    #[serde(default)]
    plugin_zips_json: String,
    #[serde(default)]
    create_site_if_missing: String,
}

fn parse_plugin_zips_json(raw: &str) -> Result<Vec<UploadedPluginZip>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    #[derive(serde::Deserialize)]
    struct ZipItem {
        #[serde(default)]
        name: String,
        #[serde(default)]
        data: String,
    }
    let items: Vec<ZipItem> = serde_json::from_str(trimmed)
        .map_err(|e| format!("Invalid plugin ZIP upload payload: {e}"))?;
    use base64::Engine;
    let mut out = Vec::new();
    for item in items {
        let cleaned = item
            .data
            .split(',')
            .next_back()
            .unwrap_or(item.data.as_str())
            .trim();
        if cleaned.is_empty() {
            continue;
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(cleaned)
            .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(cleaned))
            .map_err(|_| format!("Plugin ZIP {} is not valid base64", item.name))?;
        out.push(UploadedPluginZip {
            filename: if item.name.trim().is_empty() {
                "plugin.zip".into()
            } else {
                item.name
            },
            bytes,
        });
    }
    Ok(out)
}

#[post("/wordpress/install")]
pub async fn wordpress_install_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<WordpressInstallForm>,
) -> HttpResponse {
    let uploads = match parse_plugin_zips_json(&form.plugin_zips_json) {
        Ok(v) => v,
        Err(error) => return wp_redirect("/wordpress/install", None, Some(&error)),
    };
    wordpress_install_for_kind(http, state, form.into_inner(), uploads, false).await
}

#[post("/wordpress/subsites/install")]
pub async fn wordpress_subsites_install_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<WordpressInstallForm>,
) -> HttpResponse {
    let uploads = match parse_plugin_zips_json(&form.plugin_zips_json) {
        Ok(v) => v,
        Err(error) => return wp_redirect("/wordpress/subsites/install", None, Some(&error)),
    };
    wordpress_install_for_kind(http, state, form.into_inner(), uploads, true).await
}

async fn wordpress_install_for_kind(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: WordpressInstallForm,
    uploads: Vec<UploadedPluginZip>,
    expect_subdomain: bool,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let install_path = if expect_subdomain {
        "/wordpress/subsites/install"
    } else {
        "/wordpress/install"
    };
    let draft = draft_from_form(&form);
    let domain = form.domain.trim().to_ascii_lowercase();
    let looks_sub = crate::backups::is_subdomain_site(&domain)
        || crate::sites::resolve_parent_domain(&domain)
            .ok()
            .flatten()
            .is_some();
    if expect_subdomain && !looks_sub {
        // Creating a new nested FQDN: parent must exist even if site not registered yet.
        let parent_ok = crate::sites::resolve_parent_domain(&domain)
            .ok()
            .flatten()
            .is_some()
            || crate::sites::parent_domain_candidates(&domain)
                .iter()
                .any(|c| crate::sites::load_site(c).is_ok());
        if !parent_ok {
            return wp_redirect_with_draft(
                install_path,
                None,
                Some(
                    "WordPress Sub-site installs require a nested domain under an existing parent website.",
                ),
                Some(&draft),
            );
        }
    }
    if !expect_subdomain && looks_sub {
        return wp_redirect_with_draft(
            install_path,
            None,
            Some("Use Install WordPress Sub-site for nested domains."),
            Some(&draft),
        );
    }
    let plugin_zip_paths = match store_uploaded_plugin_zips(&user, &uploads) {
        Ok(paths) => paths,
        Err(error) => {
            return wp_redirect_with_draft(install_path, None, Some(&error), Some(&draft));
        }
    };
    let req = WordpressInstallRequest {
        domain: form.domain.clone(),
        owner: user.clone(),
        title: form.title.clone(),
        site_url: form.site_url.clone(),
        admin_user: form.admin_user.clone(),
        admin_password: form.admin_password.clone(),
        admin_email: form.admin_email.clone(),
        plugin_sources: parse_plugin_sources(&form.plugin_sources),
        plugin_zip_paths,
        create_site_if_missing: parse_bool_flag(&form.create_site_if_missing),
    };
    match install_wordpress(req) {
        Ok(result) => {
            let msg = format!(
                "WordPress installed on {}. {}",
                result.site.domain,
                result.notes.join(" ")
            );
            wp_redirect(
                &format!(
                    "/wordpress/manage?domain={}",
                    urlencoding_simple(&result.site.domain)
                ),
                Some(&msg),
                None,
            )
        }
        Err(error) => {
            let friendly = sanitize_wp_cli_error(&error);
            wp_redirect_with_draft(install_path, None, Some(&friendly), Some(&draft))
        }
    }
}

#[get("/wordpress/manage")]
pub async fn wordpress_manage_route(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let Some(user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let domain = query
        .get("domain")
        .map(String::as_str)
        .unwrap_or("")
        .to_string();
    if domain.trim().is_empty() {
        return wp_redirect("/wordpress", None, Some("Domain is required"));
    }
    let tab = query
        .get("tab")
        .cloned()
        .unwrap_or_else(|| "general".to_string());
    let notice = query.get("notice").cloned();
    let error = query.get("error").cloned();
    let snap = match tokio::time::timeout(
        crate::panel_hub_http::HUB_RENDER_BUDGET,
        tokio::task::spawn_blocking(all_sites_snapshot),
    )
    .await
    {
        Ok(Ok(Ok(list))) => list,
        Ok(Ok(Err(err))) => return wp_redirect("/wordpress", None, Some(&err)),
        Ok(Err(_)) => {
            return wp_redirect(
                "/wordpress",
                None,
                Some("Could not load WordPress sites (worker failed)"),
            );
        }
        Err(_) => {
            return wp_redirect(
                "/wordpress",
                None,
                Some("Timed out loading WordPress sites. Try again."),
            );
        }
    };
    let Some(snapshot) = snap
        .into_iter()
        .find(|s| s.site.domain.eq_ignore_ascii_case(&domain))
    else {
        return wp_redirect(
            "/wordpress",
            None,
            Some(&format!("WordPress site `{domain}` not found")),
        );
    };
    html_ok(panel_shell(
        &user,
        "wordpress",
        "Manage WordPress",
        &wordpress_manage_page(&snapshot, &tab, notice.as_deref(), error.as_deref()),
    ))
}

fn wordpress_scan_notice(results: &[crate::wordpress_manage::WordpressRefreshResult]) -> String {
    let sub_names: Vec<&str> = results
        .iter()
        .filter(|r| crate::backups::is_subdomain_site(&r.site.domain))
        .map(|r| r.site.domain.as_str())
        .collect();
    let main_count = results.len().saturating_sub(sub_names.len());
    if sub_names.is_empty() {
        format!("Scan complete. Refreshed {} site(s).", results.len())
    } else if main_count == 0 {
        format!(
            "Scan complete. Found {} WordPress sub-site(s): {}.",
            sub_names.len(),
            sub_names.join(", ")
        )
    } else {
        format!(
            "Scan complete. Refreshed {} main and {} sub-site(s). Sub-sites: {}.",
            main_count,
            sub_names.len(),
            sub_names.join(", ")
        )
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct WordpressDomainForm {
    #[serde(default)]
    domain: String,
}

#[post("/wordpress/scan")]
pub async fn wordpress_scan_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    match scan_wordpress_sites() {
        Ok(results) => {
            let notice = wordpress_scan_notice(&results);
            wp_redirect("/wordpress", Some(&notice), None)
        }
        Err(error) => wp_redirect("/wordpress", None, Some(&error)),
    }
}

#[post("/wordpress/subsites/scan")]
pub async fn wordpress_subsites_scan_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    match scan_wordpress_sites() {
        Ok(results) => {
            let notice = wordpress_scan_notice(&results);
            wp_redirect("/wordpress/subsites", Some(&notice), None)
        }
        Err(error) => wp_redirect("/wordpress/subsites", None, Some(&error)),
    }
}

#[post("/wordpress/ensure-wpcli")]
pub async fn wordpress_ensure_wpcli_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    match ensure_wp_cli() {
        Ok(status) => wp_redirect("/wordpress", Some(&status.detail), None),
        Err(error) => wp_redirect("/wordpress", None, Some(&error)),
    }
}

#[post("/wordpress/refresh")]
pub async fn wordpress_refresh_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<WordpressDomainForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let domain = form.domain.trim();
    let base = format!(
        "/wordpress/manage?domain={}",
        crate::panel_hub_http::urlencoding_simple(domain)
    );
    match refresh_wordpress_site(domain) {
        Ok(result) => wp_redirect(
            &base,
            Some(&format!("Refreshed {}.", result.site.domain)),
            None,
        ),
        Err(error) => wp_redirect(&base, None, Some(&error)),
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct WordpressDeleteForm {
    #[serde(default)]
    domain: String,
    #[serde(default)]
    remove_files: String,
}

#[post("/wordpress/delete")]
pub async fn wordpress_delete_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<WordpressDeleteForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let remove_files = parse_bool_flag(&form.remove_files);
    match delete_wordpress(&form.domain, remove_files) {
        Ok(msg) => wp_redirect("/wordpress", Some(&msg), None),
        Err(error) => wp_redirect(
            &format!(
                "/wordpress/manage?domain={}",
                crate::panel_hub_http::urlencoding_simple(form.domain.trim())
            ),
            None,
            Some(&error),
        ),
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct WordpressPluginForm {
    #[serde(default)]
    domain: String,
    #[serde(default)]
    source: String,
}

#[post("/wordpress/plugins/install")]
pub async fn wordpress_plugin_install_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<WordpressPluginForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let base = format!(
        "/wordpress/manage?domain={}&tab=plugins",
        crate::panel_hub_http::urlencoding_simple(form.domain.trim())
    );
    match install_plugin(&form.domain, &form.source) {
        Ok(msg) => wp_redirect(&base, Some(&msg), None),
        Err(error) => wp_redirect(&base, None, Some(&error)),
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct WordpressThemeForm {
    #[serde(default)]
    domain: String,
    #[serde(default)]
    slug: String,
}

#[post("/wordpress/themes/activate")]
pub async fn wordpress_theme_activate_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<WordpressThemeForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let base = format!(
        "/wordpress/manage?domain={}&tab=themes",
        crate::panel_hub_http::urlencoding_simple(form.domain.trim())
    );
    match activate_theme(&form.domain, &form.slug) {
        Ok(msg) => wp_redirect(&base, Some(&msg), None),
        Err(error) => wp_redirect(&base, None, Some(&error)),
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct WordpressToggleForm {
    #[serde(default)]
    domain: String,
    #[serde(default)]
    enabled: String,
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
}

#[post("/wordpress/toggle/search-indexing")]
pub async fn wordpress_toggle_search_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<WordpressToggleForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let enabled = parse_bool_flag(&form.enabled);
    let base = format!(
        "/wordpress/manage?domain={}",
        crate::panel_hub_http::urlencoding_simple(form.domain.trim())
    );
    match set_search_indexing(&form.domain, enabled) {
        Ok(msg) => wp_redirect(&base, Some(&msg), None),
        Err(error) => wp_redirect(&base, None, Some(&error)),
    }
}

#[post("/wordpress/toggle/debugging")]
pub async fn wordpress_toggle_debug_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<WordpressToggleForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let enabled = parse_bool_flag(&form.enabled);
    let base = format!(
        "/wordpress/manage?domain={}",
        crate::panel_hub_http::urlencoding_simple(form.domain.trim())
    );
    match set_debugging(&form.domain, enabled) {
        Ok(msg) => wp_redirect(&base, Some(&msg), None),
        Err(error) => wp_redirect(&base, None, Some(&error)),
    }
}

#[post("/wordpress/toggle/maintenance")]
pub async fn wordpress_toggle_maintenance_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<WordpressToggleForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let enabled = parse_bool_flag(&form.enabled);
    let base = format!(
        "/wordpress/manage?domain={}",
        crate::panel_hub_http::urlencoding_simple(form.domain.trim())
    );
    match set_maintenance(&form.domain, enabled) {
        Ok(msg) => wp_redirect(&base, Some(&msg), None),
        Err(error) => wp_redirect(&base, None, Some(&error)),
    }
}

#[post("/wordpress/toggle/password-protection")]
pub async fn wordpress_toggle_password_post(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<WordpressToggleForm>,
) -> HttpResponse {
    let Some(_user) = require_panel_user(&state, &http) else {
        return login_redirect(&http);
    };
    let enabled = parse_bool_flag(&form.enabled);
    let base = format!(
        "/wordpress/manage?domain={}",
        crate::panel_hub_http::urlencoding_simple(form.domain.trim())
    );
    match set_password_protection(&form.domain, enabled, &form.username, &form.password) {
        Ok(msg) => wp_redirect(&base, Some(&msg), None),
        Err(error) => wp_redirect(&base, None, Some(&error)),
    }
}
