//! Panel Apps page HTML (session-gated via routes).

use crate::apps::{AppId, AppStateKind, AppStatus, list_apps};

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

fn action_buttons(status: &AppStatus) -> String {
    let name = status.id.as_str();
    let label = status.id.label();
    match status.state {
        AppStateKind::NotInstalled => format!(
            r#"<form method="post" action="/apps/install" class="inline-form" onsubmit="return confirm('Install {label} on this host now?');">
              <input type="hidden" name="name" value="{name}">
              <button type="submit" class="btn-primary">Install</button>
            </form>"#,
            label = html_escape(label),
            name = html_escape(name),
        ),
        AppStateKind::Installed | AppStateKind::Running => format!(
            r#"<form method="post" action="/apps/reinstall" class="inline-form" onsubmit="return confirm('Reinstall {label}? Services may restart.');">
              <input type="hidden" name="name" value="{name}">
              <button type="submit" class="btn-secondary" style="min-height:44px;padding:0 14px;border:0;border-radius:999px;background:#f2f4f7;color:#344054;font-weight:700;cursor:pointer;">Reinstall</button>
            </form>
            <form method="post" action="/apps/uninstall" class="inline-form" onsubmit="return confirm('Uninstall {label}? This removes packages and stops services.');">
              <input type="hidden" name="name" value="{name}">
              <button type="submit" class="btn-danger">Uninstall</button>
            </form>"#,
            label = html_escape(label),
            name = html_escape(name),
        ),
    }
}

fn app_card(status: &AppStatus) -> String {
    let warn = status
        .warning
        .as_ref()
        .map(|w| {
            format!(
                r#"<p class="panel-notice error" style="margin-top:12px;">{msg}</p>"#,
                msg = html_escape(w)
            )
        })
        .unwrap_or_default();
    let xor_note = match status.id {
        AppId::Mariadb | AppId::Mysql => {
            r#"<p class="muted" style="margin-top:8px;">Hosts typically run MariaDB XOR MySQL. CPN refuses installing one while the other is present.</p>"#
        }
        _ => "",
    };
    format!(
        r#"<article class="section-card" style="margin-top:18px;">
        <h2>{label}</h2>
        <ul class="kv-list">
          <li><span>Status</span><strong>{state}</strong></li>
          <li><span>Id</span><strong><code>{id}</code></strong></li>
        </ul>
        <p>{detail}</p>
        {warn}
        {xor}
        <div style="display:flex;flex-wrap:wrap;gap:10px;margin-top:16px;">
          {actions}
        </div>
      </article>"#,
        label = html_escape(status.id.label()),
        state = html_escape(status.state.label()),
        id = html_escape(status.id.as_str()),
        detail = html_escape(&status.detail),
        warn = warn,
        xor = xor_note,
        actions = action_buttons(status),
    )
}

pub fn apps_main(notice: Option<&str>, error: Option<&str>) -> String {
    let apps = list_apps();
    let cards: String = apps.iter().map(app_card).collect();
    format!(
        r#"{heading}
      {ok}
      {err}
      <article class="section-card">
        <h2>Host applications</h2>
        <p>Install, reinstall, or uninstall common server apps with the host package manager (dnf or apt). Actions require panel host privileges. Progress and results show as notices on this page.</p>
        <p class="muted">CLI: <code>cpn app list|install|reinstall|uninstall --name mariadb|mysql|phpmyadmin|email|rabbitmq</code></p>
      </article>
      {cards}"#,
        heading = section_heading(
            "Apps",
            "Manage MariaDB, MySQL, phpMyAdmin, Email, and RabbitMQ on this host.",
        ),
        ok = notice_block("ok", notice),
        err = notice_block("error", error),
        cards = cards,
    )
}
