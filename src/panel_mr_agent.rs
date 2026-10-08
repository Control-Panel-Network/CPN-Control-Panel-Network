//! Panel-hosted Mr Agent full chat page (Expand target). Does not depend on site docroot.

use crate::auth_api::panel_user_from_request;
use crate::installer::AppState;
use crate::login_next::login_redirect;
use crate::mr_agent_policy::{MrAgentHostPolicy, load_host_policy, save_host_policy};
use crate::panel_admin::is_panel_admin;
use crate::panel_pages::panel_shell;
use crate::panel_plugins_markup::html_escape;
use crate::plugins_settings::{
    load_plugin_settings, plugin_visibility_allows, settings_field_truthy,
};
use crate::site_acl::{SitePerm, can_manage_site, sites_manageable_by};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct MrAgentActionForm {
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub install_mode: String,
    #[serde(default)]
    pub confirm_vhost: String,
}

#[derive(Debug, Deserialize)]
pub struct MrAgentHostPolicyForm {
    /// Checkbox: present when on.
    #[serde(default)]
    pub allow_host_chat: String,
    #[serde(default)]
    pub allow_site_install: String,
}

fn form_checkbox_on(raw: &str) -> bool {
    let t = raw.trim();
    t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("on") || t == "yes"
}

fn host_policy_card(user: &str, notice: Option<&str>, error: Option<&str>) -> String {
    if !is_panel_admin(user) {
        return String::new();
    }
    let policy = load_host_policy();
    let chat_checked = if policy.allow_host_chat {
        " checked"
    } else {
        ""
    };
    let site_checked = if policy.allow_site_install {
        " checked"
    } else {
        ""
    };
    let mode = if !policy.allow_host_chat && !policy.allow_site_install {
        "Off (no panel chat; no new site installs)"
    } else if policy.allow_host_chat && !policy.allow_site_install {
        "Panel only (default)"
    } else if policy.allow_host_chat && policy.allow_site_install {
        "Panel + optional site"
    } else {
        "Site install only (unusual: panel chat off)"
    };
    let ok = notice
        .filter(|s| !s.is_empty())
        .map(|s| {
            format!(
                r#"<p class="panel-notice ok" role="status">{msg}</p>"#,
                msg = html_escape(s)
            )
        })
        .unwrap_or_default();
    let err = error
        .filter(|s| !s.is_empty())
        .map(|s| {
            format!(
                r#"<p class="panel-notice error" role="status">{msg}</p>"#,
                msg = html_escape(s)
            )
        })
        .unwrap_or_default();
    format!(
        r#"<article class="section-card" style="margin-bottom:16px;">
  <h2>Host policy (server owner)</h2>
  {ok}{err}
  <p class="muted">Stored at <code>/var/lib/cpn/mr-agent/host-policy.json</code>. Independent of whether Mr Agent is already installed on Host. Visibility ACL (admins only / all users / packages) still controls who can use chat when panel chat is on.</p>
  <p class="muted">Current mode: <strong>{mode}</strong></p>
  <form method="post" action="/plugins/mr-agent/host-policy" class="stack-form" style="max-width:520px;">
    <label style="display:flex;align-items:flex-start;gap:10px;font-weight:600;">
      <input type="checkbox" name="allow_host_chat" value="1"{chat_checked}>
      <span>Allow host chat (panel bubble and <code>/plugins/mr-agent</code>). Default on.</span>
    </label>
    <label style="display:flex;align-items:flex-start;gap:10px;font-weight:600;">
      <input type="checkbox" name="allow_site_install" value="1"{site_checked}>
      <span>Allow site install (Store users may Install on their sites). Default off.</span>
    </label>
    <p class="muted">Turning off site install does not uninstall existing site copies. New Installs are blocked with a clear message.</p>
    <button type="submit" class="btn-primary">Save host policy</button>
  </form>
</article>"#,
        ok = ok,
        err = err,
        mode = html_escape(mode),
        chat_checked = chat_checked,
        site_checked = site_checked,
    )
}

fn encode_query(value: &str) -> String {
    let mut enc = String::with_capacity(value.len() * 3);
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                enc.push(byte as char);
            }
            b' ' => enc.push('+'),
            _ => enc.push_str(&format!("%{byte:02X}")),
        }
    }
    enc
}

fn settings_redirect(domain: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let mut url = format!(
        "/plugins/settings?domain={}&id=mrAgent",
        encode_query(domain)
    );
    if let Some(notice) = notice.filter(|s| !s.is_empty()) {
        url.push_str("&notice=");
        url.push_str(&encode_query(notice));
    }
    if let Some(error) = error.filter(|s| !s.is_empty()) {
        url.push_str("&error=");
        url.push_str(&encode_query(error));
    }
    url
}

