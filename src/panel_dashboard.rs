//! Dashboard landing HTML for the authenticated CPN Panel.

use crate::panel_pages::panel_shell;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Traffic-light stroke for resource gauges by usage percent.
/// Thresholds: green < 60%, orange 60-84%, red >= 85%.
fn gauge_stroke_for_usage(percent: u8) -> &'static str {
    if percent >= 85 {
        "#d92d20"
    } else if percent >= 60 {
        "#f79009"
    } else {
        "#12b76a"
    }
}

fn gauge_svg(value: u8) -> String {
    let radius = 42.0_f64;
    let circumference = 2.0 * std::f64::consts::PI * radius;
    let offset = circumference * (1.0 - f64::from(value) / 100.0);
    let stroke = gauge_stroke_for_usage(value);
    format!(
        r#"<svg viewBox="0 0 100 100" aria-hidden="true">
          <circle class="gauge-track" cx="50" cy="50" r="{radius}"></circle>
          <circle class="gauge-value" cx="50" cy="50" r="{radius}"
            stroke="{stroke}"
            stroke-dasharray="{circumference}" stroke-dashoffset="{offset}"></circle>
        </svg>"#
    )
}

pub fn panel_dashboard_html(username: &str) -> String {
    let user = html_escape(username);
    // Placeholder percents until host metrics are wired; colors still follow thresholds.
    let cpu_pct: u8 = 45;
    let ram_pct: u8 = 72;
    let disk_pct: u8 = 28;
    let cpu = gauge_svg(cpu_pct);
    let ram = gauge_svg(ram_pct);
    let disk = gauge_svg(disk_pct);

    let db = crate::service_detect::detect_database();
    let db_label = crate::service_detect::database_health_label(&db);
    let web_label = crate::service_detect::detect_web_server_label();
    let mail_label = crate::service_detect::detect_mail_service_label();
    let health_heading =
        if db_label == "Running" && web_label == "Running" && mail_label == "Running" {
            "All services operational"
        } else if db_label == "Not detected"
            && web_label == "Not detected"
            && mail_label == "Not detected"
        {
            "No managed services detected yet"
        } else {
            "Service status (live detection)"
        };
    let status_class =
        |label: &str| -> &'static str { if label == "Running" { "ok" } else { "warn" } };

    let main = format!(
        r#"
      <div class="dashboard-heading">
        <div>
          <p class="eyebrow">SERVER OVERVIEW</p>
          <h1>Dashboard</h1>
          <p>Signed in as {user}.</p>
        </div>
      </div>
      <div class="resource-grid">
        <article class="resource-card">
          <h2>CPU Usage</h2>
          <div class="gauge" role="img" aria-label="CPU Usage: {cpu_pct}%">
            {cpu}
            <div class="gauge-copy"><strong>{cpu_pct}%</strong><span>4 cores</span></div>
          </div>
        </article>
        <article class="resource-card">
          <h2>RAM Usage</h2>
          <div class="gauge" role="img" aria-label="RAM Usage: {ram_pct}%">
            {ram}
            <div class="gauge-copy"><strong>{ram_pct}%</strong><span>11.5 / 16 GB</span></div>
          </div>
        </article>
        <article class="resource-card">
          <h2>Disk Usage</h2>
          <div class="gauge" role="img" aria-label="Disk Usage: {disk_pct}%">
            {disk}
            <div class="gauge-copy"><strong>{disk_pct}%</strong><span>140 / 500 GB</span></div>
          </div>
        </article>
      </div>
      <div class="dashboard-lower-grid">
        <article class="status-card">
          <div class="status-card-heading">
            <div>
              <p class="eyebrow">SYSTEM HEALTH</p>
              <h2>{health_heading}</h2>
            </div>
          </div>
          <ul>
            <li><span>Web server</span><strong class="{web_cls}">{web}</strong></li>
            <li><span>MariaDB</span><strong class="{db_cls}">{db}</strong></li>
            <li><span>Mail service</span><strong class="{mail_cls}">{mail}</strong></li>
          </ul>
          <p class="muted" style="margin-top:14px;">Health uses the same live detection as the Databases and Email pages (no placeholder Running states).</p>
        </article>
        <article class="activity-card">
          <p class="eyebrow">RECENT ACTIVITY</p>
          <h2>Latest changes</h2>
          <p class="empty-state">No recent panel activity to show yet.</p>
        </article>
      </div>"#,
        user = user,
        cpu_pct = cpu_pct,
        ram_pct = ram_pct,
        disk_pct = disk_pct,
        cpu = cpu,
        ram = ram,
        disk = disk,
        health_heading = html_escape(health_heading),
        web = html_escape(&web_label),
        db = html_escape(&db_label),
        mail = html_escape(&mail_label),
        web_cls = status_class(&web_label),
        db_cls = status_class(&db_label),
        mail_cls = status_class(&mail_label),
    );
    panel_shell(username, "dashboard", "Dashboard", &main)
}
