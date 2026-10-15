//! Docker Host package manage UI: containers, images search/pull, create container.

use crate::panel_hub_pages_docker_list::{DockerListQuery, docker_list_toolbar, paginate_slice};
use crate::panel_hubs::{feature_shell, not_configured_body};
use crate::panel_ops_docker::{
    DockerContainerRow, docker_status, list_containers_detailed, list_images_detailed,
};
use crate::panel_ops_docker_inventory::filter_containers;

pub(crate) fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub(crate) fn urlencoding_simple(value: &str) -> String {
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

fn container_status_html(row: &DockerContainerRow) -> String {
    let lower = row.status.to_ascii_lowercase();
    if row.restarting {
        return format!(
            r#"<span class="docker-status docker-status-stopped"><strong>Restarting</strong><span class="docker-status-detail"> · {detail}</span></span>"#,
            detail = html_escape(row.status.trim()),
        );
    }
    if row.running {
        let uptime = row
            .status
            .strip_prefix("Up ")
            .or_else(|| row.status.strip_prefix("up "))
            .unwrap_or(row.status.as_str())
            .trim();
        format!(
            r#"<span class="docker-status docker-status-running"><strong>Running</strong><span class="docker-status-detail"> · {uptime}</span></span>"#,
            uptime = html_escape(uptime),
        )
    } else {
        let headline = if lower.contains("dead") {
            "Dead"
        } else {
            "Stopped"
        };
        format!(
            r#"<span class="docker-status docker-status-stopped"><strong>{headline}</strong><span class="docker-status-detail"> · {detail}</span></span>"#,
            headline = headline,
            detail = html_escape(row.status.trim()),
        )
    }
}

fn container_actions(row: &DockerContainerRow) -> String {
    let mut out = format!(
        r#"<a class="btn-secondary" href="/docker/view/{name}" title="Manage container">Settings</a>"#,
        name = urlencoding_simple(&row.name),
    );
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
        r#"<a class="btn-secondary" href="/docker/view/{name}#container-logs">Logs</a>"#,
        name = urlencoding_simple(&row.name),
    ));
    if row.cpn_managed {
        out.push_str(r#"<span class="muted" title="Labeled com.cpn.managed=1">Protected</span>"#);
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

fn containers_table(rows: &[&DockerContainerRow], empty_message: &str) -> String {
    if rows.is_empty() {
        return format!(
            r#"<p class="empty-state">{}</p>"#,
            html_escape(empty_message)
        );
    }
    let mut body = String::from(
        r#"<div class="table-wrap docker-list-wrap"><table class="data-table docker-containers-table docker-card-list">
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
        let owner_cell = if row.owner == "Unassigned" {
            r#"<span title="No com.cpn.owner label; created outside the panel.">Unassigned</span>"#
                .to_string()
        } else if row.owner == "Host" {
            r#"<span title="Host-managed or system container (no account owner label).">Host</span>"#
                .to_string()
        } else {
            html_escape(&row.owner)
        };
        body.push_str(&format!(
            r#"<tr>
          <td data-label="Container"><strong><a href="/docker/view/{name_link}">{name}</a></strong>{managed}<br><code class="muted">{id}</code></td>
          <td data-label="Owner">{owner}</td>
          <td data-label="Image"><code>{image}</code></td>
          <td data-label="Tag"><code>{tag}</code></td>
          <td data-label="Status">{status}</td>
          <td data-label="Actions" class="docker-actions">{actions}</td>
        </tr>"#,
            name = html_escape(&row.name),
            name_link = urlencoding_simple(&row.name),
            managed = managed,
            id = html_escape(&row.id),
            owner = owner_cell,
            image = html_escape(&row.image),
            tag = html_escape(&row.tag),
            status = container_status_html(row),
            actions = container_actions(row),
        ));
    }
    body.push_str("</tbody></table></div>");
    body
}

fn tab_class(active: &str, id: &str) -> &'static str {
    if active == id {
        "btn-primary"
    } else {
        "btn-secondary"
    }
}

pub(crate) fn toolbar(active: &str) -> String {
    format!(
        r#"<p class="stack-actions docker-tabs" style="margin-bottom:16px;display:flex;flex-wrap:wrap;gap:8px;">
      <a class="{c}" href="/docker/list">Active Containers</a>
      <a class="{a}" href="/docker/apps">Docker Apps</a>
      <a class="{s}" href="/docker/stacks">Compose Stacks</a>
      <a class="{i}" href="/docker/images">Manage Images</a>
      <a class="{h}" href="/plugins?view=store&amp;target=host&amp;q=docker">Host package</a>
    </p>
    <p class="muted">Prefer official or maintainer-published images from Docker Hub. CPN-managed stacks (label <code>com.cpn.managed=1</code>) keep data on the host under the CPN docker-data path; use Compose Stacks <strong>Pull &amp; Recreate</strong> or upgrade <code>--bypass</code> to refresh images without deleting volumes.</p>
    <p class="muted"><strong>Owner:</strong> panel-created containers use your account name; compose stacks use the owner you set on the stack. Host-managed stacks show <strong>Host</strong>. CLI or third-party containers show <strong>Unassigned</strong> until label <code>com.cpn.owner=username</code> is set. Image and tag come from the engine (<code>ps</code> plus <code>inspect</code> when needed).</p>"#,
        c = tab_class(active, "containers"),
        a = tab_class(active, "apps"),
        s = tab_class(active, "stacks"),
        i = tab_class(active, "images"),
        h = tab_class(active, "host"),
    )
}

fn create_container_form(prefill_image: &str) -> String {
    let img = html_escape(prefill_image);
    format!(
        r#"<div class="panel-card" style="margin:18px 0;">
      <h2 style="margin:0 0 8px;font-size:18px;">Create Container</h2>
      <p class="muted" style="margin:0 0 14px;">Run a local or pulled upstream image. For production data, prefer <a href="/docker/stacks">Compose Stacks</a> (host bind mounts). User containers are not labeled as CPN-managed.</p>
      <form method="post" action="/docker/create" class="docker-create-form">
        <label>Image <span class="muted">(required)</span>
          <input type="text" name="image" required maxlength="255" placeholder="nginx:alpine" value="{img}" autocomplete="off">
        </label>
        <label>Container name <span class="muted">(optional)</span>
          <input type="text" name="name" maxlength="64" placeholder="my-nginx" autocomplete="off">
        </label>
        <label>Ports <span class="muted">(host:container, comma or newline)</span>
          <input type="text" name="ports" maxlength="200" placeholder="8080:80" autocomplete="off">
        </label>
        <label>Volumes <span class="muted">(host_path:container_path, optional)</span>
          <input type="text" name="volumes" maxlength="400" placeholder="/var/lib/cpn/docker-data/myapp/data:/data" autocomplete="off">
        </label>
        <label>Environment <span class="muted">(KEY=value, one per line)</span>
          <textarea name="env" rows="3" maxlength="4000" placeholder="TZ=UTC"></textarea>
        </label>
        <label>Restart policy
          <select name="restart">
            <option value="no">no</option>
            <option value="unless-stopped" selected>unless-stopped</option>
            <option value="always">always</option>
            <option value="on-failure">on-failure</option>
          </select>
        </label>
        <label class="docker-inline-check">
          <input type="checkbox" name="start" value="1" checked>
          Start container after create
        </label>
        <p class="docker-form-actions"><button type="submit" class="btn-primary">Create Container</button></p>
      </form>
    </div>"#,
        img = img,
    )
}

/// Dedicated create-container page (form only).
pub fn docker_create_page(
    notice: Option<&str>,
    error: Option<&str>,
    prefill_image: Option<&str>,
) -> String {
    let status = docker_status();
    if !status.installed || !status.running {
        return docker_manage_page(notice, error, None);
    }
    let prefill = prefill_image.unwrap_or("");
    let body = format!(
        r##"{toolbar}
      <p class="stack-actions" style="margin:16px 0;display:flex;flex-wrap:wrap;gap:8px;">
        <a class="btn-secondary" href="/docker">← Active Containers</a>
      </p>
      {create}"##,
        toolbar = toolbar("containers"),
        create = create_container_form(prefill),
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("Docker", Some("/docker")),
            ("Create", None),
        ],
        "Create Container",
        "Run a local or pulled upstream image on this host.",
        &body,
        notice,
        error,
    )
}

