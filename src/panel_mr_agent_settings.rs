//! Tabbed Mr Agent plugin settings (Host `_host` and Site). Other plugins stay on the generic form.

use crate::mr_agent_install::HOST_DOMAIN_SENTINEL;
use crate::mr_agent_policy::{MrAgentHostPolicy, load_host_policy, save_host_policy};
use crate::mr_agent_stats::collect_stats;
use crate::plugin_activation::{host_plugin_installed, host_plugin_path};
use crate::plugins::{InstalledPlugin, list_installed};
use crate::plugins_settings::{
    PluginSettingField, PluginSettings, declared_settings_fields, load_plugin_settings,
};
use std::collections::HashMap;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn notice_block(kind: &str, message: Option<&str>) -> String {
    let Some(message) = message.filter(|value| !value.is_empty()) else {
        return String::new();
    };
    let class = if kind == "error" {
        "panel-notice error"
    } else {
        "panel-notice ok"
    };
    format!(
        r#"<p class="{class}" role="status">{msg}</p>"#,
        msg = html_escape(message)
    )
}

fn normalize_tab(raw: Option<&str>) -> &'static str {
    match raw.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "access" | "policy" => "access",
        "ai" | "providers" | "ai-providers" => "ai",
        "storage" => "storage",
        "statistics" | "stats" => "statistics",
        _ => "general",
    }
}

const GENERAL_KEYS: &[&str] = &[
    "enabled",
    "show_floating_bubble",
    "install_mode",
    "expand_via",
    "public_path",
    "default_provider",
];
const ACCESS_KEYS: &[&str] = &["visibility", "package_ids", "allow_user_keys"];
const AI_KEYS: &[&str] = &[
    "rate_limit_per_hour",
    "max_message_length",
    "max_tokens_per_reply",
    "concurrent_requests",
    "max_upload_bytes",
    "local_timeout_seconds",
    "local_max_response_bytes",
    "local_base_url",
    "local_model",
    "local_only_mode",
    "local_allow_lan",
];
const STORAGE_KEYS: &[&str] = &[
    "max_history_messages",
    "max_stored_conversations",
    "chat_retention_days",
    "max_chat_disk_mb",
];

fn field_input(key: &str, field_type: &str, value: &str) -> String {
    let ft = field_type.to_ascii_lowercase();
    if ft == "checkbox" {
        let checked = if value == "1" || value.eq_ignore_ascii_case("true") || value == "on" {
            " checked"
        } else {
            ""
        };
        return format!(
            r#"<input type="hidden" name="present_field_{key}" value="1">
        <input type="checkbox" id="f-{key}" name="field_{key}" value="1"{checked}>"#,
            key = html_escape(key),
            checked = checked,
        );
    }
    let input_type = if ft == "number" { "number" } else { "text" };
    format!(
        r#"<input id="f-{key}" name="field_{key}" type="{input_type}" value="{value}">"#,
        key = html_escape(key),
        input_type = input_type,
        value = html_escape(value),
    )
}

fn render_fields(
    keys: &[&str],
    declared: &[PluginSettingField],
    settings: &PluginSettings,
) -> String {
    let mut out = String::new();
    for key in keys {
        let Some(field) = declared.iter().find(|f| f.key.eq_ignore_ascii_case(key)) else {
            continue;
        };
        let value = settings
            .fields
            .get(&field.key)
            .cloned()
            .unwrap_or_else(|| field.default.clone());
        out.push_str(&format!(
            r#"<label for="f-{key}">{label}</label>
        {input}"#,
            key = html_escape(&field.key),
            label = html_escape(&field.label),
            input = field_input(&field.key, &field.field_type, &value),
        ));
    }
    out
}

fn tab_btn(id: &str, label: &str, active: &str) -> String {
    let selected = if id == active { "true" } else { "false" };
    let class = if id == active {
        "btn-secondary mra-set-tab is-active"
    } else {
        "btn-secondary mra-set-tab"
    };
    format!(
        r#"<a class="{class}" role="tab" aria-selected="{selected}" href="?domain={{domain}}&amp;id=mrAgent&amp;tab={id}">{label}</a>"#,
        class = class,
        selected = selected,
        id = html_escape(id),
        label = html_escape(label),
    )
}