fn html_ok(body: String) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(body)
}

fn encode_domain(d: &str) -> String {
    let mut enc = String::with_capacity(d.len() * 3);
    for byte in d.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                enc.push(byte as char);
            }
            b' ' => enc.push('+'),
            _ => enc.push_str(&format!("%{byte:02X}")),
        }
    }
    enc
}

fn pick_domain(user: &str, requested: &str) -> String {
    let req = requested.trim();
    if !req.is_empty()
        && (req.eq_ignore_ascii_case(crate::mr_agent_install::HOST_DOMAIN_SENTINEL)
            || matches!(can_manage_site(user, req, SitePerm::Enable), Ok(true)))
    {
        return req.to_string();
    }
    if crate::plugin_activation::host_plugin_installed("mrAgent") {
        return crate::mr_agent_install::HOST_DOMAIN_SENTINEL.into();
    }
    sites_manageable_by(user)
        .ok()
        .and_then(|sites| sites.into_iter().next().map(|s| s.domain))
        .unwrap_or_default()
}

fn host_settings() -> crate::plugins_settings::PluginSettings {
    let path = crate::plugin_activation::host_plugin_path("mrAgent").join("settings.json");
    if let Ok(raw) = std::fs::read_to_string(path)
        && let Ok(settings) = serde_json::from_str(&raw)
    {
        return settings;
    }
    let mut settings = crate::plugins_settings::PluginSettings {
        show_in_sidebar: true,
        ..Default::default()
    };
    settings.fields.insert("enabled".into(), "1".into());
    settings
        .fields
        .insert("show_floating_bubble".into(), "1".into());
    settings
        .fields
        .insert("visibility".into(), "admins_only".into());
    settings
}

