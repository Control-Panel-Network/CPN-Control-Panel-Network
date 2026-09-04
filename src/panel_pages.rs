//! HTML for the authenticated CPN Panel dashboard (served by the installer process).

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn gauge_svg(value: u8) -> String {
    let radius = 42.0_f64;
    let circumference = 2.0 * std::f64::consts::PI * radius;
    let offset = circumference * (1.0 - f64::from(value) / 100.0);
    format!(
        r#"<svg viewBox="0 0 100 100" aria-hidden="true">
          <circle class="gauge-track" cx="50" cy="50" r="{radius}"></circle>
          <circle class="gauge-value" cx="50" cy="50" r="{radius}"
            stroke-dasharray="{circumference}" stroke-dashoffset="{offset}"></circle>
        </svg>"#
    )
}

fn panel_styles() -> &'static str {
    r#"
:root {
  --canvas:#fff; --surface:#f5f5f7; --surface-soft:#fafafc; --ink:#1d1d1f;
  --muted:#6e6e73; --hairline:#e0e0e0; --blue:#0066cc; --blue-focus:#0071e3; --green:#18864b;
}
* { box-sizing:border-box; }
body { margin:0; background:var(--surface); color:var(--ink);
  font-family:"Segoe UI",system-ui,sans-serif; font-size:17px; line-height:1.47; }
