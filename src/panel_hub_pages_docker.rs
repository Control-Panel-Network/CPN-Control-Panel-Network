//! Docker Host package manage UI: Active Containers table and images list.

use crate::panel_hubs::{feature_shell, not_configured_body};
use crate::panel_ops_docker::{
    DockerContainerRow, docker_status, list_containers_detailed, list_images_detailed,
};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn action_form(action: &str, name: &str, label: &str, class: &str, confirm: &str) -> String {
    format!(
        r#"<form method="post" action="/docker/container" class="inline-form" style="display:inline;" onsubmit="return confirm('{confirm}');">
      <input type="hidden" name="action" value="{action}">
      <input type="hidden" name="name" value="{name}">
      <button type="submit" class="{class}" title="{label}">{label}</button>
    </form>"#,
        action = html_escape(action),
        name = html_escape(name),
        label = html_escape(label),
        class = html_escape(class),
        confirm = html_escape(confirm),
    )
}

fn container_actions(row: &DockerContainerRow) -> String {
    let mut out = String::new();
    if row.running {
        out.push_str(&action_form(
            "stop",
            &row.name,
            "Stop",
            "btn-secondary",
            &format!("Stop container {}?", row.name),
        ));
        out.push_str(&action_form(
            "restart",
            &row.name,
            "Restart",
            "btn-secondary",
            &format!("Restart container {}?", row.name),
        ));
    } else {
        out.push_str(&action_form(
            "start",
            &row.name,
            "Start",
            "btn-primary",
            &format!("Start container {}?", row.name),
        ));
    }
    out.push_str(&format!(
        r#"<a class="btn-secondary" href="/docker/logs?name={name}">Logs</a>"#,
        name = urlencoding_simple(&row.name),
    ));
    if row.cpn_managed {
        out.push_str(
            r#"<span class="muted" title="Labeled com.cpn.managed=1">Protected</span>"#,
        );
    } else {
        out.push_str(&action_form(
            "remove",
            &row.name,
            "Remove",
            "btn-danger",
            &format!(
                "Permanently remove container {}? This does not delete named volumes.",
                row.name
            ),
        ));
    }
    out
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

fn containers_table(rows: &[DockerContainerRow]) -> String {
    if rows.is_empty() {
        return r#"<p class="empty-state">No containers yet. Pull an image and run a container from the host CLI, or use Manage Images.</p>"#.into();
    }
    let mut body = String::from(
        r#"<div class="table-wrap"><table class="data-table docker-containers-table">
      <thead><tr>
        <th scope="col">Container</th>
        <th scope="col">Owner</th>
        <th scope="col">Image</th>
        <th scope="col">Tag</th>
        <th scope="col">Status</th>
        <th scope="col">Actions</th>
      </tr></thead><tbody>"#,
    );
    for row in rows {
        let managed = if row.cpn_managed {
            r#" <span class="plugin-badge">CPN</span>"#
        } else {
            ""
        };
        body.push_str(&format!(
            r#"<tr>
          <td><strong>{name}</strong>{managed}<br><code class="muted">{id}</code></td>
          <td>{owner}</td>
          <td><code>{image}</code></td>
          <td><code>{tag}</code></td>
          <td>{status}</td>
          <td class="docker-actions">{actions}</td>
        </tr>"#,
            name = html_escape(&row.name),
            managed = managed,
            id = html_escape(&row.id),
            owner = html_escape(&row.owner),
            image = html_escape(&row.image),
            tag = html_escape(&row.tag),
            status = html_escape(&row.status),
            actions = container_actions(row),
        ));
    }
    body.push_str("</tbody></table></div>");
    body
}

fn images_table() -> String {
    match list_images_detailed() {
        Ok(rows) if rows.is_empty() => {
            r#"<p class="empty-state">No images on this host.</p>"#.into()
        }
        Ok(rows) => {
            let mut body = String::from(
                r#"<div class="table-wrap"><table class="data-table">
          <thead><tr>
            <th scope="col">Repository</th>
            <th scope="col">Tag</th>
            <th scope="col">Id</th>
            <th scope="col">Size</th>
          </tr></thead><tbody>"#,
            );
            for row in rows {
                body.push_str(&format!(
                    r#"<tr>
              <td><code>{repo}</code></td>
              <td><code>{tag}</code></td>
              <td><code>{id}</code></td>
              <td>{size}</td>
            </tr>"#,
                    repo = html_escape(&row.repository),
                    tag = html_escape(&row.tag),
                    id = html_escape(&row.id),
                    size = html_escape(&row.size),
                ));
            }
            body.push_str("</tbody></table></div>");
            body
        }
        Err(e) => format!(
            r#"<p class="panel-notice error" role="status">{}</p>"#,
            html_escape(&e)
        ),
    }
}

fn toolbar() -> String {
    r#"<p class="stack-actions" style="margin-bottom:16px;">
      <a class="btn-primary" href="/docker">Active Containers</a>
      <a class="btn-secondary" href="/docker/images">Manage Images</a>
      <a class="btn-secondary" href="/plugins?view=store&amp;category=Host&amp;q=docker">Host package</a>
    </p>
    <p class="muted">CPN-managed containers (label <code>com.cpn.managed=1</code>) cannot be removed from this UI. Upgrade <code>--bypass</code> refreshes only those stacks.</p>"#
        .into()
}

/// Primary manage page: Active Containers table (Host package Manage + /docker).
pub fn docker_manage_page(notice: Option<&str>, error: Option<&str>) -> String {
    let status = docker_status();
    if !status.installed {
        return feature_shell(
            &[
                ("Dashboard", Some("/dashboard")),
                ("Server", Some("/server")),
                ("Docker", None),
            ],
            "Docker",
            "Container engine for this CPN host.",
            &format!(
                "{}{}",
                not_configured_body(
                    &status.detail,
                    "Install the Docker host package from Plugins > Store (search docker), or run cpn app install --name docker."
                ),
                r#"<p style="margin-top:12px;"><a class="btn-primary" href="/plugins?view=store&amp;category=Host&amp;q=docker">Open Store</a></p>"#
            ),
            notice,
            error,
        );
    }
    if !status.running {
        return feature_shell(
            &[
                ("Dashboard", Some("/dashboard")),
                ("Server", Some("/server")),
                ("Docker", None),
            ],
            "Docker",
            &status.detail,
            &format!(
                "{}{}",
                not_configured_body(
                    "The container CLI is installed but the engine is not running.",
                    "Use Start on the Docker host package card, or systemctl start docker / podman."
                ),
                toolbar()
            ),
            notice,
            error,
        );
    }
    let table = match list_containers_detailed() {
        Ok(rows) => containers_table(&rows),
        Err(e) => format!(
            r#"<p class="panel-notice error" role="status">{}</p>"#,
            html_escape(&e)
        ),
    };
    let body = format!(
        r#"{toolbar}
      <h2 style="margin:18px 0 10px;">Active Containers</h2>
      {table}"#,
        toolbar = toolbar(),
        table = table,
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("Docker", None),
        ],
        "Container Management",
        "Manage and monitor containers on this CPN host.",
        &body,
        notice,
        error,
    )
}

