//! HTML for Server hub and feature tile pages.

use crate::panel_admin::is_panel_admin;
use crate::panel_hub_defs::server_hub_sections;
use crate::panel_hubs::{feature_shell, hub_tiles_grid, section_heading, status_kv};
use crate::panel_ops_php::detect_php;
// PHP Extensions / Configurations live in panel_hub_pages_php_*.
// Top Processes / System Processes Monitor lives in panel_hub_pages_processes.
use crate::panel_ops_pkgmgr::package_manager_status;
use crate::panel_ops_services::{control_service, list_known_services};

pub use crate::panel_hub_pages_processes_hub::{
    normalize_spm_tab, spm_nav_id, system_processes_hub_page,
};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn server_hub_main() -> String {
    let feats = crate::panel_feature_gate::InstalledOptionalFeatures::detect();
    let mut body = section_heading(
        "Server",
        "Host services, PHP, containers, files, and DNS tools for this CPN node.",
    );
    for (title, tiles) in server_hub_sections() {
        let filtered = crate::panel_feature_gate::filter_hub_tiles(tiles, feats);
        if filtered.is_empty() {
            continue;
        }
        body.push_str(&hub_tiles_grid(title, &filtered));
    }
    body
}

fn service_status_badge(label: &str) -> String {
    let class = match label {
        "Active" | "Enabled" => "svc-badge svc-badge-ok",
        "Inactive" | "Deactivated" | "Static" | "Linked" => "svc-badge svc-badge-idle",
        "Failed" | "Masked" | "Invalid" | "Unavailable" => "svc-badge svc-badge-bad",
        "Not installed" => "svc-badge svc-badge-missing",
        "Starting" | "Stopping" | "Reloading" | "Maintenance" => "svc-badge svc-badge-busy",
        _ => "svc-badge",
    };
    format!(
        r#"<span class="{class}">{label}</span>"#,
        class = class,
        label = html_escape(label),
    )
}

fn services_page_styles() -> &'static str {
    r#"<style>
