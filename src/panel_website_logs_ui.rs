//! Manage > Logs modal markup, styles, and client script.

use crate::panel_website_manage_ui::{html_escape, section};
use crate::sites::SiteRecord;

fn log_tile_button(kind: &str, title: &str, subtitle: &str) -> String {
    let icon_href = if kind == "error" { "#error" } else { "#access" };
    format!(
        r#"<button type="button" class="manage-tile manage-log-open" data-log-kind="{kind}" aria-haspopup="dialog">
  {icon}
  <span><strong>{title}</strong><span>{subtitle}</span></span>
</button>"#,
        kind = html_escape(kind),
        icon = crate::panel_icons::manage_icon_html(icon_href),
        title = html_escape(title),
        subtitle = html_escape(subtitle),
    )
}

pub fn logs_modal_styles() -> &'static str {
    r#"
.site-manage .manage-log-open {
  cursor:pointer; font:inherit; text-align:left; width:100%;
}
.site-manage .manage-log-dialog {
  border:1px solid var(--hairline, var(--m-line)); border-radius:18px; padding:0;
  max-width:min(960px,96vw); width:100%;
  background:var(--canvas, var(--m-card)); color:var(--ink, var(--m-ink));
  box-shadow:0 18px 50px rgba(15,23,42,.35);
}
.site-manage .manage-log-dialog::backdrop { background:rgba(15,23,42,.55); }
.site-manage .manage-log-dialog-inner { margin:0; padding:16px 18px 18px; }
.site-manage .manage-log-dialog-inner header {
  display:flex; align-items:flex-start; justify-content:space-between; gap:12px; margin-bottom:10px;
}
.site-manage .manage-log-dialog-inner h2 { margin:0; font-size:1.15rem; font-weight:700; color:var(--ink, var(--m-ink)); }
.site-manage .manage-log-dialog-inner .manage-muted { color:var(--muted, var(--m-muted)); }
.site-manage .manage-log-toolbar-row {
  display:flex; flex-wrap:wrap; gap:8px; align-items:center; margin:0 0 10px;
}
.site-manage .manage-log-toolbar-row input[type=search],
.site-manage .manage-log-toolbar-row input[type=number],
.site-manage .manage-log-toolbar-row select {
  min-height:40px; padding:0 12px; border-radius:10px;
  border:1px solid var(--hairline, var(--m-line));
  background:var(--canvas, #0b0d12); color:var(--ink, var(--m-ink)); font:inherit; font-size:13px;
}
.site-manage .manage-log-toolbar-row input[type=search] { flex:1 1 180px; min-width:140px; }
.site-manage .manage-log-toolbar-row select { flex:0 0 auto; }
.site-manage .manage-log-toolbar-row .manage-btn,
.site-manage .manage-log-dialog-inner button.manage-btn {
  display:inline-flex; align-items:center; justify-content:center;
  min-height:40px; padding:0 14px; border-radius:999px; border:1px solid var(--hairline, var(--m-line));
  background:#f2f4f7; color:#344054; font-weight:700; font-size:13px; cursor:pointer;
}
[data-color-mode="dark"] .site-manage .manage-log-toolbar-row .manage-btn,
[data-color-mode="dark"] .site-manage .manage-log-dialog-inner button.manage-btn,
.site-manage .manage-log-toolbar-row .manage-btn {
  background:rgba(255,255,255,.08); color:var(--ink, var(--m-ink)); border-color:var(--hairline, var(--m-line));
}
.site-manage .manage-log-dialog .manage-log-pre {
  max-height:min(52vh, 420px); min-height:160px;
}
.site-manage .manage-log-pager {
  display:flex; flex-wrap:wrap; gap:8px; align-items:center; margin-top:10px;
}
.site-manage .manage-log-pager label { font-size:13px; color:var(--muted, var(--m-muted)); }
@media (max-width:679.98px) {
  .site-manage .manage-log-dialog-inner { padding:14px; }
  .site-manage .manage-log-toolbar-row .manage-btn { flex:1 1 auto; }
}
"#
}