fn stats_html(domain: &str) -> String {
    let s = collect_stats(domain);
    if s.empty {
        return r#"<p class="muted">No chat activity yet. Statistics appear after the first conversation.</p>"#.into();
    }
    format!(
        r#"<div class="kv-list" style="display:grid;grid-template-columns:repeat(auto-fill,minmax(160px,1fr));gap:12px;">
      <div><span class="muted">Conversations</span><br><strong>{ct}</strong></div>
      <div><span class="muted">Messages</span><br><strong>{mt}</strong></div>
      <div><span class="muted">Conversations (7d)</span><br><strong>{c7}</strong></div>
      <div><span class="muted">Messages (7d)</span><br><strong>{m7}</strong></div>
      <div><span class="muted">Conversations (30d)</span><br><strong>{c30}</strong></div>
      <div><span class="muted">Messages (30d)</span><br><strong>{m30}</strong></div>
      <div><span class="muted">Distinct users</span><br><strong>{users}</strong></div>
      <div><span class="muted">Storage</span><br><strong>{smb:.2} / {lim} MB</strong></div>
      <div><span class="muted">Last activity</span><br><strong>{last}</strong></div>
    </div>
    <p class="muted" style="margin-top:12px;">Privacy-safe counts only. Message bodies and secrets are never shown. Live JSON: <code>/plugins/mr-agent/stats?domain={domain}</code></p>"#,
        ct = s.conversations_total,
        mt = s.messages_total,
        c7 = s.conversations_7d,
        m7 = s.messages_7d,
        c30 = s.conversations_30d,
        m30 = s.messages_30d,
        users = s.distinct_users,
        smb = s.storage_mb,
        lim = s.storage_limit_mb,
        last = html_escape(if s.last_activity.is_empty() {
            "n/a"
        } else {
            &s.last_activity
        }),
        domain = html_escape(domain),
    )
}

fn access_policy_block(is_host: bool, is_admin: bool) -> String {
    let p = load_host_policy();
    if is_host && is_admin {
        let chat = if p.allow_host_chat { " checked" } else { "" };
        let site = if p.allow_site_install { " checked" } else { "" };
        return format!(
            r#"<h3>Host policy</h3>
        <p class="muted">Stored in <code>/var/lib/cpn/mr-agent/host-policy.json</code>. Applies panel-wide.</p>
        <label style="display:flex;align-items:center;gap:10px;font-weight:600;">
          <input type="checkbox" name="allow_host_chat" value="1"{chat}>
          Allow host chat (panel bubble and /plugins/mr-agent)
        </label>
        <label style="display:flex;align-items:center;gap:10px;font-weight:600;">
          <input type="checkbox" name="allow_site_install" value="1"{site}>
          Allow site install (Store Site target for Mr Agent)
        </label>
        <input type="hidden" name="save_host_policy" value="1">"#,
            chat = chat,
            site = site,
        );
    }
    format!(
        r#"<h3>Host policy (read-only)</h3>
        <p class="muted">Allow host chat: <strong>{chat}</strong>. Allow site install: <strong>{site}</strong>. Edit these on Host Mr Agent settings, Access tab (administrators).</p>"#,
        chat = if p.allow_host_chat { "On" } else { "Off" },
        site = if p.allow_site_install { "On" } else { "Off" },
    )
}

fn storage_actions(domain: &str) -> String {
    let d = html_escape(domain);
    format!(
        r#"<div style="display:flex;flex-wrap:wrap;gap:10px;margin:12px 0;">
      <form method="post" action="/plugins/mr-agent/setup" style="display:inline;">
        <input type="hidden" name="domain" value="{d}">
        <input type="hidden" name="id" value="mrAgent">
        <button type="submit" class="btn-secondary">Run setup / Publish folder</button>
      </form>
      <form method="post" action="/plugins/mr-agent/prune" style="display:inline;" onsubmit="return confirm('Prune chat logs for this scope now?');">
        <input type="hidden" name="domain" value="{d}">
        <input type="hidden" name="id" value="mrAgent">
        <button type="submit" class="btn-secondary">Prune chat logs</button>
      </form>
    </div>
    <p class="muted">Setup writes secrets under <code>/var/lib/cpn/mr-agent/</code> and publishes folder mode when applicable. Prune respects retention settings.</p>"#,
        d = d,
    )
}

