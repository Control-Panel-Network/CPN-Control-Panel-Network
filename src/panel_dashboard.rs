//! Dashboard landing HTML for the authenticated CPN Panel.

use crate::panel_pages::panel_shell;

fn host_gauges() -> [(Option<u8>, String); 3] {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let cpu = std::fs::read_to_string("/proc/stat").ok().and_then(|raw| {
        let values: Vec<u64> = raw
            .lines()
            .next()?
            .split_whitespace()
            .skip(1)
            .take(8)
            .map(str::parse)
            .collect::<Result<_, _>>()
            .ok()?;
        let total: u64 = values.iter().sum();
        let idle = values.get(3)? + values.get(4)?;
        (total > 0).then(|| (100 * total.saturating_sub(idle) / total) as u8)
    });
    let memory = std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|raw| {
            let value = |key: &str| {
                raw.lines().find_map(|line| {
                    line.strip_prefix(key)?
                        .split_whitespace()
                        .next()?
                        .parse::<u64>()
                        .ok()
                })
            };
            let total = value("MemTotal:")?;
            let used = total.saturating_sub(value("MemAvailable:")?);
            (total > 0).then(|| {
                (
                    (used * 100 / total) as u8,
                    format!(
                        "{:.1}/{:.1} GB",
                        used as f64 / 1048576.0,
                        total as f64 / 1048576.0
                    ),
                )
            })
        });
    let disk = std::process::Command::new("df")
        .args(["-Pk", "/"])
        .env("LC_ALL", "C")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|out| {
            let raw = String::from_utf8(out.stdout).ok()?;
            let fields: Vec<_> = raw.lines().nth(1)?.split_whitespace().collect();
            let total = fields.get(1)?.parse::<f64>().ok()?;
            let used = fields.get(2)?.parse::<f64>().ok()?;
            let percent = fields
                .get(4)?
                .trim_end_matches('%')
                .parse::<u8>()
                .ok()?
                .min(100);
            Some((
                percent,
                format!("{:.0}/{:.0} GB", used / 1048576.0, total / 1048576.0),
            ))
        });
    [
        (cpu, format!("{cores} cores")),
        memory
            .map(|(v, d)| (Some(v), d))
            .unwrap_or((None, "Unavailable".into())),
        disk.map(|(v, d)| (Some(v), d))
            .unwrap_or((None, "Unavailable".into())),
    ]
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn gauge_stroke_for_usage(_percent: u8) -> &'static str {
    "#0066cc"
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
    let [
        (cpu_value, cpu_detail),
        (ram_value, ram_detail),
        (disk_value, disk_detail),
    ] = host_gauges();
    let label = |v: Option<u8>| v.map(|n| n.to_string()).unwrap_or_else(|| "—".into());
    let cpu_pct = label(cpu_value);
    let ram_pct = label(ram_value);
    let disk_pct = label(disk_value);
    let cpu = gauge_svg(cpu_value.unwrap_or(0));
    let ram = gauge_svg(ram_value.unwrap_or(0));
    let disk = gauge_svg(disk_value.unwrap_or(0));

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
          <h1>Dashboard</h1>
          <p>Signed in as {user}.</p>
        </div>
      </div>
      <div class="resource-grid">
        <article class="resource-card">
          <h2 title="Average CPU usage since boot">CPU Usage</h2>
          <div class="gauge" role="img" aria-label="CPU Usage: {cpu_pct}%">
            {cpu}
            <div class="gauge-copy"><strong>{cpu_pct}%</strong><span>{cpu_detail}</span></div>
          </div>
        </article>
        <article class="resource-card">
          <h2>RAM Usage</h2>
          <div class="gauge" role="img" aria-label="RAM Usage: {ram_pct}%">
            {ram}
            <div class="gauge-copy"><strong>{ram_pct}%</strong><span>{ram_detail}</span></div>
          </div>
        </article>
        <article class="resource-card">
          <h2>Disk Usage</h2>
          <div class="gauge" role="img" aria-label="Disk Usage: {disk_pct}%">
            {disk}
            <div class="gauge-copy"><strong>{disk_pct}%</strong><span>{disk_detail}</span></div>
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