pub fn docker_images_page(notice: Option<&str>, error: Option<&str>) -> String {
    let status = docker_status();
    if !status.installed {
        return docker_manage_page(notice, error);
    }
    let body = format!(
        r#"{toolbar}
      <h2 style="margin:18px 0 10px;">Images</h2>
      {table}"#,
        toolbar = toolbar(),
        table = images_table(),
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("Docker", Some("/docker")),
            ("Images", None),
        ],
        "Docker Images",
        &status.detail,
        &body,
        notice,
        error,
    )
}

pub fn docker_logs_page(name: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let logs = crate::panel_ops_docker::container_logs(name, 200).unwrap_or_else(|e| e);
    let body = format!(
        r#"{toolbar}
      <h2 style="margin:18px 0 10px;">Logs: {name}</h2>
      <pre class="code-block" style="max-height:480px;overflow:auto;">{logs}</pre>
      <p><a class="btn-secondary" href="/docker">Back to containers</a></p>"#,
        toolbar = toolbar(),
        name = html_escape(name),
        logs = html_escape(&logs),
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Docker", Some("/docker")),
            ("Logs", None),
        ],
        "Container Logs",
        "Recent stdout/stderr from the selected container.",
        &body,
        notice,
        error,
    )
}

/// Legacy Server hub tiles still call this with a kind label.
pub fn docker_page(kind: &str) -> String {
    match kind {
        "Docker Images" => docker_images_page(None, None),
        _ => docker_manage_page(None, None),
    }
}