fn resolve_installed(domain: &str) -> Result<(String, String, String), String> {
    if domain.eq_ignore_ascii_case(HOST_DOMAIN_SENTINEL) {
        if !host_plugin_installed("mrAgent") {
            return Err("Mr Agent is not installed on the Host".into());
        }
        let path = host_plugin_path("mrAgent");
        let ver = fs_version(&path);
        return Ok(("Mr Agent".into(), ver, path.display().to_string()));
    }
    let installed = list_installed(domain).unwrap_or_default();
    let item: Option<&InstalledPlugin> = installed
        .iter()
        .find(|p| p.manifest.id.eq_ignore_ascii_case("mrAgent"));
    let Some(item) = item else {
        return Err(format!("Mr Agent is not installed on `{domain}`"));
    };
    Ok((
        item.manifest.name.clone(),
        item.manifest.version.clone(),
        item.path.display().to_string(),
    ))
}

fn fs_version(path: &std::path::Path) -> String {
    let json = path.join("cpn-plugin.json");
    if let Ok(raw) = std::fs::read_to_string(&json)
        && let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw)
        && let Some(ver) = v.get("version").and_then(|x| x.as_str())
    {
        return ver.to_string();
    }
    "installed".into()
}

/// Save host policy checkboxes when the Access tab posts `save_host_policy`.
pub fn save_host_policy_from_form(
    domain: &str,
    is_admin: bool,
    form: &HashMap<String, String>,
) -> Result<(), String> {
    if !domain.trim().eq_ignore_ascii_case(HOST_DOMAIN_SENTINEL) {
        return Ok(());
    }
    if form.get("save_host_policy").map(String::as_str) != Some("1") {
        return Ok(());
    }
    if !is_admin {
        return Err("Only panel administrators can change host policy".into());
    }
    let policy = MrAgentHostPolicy {
        allow_host_chat: form.get("allow_host_chat").map(String::as_str) == Some("1"),
        allow_site_install: form.get("allow_site_install").map(String::as_str) == Some("1"),
    };
    save_host_policy(&policy)
}