fn page_main(user: &str, domain: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let policy_ui = host_policy_card(user, notice, error);
    let domain = domain.trim();
    if !crate::mr_agent_policy::allow_host_chat() {
        return format!(
            r#"{policy}
<article class="section-card"><h1>Mr Agent</h1>
        <p class="panel-notice error">{msg}</p>
        <p class="muted">Ask the server owner to enable Allow host chat, or use a site /mr-agent URL if a site install already exists.</p>
        <p><a class="btn-secondary" href="/plugins">Back to Plugins</a></p>
        </article>"#,
            policy = policy_ui,
            msg = html_escape(crate::mr_agent_policy::HOST_CHAT_DISABLED_MSG),
        );
    }
    if domain.is_empty() {
        return format!(
            r#"{policy}
<article class="section-card"><h1>Mr Agent</h1>
        <p class="panel-notice error">Install Mr Agent on Host or on a site first.</p>
        <p><a class="btn-secondary" href="/plugins?view=store&amp;q=mrAgent">Open Plugin Store</a></p>
        </article>"#,
            policy = policy_ui,
        );
    }
    let settings = if domain.eq_ignore_ascii_case(crate::mr_agent_install::HOST_DOMAIN_SENTINEL) {
        host_settings()
    } else {
        load_plugin_settings(domain, "mrAgent").unwrap_or_default()
    };
    if !settings_field_truthy(&settings, "enabled", true)
        || !plugin_visibility_allows(user, &settings)
    {
        return format!(
            r#"{policy}
<article class="section-card"><h1>Mr Agent</h1>
        <p class="panel-notice error">Mr Agent is disabled or not allowed for your account.</p>
        <p class="muted">Domain: {domain}</p>
        <p><a class="btn-secondary" href="/plugins">Back to Plugins</a></p>
        </article>"#,
            policy = policy_ui,
            domain = html_escape(domain),
        );
    }
    if crate::mr_agent_install::resolve_mr_agent_root(domain).is_err() {
        return format!(
            r#"{policy}
<article class="section-card"><h1>Mr Agent</h1>
        <p class="panel-notice error">Mr Agent files were not found for {domain}.</p>
        <p><a class="btn-secondary" href="/plugins?view=store&amp;q=mrAgent">Install from Store</a></p>
        </article>"#,
            policy = policy_ui,
            domain = html_escape(domain),
        );
    }
    let chat_url = format!(
        "/plugins/float-chat?domain={}&id=mrAgent",
        encode_domain(domain)
    );
    let site_link = if domain.eq_ignore_ascii_case(crate::mr_agent_install::HOST_DOMAIN_SENTINEL) {
        String::new()
    } else {
        format!(
            r#"<p class="muted">Optional site folder URL: <a href="https://{domain}/mr-agent" target="_blank" rel="noopener noreferrer">https://{domain}/mr-agent</a> (after folder publish).</p>"#,
            domain = html_escape(domain),
        )
    };
    let settings_link =
        if domain.eq_ignore_ascii_case(crate::mr_agent_install::HOST_DOMAIN_SENTINEL) {
            r#"<a href="/plugins?view=store&amp;target=host&amp;q=mrAgent">Host plugin</a>"#.into()
        } else {
            format!(
                r#"<a href="/plugins/settings?domain={}&amp;id=mrAgent">Plugin settings</a>"#,
                html_escape(domain)
            )
        };
    format!(
        r#"{policy}
<article class="section-card">
  <h1>Mr Agent</h1>
  <p class="muted">Panel-hosted chat for <strong>{domain}</strong>. Expand from the bubble uses this page so it does not depend on the site document root.</p>
  {site_link}
  <div id="mra-panel-log" style="min-height:220px;max-height:50vh;overflow:auto;border:1px solid var(--cpn-border,#334155);border-radius:8px;padding:12px;margin:12px 0;"></div>
  <form id="mra-panel-form" style="display:flex;gap:8px;flex-wrap:wrap;">
    <input id="mra-panel-input" type="text" maxlength="8000" placeholder="Ask Mr Agentâ€¦" autocomplete="off" style="flex:1;min-width:200px;">
    <button type="submit" class="btn-primary">Send</button>
  </form>
  <p class="muted" style="margin-top:12px;">{settings_link} Â· <a href="/plugins">Plugins</a></p>
</article>
<script>
(function () {{
  "use strict";
  var chatUrl = {chat_json};
  var log = document.getElementById("mra-panel-log");
  var form = document.getElementById("mra-panel-form");
  var input = document.getElementById("mra-panel-input");
  function add(role, text) {{
    var row = document.createElement("div");
    row.style.margin = "0 0 8px";
    row.style.whiteSpace = "pre-wrap";
    row.textContent = (role === "user" ? "You: " : role === "err" ? "Error: " : "Mr Agent: ") + (text || "");
    log.appendChild(row);
    log.scrollTop = log.scrollHeight;
  }}
  add("assistant", "Ask about CPN menus, websites, packages, or plugins. Free help works without a provider API key.");
  form.addEventListener("submit", function (ev) {{
    ev.preventDefault();
    var message = (input.value || "").trim();
    if (!message) return;
    add("user", message);
    input.value = "";
    fetch(chatUrl, {{
      method: "POST",
      credentials: "same-origin",
      headers: {{ "Accept": "application/json", "Content-Type": "application/json" }},
      body: JSON.stringify({{ message: message, provider: "auto", model: "" }})
    }}).then(function (res) {{
      return res.json().then(function (data) {{
        if (!res.ok || data.ok === false) throw new Error((data && data.error) || "Chat failed");
        return data;
      }});
    }}).then(function (data) {{
      add("assistant", data.reply || "(empty reply)");
    }}).catch(function (err) {{
      add("err", (err && err.message) || "Chat failed");
    }});
  }});
}})();
</script>"#,
        policy = policy_ui,
        domain = html_escape(domain),
        site_link = site_link,
        settings_link = settings_link,
        chat_json = serde_json::to_string(&chat_url).unwrap_or_else(|_| "\"\"".into()),
    )
}

fn host_policy_redirect(notice: Option<&str>, error: Option<&str>) -> String {
    let mut url = "/plugins/mr-agent".to_string();
    let mut first = true;
    if let Some(notice) = notice.filter(|s| !s.is_empty()) {
        url.push(if first { '?' } else { '&' });
        first = false;
        url.push_str("notice=");
        url.push_str(&encode_query(notice));
    }
    if let Some(error) = error.filter(|s| !s.is_empty()) {
        url.push(if first { '?' } else { '&' });
        let _ = first;
        url.push_str("error=");
        url.push_str(&encode_query(error));
    }
    url
}

#[derive(Debug, Deserialize)]
pub struct MrAgentPageQueryFull {
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    pub notice: String,
    #[serde(default)]
    pub error: String,
}

#[get("/plugins/mr-agent")]
pub async fn plugins_mr_agent_page(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    query: web::Query<MrAgentPageQueryFull>,
) -> HttpResponse {
    let Some(user) = panel_user_from_request(&state, &http) else {
        return login_redirect(&http);
    };
    let domain = pick_domain(&user, &query.domain);
    let notice = if query.notice.trim().is_empty() {
        None
    } else {
        Some(query.notice.as_str())
    };
    let error = if query.error.trim().is_empty() {
        None
    } else {
        Some(query.error.as_str())
    };
    let body = page_main(&user, &domain, notice, error);
    html_ok(panel_shell(&user, "plugins", "Mr Agent", &body))
}

