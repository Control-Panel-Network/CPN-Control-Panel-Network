//! Compose stacks UI: persistent host data + pull/up refresh.

use crate::panel_hub_pages_docker::{html_escape, toolbar};
use crate::panel_hubs::{feature_shell, not_configured_body};
use crate::panel_ops_docker::docker_status;
use crate::panel_ops_docker_compose::{
    ComposeStackRow, OfficialStackTemplate, compose_host_data_root, compose_projects_root,
    list_compose_stacks,
};

fn stacks_table(rows: &[ComposeStackRow]) -> String {
    if rows.is_empty() {
        return format!(
            r#"<p class="empty-state">No compose stacks yet. Create one below with an official upstream image and host data under <code>{}</code>.</p>"#,
            html_escape(&compose_host_data_root().display().to_string())
        );
    }
    let mut body = String::from(
        r#"<div class="table-wrap"><table class="data-table">
      <thead><tr>
        <th scope="col">Stack</th>
        <th scope="col">Image</th>
        <th scope="col">Host data</th>
        <th scope="col">Actions</th>
      </tr></thead><tbody>"#,
    );
    for row in rows {
        body.push_str(&format!(
            r#"<tr>
          <td><strong>{id}</strong><br><code class="muted">{dir}</code></td>
          <td><code>{image}</code></td>
          <td><code>{data}</code></td>
          <td class="docker-actions">
            <form method="post" action="/docker/stacks/refresh" class="inline-form" style="display:inline;" onsubmit="return confirm('Pull latest images and recreate containers for stack {id}? Host data is kept.');">
              <input type="hidden" name="stack" value="{id}">
              <button type="submit" class="btn-primary" title="docker compose pull && up -d">Pull &amp; Recreate</button>
            </form>
          </td>
        </tr>"#,
            id = html_escape(&row.id),
            dir = html_escape(&row.project_dir.display().to_string()),
            image = html_escape(&row.image_hint),
            data = html_escape(&row.data_dir.display().to_string()),
        ));
    }
    body.push_str("</tbody></table></div>");
    body
}

fn template_options(selected: OfficialStackTemplate) -> String {
    let choices = [
        (OfficialStackTemplate::Custom, "custom"),
        (OfficialStackTemplate::Nginx, "nginx"),
        (OfficialStackTemplate::MariaDb, "mariadb"),
        (OfficialStackTemplate::Redis, "redis"),
    ];
    let mut out = String::new();
    for (tpl, value) in choices {
        let sel = if tpl == selected { " selected" } else { "" };
        out.push_str(&format!(
            r#"<option value="{value}"{sel}>{label}</option>"#,
            value = value,
            sel = sel,
            label = html_escape(tpl.label()),
        ));
    }
    out
}

fn create_stack_form(prefill_image: Option<&str>, prefill_template: Option<&str>) -> String {
    let tpl = OfficialStackTemplate::from_form(prefill_template.unwrap_or("nginx"));
    let default_image = prefill_image
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| tpl.default_image().map(|s| s.to_string()))
        .unwrap_or_else(|| "nginx:alpine".into());
    let data_path = tpl.default_container_data_path();
    let ports = match tpl {
        OfficialStackTemplate::Nginx => "8080:80",
        OfficialStackTemplate::MariaDb => "3306:3306",
        OfficialStackTemplate::Redis => "6379:6379",
        OfficialStackTemplate::Custom => "",
    };
    format!(
        r#"<div class="panel-card" style="margin:18px 0;">
      <h2 style="margin:0 0 8px;font-size:18px;">Create Compose Stack</h2>
      <p class="muted" style="margin:0 0 14px;">Prefer official or maintainer-published images from Docker Hub (Docker Official Images, verified publishers, or the software vendor). CPN writes <code>compose.yml</code> under <code>{compose_root}</code> and bind-mounts host data under <code>{data_root}/&lt;stack&gt;/data</code> so <code>docker compose pull</code> and <code>docker compose up -d</code> keep your files.</p>
      <form method="post" action="/docker/stacks/create" class="docker-stack-form" style="display:grid;gap:12px;max-width:720px;">
        <label>Quick template <span class="muted">(official upstream defaults)</span>
          <select name="template" style="width:100%;max-width:420px;">{template_opts}</select>
        </label>
        <label>Stack name <span class="muted">(lowercase, unique)</span>
          <input type="text" name="stack" required maxlength="48" pattern="[a-z0-9][a-z0-9-]*" placeholder="my-nginx" autocomplete="off" style="width:100%;">
        </label>
        <label>Image <span class="muted">(upstream ref)</span>
          <input type="text" name="image" required maxlength="255" value="{image}" autocomplete="off" style="width:100%;">
        </label>
        <label>Container data path
          <input type="text" name="container_data_path" required maxlength="120" value="{data_path}" autocomplete="off" style="width:100%;">
        </label>
        <label>Ports <span class="muted">(host:container)</span>
          <input type="text" name="ports" maxlength="200" value="{ports}" placeholder="8080:80" autocomplete="off" style="width:100%;">
        </label>
        <label>Environment <span class="muted">(KEY=value, one per line)</span>
          <textarea name="env" rows="3" maxlength="4000" placeholder="TZ=UTC" style="width:100%;font-family:monospace;"></textarea>
        </label>
        <p style="margin:0;"><button type="submit" class="btn-primary">Create stack and start</button></p>
      </form>
    </div>"#,
        compose_root = html_escape(&compose_projects_root().display().to_string()),
        data_root = html_escape(&compose_host_data_root().display().to_string()),
        template_opts = template_options(tpl),
        image = html_escape(&default_image),
        data_path = html_escape(data_path),
        ports = html_escape(ports),
    )
}

pub fn docker_stacks_page(
    notice: Option<&str>,
    error: Option<&str>,
    prefill_image: Option<&str>,
    prefill_template: Option<&str>,
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
            &not_configured_body(
                &status.detail,
                "Install the Docker host package from Plugins > Store (search docker).",
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
            &not_configured_body(
                "The container CLI is installed but the engine is not running.",
                "Start docker or podman on this host.",
            ),
            notice,
            error,
        );
    }
    let table = match list_compose_stacks() {
        Ok(rows) => stacks_table(&rows),
        Err(e) => format!(
            r#"<p class="panel-notice error" role="status">{}</p>"#,
            html_escape(&e)
        ),
    };
    let body = format!(
        r##"{toolbar}
      <p class="muted" style="margin:0 0 12px;">Workflow: pull an official image, create a stack with host volumes, then use <strong>Pull &amp; Recreate</strong> after upstream updates (same as upgrade <code>--bypass</code> for CPN-managed stacks).</p>
      <h2 style="margin:18px 0 10px;">Compose stacks</h2>
      {table}
      {create}
      <p class="muted" style="margin-top:16px;">Need a one-off container without compose? <a href="/docker/create">Create Container</a> (add a bind mount under the CPN data root for persistence).</p>"##,
        toolbar = toolbar("stacks"),
        table = table,
        create = create_stack_form(prefill_image, prefill_template),
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("Docker", Some("/docker")),
            ("Compose Stacks", None),
        ],
        "Compose Stacks",
        "Persistent host data with official upstream images.",
        &body,
        notice,
        error,
    )
}