/// Tabbed Host/Site settings page for Mr Agent only.
pub fn mr_agent_settings_main(
    domain: &str,
    notice: Option<&str>,
    error: Option<&str>,
    tab: Option<&str>,
    is_admin: bool,
) -> String {
    let active = normalize_tab(tab);
    let is_host = domain.eq_ignore_ascii_case(HOST_DOMAIN_SENTINEL);
    let Ok((name, ver, path)) = resolve_installed(domain) else {
        return format!(
            r#"
      <div class="dashboard-heading"><div><p class="eyebrow">CPN PANEL</p><h1>Mr Agent settings</h1></div></div>
      {err}
      <article class="section-card">
        <p class="panel-notice error">Mr Agent is not installed for this scope.</p>
        <p><a class="btn-secondary" href="/plugins?view=store&amp;q=mrAgent">Open Plugin Store</a></p>
      </article>"#,
            err = notice_block("error", error),
        );
    };
    let settings = load_plugin_settings(domain, "mrAgent").unwrap_or_default();
    let declared = declared_settings_fields(domain, "mrAgent");
    let sidebar_checked = if settings.show_in_sidebar {
        " checked"
    } else {
        ""
    };
    let scope_label = if is_host { "Host" } else { "Site" };
    let back = if is_host {
        r#"<a href="/plugins?view=installed&amp;target=host">Back to Installed Plugins</a>"#.into()
    } else {
        format!(
            r#"<a href="/plugins?view=installed&amp;domain={d}">Back to Installed Plugins</a>"#,
            d = html_escape(domain),
        )
    };

    let mut tabs = String::new();
    for (id, label) in [
        ("general", "General"),
        ("access", "Access"),
        ("ai", "AI / Providers"),
        ("storage", "Storage"),
        ("statistics", "Statistics"),
    ] {
        tabs.push_str(&tab_btn(id, label, active).replace("{domain}", &html_escape(domain)));
        tabs.push('\n');
    }

    let (panel, tab_keys, sidebar_present) = match active {
        "access" => (
            format!(
                "{}{}",
                render_fields(ACCESS_KEYS, &declared, &settings),
                access_policy_block(is_host, is_admin),
            ),
            ACCESS_KEYS.join(","),
            false,
        ),
        "ai" => (
            render_fields(AI_KEYS, &declared, &settings),
            AI_KEYS.join(","),
            false,
        ),
        "storage" => (
            format!(
                "{}{}",
                render_fields(STORAGE_KEYS, &declared, &settings),
                storage_actions(domain),
            ),
            STORAGE_KEYS.join(","),
            false,
        ),
        "statistics" => (stats_html(domain), String::new(), false),
        _ => (
            format!(
                r#"<input type="hidden" name="sidebar_present" value="1">
          <label style="display:flex;align-items:center;gap:10px;font-weight:600;">
            <input type="checkbox" name="show_in_sidebar" value="1"{sidebar}>
            Show in sidebar
          </label>
          <p class="muted">Sidebar listing is separate from the floating chat bubble.</p>
          {fields}"#,
                sidebar = sidebar_checked,
                fields = render_fields(GENERAL_KEYS, &declared, &settings),
            ),
            GENERAL_KEYS.join(","),
            true,
        ),
    };
    let _ = sidebar_present;

    let save_form = if active == "statistics" {
        String::new()
    } else {
        format!(
            r#"<form method="post" action="/plugins/settings" class="stack-form" style="max-width:560px;">
          <input type="hidden" name="domain" value="{domain}">
          <input type="hidden" name="id" value="mrAgent">
          <input type="hidden" name="tab" value="{tab}">
          <input type="hidden" name="mra_tab_keys" value="{tab_keys}">
          {panel}
          <button type="submit" class="btn-primary" style="margin-top:14px;">Save settings</button>
        </form>"#,
            domain = html_escape(domain),
            tab = html_escape(active),
            tab_keys = html_escape(&tab_keys),
            panel = panel,
        )
    };
    let stats_only = if active == "statistics" {
        format!(
            r#"<div class="stack-form" style="max-width:720px;">{panel}</div>"#,
            panel = panel
        )
    } else {
        String::new()
    };

    format!(
        r#"
      <div class="dashboard-heading">
        <div>
          <p class="eyebrow">CPN PANEL</p>
          <h1>Mr Agent settings</h1>
          <p>{scope} options for Host and Site installs. Use tabs to keep General, Access, AI, Storage, and Statistics separate.</p>
        </div>
      </div>
      {ok}
      {err}
      <article class="section-card">
        <p class="muted">{back}</p>
        <h2>{name}</h2>
        <p class="muted">mrAgent v{ver} on {domain_label} ({scope})</p>
        <p class="muted">Settings file: <code>{path}</code></p>
        <div class="plugin-tabs" role="tablist" aria-label="Mr Agent settings" style="display:flex;flex-wrap:wrap;gap:8px;margin:14px 0;">
          {tabs}
        </div>
        {save_form}
        {stats_only}
        <p style="margin-top:16px;"><a class="btn-secondary" href="/plugins/mr-agent?domain={domain}">Open chat</a></p>
      </article>
      <style>
        .mra-set-tab.is-active {{ font-weight:700; border-color: var(--accent, #006cfa); }}
      </style>"#,
        scope = scope_label,
        ok = notice_block("ok", notice),
        err = notice_block("error", error),
        back = back,
        name = html_escape(&name),
        ver = html_escape(&ver),
        domain_label = html_escape(domain),
        path = html_escape(&path),
        tabs = tabs,
        save_form = save_form,
        stats_only = stats_only,
        domain = html_escape(domain),
    )
}
