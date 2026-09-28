//! Single-container detail page (reference-panel inspired layout, CPN branded).

use crate::panel_hub_pages_docker::{html_escape, toolbar, urlencoding_simple};
use crate::panel_hubs::feature_shell;
use crate::panel_ops_docker::container_logs;
use crate::panel_ops_docker_detail::{
    DockerContainerDetail, container_processes, load_container_detail,
};

fn action_post(action: &str, name: &str, label: &str, class: &str, confirm: &str) -> String {
    format!(
        r#"<form method="post" action="/docker/container" class="inline-form" style="display:inline;" onsubmit="return confirm('{confirm}');">
      <input type="hidden" name="action" value="{action}">
      <input type="hidden" name="name" value="{name}">
      <input type="hidden" name="return_to" value="/docker/view/{enc}">
      <button type="submit" class="{class}">{label}</button>
    </form>"#,
        action = html_escape(action),
        name = html_escape(name),
        enc = urlencoding_simple(name),
        label = html_escape(label),
        class = html_escape(class),
        confirm = html_escape(confirm),
    )
}

fn stat_card(title: &str, value: &str, hint: &str) -> String {
    format!(
        r#"<div class="panel-card" style="flex:1;min-width:140px;text-align:center;">
      <p class="muted" style="margin:0 0 6px;font-size:12px;text-transform:uppercase;letter-spacing:.04em;">{title}</p>
      <p style="margin:0;font-size:22px;font-weight:600;">{value}</p>
      <p class="muted" style="margin:6px 0 0;font-size:12px;">{hint}</p>
    </div>"#,
        title = html_escape(title),
        value = html_escape(value),
        hint = html_escape(hint),
    )
}

fn action_grid(detail: &DockerContainerDetail) -> String {
    let name = &detail.row.name;
    let mut buttons = String::new();
    if detail.row.running {
        buttons.push_str(&action_post(
            "stop",
            name,
            "Stop",
            "btn-secondary",
            &format!("Stop container {name}?"),
        ));
        buttons.push_str(&action_post(
            "restart",
            name,
            "Restart",
            "btn-secondary",
            &format!("Restart container {name}?"),
        ));
        if detail.paused {
            buttons.push_str(&action_post(
                "unpause",
                name,
                "Unpause",
                "btn-secondary",
                &format!("Unpause container {name}?"),
            ));
        } else {
            buttons.push_str(&action_post(
                "pause",
                name,
                "Pause",
                "btn-secondary",
                &format!("Pause container {name}?"),
            ));
        }
    } else {
        buttons.push_str(&action_post(
            "start",
            name,
            "Start",
            "btn-primary",
            &format!("Start container {name}?"),
        ));
    }
    let hash = "#";
    buttons.push_str(&format!(
        r#"<a class="btn-secondary" href="{hash}container-settings">Settings</a>
      <a class="btn-secondary" href="/docker/export?name={enc}" title="Download filesystem export">Export</a>
      <a class="btn-secondary" href="{hash}container-processes">Processes</a>
      <a class="btn-secondary" href="{hash}container-exec">Run Command</a>"#,
        hash = hash,
        enc = urlencoding_simple(name),
    ));
    if !detail.row.cpn_managed {
        buttons.push_str(&action_post(
            "remove",
            name,
            "Remove",
            "btn-danger",
            &format!(
                "Permanently remove container {name}? This does not delete named volumes.",
                name = name
            ),
        ));
    } else {
        buttons.push_str(
            r#"<span class="muted" title="CPN-managed compose container">Recreate via Compose Stacks</span>"#,
        );
    }
    format!(
        r#"<div class="stack-actions" style="display:flex;flex-wrap:wrap;gap:8px;">{buttons}</div>"#,
        buttons = buttons
    )
}

