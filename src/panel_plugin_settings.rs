//! Plugin Settings and Dashboard HTML for CPN Panel.

use crate::plugins::{InstalledPlugin, list_installed};
use crate::plugins_settings::{
    PluginSettings, declared_settings_fields, load_plugin_settings, manifest_has_dashboard,
};
use crate::sites::SiteRecord;
use std::collections::{BTreeMap, HashMap};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn section_heading(title: &str, blurb: &str) -> String {
    format!(
        r#"
      <div class="dashboard-heading">
        <div>
          <p class="eyebrow">CPN PANEL</p>
          <h1>{title}</h1>
          <p>{blurb}</p>
        </div>
      </div>"#,
        title = html_escape(title),
        blurb = html_escape(blurb),
    )
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

fn find_plugin<'a>(plugins: &'a [InstalledPlugin], id: &str) -> Option<&'a InstalledPlugin> {
    plugins
        .iter()
        .find(|p| p.manifest.id.eq_ignore_ascii_case(id))
}

fn field_input(key: &str, field_type: &str, value: &str) -> String {
    let ft = field_type.to_ascii_lowercase();
    if ft == "checkbox" {
        let checked = if value == "1" || value.eq_ignore_ascii_case("true") || value == "on" {
            " checked"
        } else {
            ""
        };
        return format!(
            r#"<input type="checkbox" id="f-{key}" name="field_{key}" value="1"{checked}>"#,
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

pub fn plugin_settings_main(
    sites: &[SiteRecord],
    domain: &str,
    plugin_id: &str,
    notice: Option<&str>,
    error: Option<&str>,
) -> String {
    if domain.trim().is_empty() || plugin_id.trim().is_empty() {
        return format!(
            r#"{heading}
      {err}
      <article class="section-card">
        <p class="panel-notice error">Choose a site and plugin from Installed Plugins.</p>
        <p><a class="btn-secondary" href="/plugins">Back to Plugins</a></p>
      </article>"#,
            heading = section_heading("Plugin settings", "Configure a plugin for one site."),
            err = notice_block("error", error),
        );
    }
    let installed = list_installed(domain).unwrap_or_default();
    let Some(item) = find_plugin(&installed, plugin_id) else {
        return format!(
            r#"{heading}
      {err}
      <article class="section-card">
        <p class="panel-notice error">Plugin `{id}` is not installed on `{domain}`.</p>
        <p><a class="btn-secondary" href="/plugins?domain={domain_q}">Back to Plugins</a></p>
      </article>"#,
            heading = section_heading("Plugin settings", "Configure a plugin for one site."),
            err = notice_block("error", error),
            id = html_escape(plugin_id),
            domain = html_escape(domain),
            domain_q = html_escape(domain),
        );
    };
    let m = &item.manifest;
    let settings = load_plugin_settings(domain, &m.id).unwrap_or_default();
    let declared = declared_settings_fields(domain, &m.id);
    let sidebar_checked = if settings.show_in_sidebar {
        " checked"
    } else {
        ""
    };
    let mut custom_fields = String::new();
    if declared.is_empty() {
        custom_fields.push_str(
            r#"<p class="muted">This plugin did not declare settings fields. Add optional key/value pairs below, or use Show in sidebar.</p>
        <label for="kv_key">Custom key</label>
        <input id="kv_key" name="kv_key" type="text" placeholder="option_name" autocomplete="off">
        <label for="kv_value">Custom value</label>
        <input id="kv_value" name="kv_value" type="text" placeholder="value" autocomplete="off">"#,
        );
        if !settings.fields.is_empty() {
            custom_fields.push_str(r#"<ul class="kv-list">"#);
            for (key, value) in &settings.fields {
                custom_fields.push_str(&format!(
                    r#"<li><span>{key}</span><strong>{value}</strong>
            <label style="display:flex;align-items:center;gap:8px;font-weight:500;">
              <input type="checkbox" name="delete_field_{key}" value="1"> Remove
            </label>
            <input type="hidden" name="field_{key}" value="{value}">
          </li>"#,
                    key = html_escape(key),
                    value = html_escape(value),
                ));
            }
            custom_fields.push_str("</ul>");
        }
    } else {
        for field in &declared {
            let value = settings
                .fields
                .get(&field.key)
                .cloned()
                .unwrap_or_else(|| field.default.clone());
            custom_fields.push_str(&format!(
                r#"<label for="f-{key}">{label}</label>
        {input}"#,
                key = html_escape(&field.key),
                label = html_escape(&field.label),
                input = field_input(&field.key, &field.field_type, &value),
            ));
        }
    }
    let dash_link = if m.enabled {
        format!(
            r#"<p><a class="btn-secondary" href="/plugins/dashboard?domain={domain}&amp;id={id}">Open plugin dashboard</a></p>"#,
            domain = html_escape(domain),
            id = html_escape(&m.id),
        )
    } else {
        r#"<p class="muted">Activate the plugin to use its dashboard route.</p>"#.into()
    };
    let _ = sites;
    format!(
        r#"{heading}
      {ok}
      {err}
      <article class="section-card">
        <p class="muted"><a href="/plugins?view=installed&amp;domain={domain}">Back to Installed Plugins</a></p>
        <h2>{name}</h2>
        <p class="muted">{id} v{ver} on {domain}</p>
        <p class="muted">Settings file: <code>{path}/settings.json</code></p>
        <form method="post" action="/plugins/settings" class="stack-form" style="max-width:520px;">
          <input type="hidden" name="domain" value="{domain}">
          <input type="hidden" name="id" value="{id}">
          <label style="display:flex;align-items:center;gap:10px;font-weight:600;">
            <input type="checkbox" name="show_in_sidebar" value="1"{sidebar}>
            Show in sidebar
          </label>
          <p class="muted">When enabled and the plugin is Active, it appears under the Plugins section in the panel nav.</p>
          {fields}
          <button type="submit" class="btn-primary">Save settings</button>
        </form>
        {dash}
      </article>"#,
        heading = section_heading(
            "Plugin settings",
            "Options for this plugin on the selected site.",
        ),
        ok = notice_block("ok", notice),
        err = notice_block("error", error),
        domain = html_escape(domain),
        name = html_escape(&m.name),
        id = html_escape(&m.id),
        ver = html_escape(&m.version),
        path = html_escape(&item.path.display().to_string()),
        sidebar = sidebar_checked,
        fields = custom_fields,
        dash = dash_link,
    )
}

pub fn plugin_dashboard_main(
    domain: &str,
    plugin_id: &str,
    notice: Option<&str>,
    error: Option<&str>,
) -> String {
    if domain.trim().is_empty() || plugin_id.trim().is_empty() {
        return format!(
            r#"{heading}
      {err}
      <article class="section-card">
        <p class="panel-notice error">Missing domain or plugin id.</p>
        <p><a class="btn-secondary" href="/plugins">Back to Plugins</a></p>
      </article>"#,
            heading = section_heading("Plugin dashboard", "Plugin overview for one site."),
            err = notice_block("error", error),
        );
    }
    let installed = list_installed(domain).unwrap_or_default();
    let Some(item) = find_plugin(&installed, plugin_id) else {
        return format!(
            r#"{heading}
      {err}
      <article class="section-card">
        <p class="panel-notice error">Plugin not found on this site.</p>
        <p><a class="btn-secondary" href="/plugins?domain={domain}">Back to Plugins</a></p>
      </article>"#,
            heading = section_heading("Plugin dashboard", "Plugin overview for one site."),
            err = notice_block("error", error),
            domain = html_escape(domain),
        );
    };
    let m = &item.manifest;
    if !m.enabled {
        return format!(
            r#"{heading}
      {err}
      <article class="section-card">
        <p class="panel-notice error">{name} is installed but not Active.</p>
        <p><a class="btn-secondary" href="/plugins/settings?domain={domain}&amp;id={id}">Open settings</a></p>
      </article>"#,
            heading = section_heading("Plugin dashboard", "Plugin overview for one site."),
            err = notice_block("error", error),
            name = html_escape(&m.name),
            domain = html_escape(domain),
            id = html_escape(&m.id),
        );
    }
    let settings = load_plugin_settings(domain, &m.id).unwrap_or_default();
    let has_dash = manifest_has_dashboard(domain, &m.id);
    let mut kv = String::from(r#"<ul class="kv-list">"#);
    kv.push_str(&format!(
        r#"<li><span>Status</span><strong>Active</strong></li>
      <li><span>Show in sidebar</span><strong>{side}</strong></li>
      <li><span>Install path</span><strong><code>{path}</code></strong></li>"#,
        side = if settings.show_in_sidebar {
            "Yes"
        } else {
            "No"
        },
        path = html_escape(&item.path.display().to_string()),
    ));
    for (key, value) in &settings.fields {
        kv.push_str(&format!(
            r#"<li><span>{key}</span><strong>{value}</strong></li>"#,
            key = html_escape(key),
            value = html_escape(value),
        ));
    }
    kv.push_str("</ul>");
    let note = if has_dash {
        "<p class=\"muted\">This plugin declared a dashboard. Runtime hooks are still limited; this page shows status and saved settings.</p>"
    } else {
        "<p class=\"muted\">Minimal dashboard for this active plugin. Use Settings to change options.</p>"
    };
    format!(
        r#"{heading}
      {ok}
      {err}
      <article class="section-card">
        <p class="muted"><a href="/plugins?view=installed&amp;domain={domain}">Back to Installed Plugins</a></p>
        <h2>{name}</h2>
        <p class="muted">{id} v{ver} on {domain}</p>
        {note}
        {kv}
        <p style="margin-top:16px;"><a class="btn-primary" href="/plugins/settings?domain={domain}&amp;id={id}">Settings</a></p>
      </article>"#,
        heading = section_heading(&m.name, "Plugin dashboard for the selected site."),
        ok = notice_block("ok", notice),
        err = notice_block("error", error),
        domain = html_escape(domain),
        name = html_escape(&m.name),
        id = html_escape(&m.id),
        ver = html_escape(&m.version),
        note = note,
        kv = kv,
    )
}

/// Build settings map from POST form keys (`field_*`, optional kv pair, deletes).
pub fn settings_from_form(
    form: &HashMap<String, String>,
    previous: &PluginSettings,
    declared_keys: &[String],
) -> PluginSettings {
    let show_in_sidebar = form.get("show_in_sidebar").map(String::as_str) == Some("1");
    let mut fields = BTreeMap::new();
    if declared_keys.is_empty() {
        for (key, value) in &previous.fields {
            let delete_key = format!("delete_field_{key}");
            if form.get(&delete_key).map(String::as_str) == Some("1") {
                continue;
            }
            if let Some(v) = form.get(&format!("field_{key}")) {
                fields.insert(key.clone(), v.clone());
            } else {
                fields.insert(key.clone(), value.clone());
            }
        }
        let kv_key = form
            .get("kv_key")
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let kv_value = form.get("kv_value").cloned().unwrap_or_default();
        if !kv_key.is_empty()
            && kv_key
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
            && kv_key.len() <= 64
        {
            fields.insert(kv_key, kv_value);
        }
    } else {
        for key in declared_keys {
            let form_key = format!("field_{key}");
            if let Some(v) = form.get(&form_key) {
                fields.insert(key.clone(), v.clone());
            } else {
                // Unchecked checkboxes omit the key.
                fields.insert(key.clone(), String::new());
            }
        }
    }
    PluginSettings {
        show_in_sidebar,
        fields,
    }
}