/// Primary manage page: Active Containers list.
pub fn docker_manage_page(
    notice: Option<&str>,
    error: Option<&str>,
    list_query: Option<&DockerListQuery>,
) -> String {
    let list_query = list_query.cloned().unwrap_or(DockerListQuery {
        q: String::new(),
        page: 1,
        per_page: 10,
    });
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
                r#"<p style="margin-top:12px;"><a class="btn-primary" href="/plugins?view=store&amp;target=host&amp;q=docker">Open Store</a></p>"#
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
                toolbar("containers")
            ),
            notice,
            error,
        );
    }
    let image_count = list_images_detailed().map(|r| r.len()).unwrap_or(0);
    let (table, list_bar) = match list_containers_detailed() {
        Ok(rows) => {
            let filtered = filter_containers(&rows, &list_query.q);
            let owned: Vec<DockerContainerRow> = filtered.into_iter().cloned().collect();
            let total = owned.len();
            let (page_rows, total_pages) =
                paginate_slice(&owned, list_query.page, list_query.per_page);
            let refs: Vec<&DockerContainerRow> = page_rows.iter().collect();
            let empty = if rows.is_empty() {
                "No containers yet. Pull an image from Manage Images, then use Create Container."
            } else {
                "No containers match this search."
            };
            let bar = docker_list_toolbar(
                "/docker/list",
                "q",
                "name, image, owner, status",
                &list_query.q,
                list_query.page.min(total_pages),
                list_query.per_page,
                total_pages,
                total,
                "",
            );
            (containers_table(&refs, empty), bar)
        }
        Err(e) => (
            format!(
                r#"<p class="panel-notice error" role="status">{}</p>"#,
                html_escape(&e)
            ),
            String::new(),
        ),
    };
    let body = format!(
        r##"{toolbar}
      <p class="stack-actions" style="margin:16px 0;display:flex;flex-wrap:wrap;gap:8px;">
        <a class="btn-primary" href="/docker/create">+ Create Container</a>
        <a class="btn-secondary" href="/docker/stacks">Compose Stacks</a>
        <a class="btn-secondary" href="/docker/images">Manage Images ({image_count})</a>
      </p>
      <h2 style="margin:18px 0 10px;">Active Containers</h2>
      <p class="muted" style="margin:0 0 10px;">Running and stopped containers from the local Docker or Podman engine.</p>
      {list_bar}
      {table}"##,
        toolbar = toolbar("containers"),
        image_count = image_count,
        list_bar = list_bar,
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

pub fn docker_logs_page(name: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let logs = crate::panel_ops_docker::container_logs(name, 200).unwrap_or_else(|e| e);
    let body = format!(
        r#"{toolbar}
      <h2 style="margin:18px 0 10px;">Logs: {name}</h2>
      <pre class="code-block" style="max-height:480px;overflow:auto;">{logs}</pre>
      <p><a class="btn-secondary" href="/docker">Back to containers</a></p>"#,
        toolbar = toolbar("containers"),
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
        "Docker Images" => {
            crate::panel_hub_pages_docker_images_ui::docker_images_page(None, None, None, &[], None)
        }
        _ => docker_manage_page(None, None, None),
    }
}
