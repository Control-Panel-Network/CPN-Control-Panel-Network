//! HTML for Server hub and feature tile pages.

use crate::panel_admin::is_panel_admin;
use crate::panel_hub_defs::server_hub_sections;
use crate::panel_hubs::{
    feature_shell, hub_tiles_grid, not_configured_body, section_heading, status_kv,
};
use crate::panel_ops_docker::docker_status;
use crate::panel_ops_path::{list_dir, resolve_under_allowlist};
use crate::panel_ops_php::{detect_php, read_php_ini_preview};
use crate::panel_ops_pkgmgr::package_manager_status;
use crate::panel_ops_process::snapshot_top_processes;
use crate::panel_ops_services::{control_service, list_known_services};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn server_hub_main() -> String {
    let mut body = section_heading(
        "Server",
        "Host services, PHP, containers, files, and DNS tools for this CPN node.",
    );
    for (title, tiles) in server_hub_sections() {
        body.push_str(&hub_tiles_grid(title, &tiles));
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

pub fn processes_page() -> String {
    let body = match snapshot_top_processes(25) {
        Ok(rows) if rows.is_empty() => "<p class=\"empty-state\">No processes returned.</p>".into(),
        Ok(rows) => {
            let mut t = String::from(
                r#"<div class="table-wrap"><table class="data-table"><thead><tr><th>User</th><th>PID</th><th>CPU%</th><th>MEM%</th><th>Command</th></tr></thead><tbody>"#,
            );
            for r in rows {
                t.push_str(&format!(
                    r#"<tr><td>{user}</td><td>{pid}</td><td>{cpu}</td><td>{mem}</td><td><code>{cmd}</code></td></tr>"#,
                    user = html_escape(&r.user),
                    pid = html_escape(&r.pid),
                    cpu = html_escape(&r.cpu),
                    mem = html_escape(&r.mem),
                    cmd = html_escape(&r.command),
                ));
            }
            t.push_str("</tbody></table></div>");
            t
        }
        Err(err) => format!(
            "<p class=\"panel-notice error\">{e}</p>",
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

pub fn php_extensions_page() -> String {
    let info = detect_php();
    let mods = if info.modules.is_empty() {
        "<p class=\"empty-state\">No modules listed.</p>".into()
    } else {
        let mut ul = String::from("<ul>");
        for m in &info.modules {
            ul.push_str(&format!("<li><code>{}</code></li>", html_escape(m)));
        }
        ul.push_str("</ul>");
        ul
    };
    let kv = status_kv(&[
        ("Binary", info.binary.as_deref().unwrap_or("Not found")),
        ("Version", info.version.as_deref().unwrap_or("-")),
    ]);
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("PHP Extensions", None),
        ],
        "PHP Extensions",
        "Modules reported by the PHP CLI.",
        &format!(
            "{kv}<p class=\"muted\">{}</p>{mods}",
            html_escape(&info.detail)
        ),
        None,
        None,
    )
}

pub fn php_configs_page() -> String {
    let body = match read_php_ini_preview(12_000) {
        Ok((path, text)) => format!(
            r#"<p>Loaded configuration: <code>{path}</code></p>
            <pre style="max-height:420px;overflow:auto;white-space:pre-wrap;font-size:12px;">{preview}</pre>
            <p class="muted">Read-only preview (truncated). Writes with backup land in a later release.</p>"#,
            path = html_escape(&path.display().to_string()),
            preview = html_escape(&text),
        ),
        Err(err) => not_configured_body(&err, "Install PHP CLI so CPN can locate php.ini."),
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("PHP Configs", None),
        ],
        "PHP Configs",
        "Show the loaded php.ini path and a safe preview.",
        &body,
        None,
        None,
    )
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
    let status = docker_status();
    if !status.installed {
        return feature_shell(
            &[
                ("Dashboard", Some("/dashboard")),
                ("Server", Some("/server")),
                (kind, None),
            ],
            kind,
            "Container tooling on this host.",
            &not_configured_body(
                &status.detail,
                "Install Docker or Podman to enable this tile.",
            ),
            None,
            None,
        );
    }
    let lines = match kind {
        "Docker Images" => &status.images,
        "Containers" => &status.containers,
        _ => &status.containers,
    };
    let list = if lines.is_empty() {
        "<p class=\"empty-state\">No entries.</p>".into()
    } else {
        let mut ul = String::from("<ul>");
        for line in lines {
            ul.push_str(&format!("<li><code>{}</code></li>", html_escape(line)));
        }
        ul.push_str("</ul>");
        ul
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            (kind, None),
        ],
        kind,
        &status.detail,
        &list,
        None,
        None,
    )
}

pub fn files_page(path_q: &str, error: Option<&str>) -> String {
    let resolved = resolve_under_allowlist(path_q);
    let body = match resolved {
        Ok(path) => match list_dir(&path) {
            Ok(entries) => {
                let mut t = format!(
                    r#"<p>Browsing <code>{path}</code> (allowlisted roots only).</p>
                    <form method="get" action="/server/files" class="stack-form" style="max-width:560px;">
                      <label for="path">Path</label>
                      <input id="path" name="path" type="text" value="{path}">
                      <button type="submit" class="btn-primary">Open</button>
                    </form>
                    <div class="table-wrap"><table class="data-table"><thead><tr><th>Name</th><th>Type</th><th>Size</th></tr></thead><tbody>"#,
                    path = html_escape(&path.display().to_string()),
                );
                if let Some(parent) = path.parent() {
                    t.push_str(&format!(
                        r#"<tr><td><a href="/server/files?path={p}">..</a></td><td>dir</td><td></td></tr>"#,
                        p = urlencoding_simple(&parent.display().to_string()),
                    ));
                }
                for (name, is_dir, size) in entries {
                    let child = path.join(&name);
                    if is_dir {
                        t.push_str(&format!(
                            r#"<tr><td><a href="/server/files?path={p}">{name}</a></td><td>dir</td><td></td></tr>"#,
                            p = urlencoding_simple(&child.display().to_string()),
                            name = html_escape(&name),
                        ));
                    } else {
                        t.push_str(&format!(
                            r#"<tr><td>{name}</td><td>file</td><td>{size}</td></tr>"#,
                            name = html_escape(&name),
                            size = size,
                        ));
                    }
                }
                t.push_str("</tbody></table></div>");
                t
            }
            Err(err) => format!(
                "{}{}",
                not_configured_body(&err, "Pick an existing directory under /home or /var/www."),
                ""
            ),
        },
        Err(err) => not_configured_body(
            &err,
            "Only /home, /var/www, and the CPN data dir are allowed.",
        ),
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("Root File Manager", None),
        ],
        "Root File Manager",
        "Browse allowlisted roots with path traversal guards.",
        &body,
        None,
        error,
    )
}

fn urlencoding_simple(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 3);
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