.svc-badge{
  display:inline-flex;align-items:center;min-height:24px;padding:0 8px;border-radius:999px;
  font-size:11px;font-weight:700;letter-spacing:.04em;text-transform:uppercase;
  background:#eef2f6;color:#0f172a;
}
.svc-badge-ok{background:#dcfce7;color:#14532d;}
.svc-badge-idle{background:#e2e8f0;color:#334155;}
.svc-badge-bad{background:#fee2e2;color:#991b1b;}
.svc-badge-missing{background:#f1f5f9;color:#64748b;}
.svc-badge-busy{background:#ffedd5;color:#9a3412;}
[data-color-mode="dark"] .svc-badge{background:#334155;color:#f8fafc;}
[data-color-mode="dark"] .svc-badge-ok{background:rgba(22,163,74,.28);color:#bbf7d0;}
[data-color-mode="dark"] .svc-badge-idle{background:#475569;color:#e2e8f0;}
[data-color-mode="dark"] .svc-badge-bad{background:rgba(220,38,38,.28);color:#fecaca;}
[data-color-mode="dark"] .svc-badge-missing{background:#1e293b;color:#94a3b8;}
[data-color-mode="dark"] .svc-badge-busy{background:rgba(234,88,12,.28);color:#fed7aa;}
.inline-form{display:flex;flex-wrap:wrap;gap:6px;align-items:center;}
</style>"#
}

fn service_unit_label(row: &crate::panel_ops_services::ServiceRow) -> String {
    let unit = html_escape(&row.unit);
    match row.via.as_deref() {
        Some("cli") => format!(r#"<code>{unit}</code> <span class="muted">(CLI)</span>"#),
        Some(via) => format!(
            r#"<code>{unit}</code> <span class="muted">(via {via})</span>"#,
            via = html_escape(via),
        ),
        None => format!(r#"<code>{unit}</code>"#),
    }
}

fn service_install_action(row: &crate::panel_ops_services::ServiceRow) -> String {
    let href = row
        .install_href
        .clone()
        .unwrap_or_else(|| crate::panel_ops_services::store_install_href(&row.unit));
    format!(
        r#"<a class="btn-secondary" href="{href}">Install from Plugin Store</a>"#,
        href = html_escape(&href),
    )
}

pub fn services_page(notice: Option<&str>, error: Option<&str>, is_admin: bool) -> String {
    let rows = list_known_services();
    let mut table = String::from(
        r#"<div class="table-wrap"><table class="data-table"><thead><tr><th>Unit</th><th>Active</th><th>Enabled</th><th></th></tr></thead><tbody>"#,
    );
    for row in &rows {
        let actions = if is_admin && row.present {
            format!(
                r#"<form method="post" action="/server/services/control" class="inline-form">
              <input type="hidden" name="unit" value="{unit}">
              <button name="action" value="start" type="submit" class="btn-secondary">Start</button>
              <button name="action" value="stop" type="submit" class="btn-danger">Stop</button>
              <button name="action" value="restart" type="submit" class="btn-secondary">Restart</button>
            </form>"#,
                unit = html_escape(&row.unit),
            )
        } else if !is_admin {
            "<span class=\"muted\">Admin only</span>".into()
        } else if !row.present {
            service_install_action(row)
        } else {
            "<span class=\"muted\">Unavailable</span>".into()
        };
        table.push_str(&format!(
            r#"<tr><td>{unit}</td><td>{active}</td><td>{enabled}</td><td>{actions}</td></tr>"#,
            unit = service_unit_label(row),
            active = service_status_badge(&row.active),
            enabled = service_status_badge(&row.enabled),
            actions = actions,
        ));
    }
    table.push_str("</tbody></table></div>");
    let note = if is_admin {
        "<p class=\"muted\">Runtime shows Active or Inactive. Boot policy shows Enabled or Deactivated (Plugins-style). Missing units offer Install from Plugin Store (Host packages). The <code>docker</code> row also detects Podman (<code>podman.socket</code> / <code>podman</code>). Actions call systemctl for an allowlisted unit set only.</p>"
    } else {
        "<p class=\"muted\">Only the panel admin can start or stop services.</p>"
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("Services Status", None),
        ],
        "Services Status",
        "Start and stop known hosting services.",
        &format!("{}{table}{note}", services_page_styles()),
        notice,
        error,
    )
}

pub fn run_service_control(user: &str, unit: &str, action: &str) -> Result<String, String> {
    if !is_panel_admin(user) {
        return Err("Only the panel admin can control services".into());
    }
    control_service(unit, action)
}

pub fn php_tuning_page() -> String {
    let info = detect_php();
    let kv = status_kv(&[
        ("Binary", info.binary.as_deref().unwrap_or("Not found")),
        ("Version", info.version.as_deref().unwrap_or("-")),
        ("ini", info.ini_path.as_deref().unwrap_or("-")),
        ("Modules", &info.modules.len().to_string()),
    ]);
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("PHP Tuning", None),
        ],
        "PHP Tuning",
        "Read-only overview until guided writes ship.",
        &format!("{kv}<p class=\"muted\">{}</p>", html_escape(&info.detail)),
        None,
        None,
    )
}

pub fn package_manager_page(query: &str) -> String {
    let status = package_manager_status(query);
    let sample = if status.sample.is_empty() {
        "<p class=\"empty-state\">No package lines to show.</p>".into()
    } else {
        let mut pre =
            String::from(r#"<pre style="max-height:420px;overflow:auto;font-size:12px;">"#);
        for line in &status.sample {
            pre.push_str(&html_escape(line));
            pre.push('\n');
        }
        pre.push_str("</pre>");
        pre
    };
    let form = r#"<form method="get" action="/server/packages" class="stack-form" style="max-width:420px;">
      <label for="q">Search (read-only)</label>
      <input id="q" name="q" type="text" placeholder="nginx">
      <button type="submit" class="btn-primary">Search</button>
    </form>"#;
    let kv = status_kv(&[("Tool", status.tool.as_deref().unwrap_or("None"))]);
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("Package Manager", None),
        ],
        "Package Manager",
        "Read-only dnf/apt status. Installs stay allowlisted later.",
        &format!(
            "{kv}<p class=\"muted\">{}</p>{form}{sample}",
            html_escape(&status.detail)
        ),
        None,
        None,
    )
}

pub fn docker_page(kind: &str) -> String {
    crate::panel_hub_pages_docker::docker_page(kind)
}
