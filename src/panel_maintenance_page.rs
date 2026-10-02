//! Fancy dark maintenance HTML for panel upgrades (CPN branding only).

use crate::panel_brand::brand_favicon_links;
use crate::panel_maintenance_mode::{self, MaintenanceFlag};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn witty_line(phase: &str) -> &'static str {
    match phase {
        "downloading" => "Fetching the new bits. Coffee optional, patience required.",
        "installing" => "Swapping packages carefully. Sites and data stay put.",
        "testing" | "verifying" => "Running post-update checks before opening the doors.",
        "restarting" => "Restarting the panel process. This usually takes a few seconds.",
        "failed" => "The update stopped safely. Staff can clear maintenance and try again.",
        "completed" => "Almost there. Waiting for the panel to come back online.",
        _ => "Hang tight. CPN is applying an update and will return shortly.",
    }
}

/// Full-page maintenance card (X4T-inspired layout, CPN copy and colors).
pub fn render_html(flag: &MaintenanceFlag) -> String {
    let title = if flag.title.trim().is_empty() {
        "Updating CPN Panel".to_string()
    } else {
        flag.title.clone()
    };
    let message = if flag.message.trim().is_empty() {
        "The control panel is temporarily offline for an update.".to_string()
    } else {
        flag.message.clone()
    };
    let progress = flag.progress.min(100);
    let phase = html_escape(&flag.phase);
    let target = flag
        .target
        .as_deref()
        .filter(|t| !t.trim().is_empty())
        .map(|t| format!("Target: {}", html_escape(t)))
        .unwrap_or_default();
    let started = panel_maintenance_mode::format_nb_datetime(flag.started_at_unix);
    let updated = panel_maintenance_mode::format_nb_datetime(flag.updated_at_unix);
    let witty = witty_line(&flag.phase);
    let favicons = brand_favicon_links();
    let bypass = html_escape(&flag.bypass_token);
    let title_e = html_escape(&title);
    let message_e = html_escape(&message);
    let started_e = html_escape(&started);
    let updated_e = html_escape(&updated);

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta http-equiv="Cache-Control" content="no-store">
  <title>{title_e} · CPN</title>
  {favicons}
  <style>
    :root {{
      --bg: #0a0b10;
      --card: #10131c;
      --text: #f4f6fb;
      --muted: #9aa3b5;
      --accent: #ffcc33;
      --brand: #5865f2;
      --brand-2: #006cfa;
      --ok: #34d399;
      --border: rgba(255,204,51,0.35);
    }}
    * {{ box-sizing: border-box; }}
    body {{
      margin: 0; min-height: 100vh; color: var(--text);
      font-family: "Segoe UI", "Trebuchet MS", sans-serif;
      background:
        radial-gradient(1200px 600px at 10% -10%, rgba(88,101,242,0.18), transparent 55%),
        radial-gradient(900px 500px at 90% 0%, rgba(0,108,250,0.14), transparent 50%),
        var(--bg);
      display: grid; grid-template-rows: auto 1fr auto;
    }}
    .rail {{
      display: flex; align-items: center; gap: 12px;
      padding: 16px 18px; border-bottom: 1px solid rgba(255,255,255,0.06);
    }}
    .rail img {{ height: 36px; width: auto; }}
    .rail .staff {{
      margin-left: auto; appearance: none; border: 0; cursor: pointer;
      background: var(--brand); color: #fff; font-weight: 700;
      border-radius: 10px; padding: 10px 14px; font-size: 0.9rem;
      text-decoration: none;
    }}
    .wrap {{ display: grid; place-items: center; padding: 28px 16px 40px; }}
    .card {{
      width: min(520px, 100%);
      border-radius: 18px; padding: 1px;
      background: linear-gradient(135deg, rgba(255,180,40,0.75), rgba(0,108,250,0.55), rgba(88,101,242,0.7));
      box-shadow: 0 24px 60px rgba(0,0,0,0.45);
    }}
    .inner {{
      border-radius: 17px; background: linear-gradient(180deg, #141824 0%, var(--card) 100%);
      overflow: hidden;
    }}
    .hero {{
      margin: 14px 14px 0; border-radius: 12px; min-height: 110px;
      display: grid; place-items: center; text-align: center;
      background: #05060a; border: 1px solid rgba(255,255,255,0.06);
      position: relative; overflow: hidden;
    }}
    .hero span {{
      font-weight: 800; letter-spacing: 0.08em; font-size: 0.95rem;
      color: #e8ecf7; z-index: 1;
    }}
    .dot {{ position: absolute; width: 8px; height: 8px; border-radius: 50%; opacity: 0.85; }}
    .dot.a {{ background: #f472b6; top: 18%; left: 14%; }}
    .dot.b {{ background: #facc15; top: 28%; right: 18%; }}
    .dot.c {{ background: #4ade80; bottom: 22%; left: 22%; }}
    .dot.d {{ background: #a78bfa; bottom: 18%; right: 14%; }}
    .body {{ padding: 22px 22px 24px; }}
    .eyebrow {{
      margin: 0 0 8px; color: var(--muted); font-size: 0.72rem;
      letter-spacing: 0.12em; text-transform: uppercase; font-weight: 700;
    }}
    h1 {{
      margin: 0 0 12px; font-size: clamp(1.55rem, 4vw, 2rem);
      line-height: 1.15; letter-spacing: -0.02em; color: var(--accent);
    }}
    .status {{ margin: 0 0 8px; font-size: 1.02rem; color: #eef2ff; }}
    .flavor {{ margin: 0 0 18px; color: var(--muted); font-style: italic; font-size: 0.95rem; }}
    .meta {{ margin: 0 0 14px; color: var(--muted); font-size: 0.82rem; }}
    .bar {{
      height: 10px; border-radius: 999px; overflow: hidden;
      background: rgba(148,163,184,0.18); margin-bottom: 8px;
    }}
    .bar > i {{
      display: block; height: 100%; width: {progress}%;
      background: linear-gradient(90deg, #22d3ee, #facc15, #fb923c);
      transition: width 0.35s ease;
    }}
    .bar-label {{ color: var(--muted); font-size: 0.85rem; margin: 0 0 16px; }}
    .bypass {{
      margin: 0 0 18px; color: #c7d2fe; font-size: 0.88rem; line-height: 1.45;
    }}
    .bypass a {{ color: #93c5fd; }}
    .actions {{ display: flex; flex-wrap: wrap; gap: 10px; }}
    .btn {{
      appearance: none; border-radius: 999px; padding: 11px 16px;
      font-weight: 700; font-size: 0.92rem; cursor: pointer; text-decoration: none;
      display: inline-flex; align-items: center; justify-content: center;
    }}
    .btn-ghost {{
      background: transparent; color: #e2e8f0;
      border: 1px solid rgba(226,232,240,0.45);
    }}
    .btn-primary {{
      background: var(--brand); color: #fff; border: 1px solid transparent;
    }}
    .btn:disabled {{ opacity: 0.55; cursor: wait; }}
    footer {{
      display: flex; flex-wrap: wrap; gap: 14px 18px; justify-content: center;
      padding: 18px 16px 28px; color: #7b8499; font-size: 0.85rem;
    }}
    footer a {{ color: #9aa3b5; text-decoration: none; }}
    footer a:hover {{ color: #dbe3f5; }}
    @media (max-width: 520px) {{
      .rail {{ padding: 12px; }}
      .body {{ padding: 18px 16px 20px; }}
      .actions {{ flex-direction: column; }}
      .btn {{ width: 100%; }}
    }}
  </style>
</head>
<body>
  <header class="rail">
    <img src="/cpn-logo.png" alt="CPN Control Panel Network" width="160" height="36"
      onerror="this.style.display='none'">
    <a class="staff" href="/login?cpn_maint_bypass={bypass}">Staff sign in</a>
  </header>
  <div class="wrap">
    <section class="card" aria-live="polite">
      <div class="inner">
        <div class="hero" aria-hidden="true">
          <span>PANEL UPDATE IN PROGRESS</span>
          <i class="dot a"></i><i class="dot b"></i><i class="dot c"></i><i class="dot d"></i>
        </div>
        <div class="body">
          <p class="eyebrow">CPN Control Panel</p>
          <h1 id="cpn-maint-title">{title_e}</h1>
          <p class="status" id="cpn-maint-message">{message_e}</p>
          <p class="flavor" id="cpn-maint-flavor">{witty}</p>
          <p class="meta" id="cpn-maint-meta">
            Phase: <strong id="cpn-maint-phase">{phase}</strong>
            {target_line}
            <br>Started: {started_e}
            <span id="cpn-maint-updated"> · Updated: {updated_e}</span>
          </p>
          <div class="bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow="{progress}">
            <i id="cpn-maint-bar" style="width:{progress}%"></i>
          </div>
          <p class="bar-label" id="cpn-maint-pct">{progress}% · waiting for the panel…</p>
          <p class="bypass">
            Owners can keep working with a signed-in admin session, or open
            <a href="/settings/version?cpn_maint_bypass={bypass}">Version progress</a>
            with the upgrade bypass.
          </p>
          <div class="actions">
            <button type="button" class="btn btn-ghost" id="cpn-maint-retry">Try again</button>
            <a class="btn btn-primary" id="cpn-maint-home" href="/">Open panel when ready</a>
          </div>
        </div>
      </div>
    </section>
  </div>
  <footer>
    <a href="https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases" target="_blank" rel="noopener noreferrer">Changelog</a>
    <a href="https://github.com/Control-Panel-Network/CPN-Control-Panel-Network#readme" target="_blank" rel="noopener noreferrer">Docs</a>
    <a href="/login">Sign in</a>
  </footer>
  <script>
  (function () {{
    var retryBtn = document.getElementById("cpn-maint-retry");
    var bar = document.getElementById("cpn-maint-bar");
    var pct = document.getElementById("cpn-maint-pct");
    var msg = document.getElementById("cpn-maint-message");
    var phaseEl = document.getElementById("cpn-maint-phase");
    var titleEl = document.getElementById("cpn-maint-title");
    var flavorEl = document.getElementById("cpn-maint-flavor");
    var updatedEl = document.getElementById("cpn-maint-updated");
    var timer = null;
    var fails = 0;
    function setProgress(n, label) {{
      n = Math.max(0, Math.min(100, Math.round(Number(n) || 0)));
      if (bar) bar.style.width = n + "%";
      if (pct) pct.textContent = n + "% · " + (label || "waiting for the panel…");
    }}
    function apply(data) {{
      if (!data) return;
      if (titleEl && data.title) titleEl.textContent = data.title;
      if (msg && data.message) msg.textContent = data.message;
      if (phaseEl && data.phase) phaseEl.textContent = data.phase;
      if (updatedEl && data.updated_at) updatedEl.textContent = " · Updated: " + data.updated_at;
      setProgress(data.progress, data.message || data.phase || "");
      if (flavorEl && data.flavor) flavorEl.textContent = data.flavor;
      if (data.active === false) {{
        setProgress(100, "Panel is back");
        window.location.replace("/");
      }}
    }}
    function probe() {{
      fetch("/api/panel-maintenance", {{
        credentials: "same-origin",
        cache: "no-store",
        headers: {{ "Accept": "application/json" }}
      }}).then(function (res) {{
        if (!res.ok) throw new Error("HTTP " + res.status);
        return res.json();
      }}).then(function (data) {{
        fails = 0;
        apply(data);
        schedule(data && data.active ? 1500 : 800);
      }}).catch(function () {{
        fails += 1;
        setProgress(null, "Panel unreachable · retry " + fails);
        // Soft probe: if /login answers, maintenance may have cleared mid-restart.
        fetch("/login", {{ method: "GET", credentials: "same-origin", cache: "no-store", redirect: "manual" }})
          .then(function (res) {{
            if (res && (res.ok || res.status === 301 || res.status === 302 || res.status === 303)) {{
              window.location.replace("/");
              return;
            }}
            schedule(Math.min(5000, 1200 + fails * 400));
          }})
          .catch(function () {{ schedule(Math.min(5000, 1200 + fails * 400)); }});
      }});
    }}
    function schedule(ms) {{
      if (timer) clearTimeout(timer);
      timer = setTimeout(probe, ms);
    }}
    if (retryBtn) {{
      retryBtn.addEventListener("click", function () {{
        retryBtn.disabled = true;
        probe();
        setTimeout(function () {{ retryBtn.disabled = false; }}, 800);
      }});
    }}
    schedule(1200);
  }})();
  </script>
</body>
</html>"##,
        title_e = title_e,
        favicons = favicons,
        progress = progress,
        bypass = bypass,
        witty = html_escape(witty),
        message_e = message_e,
        phase = phase,
        target_line = if target.is_empty() {
            String::new()
        } else {
            format!(" · {target}")
        },
        started_e = started_e,
        updated_e = updated_e,
    )
}

pub fn witty_for_phase(phase: &str) -> &'static str {
    witty_line(phase)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panel_maintenance_mode::MaintenanceFlag;

    #[test]
    fn page_has_cpn_branding_not_casino() {
        let flag = MaintenanceFlag {
            active: true,
            phase: "installing".into(),
            progress: 40,
            message: "Installing commit abc1234".into(),
            title: "Updating CPN Panel".into(),
            target: Some("abc1234".into()),
            source: "ui".into(),
            started_at_unix: 1_767_225_600,
            updated_at_unix: 1_767_225_600,
            expires_at_unix: 1_767_225_600 + 3600,
            bypass_token: "tokentokentokentokentokentoken12".into(),
        };
        let html = render_html(&flag);
        assert!(html.contains("Updating CPN Panel"));
        assert!(html.contains("CPN Control Panel"));
        assert!(html.contains("/api/panel-maintenance"));
        assert!(html.contains("Try again"));
        assert!(!html.to_lowercase().contains("casino"));
        assert!(!html.to_lowercase().contains("cyberpanel"));
        assert!(!html.contains("404: Your luck"));
        assert!(!html.contains("Discord"));
    }
}
