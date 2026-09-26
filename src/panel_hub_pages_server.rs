//! HTML for Server hub and feature tile pages.

use crate::panel_admin::is_panel_admin;
use crate::panel_hub_defs::server_hub_sections;
use crate::panel_hubs::{feature_shell, hub_tiles_grid, section_heading, status_kv};
use crate::panel_ops_php::detect_php;
// PHP Extensions / Configurations live in panel_hub_pages_php_*.
use crate::panel_ops_pkgmgr::package_manager_status;
use crate::panel_ops_process::{
    ProcessRow, cpu_heat_class, snapshot_top_processes, truncate_command,
};
use crate::panel_ops_services::{control_service, list_known_services};

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
        } else {
            "<span class=\"muted\">Not present</span>".into()
        };
        table.push_str(&format!(
            r#"<tr><td><code>{unit}</code></td><td>{active}</td><td>{enabled}</td><td>{actions}</td></tr>"#,
            unit = html_escape(&row.unit),
            active = html_escape(&row.active),
            enabled = html_escape(&row.enabled),
            actions = actions,
        ));
    }
    table.push_str("</tbody></table></div>");
    let note = if is_admin {
        "<p class=\"muted\">Actions call systemctl for an allowlisted unit set only.</p>"
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
        &format!("{table}{note}"),
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

fn processes_page_styles() -> &'static str {
    r#"<style>
.proc-toolbar{display:flex;flex-wrap:wrap;gap:12px;align-items:center;justify-content:space-between;margin:0 0 14px;}
.proc-toolbar .muted{margin:0;max-width:52ch;}
.proc-toolbar .btn-secondary{
  display:inline-flex;align-items:center;justify-content:center;min-height:40px;padding:0 16px;
  border:0;border-radius:999px;font-weight:700;text-decoration:none;cursor:pointer;
  background:#e2e8f0;color:#0f172a;white-space:nowrap;
}
[data-color-mode="dark"] .proc-toolbar .btn-secondary{background:#334155;color:#f8fafc;}
.proc-table-wrap{margin-top:4px;overflow-x:hidden;}
.proc-table-wrap .data-table{width:100%;min-width:0;table-layout:fixed;}
.proc-table-wrap .data-table th:nth-child(1),.proc-table-wrap .data-table td:nth-child(1){width:14%;}
.proc-table-wrap .data-table th:nth-child(2),.proc-table-wrap .data-table td:nth-child(2){width:12%;}
.proc-table-wrap .data-table th:nth-child(3),.proc-table-wrap .data-table td:nth-child(3),
.proc-table-wrap .data-table th:nth-child(4),.proc-table-wrap .data-table td:nth-child(4){width:11%;}
.proc-table-wrap .data-table th:nth-child(5),.proc-table-wrap .data-table td:nth-child(5){width:52%;}
.proc-table-wrap .data-table th:last-child,.proc-table-wrap .data-table td:last-child{white-space:normal;}
.proc-cmd{display:block;max-width:100%;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;}
.proc-metric{font-variant-numeric:tabular-nums;font-weight:600;}
.proc-hot .proc-metric-cpu,.proc-card.proc-hot .proc-metric-cpu{color:#b42318;}
.proc-warm .proc-metric-cpu,.proc-card.proc-warm .proc-metric-cpu{color:#b54708;}
[data-color-mode="dark"] .proc-hot .proc-metric-cpu,
[data-color-mode="dark"] .proc-card.proc-hot .proc-metric-cpu{color:#fda29b;}
[data-color-mode="dark"] .proc-warm .proc-metric-cpu,
[data-color-mode="dark"] .proc-card.proc-warm .proc-metric-cpu{color:#fec84b;}
.proc-list{display:none;margin-top:8px;gap:12px;}
.proc-card{border:1px solid var(--hairline);border-radius:12px;padding:12px 14px;background:var(--canvas);min-width:0;}
.proc-card-top{display:flex;flex-wrap:wrap;gap:8px;align-items:center;justify-content:space-between;}
.proc-card-user{font-weight:700;font-size:14px;}
.proc-card-meta{display:flex;flex-wrap:wrap;gap:8px 14px;margin:10px 0 0;color:var(--muted);font-size:.88rem;}
.proc-card-cmd{margin:10px 0 0;font-size:13px;line-height:1.35;overflow:hidden;display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;word-break:break-word;}
@media (max-width:719.98px){
  .proc-table-wrap{display:none;}
  .proc-list{display:grid;}
  .proc-toolbar .btn-secondary{width:100%;}
}
@media (min-width:720px){
  .proc-list{display:none !important;}
}
</style>"#
}

fn processes_toolbar() -> &'static str {
    r#"<div class="proc-toolbar">
  <p class="muted">Live snapshot from <code>ps</code>, sorted by CPU. High CPU is highlighted.</p>
  <a class="btn-secondary" href="/server/processes">Refresh</a>
</div>"#
}

fn render_process_rows(rows: &[ProcessRow]) -> (String, String) {
    let mut table = String::from(
        r#"<div class="table-wrap proc-table-wrap"><table class="data-table"><thead><tr>
        <th>User</th><th>PID</th><th>CPU%</th><th>MEM%</th><th>Command</th>
        </tr></thead><tbody>"#,
    );
    let mut cards = String::from(r#"<div class="proc-list" aria-label="Top processes">"#);
    for row in rows {
        let heat = cpu_heat_class(&row.cpu);
        let row_class = if heat.is_empty() {
            String::new()
        } else {
            format!(r#" class="{heat}""#)
        };
        let card_class = if heat.is_empty() {
            "proc-card".to_string()
        } else {
            format!("proc-card {heat}")
        };
        let cmd_full = html_escape(&row.command);
        let cmd_short = html_escape(&truncate_command(&row.command, 72));
        let user = html_escape(&row.user);
        let pid = html_escape(&row.pid);
        let cpu = html_escape(&row.cpu);
        let mem = html_escape(&row.mem);
        table.push_str(&format!(
            r#"<tr{row_class}>
              <td>{user}</td>
              <td><code>{pid}</code></td>
              <td><span class="proc-metric proc-metric-cpu">{cpu}</span></td>
              <td><span class="proc-metric">{mem}</span></td>
              <td><code class="proc-cmd" title="{cmd_full}">{cmd_short}</code></td>
            </tr>"#
        ));
        cards.push_str(&format!(
            r#"<article class="{card_class}">
              <div class="proc-card-top">
                <span class="proc-card-user">{user}</span>
                <code>PID {pid}</code>
              </div>
              <div class="proc-card-meta">
                <span>CPU <span class="proc-metric proc-metric-cpu">{cpu}%</span></span>
                <span>MEM <span class="proc-metric">{mem}%</span></span>
              </div>
              <p class="proc-card-cmd" title="{cmd_full}"><code>{cmd_short}</code></p>
            </article>"#
        ));
    }
    table.push_str("</tbody></table></div>");
    cards.push_str("</div>");
    (table, cards)
}

pub fn processes_page() -> String {
    let styles = processes_page_styles();
    let toolbar = processes_toolbar();
    let body = match snapshot_top_processes(25) {
        Ok(rows) if rows.is_empty() => {
            format!("{styles}{toolbar}<p class=\"empty-state\">No processes returned.</p>")
        }
        Ok(rows) => {
            let (table, cards) = render_process_rows(&rows);
            format!("{styles}{toolbar}{table}{cards}")
        }
        Err(err) => format!(
            "{styles}{toolbar}<p class=\"panel-notice error\">{e}</p>",
            e = html_escape(&err)
        ),
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("Top Processes", None),
        ],
        "Top Processes",
        "Snapshot from ps (CPU sorted).",
        &body,
        None,
        None,
    )
}

#[cfg(test)]
mod processes_page_tests {
    use super::processes_page;

    #[test]
    fn processes_page_includes_responsive_chrome() {
        let html = processes_page();
        assert!(html.contains("proc-toolbar"));
        assert!(html.contains("Refresh"));
        assert!(html.contains("proc-table-wrap") || html.contains("panel-notice error"));
        assert!(
            html.contains("proc-list")
                || html.contains("empty-state")
                || html.contains("panel-notice error")
        );
    }
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
