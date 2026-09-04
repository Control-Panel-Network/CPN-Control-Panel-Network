//! HTML for authenticated CPN Panel pages (served by the installer process).

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
  --sidebar-width:260px;
}
* { box-sizing:border-box; }
html, body { margin:0; max-width:100%; overflow-x:hidden; }
body { background:var(--surface); color:var(--ink);
  font-family:"Segoe UI",system-ui,sans-serif; font-size:17px; line-height:1.47; }
a { color:inherit; text-decoration:none; }
button { font:inherit; cursor:pointer; }
.eyebrow { margin:0 0 8px; color:var(--blue); font-size:12px; font-weight:600; letter-spacing:.1em; }
.panel-layout { min-height:100vh; display:flex; background:var(--surface); position:relative; }
.sidebar-backdrop {
  display:none; position:fixed; inset:0; z-index:40; border:0; padding:0;
  background:rgba(29,29,31,.42); cursor:pointer;
}
.sidebar {
  position:sticky; top:0; width:var(--sidebar-width); height:100vh; flex:0 0 var(--sidebar-width);
  display:flex; flex-direction:column; justify-content:space-between;
  padding:28px 17px 20px; background:rgba(250,250,252,.96);
  border-right:1px solid var(--hairline); z-index:50;
}
.panel-brand { display:flex; align-items:center; gap:11px; min-height:44px; padding:0 10px; font-size:17px; font-weight:600; }
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
.logout {
  margin-left:auto; display:inline-flex; align-items:center; justify-content:center;
  min-height:44px; min-width:44px; padding:0 12px; color:var(--muted); font-size:13px;
}
.panel-main { min-width:0; flex:1; padding:64px clamp(20px,5vw,72px) 80px; }
.mobile-header { display:none; }
.icon-btn {
  width:44px; height:44px; display:inline-grid; place-items:center;
  border:0; border-radius:12px; background:transparent; color:var(--ink);
}
.icon-btn:focus-visible, .sidebar nav a:focus-visible, .logout:focus-visible {
  outline:2px solid var(--blue-focus); outline-offset:2px;
}
.hamburger-bars {
  width:18px; height:14px; display:flex; flex-direction:column; justify-content:space-between;
}
.hamburger-bars span { display:block; height:2px; border-radius:2px; background:currentColor; }
.dashboard-heading {
  max-width:1200px; margin:0 auto 42px; display:flex; justify-content:space-between;
  align-items:flex-end; gap:32px;
}
.dashboard-heading h1 {
  margin:0; font-size:clamp(32px,5vw,56px); line-height:1.07; letter-spacing:-.045em; font-weight:600;
}
.dashboard-heading > div > p:last-child { margin:14px 0 0; color:var(--muted); max-width:600px; }
.resource-grid {
  max-width:1200px; margin:0 auto; display:grid; grid-template-columns:repeat(3,minmax(0,1fr)); gap:22px;
}
.resource-card {
  min-height:220px; display:flex; flex-direction:column; align-items:center; justify-content:center;
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
.status-card, .activity-card, .section-card {
  padding:28px; border-radius:18px; background:var(--canvas); border:1px solid var(--hairline);
}
.status-card-heading { display:flex; align-items:flex-start; justify-content:space-between; color:var(--green); }
.status-card h2, .activity-card h2, .section-card h2 {
  margin:0; color:var(--ink); font-size:23px; letter-spacing:-.025em;
}
.status-card ul { list-style:none; padding:0; margin:25px 0 0; }
.status-card li, .activity-card > div {
  display:flex; justify-content:space-between; align-items:center; padding:12px 0;
  border-top:1px solid #eeeef0; font-size:14px;
}
.status-card li strong { color:var(--green); font-size:12px; }
.activity-card time { color:var(--muted); font-size:12px; }
.section-card p { margin:12px 0 0; color:var(--muted); max-width:62ch; }
.panel-notice { margin:0 0 16px; padding:10px 12px; border-radius:10px; font-size:.92rem; }
.panel-notice.ok { background:#ecfdf3; border:1px solid #abefc6; color:#067647; }
.panel-notice.error { background:#fef2f2; border:1px solid #fecaca; color:#b91c1c; }
.empty-state { margin:0; color:var(--muted); }
.muted { color:var(--muted); font-size:.9rem; }
.table-wrap { overflow-x:auto; margin-top:14px; }
.data-table { width:100%; border-collapse:collapse; font-size:14px; }
.data-table th, .data-table td { text-align:left; padding:12px 10px; border-top:1px solid #eeeef0; vertical-align:top; }
.data-table th { color:var(--muted); font-weight:600; border-top:0; }
.kv-list { list-style:none; padding:0; margin:18px 0 0; }
.kv-list li { display:flex; justify-content:space-between; gap:16px; padding:12px 0; border-top:1px solid #eeeef0; font-size:14px; }
.stack-form { display:grid; gap:10px; margin-top:16px; max-width:420px; }
.stack-form label { font-weight:600; font-size:.92rem; }
.stack-form input {
  width:100%; box-sizing:border-box; border:1px solid #d0d5dd; border-radius:10px; padding:11px 12px; font:inherit;
}
.btn-primary, .btn-danger {
  display:inline-flex; align-items:center; justify-content:center; min-height:44px; padding:0 16px;
  border:0; border-radius:999px; font-weight:700; cursor:pointer; text-decoration:none;
}
.btn-primary { background:var(--blue); color:#fff; }
.btn-danger { background:#fee4e2; color:#b42318; }
.inline-form { display:inline; margin:0; }
code { font-size:.9em; }
@media (max-width:1023.98px) {
  body.nav-open { overflow:hidden; }
  .sidebar-backdrop { display:none; }
  body.nav-open .sidebar-backdrop { display:block; }
  .sidebar {
    position:fixed; left:0; top:0; height:100%; transform:translateX(-105%);
    transition:transform 180ms ease; box-shadow:none; flex:none;
  }
  body.nav-open .sidebar { transform:translateX(0); box-shadow:12px 0 32px rgba(0,0,0,.12); }
  .panel-main { padding:0 20px 64px; width:100%; }
  .mobile-header {
    height:58px; margin:0 -20px 28px; padding:0 12px 0 8px; display:flex; align-items:center;
    justify-content:space-between; gap:12px; position:sticky; top:0; z-index:30;
    background:rgba(250,250,252,.94); border-bottom:1px solid var(--hairline);
  }
  .mobile-header strong { flex:1; font-size:16px; }
  .resource-grid { grid-template-columns:repeat(2,minmax(0,1fr)); }
  .dashboard-lower-grid { grid-template-columns:1fr; }
}
@media (max-width:679.98px) {
  .dashboard-heading { flex-direction:column; align-items:stretch; }
  .resource-grid { grid-template-columns:1fr; }
  .resource-card { min-height:200px; }
  .dashboard-heading h1 { font-size:clamp(28px,9vw,40px); }
}
@media (prefers-reduced-motion:reduce) {
  .sidebar { transition:none; }
}
"#
}

fn panel_nav_script() -> &'static str {
    r#"
<script>
(function () {
  var body = document.body;
  var toggle = document.getElementById('nav-toggle');
  var backdrop = document.getElementById('nav-backdrop');
  var sidebar = document.getElementById('panel-sidebar');
  if (!toggle || !sidebar) return;

  function setOpen(open) {
    body.classList.toggle('nav-open', open);
    toggle.setAttribute('aria-expanded', open ? 'true' : 'false');
    sidebar.setAttribute('aria-hidden', open ? 'false' : String(window.matchMedia('(max-width: 1023.98px)').matches));
    if (open) {
      var first = sidebar.querySelector('a, button');
      if (first) first.focus();
    } else {
      toggle.focus();
    }
  }

  function syncDesktop() {
    if (!window.matchMedia('(max-width: 1023.98px)').matches) {
      body.classList.remove('nav-open');
      toggle.setAttribute('aria-expanded', 'false');
      sidebar.setAttribute('aria-hidden', 'false');
    } else if (!body.classList.contains('nav-open')) {
      sidebar.setAttribute('aria-hidden', 'true');
    }
  }

  toggle.addEventListener('click', function () {
    setOpen(!body.classList.contains('nav-open'));
  });
  if (backdrop) {
    backdrop.addEventListener('click', function () { setOpen(false); });
  }
  document.addEventListener('keydown', function (event) {
    if (event.key === 'Escape' && body.classList.contains('nav-open')) {
      setOpen(false);
    }
  });
  window.addEventListener('resize', syncDesktop);
  syncDesktop();
})();
</script>
"#
}

fn nav_links(active: &str) -> String {
    let items = [
        ("dashboard", "/dashboard", "Dashboard"),
        ("websites", "/websites", "Websites"),
        ("email", "/email", "Email"),
        ("databases", "/databases", "Databases"),
        ("backups", "/backups", "Backups"),
        ("plugins", "/plugins", "Plugins"),
    ];
    items
        .iter()
        .map(|(id, href, label)| {
            let class = if *id == active {
                " class=\"active\""
            } else {
                ""
            };
            format!(r#"<a{class} href="{href}">{label}</a>"#)
        })
        .collect::<Vec<_>>()
        .join("\n          ")
}

pub fn panel_shell(username: &str, active: &str, title: &str, main: &str) -> String {
    let user = html_escape(username);
    let nav = nav_links(active);
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title} · CPN Panel</title>
  <style>{styles}</style>
</head>
<body>
  <button type="button" id="nav-backdrop" class="sidebar-backdrop" aria-label="Close navigation" tabindex="-1"></button>
  <div class="panel-layout" data-page="{active}">
    <aside id="panel-sidebar" class="sidebar" aria-label="Panel navigation">
      <div>
        <a class="panel-brand" href="/dashboard">CPN Panel</a>
        <div class="server-summary">
          <div>
            <strong>{user}</strong>
            <span>Signed in</span>
          </div>
        </div>
        <nav aria-label="Primary navigation">
          {nav}
        </nav>
      </div>
      <div class="sidebar-footer">
        <a class="logout" href="/logout">Log out</a>
      </div>
    </aside>
    <section class="panel-main">
      <header class="mobile-header">
        <button type="button" id="nav-toggle" class="icon-btn" aria-controls="panel-sidebar" aria-expanded="false" aria-label="Open navigation">
          <span class="hamburger-bars" aria-hidden="true"><span></span><span></span><span></span></span>
        </button>
        <strong>CPN Panel</strong>
        <a class="logout" href="/logout">Log out</a>
      </header>
      {main}
    </section>
  </div>
  {script}
</body>
</html>"#,
        title = html_escape(title),
        styles = panel_styles(),
        active = html_escape(active),
        user = user,
        nav = nav,
        main = main,
        script = panel_nav_script(),
    )
}

pub fn panel_dashboard_html(username: &str) -> String {
    let user = html_escape(username);
    let cpu = gauge_svg(45);
    let ram = gauge_svg(72);
    let disk = gauge_svg(28);
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
      </div>"#
    );
    panel_shell(username, "dashboard", "Dashboard", &main)
}
