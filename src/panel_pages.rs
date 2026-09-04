//! HTML for authenticated CPN Panel pages (served by the installer process).

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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
.sidebar nav a.nav-child { padding-left:22px; font-size:14px; min-height:40px; }
.nav-section {
  margin:14px 10px 6px; color:var(--muted); font-size:11px; font-weight:700;
  letter-spacing:.08em; text-transform:uppercase;
}
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
.gauge-value { stroke-linecap:round; }
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
.status-card li strong { font-size:12px; }
.status-card li strong.ok { color:var(--green); }
.status-card li strong.warn { color:#b54708; }
.status-card li strong.bad { color:#b42318; }
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
.badge-ok { display:inline-block; padding:2px 8px; border-radius:999px; background:#ecfdf3; color:#027a48; font-size:12px; font-weight:700; }
.status-dot { display:inline-block; width:8px; height:8px; border-radius:50%; margin-right:6px; vertical-align:middle; }
.status-dot.ok { background:#12b76a; }
.status-dot.off { background:#98a2b3; }
.panel-card { max-width:1200px; margin:0 auto; padding:22px; background:var(--canvas); border:1px solid var(--hairline); border-radius:18px; }
.stack-form label { display:grid; gap:6px; font-size:14px; font-weight:600; color:#344054; }
.stack-form input, .stack-form select, .stack-form textarea {
  font:inherit; padding:10px 12px; border:1px solid var(--hairline); border-radius:10px; background:#fff;
}
.btn-primary {
  display:inline-flex; align-items:center; justify-content:center; min-height:40px; padding:0 16px;
  border-radius:999px; background:var(--blue); color:#fff; font-weight:700; border:0;
}
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

fn nav_link(id: &str, href: &str, label: &str, active: &str, child: bool) -> String {
    let mut class = String::new();
    if id == active {
        class.push_str("active");
    }
    if child {
        if !class.is_empty() {
            class.push(' ');
        }
        class.push_str("nav-child");
    }
    let class_attr = if class.is_empty() {
        String::new()
    } else {
        format!(r#" class="{class}""#)
    };
    format!(r#"<a{class_attr} href="{href}">{label}</a>"#)
}

fn nav_links(active: &str, username: &str) -> String {
    let hosting = [
        ("dashboard", "/dashboard", "Dashboard"),
        ("websites", "/websites", "Websites"),
        ("email", "/email", "Email"),
        ("databases", "/databases", "Databases & FTP"),
        ("backups", "/backups", "Backups"),
        ("apps", "/apps", "Apps"),
    ];
    let mut parts = Vec::new();
    parts.push(r#"<div class="nav-section">Hosting</div>"#.to_string());
    for (id, href, label) in hosting {
        parts.push(nav_link(id, href, label, active, false));
    }
    parts.push(r#"<div class="nav-section">Account</div>"#.to_string());
    parts.push(nav_link(
        "users",
        "/account/users",
        "Users & Plans",
        active,
        false,
    ));
    parts.push(nav_link("packages", "/packages", "Packages", active, false));
    parts.push(r#"<div class="nav-section">Administration</div>"#.to_string());
    for (id, href, label) in [
        ("server", "/server", "Server"),
        ("security", "/security", "Security"),
        ("settings", "/settings", "Settings"),
    ] {
        parts.push(nav_link(id, href, label, active, false));
    }
    parts.push(r#"<div class="nav-section">Plugins</div>"#.to_string());
    parts.push(nav_link(
        "plugins",
        "/plugins",
        "Installed / Store",
        active,
        false,
    ));
    let plugin_links = crate::plugins_settings::sidebar_plugin_links(username);
    let mut domains: Vec<&str> = plugin_links.iter().map(|l| l.domain.as_str()).collect();
    domains.sort_unstable();
    domains.dedup();
    let need_domain_hint = domains.len() > 1;
    for link in &plugin_links {
        let label = if need_domain_hint {
            format!("{} ({})", link.name, link.domain)
        } else {
            link.name.clone()
        };
        let id = format!("plugin-{}-{}", link.domain, link.id);
        parts.push(nav_link(
            &id,
            &link.href,
            &html_escape(&label),
            active,
            true,
        ));
    }
    parts.join("\n          ")
}

pub fn panel_shell(username: &str, active: &str, title: &str, main: &str) -> String {
    let user = html_escape(username);
    let nav = nav_links(active, username);
    let color_mode = crate::panel_theme::load_user_color_mode(username);
    let design = crate::panel_theme::load_panel_design();
    let styles = format!(
        "{}{}{}{}",
        panel_styles(),
        crate::panel_hubs::hub_styles(),
        crate::panel_theme::color_mode_styles(),
        crate::panel_theme::design_css_vars(&design),
    );
    let boot = crate::panel_theme_chrome::color_mode_boot_script(color_mode);
    let toggle = crate::panel_theme_chrome::sidebar_theme_toggle(color_mode);
    let script = format!(
        "{}{}",
        panel_nav_script(),
        crate::panel_theme_chrome::color_mode_toggle_script()
    );
    format!(
        r#"<!DOCTYPE html>
<html lang="en" data-color-mode="{mode}" data-design-preset="{preset}">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title} · CPN Panel</title>
  <style>{styles}</style>
  {boot}
</head>
<body data-color-mode="{mode}">
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
        {toggle}
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
        styles = styles,
        boot = boot,
        mode = color_mode.as_str(),
        preset = design.preset.as_str(),
        active = html_escape(active),
        user = user,
        nav = nav,
        toggle = toggle,
        main = main,
        script = script,
    )
}
