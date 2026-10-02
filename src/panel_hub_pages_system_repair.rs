//! Owner-only System Repair hub UI (progressive check cards via JSON API).

use crate::panel_hubs::feature_shell;
use crate::system_repair::PRODUCT_NAME;

fn styles() -> &'static str {
    r#"<style>
.sr-summary{display:flex;flex-wrap:wrap;gap:10px;margin:0 0 16px;}
.sr-pill{display:inline-flex;align-items:center;gap:6px;min-height:36px;padding:0 12px;border-radius:10px;font-weight:700;font-size:13px;background:#e2e8f0;color:#0f172a;}
.sr-pill-pass{background:#dcfce7;color:#14532d;}
.sr-pill-warn{background:#ffedd5;color:#9a3412;}
.sr-pill-fail{background:#fee2e2;color:#991b1b;}
[data-color-mode="dark"] .sr-pill{background:#334155;color:#f8fafc;}
[data-color-mode="dark"] .sr-pill-pass{background:rgba(22,163,74,.28);color:#bbf7d0;}
[data-color-mode="dark"] .sr-pill-warn{background:rgba(234,88,12,.28);color:#fed7aa;}
[data-color-mode="dark"] .sr-pill-fail{background:rgba(220,38,38,.28);color:#fecaca;}
.sr-actions{display:flex;flex-wrap:wrap;gap:8px;margin:0 0 18px;}
.sr-actions form{display:inline;}
.sr-grid{display:flex;flex-direction:column;gap:12px;}
.sr-card{border:1px solid #e2e8f0;border-radius:12px;padding:14px 16px;background:#fff;}
[data-color-mode="dark"] .sr-card{border-color:#334155;background:#1e293b;}
.sr-card-head{display:flex;flex-wrap:wrap;gap:8px;align-items:center;justify-content:space-between;margin:0 0 8px;}
.sr-card-title{margin:0;font-size:16px;font-weight:700;}
.sr-meta{margin:0;font-size:12px;color:#64748b;}
[data-color-mode="dark"] .sr-meta{color:#94a3b8;}
.sr-detail{margin:6px 0 0;font-size:14px;line-height:1.45;word-break:break-word;}
.sr-badge{display:inline-flex;align-items:center;min-height:24px;padding:0 8px;border-radius:999px;font-size:11px;font-weight:700;letter-spacing:.04em;text-transform:uppercase;}
.sr-badge-pass{background:#dcfce7;color:#14532d;}
.sr-badge-warn{background:#ffedd5;color:#9a3412;}
.sr-badge-fail{background:#fee2e2;color:#991b1b;}
[data-color-mode="dark"] .sr-badge-pass{background:rgba(22,163,74,.28);color:#bbf7d0;}
[data-color-mode="dark"] .sr-badge-warn{background:rgba(234,88,12,.28);color:#fed7aa;}
[data-color-mode="dark"] .sr-badge-fail{background:rgba(220,38,38,.28);color:#fecaca;}
.sr-heal{margin-top:10px;}
.sr-loading{padding:18px 4px;color:#64748b;}
[data-color-mode="dark"] .sr-loading{color:#94a3b8;}
@media (max-width:720px){
  .sr-card-head{align-items:flex-start;}
  .sr-actions .btn-primary,.sr-actions .btn-secondary,.sr-heal .btn-secondary{width:100%;justify-content:center;}
}
</style>"#
}

fn client_script() -> &'static str {
    r#"<script>
(function () {
  var root = document.getElementById("sr-root");
  if (!root) return;
  var passEl = document.getElementById("sr-pass");
  var warnEl = document.getElementById("sr-warn");
  var failEl = document.getElementById("sr-fail");
  var grid = document.getElementById("sr-grid");
  var statusLine = document.getElementById("sr-status");

  function esc(s) {
    return String(s == null ? "" : s)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function badge(status) {
    var cls = "sr-badge sr-badge-warn";
    var label = "Warn";
    if (status === "pass") { cls = "sr-badge sr-badge-pass"; label = "Pass"; }
    else if (status === "fail") { cls = "sr-badge sr-badge-fail"; label = "Fail"; }
    return '<span class="' + cls + '">' + label + "</span>";
  }

  function card(check) {
    var heal = "";
    if (check.heal_id && check.status !== "pass") {
      heal =
        '<form method="post" action="/server/system-repair/heal" class="sr-heal">' +
        '<input type="hidden" name="id" value="' + esc(check.heal_id) + '">' +
        '<button type="submit" class="btn-secondary">Heal</button></form>';
    }
    return (
      '<article class="sr-card" data-status="' + esc(check.status) + '">' +
      '<div class="sr-card-head"><h3 class="sr-card-title">' + esc(check.title) + "</h3>" +
      badge(check.status) + "</div>" +
      '<p class="sr-meta"><code>' + esc(check.id) + "</code> · " + esc(check.category) + "</p>" +
      '<p class="sr-detail">' + esc(check.detail) + "</p>" + heal + "</article>"
    );
  }

  function render(report) {
    if (passEl) passEl.textContent = "Pass " + (report.pass || 0);
    if (warnEl) warnEl.textContent = "Warn " + (report.warn || 0);
    if (failEl) failEl.textContent = "Fail " + (report.fail || 0);
    var html = "";
    (report.checks || []).forEach(function (c) { html += card(c); });
    if (!html) {
      html = '<p class="sr-loading">No checks returned. Reload to try again.</p>';
    }
    grid.innerHTML = html;
    if (statusLine) {
      statusLine.textContent = report.cached
        ? "Showing cached results (refreshed in the background)."
        : "Checks finished.";
    }
  }

  function fail(msg) {
    if (statusLine) statusLine.textContent = msg;
    grid.innerHTML =
      '<article class="sr-card"><div class="sr-card-head"><h3 class="sr-card-title">Checks unavailable</h3>' +
      badge("warn") + '</div><p class="sr-detail">' + esc(msg) +
      '</p><p class="sr-detail"><a class="btn-secondary" href="/server/system-repair?refresh=1">Retry</a></p></article>';
  }

  var refresh = /(?:\?|&)refresh=1(?:&|$)/.test(location.search) ? "?refresh=1" : "";
  if (statusLine) statusLine.textContent = "Running host checks…";
  var ctrl = typeof AbortController !== "undefined" ? new AbortController() : null;
  var timer = setTimeout(function () { if (ctrl) ctrl.abort(); }, 18000);
  fetch("/server/system-repair/api" + refresh, {
    credentials: "same-origin",
    headers: { Accept: "application/json" },
    signal: ctrl ? ctrl.signal : undefined
  })
    .then(function (res) {
      if (!res.ok) throw new Error("HTTP " + res.status);
      return res.json();
    })
    .then(function (data) {
      clearTimeout(timer);
      if (!data || data.ok === false) {
        fail((data && data.error) || "System Repair API failed.");
        return;
      }
      render(data);
    })
    .catch(function (err) {
      clearTimeout(timer);
      fail(
        err && err.name === "AbortError"
          ? "Checks timed out. Reload to retry; slow probes are skipped per group."
          : "Could not load checks (" + (err && err.message ? err.message : "network") + ")."
      );
    });
})();
</script>"#
}

/// Fast System Repair shell. Check cards load from `/server/system-repair/api`.
pub fn system_repair_page(notice: Option<&str>, error: Option<&str>) -> String {
    let mut body = String::new();
    body.push_str(styles());
    body.push_str(
        r#"<div id="sr-root">
<div class="sr-summary">
  <span class="sr-pill sr-pill-pass" id="sr-pass">Pass …</span>
  <span class="sr-pill sr-pill-warn" id="sr-warn">Warn …</span>
  <span class="sr-pill sr-pill-fail" id="sr-fail">Fail …</span>
</div>
<p class="muted">Owner diagnostics for install, upgrade, update, and downgrade recovery. Safe heals never wipe MFA. CLI: <code>cpn doctor</code>, <code>cpn troubleshoot</code>, or <code>cpn repair</code>.</p>
<p class="muted" id="sr-status">Loading checks…</p>
<div class="sr-actions">
  <a class="btn-secondary" href="/server/system-repair?refresh=1">Refresh checks</a>
  <form method="post" action="/server/system-repair/heal">
    <button type="submit" class="btn-primary">Heal all safe issues</button>
  </form>
  <a class="btn-secondary" href="/server/system-repair/api?refresh=1">JSON report</a>
</div>
<div class="sr-grid" id="sr-grid"><p class="sr-loading">Gathering host checks (progressive). The page shell stays available while probes run with timeouts.</p></div>
</div>"#,
    );
    body.push_str(client_script());
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            (PRODUCT_NAME, None),
        ],
        PRODUCT_NAME,
        "Diagnose and safely heal panel, email, PHP, phpMyAdmin, and host services.",
        &body,
        notice,
        error,
    )
}