pub fn logs_tab_html(site: &SiteRecord) -> String {
    let domain = html_escape(&site.domain);
    let mut tiles = String::from(r#"<div class="manage-tile-grid">"#);
    tiles.push_str(&log_tile_button(
        "access",
        "Access Logs",
        "Search and paginate allowlisted access logs",
    ));
    tiles.push_str(&log_tile_button(
        "error",
        "Error Logs",
        "Search and paginate allowlisted error logs",
    ));
    tiles.push_str("</div>");

    let modal = format!(
        r#"<dialog class="manage-log-dialog" id="manage-log-dialog" data-domain="{domain}">
  <div class="manage-log-dialog-inner">
    <header>
      <div>
        <h2 id="manage-log-dialog-title">Access Logs</h2>
        <p id="manage-log-dialog-source" class="manage-muted" style="margin:6px 0 0;"></p>
      </div>
      <form method="dialog"><button type="submit" class="manage-btn" aria-label="Close">Close</button></form>
    </header>
    <div class="manage-log-toolbar-row">
      <input type="search" id="manage-log-search" placeholder="Search lines" autocomplete="off">
      <select id="manage-log-per-page" aria-label="Lines per page">
        <option value="10">10 / page</option>
        <option value="25" selected>25 / page</option>
        <option value="50">50 / page</option>
      </select>
      <button type="button" class="manage-btn" id="manage-log-refresh">Refresh</button>
    </div>
    <pre class="manage-log-pre" id="manage-log-body" aria-live="polite"></pre>
    <div class="manage-log-pager">
      <button type="button" class="manage-btn" id="manage-log-prev">Prev</button>
      <button type="button" class="manage-btn" id="manage-log-next">Next</button>
      <label>Page <input type="number" id="manage-log-goto" min="1" value="1" style="width:4.5rem;"> of <span id="manage-log-total-pages">1</span></label>
      <button type="button" class="manage-btn" id="manage-log-go">Go</button>
    </div>
  </div>
</dialog>
{script}"#,
        domain = domain,
        script = logs_modal_script(),
    );

    format!(
        "{tiles}{modal}<p class=\"manage-muted\" style=\"margin-top:12px;\">Logs are jailed to this domain site home only. Admin retention: <a href=\"/settings/logs\">Settings &gt; Log retention</a> (default 30 days).</p>",
        tiles = section("Logs", &tiles),
        modal = modal,
    )
}

fn logs_modal_script() -> String {
    String::from(
        r#"<script>
(function () {
  var dialog = document.getElementById("manage-log-dialog");
  if (!dialog) return;
  var domain = dialog.getAttribute("data-domain") || "";
  var titleEl = document.getElementById("manage-log-dialog-title");
  var sourceEl = document.getElementById("manage-log-dialog-source");
  var bodyEl = document.getElementById("manage-log-body");
  var searchEl = document.getElementById("manage-log-search");
  var perPageEl = document.getElementById("manage-log-per-page");
  var gotoEl = document.getElementById("manage-log-goto");
  var totalEl = document.getElementById("manage-log-total-pages");
  var kind = "access";
  var page = 1;
  var searchTimer = null;

  function setBusy(busy) {
    if (bodyEl) bodyEl.setAttribute("aria-busy", busy ? "true" : "false");
  }

  function loadPage() {
    if (!domain) return;
    setBusy(true);
    var q = new URLSearchParams();
    q.set("domain", domain);
    q.set("kind", kind);
    q.set("page", String(page));
    q.set("per_page", String(perPageEl ? perPageEl.value : "25"));
    if (searchEl && searchEl.value.trim()) q.set("search", searchEl.value.trim());
    fetch("/api/websites/manage/logs?" + q.toString(), { credentials: "same-origin" })
      .then(function (r) { return r.json(); })
      .then(function (data) {
        setBusy(false);
        if (!data || !data.ok) {
          if (bodyEl) bodyEl.textContent = (data && data.error) ? data.error : "Could not load logs";
          return;
        }
        page = data.page || 1;
        if (titleEl) titleEl.textContent = kind === "error" ? "Error Logs" : "Access Logs";
        if (sourceEl) sourceEl.textContent = data.message || ("Source: " + (data.path || ""));
        if (bodyEl) bodyEl.textContent = (data.lines && data.lines.length) ? data.lines.join("\n") : "";
        if (gotoEl) gotoEl.value = String(page);
        if (totalEl) totalEl.textContent = String(data.total_pages || 1);
      })
      .catch(function () {
        setBusy(false);
        if (bodyEl) bodyEl.textContent = "Network error loading logs";
      });
  }

  function openKind(nextKind) {
    kind = nextKind === "error" ? "error" : "access";
    page = 1;
    if (searchEl) searchEl.value = "";
    if (typeof dialog.showModal === "function") dialog.showModal();
    loadPage();
  }

  document.querySelectorAll(".manage-log-open").forEach(function (btn) {
    btn.addEventListener("click", function () {
      openKind(btn.getAttribute("data-log-kind") || "access");
    });
  });
  var refresh = document.getElementById("manage-log-refresh");
  if (refresh) refresh.addEventListener("click", function () { loadPage(); });
  var prev = document.getElementById("manage-log-prev");
  if (prev) prev.addEventListener("click", function () {
    if (page > 1) { page -= 1; loadPage(); }
  });
  var next = document.getElementById("manage-log-next");
  if (next) next.addEventListener("click", function () {
    page += 1; loadPage();
  });
  var go = document.getElementById("manage-log-go");
  if (go) go.addEventListener("click", function () {
    var n = parseInt(gotoEl && gotoEl.value, 10);
    if (!isNaN(n) && n >= 1) { page = n; loadPage(); }
  });
  if (perPageEl) perPageEl.addEventListener("change", function () { page = 1; loadPage(); });
  if (searchEl) searchEl.addEventListener("input", function () {
    if (searchTimer) clearTimeout(searchTimer);
    searchTimer = setTimeout(function () { page = 1; loadPage(); }, 280);
  });
})();
</script>"#,
    )
}