pub fn docker_container_view_page(
    name: &str,
    notice: Option<&str>,
    error: Option<&str>,
    exec_output: Option<&str>,
    log_lines: usize,
) -> String {
    let detail = match load_container_detail(name) {
        Ok(d) => d,
        Err(e) => {
            return feature_shell(
                &[
                    ("Dashboard", Some("/dashboard")),
                    ("Server", Some("/server")),
                    ("Docker", Some("/docker")),
                    ("Container", None),
                ],
                "Container",
                "Could not load container details.",
                &format!(
                    r#"{toolbar}<p class="panel-notice error" role="status">{}</p>
          <p><a class="btn-secondary" href="/docker">Back to Active Containers</a></p>"#,
                    html_escape(&e),
                    toolbar = toolbar("containers"),
                ),
                notice,
                error,
            );
        }
    };
    let row = &detail.row;
    let status_label = if detail.paused {
        "paused".to_string()
    } else if row.running {
        "running".to_string()
    } else {
        row.status.clone()
    };
    let owner_cell = if row.owner == "Unassigned" {
        r#"<span title="No com.cpn.owner label; created outside the panel.">Unassigned</span>"#.to_string()
    } else {
        html_escape(&row.owner)
    };
    let logs = container_logs(name, log_lines).unwrap_or_else(|e| e);
    let processes = container_processes(name).unwrap_or_else(|e| e);
    let exec_block = exec_output
        .map(|o| format!(
            r#"<pre class="code-block" style="max-height:240px;overflow:auto;margin-top:10px;">{}</pre>"#,
            html_escape(o)
        ))
        .unwrap_or_default();

    let body = format!(
        r##"{toolbar}
      <p style="margin:12px 0;"><a class="btn-secondary" href="/docker">← Active Containers</a></p>
      <header style="margin-bottom:18px;">
        <h1 style="margin:0 0 6px;font-size:24px;">{name} <span class="plugin-badge">{status}</span></h1>
        <p class="muted" style="margin:0;">Container ID: <code>{id}</code> · Owner: {owner}</p>
      </header>
      <div style="display:flex;flex-wrap:wrap;gap:12px;margin-bottom:20px;">
        {cpu_card}
        {mem_pct_card}
        {mem_limit_card}
      </div>
      <div class="panel-card" style="margin-bottom:18px;">
        <h2 style="margin:0 0 12px;font-size:18px;">Container Information</h2>
        <dl class="detail-grid" style="display:grid;grid-template-columns:minmax(120px,160px) 1fr;gap:8px 16px;margin:0;">
          <dt class="muted">Image</dt><dd><code>{image}</code>:<code>{tag}</code></dd>
          <dt class="muted">Port mappings</dt><dd><code>{ports}</code></dd>
          <dt class="muted">Restart policy</dt><dd>{restart}</dd>
          <dt class="muted">Start on boot</dt><dd>{boot}</dd>
          <dt class="muted">Memory usage</dt><dd>{mem_usage} ({mem_pct} of limit)</dd>
        </dl>
      </div>
      <div class="panel-card" style="margin-bottom:18px;">
        <h2 style="margin:0 0 12px;font-size:18px;">Container Actions</h2>
        {actions}
        <p class="muted" style="margin:12px 0 0;font-size:13px;">Recreate with a new image from <a href="/docker/images">Manage Images</a> or update CPN compose stacks under <a href="/docker/stacks">Compose Stacks</a>.</p>
      </div>
      <div class="panel-card" style="margin-bottom:18px;" id="container-logs">
        <div style="display:flex;flex-wrap:wrap;align-items:center;justify-content:space-between;gap:8px;margin-bottom:10px;">
          <h2 style="margin:0;font-size:18px;">Container Logs</h2>
          <div class="stack-actions" style="display:flex;gap:8px;">
            <a class="btn-secondary" href="/docker/view/{enc}?tail=200">Refresh</a>
            <a class="btn-secondary" href="/docker/view/{enc}?tail=200#container-logs">Top</a>
            <a class="btn-secondary" href="/docker/view/{enc}?tail=500#container-logs-bottom">Bottom</a>
          </div>
        </div>
        <pre class="code-block docker-log-view" style="max-height:420px;overflow:auto;margin:0;" id="container-logs-pre">{logs}</pre>
        <span id="container-logs-bottom"></span>
      </div>
      <div class="panel-card" style="margin-bottom:18px;" id="container-settings">
        <h2 style="margin:0 0 8px;font-size:18px;">Container Settings</h2>
        <p class="muted" style="margin:0 0 12px;">Read-only summary. Change restart policy or volumes by recreating the container or editing a CPN compose stack.</p>
        <p style="margin:0;"><strong>Restart:</strong> {restart} · <strong>Start on boot:</strong> {boot}</p>
      </div>
      <div class="panel-card" style="margin-bottom:18px;" id="container-processes">
        <h2 style="margin:0 0 10px;font-size:18px;">Container Processes</h2>
        <pre class="code-block" style="max-height:280px;overflow:auto;margin:0;">{processes}</pre>
      </div>
      <div class="panel-card" style="margin-bottom:18px;" id="container-exec">
        <h2 style="margin:0 0 10px;font-size:18px;">Run Command</h2>
        <p class="muted" style="margin:0 0 10px;">Runs inside the container via <code>sh -c</code> (admin only). Avoid shell metacharacters.</p>
        <form method="post" action="/docker/view/{enc}/exec" style="display:flex;flex-wrap:wrap;gap:8px;align-items:flex-end;">
          <label style="flex:1;min-width:220px;">Command
            <input type="text" name="command" maxlength="200" placeholder="ls -la /" style="width:100%;" autocomplete="off">
          </label>
          <button type="submit" class="btn-primary">Run</button>
        </form>
        {exec_block}
      </div>"##,
        toolbar = toolbar("containers"),
        name = html_escape(&row.name),
        status = html_escape(&status_label),
        id = html_escape(&detail.short_id),
        owner = owner_cell,
        cpu_card = stat_card("CPU usage", &detail.cpu_percent, "Processing power"),
        mem_pct_card = stat_card("Memory", &detail.mem_percent, &detail.mem_usage),
        mem_limit_card = stat_card("Memory limit", &detail.mem_limit, "Allocated memory"),
        image = html_escape(&row.image),
        tag = html_escape(&row.tag),
        ports = html_escape(&detail.port_mappings),
        restart = html_escape(&detail.restart_policy),
        boot = html_escape(&detail.start_on_boot),
        mem_usage = html_escape(&detail.mem_usage),
        mem_pct = html_escape(&detail.mem_percent),
        actions = action_grid(&detail),
        enc = urlencoding_simple(name),
        logs = html_escape(&logs),
        processes = html_escape(&processes),
        exec_block = exec_block,
    );

    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("Docker", Some("/docker")),
            (name, None),
        ],
        &format!("{} · Container", row.name),
        "Inspect stats, logs, and actions for this container.",
        &body,
        notice,
        error,
    )
}