a { color:inherit; text-decoration:none; }
.eyebrow { margin:0 0 8px; color:var(--blue); font-size:12px; font-weight:600; letter-spacing:.1em; }
.panel-layout { min-height:100vh; display:flex; background:var(--surface); }
.sidebar {
  position:sticky; top:0; width:260px; height:100vh; flex:0 0 260px;
  display:flex; flex-direction:column; justify-content:space-between;
  padding:28px 17px 20px; background:rgba(250,250,252,.92);
  border-right:1px solid var(--hairline);
}
.panel-brand { display:flex; align-items:center; gap:11px; padding:0 10px; font-size:17px; font-weight:600; }
.server-summary {
  display:flex; align-items:center; gap:12px; margin:28px 0 25px; padding:14px;
  border-radius:18px; background:var(--canvas); border:1px solid var(--hairline); color:var(--blue);
}
.server-summary div { display:flex; flex-direction:column; min-width:0; }
.server-summary strong { overflow:hidden; text-overflow:ellipsis; font-size:13px; }
.server-summary span { color:var(--muted); font-size:12px; }
.sidebar nav { display:grid; gap:4px; }
.sidebar nav a {
  display:flex; align-items:center; gap:12px; min-height:44px; padding:0 13px;
  border-radius:999px; color:#4b4b50; font-size:15px;
}
.sidebar nav a.active { background:#e7f1ff; color:var(--blue); font-weight:600; }
.sidebar-footer {
  display:flex; align-items:center; gap:6px; padding-top:18px; border-top:1px solid var(--hairline);
}
.logout { margin-left:auto; display:flex; align-items:center; gap:7px; color:var(--muted); font-size:13px; }
.panel-main { min-width:0; flex:1; padding:64px clamp(24px,5vw,72px) 80px; }
.mobile-header { display:none; }
.dashboard-heading {
  max-width:1200px; margin:0 auto 42px; display:flex; justify-content:space-between;
  align-items:flex-end; gap:32px;
}
.dashboard-heading h1 {
  margin:0; font-size:clamp(36px,5vw,56px); line-height:1.07; letter-spacing:-.045em; font-weight:600;
}
.dashboard-heading > div > p:last-child { margin:14px 0 0; color:var(--muted); max-width:600px; }
.resource-grid {
  max-width:1200px; margin:0 auto; display:grid; grid-template-columns:repeat(3,minmax(0,1fr)); gap:22px;
}
.resource-card {
  min-height:240px; display:flex; flex-direction:column; align-items:center; justify-content:center;
  padding:24px; background:var(--canvas); border:1px solid var(--hairline); border-radius:18px;
}
.resource-card h2 { margin:0 0 22px; font-size:16px; font-weight:600; }
.gauge { position:relative; width:112px; height:112px; }
.gauge svg { width:100%; height:100%; transform:rotate(-90deg); overflow:visible; }
.gauge circle { fill:none; stroke-width:9; }
.gauge-track { stroke:#e5e5e7; }
.gauge-value { stroke:var(--blue); stroke-linecap:round; }
.gauge-copy {
  position:absolute; inset:0; display:flex; flex-direction:column; align-items:center; justify-content:center;
}
.gauge-copy strong { font-size:21px; line-height:1.05; }
.gauge-copy span { margin-top:3px; color:var(--muted); font-size:11px; }
.dashboard-lower-grid {
  max-width:1200px; margin:22px auto 0; display:grid; grid-template-columns:1.15fr .85fr; gap:22px;
}
.status-card, .activity-card {
  padding:28px; border-radius:18px; background:var(--canvas); border:1px solid var(--hairline);
}
.status-card-heading { display:flex; align-items:flex-start; justify-content:space-between; color:var(--green); }
.status-card h2, .activity-card h2 {
  margin:0; color:var(--ink); font-size:23px; letter-spacing:-.025em;
}
.status-card ul { list-style:none; padding:0; margin:25px 0 0; }
.status-card li, .activity-card > div {
  display:flex; justify-content:space-between; align-items:center; padding:12px 0;
  border-top:1px solid #eeeef0; font-size:14px;
}
.status-card li strong { color:var(--green); font-size:12px; }
.activity-card time { color:var(--muted); font-size:12px; }
@media (max-width:980px) {
  .sidebar { display:none; }
  .panel-main { padding:0 24px 64px; }
  .mobile-header {
    height:58px; margin:0 -24px 42px; padding:0 20px; display:flex; align-items:center;
    justify-content:space-between; position:sticky; top:0; z-index:10;
    background:rgba(250,250,252,.86); border-bottom:1px solid var(--hairline);
  }
  .resource-grid { grid-template-columns:repeat(2,1fr); }
}
@media (max-width:680px) {
  .dashboard-heading { flex-direction:column; align-items:stretch; }
  .resource-grid, .dashboard-lower-grid { grid-template-columns:1fr; }
}
"#
}

pub fn panel_dashboard_html(username: &str) -> String {
    let user = html_escape(username);
    let cpu = gauge_svg(45);
    let ram = gauge_svg(72);
    let disk = gauge_svg(28);
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Dashboard · CPN Panel</title>
  <style>{styles}</style>
</head>
<body>
  <main class="panel-layout" data-page="dashboard">
    <aside class="sidebar">
      <div>
        <a class="panel-brand" href="/dashboard">CPN Panel</a>
        <div class="server-summary">
          <div>
            <strong>{user}</strong>
            <span>Signed in</span>
          </div>
        </div>
        <nav aria-label="Primary navigation">
          <a class="active" href="/dashboard">Dashboard</a>
          <a href="/dashboard">Websites</a>
          <a href="/dashboard">Email</a>
          <a href="/dashboard">Databases</a>
          <a href="/dashboard">Backups</a>
        </nav>
      </div>
      <div class="sidebar-footer">
        <a class="logout" href="/logout">Log out</a>
      </div>
    </aside>
    <section class="panel-main">
      <header class="mobile-header">
        <strong>CPN Panel</strong>
        <a href="/logout">Log out</a>
      </header>
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
          <div class="gauge" role="img" aria-label="CPU Usage: 45%">
            {cpu}
            <div class="gauge-copy"><strong>45%</strong><span>4 cores</span></div>
          </div>
        </article>
        <article class="resource-card">
          <h2>RAM Usage</h2>
          <div class="gauge" role="img" aria-label="RAM Usage: 72%">
            {ram}
            <div class="gauge-copy"><strong>72%</strong><span>11.5 / 16 GB</span></div>
          </div>
        </article>
        <article class="resource-card">
          <h2>Disk Usage</h2>
          <div class="gauge" role="img" aria-label="Disk Usage: 28%">
            {disk}
            <div class="gauge-copy"><strong>28%</strong><span>140 / 500 GB</span></div>
          </div>
        </article>
      </div>
      <div class="dashboard-lower-grid">
        <article class="status-card">
          <div class="status-card-heading">
            <div>
              <p class="eyebrow">SYSTEM HEALTH</p>
              <h2>All services operational</h2>
            </div>
          </div>
          <ul>
            <li><span>Nginx</span><strong>Running</strong></li>
            <li><span>MariaDB</span><strong>Running</strong></li>
            <li><span>Mail service</span><strong>Running</strong></li>
          </ul>
        </article>
        <article class="activity-card">
          <p class="eyebrow">RECENT ACTIVITY</p>
          <h2>Latest changes</h2>
          <div><span>SSL certificate renewed</span><time>12 min ago</time></div>
          <div><span>Automated backup completed</span><time>2 hr ago</time></div>
          <div><span>System packages updated</span><time>Yesterday</time></div>
        </article>
      </div>
    </section>
  </main>
</body>
</html>"#,
        styles = panel_styles(),
        user = user,
        cpu = cpu,
        ram = ram,
        disk = disk,
    )
}
