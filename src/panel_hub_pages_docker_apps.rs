//! Docker Apps one-click UI for n8n and Hermes Agent.

use crate::panel_hub_pages_docker::{html_escape, toolbar};
use crate::panel_hubs::feature_shell;
use crate::panel_ops_docker_csrf::docker_csrf_hidden;
use crate::panel_ops_docker_oneclick::{
    compose_provider_hint, list_oneclick_with_live, open_url_for_stack,
};

fn app_card(title: &str, template: &str, blurb: &str, default_stack: &str, csrf: &str) -> String {
    format!(
        r#"<div class="panel-card" style="flex:1;min-width:260px;">
      <h2 style="margin:0 0 8px;font-size:18px;">{title}</h2>
      <p class="muted" style="margin:0 0 12px;">{blurb}</p>
      <form method="post" action="/docker/apps/deploy" style="display:grid;gap:10px;">
        {csrf}
        <input type="hidden" name="template" value="{template}">
        <label>Stack name
          <input type="text" name="stack" required maxlength="48" pattern="[a-z0-9][a-z0-9-]*" value="{stack}" autocomplete="off" style="width:100%;">
        </label>
        <label>FQDN <span class="muted">(optional; enables reverse-proxy snippet + Let's Encrypt)</span>
          <input type="text" name="fqdn" maxlength="253" placeholder="n8n.example.com" autocomplete="off" style="width:100%;">
        </label>
        <button type="submit" class="btn-primary">Create and start</button>
      </form>
    </div>"#,
        title = html_escape(title),
        blurb = html_escape(blurb),
        template = html_escape(template),
        stack = html_escape(default_stack),
        csrf = csrf,
    )
}

fn deployed_table(username: &str) -> String {
    let rows = list_oneclick_with_live();
    if rows.is_empty() {
        return r#"<p class="empty-state">No one-click apps deployed yet.</p>"#.into();
    }
    let csrf = docker_csrf_hidden(username);
    let mut body = String::from(
        r#"<div class="table-wrap"><table class="data-table">
      <thead><tr>
        <th scope="col">Stack</th>
        <th scope="col">App</th>
        <th scope="col">FQDN</th>
        <th scope="col">Status</th>
        <th scope="col">Actions</th>
      </tr></thead><tbody>"#,
    );
    for (row, present) in rows {
        let status = if present { "Present" } else { "Missing" };
        let open = open_url_for_stack(&row.stack_id)
            .map(|u| {
                format!(
                    r#"<a class="btn-secondary" href="{u}" target="_blank" rel="noopener noreferrer">Open</a>"#,
                    u = html_escape(&u)
                )
            })
            .unwrap_or_default();
        body.push_str(&format!(
            r#"<tr>
          <td data-label="Stack"><code>{id}</code></td>
          <td data-label="App">{tpl}</td>
          <td data-label="FQDN">{fqdn}</td>
          <td data-label="Status">{status}</td>
          <td data-label="Actions" class="docker-actions" style="display:flex;flex-wrap:wrap;gap:6px;">
            {open}
            <form method="post" action="/docker/apps/action" class="inline-form" style="display:inline;">
              {csrf}<input type="hidden" name="stack" value="{id}"><input type="hidden" name="action" value="start">
              <button type="submit" class="btn-secondary">Start</button>
            </form>
            <form method="post" action="/docker/apps/action" class="inline-form" style="display:inline;">
              {csrf}<input type="hidden" name="stack" value="{id}"><input type="hidden" name="action" value="stop">
              <button type="submit" class="btn-secondary">Stop</button>
            </form>
          </td>
        </tr>"#,
            id = html_escape(&row.stack_id),
            tpl = html_escape(&row.template),
            fqdn = html_escape(if row.fqdn.is_empty() { "—" } else { &row.fqdn }),
            status = status,
            open = open,
            csrf = csrf,
        ));
    }
    body.push_str("</tbody></table></div>");
    body
}

pub fn docker_apps_page(username: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let csrf = docker_csrf_hidden(username);
    let hint = compose_provider_hint();
    let body = format!(
        r##"{toolbar}
      <p class="muted" style="margin:0 0 14px;">One-click compose apps use the Docker Host package and persist data under the CPN docker-data root. Optional FQDN writes a reverse-proxy snippet and attempts Let's Encrypt.</p>
      <p class="muted" style="margin:0 0 18px;">Compose: {hint}</p>
      <div style="display:flex;flex-wrap:wrap;gap:16px;margin-bottom:22px;">
        {n8n}
        {hermes}
      </div>
      <h2 style="margin:0 0 10px;font-size:18px;">Deployed one-click apps</h2>
      {table}
      <p class="muted" style="margin-top:16px;">Advanced compose templates: <a href="/docker/stacks">Compose Stacks</a>.</p>"##,
        toolbar = toolbar("apps"),
        hint = html_escape(&hint),
        n8n = app_card(
            "n8n",
            "n8n",
            "Workflow automation with persistent volume under /var/lib/cpn/docker-data.",
            "n8n",
            &csrf,
        ),
        hermes = app_card(
            "Hermes Agent",
            "hermes-agent",
            "CPN Hermes Agent stack with host data bind-mount and optional public FQDN.",
            "hermes-agent",
            &csrf,
        ),
        table = deployed_table(username),
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("Docker", Some("/docker")),
            ("Docker Apps", None),
        ],
        "Docker Apps",
        "One-click n8n and Hermes Agent compose stacks.",
        &body,
        notice,
        error,
    )
}
