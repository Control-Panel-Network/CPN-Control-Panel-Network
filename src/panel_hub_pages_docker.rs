//! Docker Host package manage UI: containers, images search/pull, create container.

use crate::panel_hubs::{feature_shell, not_configured_body};
use crate::panel_ops_docker::{
    DockerContainerRow, DockerImageRow, docker_status, list_containers_detailed,
    list_images_detailed,
};
use crate::panel_ops_docker_images::{DockerHubSearchHit, image_in_use};

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

fn containers_table(rows: &[DockerContainerRow]) -> String {
    if rows.is_empty() {
        return r#"<p class="empty-state">No containers yet. Pull an image from Manage Images, then use Create Container.</p>"#.into();
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
      <a class="{c}" href="/docker">Active Containers</a>
      <a class="{s}" href="/docker/stacks">Compose Stacks</a>
      <a class="{i}" href="/docker/images">Manage Images</a>
      <a class="{h}" href="/plugins?view=store&amp;category=Host&amp;q=docker">Host package</a>
    </p>
    <p class="muted">Prefer official or maintainer-published images from Docker Hub. CPN-managed stacks (label <code>com.cpn.managed=1</code>) keep data on the host under the CPN docker-data path; use Compose Stacks <strong>Pull &amp; Recreate</strong> or upgrade <code>--bypass</code> to refresh images without deleting volumes.</p>"#,
        c = tab_class(active, "containers"),
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

fn search_results_html(hits: &[DockerHubSearchHit]) -> String {
    if hits.is_empty() {
        return String::new();
    }
    let mut body = String::from(
        r#"<div class="table-wrap" style="margin-top:12px;"><table class="data-table">
      <thead><tr>
        <th scope="col">Repository</th>
        <th scope="col">Description</th>
        <th scope="col">Stars</th>
        <th scope="col">Actions</th>
      </tr></thead><tbody>"#,
    );
    for hit in hits {
        let official = if hit.is_official {
            r#" <span class="plugin-badge">Official</span>"#
        } else {
            ""
        };
        let desc: String = hit.description.chars().take(120).collect();
        body.push_str(&format!(
            r#"<tr>
          <td><code>{name}</code>{official}</td>
          <td>{desc}</td>
          <td>{stars}</td>
          <td class="docker-actions">
            <form method="post" action="/docker/images/pull" class="inline-form" style="display:inline;">
              <input type="hidden" name="image" value="{name}:latest">
              <button type="submit" class="btn-primary">Pull</button>
            </form>
            <a class="btn-secondary" href="/docker?image={enc}">Create</a>
            <a class="btn-secondary" href="/docker/stacks?image={enc}&amp;template=custom">Stack</a>
          </td>
        </tr>"#,
            name = html_escape(&hit.name),
            official = official,
            desc = html_escape(&desc),
            stars = hit.star_count,
            enc = urlencoding_simple(&format!("{}:latest", hit.name)),
        ));
    }
    body.push_str("</tbody></table></div>");
    body
}

fn images_table(rows: &[DockerImageRow]) -> String {
    if rows.is_empty() {
        return r#"<p class="empty-state">No images on this host. Search Docker Hub above or pull by name (for example nginx:alpine).</p>"#.into();
    }
    let mut body = String::from(
        r#"<div class="table-wrap"><table class="data-table">
      <thead><tr>
        <th scope="col">Image name</th>
        <th scope="col">Tags</th>
        <th scope="col">Size</th>
        <th scope="col">Actions</th>
      </tr></thead><tbody>"#,
    );
    for row in rows {
        let ref_name = if row.repository == "<none>" || row.tag == "<none>" {
            row.id.clone()
        } else {
            format!("{}:{}", row.repository, row.tag)
        };
        let in_use = image_in_use(&row.repository, &row.tag, &row.id);
        let delete_btn = if in_use {
            r#"<span class="muted" title="In use by a container">In use</span>"#.to_string()
        } else {
            format!(
                r#"<form method="post" action="/docker/images/delete" class="inline-form" style="display:inline;" onsubmit="return confirm('Delete image {label}?');">
              <input type="hidden" name="repository" value="{repo}">
              <input type="hidden" name="tag" value="{tag}">
              <input type="hidden" name="id" value="{id}">
              <button type="submit" class="btn-danger" title="Delete">Delete</button>
            </form>"#,
                label = html_escape(&ref_name),
                repo = html_escape(&row.repository),
                tag = html_escape(&row.tag),
                id = html_escape(&row.id),
            )
        };
        body.push_str(&format!(
            r#"<tr>
          <td><code>{repo}</code><br><code class="muted">{id}</code></td>
          <td><code>{tag}</code></td>
          <td>{size}</td>
          <td class="docker-actions">
            <form method="post" action="/docker/images/pull" class="inline-form" style="display:inline;">
              <input type="hidden" name="image" value="{ref_name}">
              <button type="submit" class="btn-primary" title="Pull / update this tag">Pull</button>
            </form>
            <a class="btn-secondary" href="/docker?image={enc}">Create</a>
            {delete}
          </td>
        </tr>"#,
            repo = html_escape(&row.repository),
            id = html_escape(&row.id),
            tag = html_escape(&row.tag),
            size = html_escape(&row.size),
            ref_name = html_escape(&ref_name),
            enc = urlencoding_simple(&ref_name),
            delete = delete_btn,
        ));
    }
    body.push_str("</tbody></table></div>");
    body
}

fn search_pull_card(query: &str, hits: &[DockerHubSearchHit]) -> String {
    let q = html_escape(query);
    format!(
        r#"<div class="panel-card" style="margin:18px 0;">
      <h2 style="margin:0 0 8px;font-size:18px;">Search &amp; Pull Images</h2>
      <p class="muted" style="margin:0 0 12px;">Search Docker Hub for official or maintainer images (Official badge first), pull, then run as a container or <a href="/docker/stacks">compose stack</a> with host volumes.</p>
      <form method="get" action="/docker/images" class="docker-hub-form" style="grid-template-columns:1fr auto;align-items:end;margin-bottom:12px;">
        <label>Search Docker Hub
          <input type="search" name="q" value="{q}" maxlength="100" placeholder="nginx, mariadb, redis" autocomplete="off">
        </label>
        <button type="submit" class="btn-secondary">Search</button>
      </form>
      <form method="post" action="/docker/images/pull" class="docker-hub-form" style="grid-template-columns:1fr auto;align-items:end;">
        <label>Image to pull
          <input type="text" name="image" required maxlength="255" placeholder="nginx:alpine (Docker Official Image)" value="{q}" autocomplete="off">
        </label>
        <button type="submit" class="btn-primary">Pull</button>
      </form>
      {results}
    </div>"#,
        q = q,
        results = search_results_html(hits),
    )
}

/// Primary manage page: Active Containers + Create Container.
pub fn docker_manage_page(
    notice: Option<&str>,
    error: Option<&str>,
    prefill_image: Option<&str>,
) -> String {
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
                toolbar("containers")
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
    let prefill = prefill_image.unwrap_or("");
    let body = format!(
        r##"{toolbar}
      <p class="stack-actions" style="margin:16px 0;display:flex;flex-wrap:wrap;gap:8px;">
        <a class="btn-primary" href="#create-container">+ Create Container</a>
        <a class="btn-secondary" href="/docker/stacks">Compose Stacks</a>
        <a class="btn-secondary" href="/docker/images">Manage Images</a>
      </p>
      <h2 style="margin:18px 0 10px;">Active Containers</h2>
      {table}
      <div id="create-container">{create}</div>"##,
        toolbar = toolbar("containers"),
        table = table,
        create = create_container_form(prefill),
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

pub fn docker_images_page(
    notice: Option<&str>,
    error: Option<&str>,
    query: Option<&str>,
    hits: &[DockerHubSearchHit],
) -> String {
    let status = docker_status();
    if !status.installed {
        return docker_manage_page(notice, error, None);
    }
    let q = query.unwrap_or("");
    let table = match list_images_detailed() {
        Ok(rows) => images_table(&rows),
        Err(e) => format!(
            r#"<p class="panel-notice error" role="status">{}</p>"#,
            html_escape(&e)
        ),
    };
    let body = format!(
        r##"{toolbar}
      <p class="stack-actions" style="margin:16px 0;display:flex;flex-wrap:wrap;gap:8px;">
        <a class="btn-primary" href="/docker#create-container">+ Create Container</a>
        <a class="btn-secondary" href="/docker/images">Manage Images</a>
      </p>
      {search}
      <div class="panel-card" style="margin:18px 0;">
        <div style="display:flex;flex-wrap:wrap;align-items:center;justify-content:space-between;gap:10px;margin-bottom:12px;">
          <h2 style="margin:0;font-size:18px;">Local Images</h2>
          <form method="post" action="/docker/images/prune" onsubmit="return confirm('Prune all unused images on this host?');">
            <button type="submit" class="btn-secondary" style="background:#c2410c;border-color:#c2410c;color:#fff;">Prune Unused</button>
          </form>
        </div>
        {table}
      </div>"##,
        toolbar = toolbar("images"),
        search = search_pull_card(q, hits),
        table = table,
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("Docker", Some("/docker")),
            ("Images", None),
        ],
        "Docker Images",
        "Pull, manage, and organize images for your containers.",
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
        "Docker Images" => docker_images_page(None, None, None, &[]),
        _ => docker_manage_page(None, None, None),
    }
}
