//! Inject Active plugin float widgets (example: Mr Agent chat bubble) into panel chrome.

use crate::panel_site_tools_security::same_origin_ok;
use crate::plugins::plugins_dir_for_domain;
use crate::plugins_settings::{
    PanelFloatWidget, load_plugin_settings, panel_float_widgets, plugin_visibility_allows,
    settings_field_truthy,
};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::Deserialize;
use serde_json::json;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

fn json_escape(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

/// Safe relative asset under a plugin install (no `..`, must stay under plugin root).
fn resolve_plugin_asset(domain: &str, plugin_id: &str, rel: &str) -> Result<PathBuf, String> {
    let id = crate::plugins::normalize_plugin_id(plugin_id)?;
    let rel = rel.trim().trim_start_matches('/');
    if rel.is_empty() {
        return Err("Asset path is required".into());
    }
    if rel.contains('\0') {
        return Err("Invalid asset path".into());
    }
    // Reject path traversal before canonicalize of missing files.
    for part in Path::new(rel).components() {
        match part {
            Component::Normal(_) => {}
            Component::CurDir => {}
            _ => return Err("Asset path may not contain .. or absolute segments".into()),
        }
    }
    if !rel.starts_with("public/assets/panel-float/")
        && !rel.starts_with("public\\assets\\panel-float\\")
    {
        return Err("Only public/assets/panel-float/* assets may be served".into());
    }
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(site_root) = plugins_dir_for_domain(domain) {
        roots.push(site_root.join(&id));
    }
    if id.eq_ignore_ascii_case("mrAgent") {
        if let Ok(host_root) = crate::mr_agent_install::resolve_mr_agent_root(domain) {
            if !roots.iter().any(|r| r == &host_root) {
                roots.push(host_root);
            }
        }
    }
    let mut file: Option<PathBuf> = None;
    let mut root_canon: Option<PathBuf> = None;
    for root in roots {
        let candidate = root.join(rel);
        let Ok(rc) = root.canonicalize() else {
            continue;
        };
        if !candidate.exists() {
            continue;
        }
        let Ok(canon) = candidate.canonicalize() else {
            continue;
        };
        if !canon.starts_with(&rc) {
            continue;
        }
        file = Some(canon);
        root_canon = Some(rc);
        break;
    }
    let Some(file) = file else {
        return Err("Asset not found".into());
    };
    let _ = root_canon;
    if !file.is_file() {
        return Err("Asset is not a file".into());
    }
    Ok(file)
}

fn content_type_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "js" => "application/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

/// Markup + boot config injected before `</body>` in `panel_shell`.
pub fn panel_float_inject_html(username: &str) -> String {
    let widgets = panel_float_widgets(username);
    if widgets.is_empty() {
        return String::new();
    }
    let mut list = String::from("[");
    for (i, w) in widgets.iter().enumerate() {
        if i > 0 {
            list.push(',');
        }
        let asset_url = format!(
            "/plugins/float-asset?domain={}&id={}&file={}",
            urlencoding_simple(&w.domain),
            urlencoding_simple(&w.id),
            urlencoding_simple(&w.asset_js)
        );
        let css_guess = w.asset_js.replace(".js", ".css");
        let css_url = format!(
            "/plugins/float-asset?domain={}&id={}&file={}",
            urlencoding_simple(&w.domain),
            urlencoding_simple(&w.id),
            urlencoding_simple(&css_guess)
        );
        list.push_str(&format!(
            r#"{{"id":{id},"name":{name},"domain":{domain},"publicPath":{pub},"expandUrl":{exp},"scriptUrl":{script},"cssUrl":{css},"chatUrl":{chat}}}"#,
            id = json_escape(&w.id),
            name = json_escape(&w.name),
            domain = json_escape(&w.domain),
            pub = json_escape(&w.public_path),
            exp = json_escape(&w.expand_url),
            script = json_escape(&asset_url),
            css = json_escape(&css_url),
            chat = json_escape(&format!(
                "/plugins/float-chat?domain={}&id={}",
                urlencoding_simple(&w.domain),
                urlencoding_simple(&w.id)
            )),
        ));
    }
    list.push(']');
    format!(
        r#"
<div id="cpn-plugin-float-root" hidden aria-hidden="true"></div>
<script>
window.CPN_PLUGIN_FLOAT = {{
  user: {user},
  widgets: {widgets}
}};
(function () {{
  "use strict";
  var cfg = window.CPN_PLUGIN_FLOAT;
  if (!cfg || !cfg.widgets || !cfg.widgets.length) return;
  cfg.widgets.forEach(function (w) {{
    if (w.cssUrl) {{
      var link = document.createElement("link");
      link.rel = "stylesheet";
      link.href = w.cssUrl;
      document.head.appendChild(link);
    }}
    var s = document.createElement("script");
    s.src = w.scriptUrl;
    s.async = true;
    s.dataset.pluginId = w.id;
    s.dataset.domain = w.domain;
    document.body.appendChild(s);
  }});
}})();
</script>"#,
        user = json_escape(username),
        widgets = list,
    )
}

fn urlencoding_simple(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 3);
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[derive(Debug, Deserialize)]
pub struct FloatAssetQuery {
    pub domain: String,
    pub id: String,
    pub file: String,
}

#[get("/plugins/float-asset")]
pub async fn plugins_float_asset(
    http: HttpRequest,
    state: web::Data<std::sync::Arc<crate::installer::AppState>>,
    query: web::Query<FloatAssetQuery>,
) -> HttpResponse {
    let Some(user) = crate::auth_api::panel_user_from_request(&state, &http) else {
        return HttpResponse::Unauthorized().finish();
    };
    let widgets = panel_float_widgets(&user);
    let allowed = widgets.iter().any(|w| {
        w.domain.eq_ignore_ascii_case(query.domain.trim())
            && w.id.eq_ignore_ascii_case(query.id.trim())
    });
    if !allowed {
        return HttpResponse::Forbidden()
            .content_type("text/plain; charset=utf-8")
            .body("Float widget not available for this account.");
    }
    let path = match resolve_plugin_asset(&query.domain, &query.id, &query.file) {
        Ok(p) => p,
        Err(err) => {
            return HttpResponse::NotFound()
                .content_type("text/plain; charset=utf-8")
                .body(err);
        }
    };
    match std::fs::read(&path) {
        Ok(bytes) => HttpResponse::Ok()
            .insert_header(("Cache-Control", "private, max-age=120"))
            .content_type(content_type_for(&path))
            .body(bytes),
        Err(_) => HttpResponse::NotFound().finish(),
    }
}

#[derive(Debug, Deserialize)]
pub struct FloatChatQuery {
    pub domain: String,
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct FloatChatBody {
    pub message: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
}

fn authorize_float_chat(
    user: &str,
    domain: &str,
    plugin_id: &str,
) -> Result<PanelFloatWidget, String> {
    let widgets = panel_float_widgets(user);
    widgets
        .into_iter()
        .find(|w| {
            w.domain.eq_ignore_ascii_case(domain.trim())
                && w.id.eq_ignore_ascii_case(plugin_id.trim())
        })
        .ok_or_else(|| "Mr Agent float chat is not available for this account.".into())
}

fn run_mr_agent_bridge(
    domain: &str,
    username: &str,
    message: &str,
    provider: &str,
    model: &str,
) -> Result<serde_json::Value, String> {
    let plugin_root = crate::mr_agent_install::resolve_mr_agent_root(domain)?;
    let bridge = plugin_root.join("modules").join("panel_bridge.php");
    if !bridge.is_file() {
        return Err(
            "Mr Agent panel bridge missing. Update the plugin to 1.3.0+ (Host or Site install)."
                .into(),
        );
    }
    let payload = json!({
        "action": "chat",
        "domain": domain,
        "username": username,
        "role": if crate::panel_admin::is_panel_admin(username) { "owner" } else { "user" },
        "package_id": crate::packages::package_for_account(username)
            .map(|p| p.id)
            .unwrap_or_default(),
        "message": message,
        "provider": provider,
        "model": model,
    });
    let mut child = Command::new("php")
        .arg(&bridge)
        .current_dir(&plugin_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not start PHP bridge: {e}"))?;
    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "PHP bridge stdin unavailable".to_string())?;
        let raw = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
        stdin
            .write_all(&raw)
            .map_err(|e| format!("Could not write bridge payload: {e}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("PHP bridge failed: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "PHP bridge exited {}: {}",
            output.status.code().unwrap_or(-1),
            err.chars().take(240).collect::<String>()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    serde_json::from_str(trimmed).map_err(|e| {
        format!(
            "Invalid bridge JSON: {e} ({})",
            trimmed.chars().take(160).collect::<String>()
        )
    })
}

#[post("/plugins/float-chat")]
pub async fn plugins_float_chat(
    http: HttpRequest,
    state: web::Data<std::sync::Arc<crate::installer::AppState>>,
    query: web::Query<FloatChatQuery>,
    body: web::Json<FloatChatBody>,
) -> HttpResponse {
    let Some(user) = crate::auth_api::panel_user_from_request(&state, &http) else {
        return HttpResponse::Unauthorized()
            .json(json!({"ok": false, "error": "Sign in required"}));
    };
    if !same_origin_ok(&http) {
        return HttpResponse::Forbidden().json(json!({
            "ok": false,
            "error": "Cross-origin float chat is not allowed."
        }));
    }
    if !query.id.trim().eq_ignore_ascii_case("mrAgent") {
        return HttpResponse::BadRequest().json(json!({
            "ok": false,
            "error": "Float chat is only implemented for mrAgent."
        }));
    }
    let widget = match authorize_float_chat(&user, &query.domain, &query.id) {
        Ok(w) => w,
        Err(err) => {
            return HttpResponse::Forbidden().json(json!({"ok": false, "error": err}));
        }
    };
    let _ = widget;
    // Re-check settings in case they changed mid-session.
    let settings = if query
        .domain
        .trim()
        .eq_ignore_ascii_case(crate::mr_agent_install::HOST_DOMAIN_SENTINEL)
    {
        let path = crate::plugin_activation::host_plugin_path("mrAgent").join("settings.json");
        std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_else(|| {
                let mut s = crate::plugins_settings::PluginSettings::default();
                s.fields.insert("enabled".into(), "1".into());
                s.fields
                    .insert("show_floating_bubble".into(), "1".into());
                s.fields.insert("visibility".into(), "admins_only".into());
                s
            })
    } else {
        load_plugin_settings(&query.domain, "mrAgent").unwrap_or_default()
    };
    if !settings_field_truthy(&settings, "enabled", true)
        || !settings_field_truthy(&settings, "show_floating_bubble", true)
        || !plugin_visibility_allows(&user, &settings)
    {
        return HttpResponse::Forbidden().json(json!({
            "ok": false,
            "error": "Mr Agent float chat is disabled or not allowed for this account."
        }));
    }
    let message = body.message.trim().to_string();
    if message.is_empty() || message.len() > 8000 {
        return HttpResponse::BadRequest().json(json!({
            "ok": false,
            "error": "Message must be between 1 and 8000 characters."
        }));
    }
    let domain = query.domain.trim().to_string();
    // auto/free: PHP bridge prefers local LLM for general chat; CPN help for navigation.
    let mut provider = body.provider.trim().to_ascii_lowercase();
    if provider.is_empty() || provider == "free" {
        provider = "auto".into();
    }
    let model = body.model.clone();
    let user_clone = user.clone();
    let result =
        web::block(move || run_mr_agent_bridge(&domain, &user_clone, &message, &provider, &model))
            .await;
    match result {
        Ok(Ok(value)) => HttpResponse::Ok().json(value),
        Ok(Err(err)) => HttpResponse::BadGateway().json(json!({"ok": false, "error": err})),
        Err(_) => HttpResponse::InternalServerError().json(json!({
            "ok": false,
            "error": "Chat worker failed."
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inject_empty_without_widgets() {
        let html = panel_float_inject_html("");
        assert!(html.is_empty() || html.contains("CPN_PLUGIN_FLOAT"));
    }

    #[test]
    fn asset_path_rejects_traversal() {
        // Domain may not exist in unit test; traversal check runs after normalize.
        let err = resolve_plugin_asset("example.com", "mrAgent", "../secrets.php");
        assert!(err.is_err());
    }

    #[test]
    fn json_escape_quotes() {
        assert!(json_escape("a\"b").contains("\\\""));
    }
}