/// Owner-only host policy toggles (allow_host_chat / allow_site_install).
#[post("/plugins/mr-agent/host-policy")]
pub async fn plugins_mr_agent_host_policy(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<MrAgentHostPolicyForm>,
) -> HttpResponse {
    let Some(user) = panel_user_from_request(&state, &http) else {
        return login_redirect(&http);
    };
    if !is_panel_admin(&user) {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                host_policy_redirect(
                    None,
                    Some("Only the panel owner can change Mr Agent host policy"),
                ),
            ))
            .finish();
    }
    let policy = MrAgentHostPolicy {
        allow_host_chat: form_checkbox_on(&form.allow_host_chat),
        allow_site_install: form_checkbox_on(&form.allow_site_install),
    };
    match save_host_policy(&policy) {
        Ok(()) => HttpResponse::SeeOther()
            .append_header((
                "Location",
                host_policy_redirect(Some("Host policy saved"), None),
            ))
            .finish(),
        Err(err) => HttpResponse::SeeOther()
            .append_header(("Location", host_policy_redirect(None, Some(&err))))
            .finish(),
    }
}

/// Panel-mediated setup (secrets + folder publish). Replaces SSH `install.sh` for normal use.
#[post("/plugins/mr-agent/setup")]
pub async fn plugins_mr_agent_setup(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<MrAgentActionForm>,
) -> HttpResponse {
    let Some(user) = panel_user_from_request(&state, &http) else {
        return login_redirect(&http);
    };
    let domain = form.domain.trim();
    if domain.is_empty() {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                settings_redirect("", None, Some("Missing domain")),
            ))
            .finish();
    }
    if !form.id.trim().is_empty() && !crate::mr_agent_install::is_mr_agent(&form.id) {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                settings_redirect(domain, None, Some("Setup is only for Mr Agent")),
            ))
            .finish();
    }
    if !domain.eq_ignore_ascii_case(crate::mr_agent_install::HOST_DOMAIN_SENTINEL)
        && !matches!(can_manage_site(&user, domain, SitePerm::Enable), Ok(true))
    {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                settings_redirect(domain, None, Some("Not allowed to manage this site")),
            ))
            .finish();
    }
    let mode = crate::mr_agent_install::normalize_install_mode(&form.install_mode);
    let mode = if form.install_mode.trim().is_empty() {
        crate::mr_agent_install::install_mode_from_settings(domain)
    } else {
        mode
    };
    match crate::mr_agent_install::run_setup(domain, &mode, &form.confirm_vhost) {
        Ok(msg) => HttpResponse::SeeOther()
            .append_header(("Location", settings_redirect(domain, Some(&msg), None)))
            .finish(),
        Err(err) => HttpResponse::SeeOther()
            .append_header(("Location", settings_redirect(domain, None, Some(&err))))
            .finish(),
    }
}

/// Panel-mediated chat log prune (no SSH `cli_prune.php` required).
#[post("/plugins/mr-agent/prune")]
pub async fn plugins_mr_agent_prune(
    http: HttpRequest,
    state: web::Data<Arc<AppState>>,
    form: web::Form<MrAgentActionForm>,
) -> HttpResponse {
    let Some(user) = panel_user_from_request(&state, &http) else {
        return login_redirect(&http);
    };
    let domain = form.domain.trim();
    if domain.is_empty() {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                settings_redirect("", None, Some("Missing domain")),
            ))
            .finish();
    }
    if !form.id.trim().is_empty() && !crate::mr_agent_install::is_mr_agent(&form.id) {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                settings_redirect(domain, None, Some("Prune is only for Mr Agent")),
            ))
            .finish();
    }
    if !domain.eq_ignore_ascii_case(crate::mr_agent_install::HOST_DOMAIN_SENTINEL)
        && !matches!(can_manage_site(&user, domain, SitePerm::Enable), Ok(true))
    {
        return HttpResponse::SeeOther()
            .append_header((
                "Location",
                settings_redirect(domain, None, Some("Not allowed to manage this site")),
            ))
            .finish();
    }
    match crate::mr_agent_install::prune_chat_logs(domain) {
        Ok(msg) => HttpResponse::SeeOther()
            .append_header(("Location", settings_redirect(domain, Some(&msg), None)))
            .finish(),
        Err(err) => HttpResponse::SeeOther()
            .append_header(("Location", settings_redirect(domain, None, Some(&err))))
            .finish(),
    }
}
