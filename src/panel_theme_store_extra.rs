//! Theme Store header actions + recent activity markup (keeps panel_theme_store under 500 lines).

/// Header action buttons for Store / Installed (Refresh, Update all).
pub fn theme_header_actions(installed_view: bool, can_edit: bool) -> String {
    let mut out = String::from(r#"<div class="cpn-themes-head-actions">"#);
    if !installed_view {
        out.push_str(
            r#"<button type="button" class="manage-btn" id="cpn-themes-refresh">Refresh catalog</button>"#,
        );
    }
    if installed_view && can_edit {
        out.push_str(
            r#"<button type="button" class="btn-primary" id="cpn-themes-update-all">Update all</button>"#,
        );
    }
    out.push_str("</div>");
    out
}

/// Recent theme activity panel (filled by client from `/api/panel/themes/actions`).
pub fn theme_activity_section() -> String {
    r#"<section class="cpn-themes-activity" id="cpn-themes-activity" aria-label="Theme activity">
  <div class="cpn-themes-activity-head">
    <h3>Recent theme activity</h3>
    <a class="plugin-store-meta" href="/settings/logs">Open Server Logs</a>
  </div>
  <p id="cpn-themes-activity-status" class="plugin-store-meta" role="status">Loading activity...</p>
  <ul id="cpn-themes-activity-list" class="cpn-themes-activity-list"></ul>
</section>
<style>
.cpn-themes-head-actions { display:flex; flex-wrap:wrap; gap:8px; align-items:center; }
.cpn-themes-activity { margin-top:18px; padding-top:14px; border-top:1px solid var(--hairline); }
.cpn-themes-activity-head { display:flex; flex-wrap:wrap; gap:10px; align-items:baseline; justify-content:space-between; margin-bottom:8px; }
.cpn-themes-activity h3 { margin:0; font-size:15px; color:var(--ink); }
.cpn-themes-activity-list { list-style:none; margin:0; padding:0; display:flex; flex-direction:column; gap:8px; }
.cpn-themes-activity-list li {
  display:flex; flex-wrap:wrap; gap:8px 12px; align-items:baseline;
  padding:8px 10px; border:1px solid var(--hairline); border-radius:10px; background:var(--canvas); font-size:13px;
}
.cpn-themes-activity-list .ok { color:#065f46; font-weight:700; }
.cpn-themes-activity-list .fail { color:#9f1239; font-weight:700; }
.cpn-themes-activity-list time { opacity:.75; }
</style>"#
        .to_string()
}

/// Client script fragment: Update all + activity loader (appended inside the Theme Store IIFE).
pub fn theme_update_all_and_activity_js() -> &'static str {
    r#"
  var updateAllBtn = document.getElementById("cpn-themes-update-all");
  var activityStatus = document.getElementById("cpn-themes-activity-status");
  var activityList = document.getElementById("cpn-themes-activity-list");

  function fmtLocal(ts) {
    var d = new Date((ts || 0) * 1000);
    if (isNaN(d.getTime())) return "";
    function p(n) { return String(n).padStart(2, "0"); }
    return p(d.getDate()) + "/" + p(d.getMonth() + 1) + "/" + d.getFullYear() + " " +
      p(d.getHours()) + ":" + p(d.getMinutes()) + ":" + p(d.getSeconds());
  }

  function loadActivity() {
    if (!activityStatus || !activityList) return;
    activityStatus.textContent = "Loading activity...";
    cpnDesignFetchJson("/api/panel/themes/actions", { headers: { "Accept": "application/json" } })
      .then(function (data) {
        var rows = data.actions || [];
        if (!rows.length) {
          activityStatus.textContent = "No theme installs, updates, or uninstalls logged yet.";
          activityList.innerHTML = "";
          return;
        }
        activityStatus.textContent = rows.length + " recent theme action(s). Full history: Server Logs.";
        activityList.innerHTML = rows.map(function (r) {
          var cls = r.ok ? "ok" : "fail";
          var result = r.ok ? "OK" : "Failed";
          return "<li><time>" + esc(fmtLocal(r.ts)) + "</time>" +
            "<span>" + esc(r.actor || "") + "</span>" +
            "<strong>" + esc(r.label || r.action || "") + "</strong>" +
            (r.theme_id ? ("<code>" + esc(r.theme_id) + "</code>") : "") +
            "<span class=\"" + cls + "\">" + result + "</span>" +
            "<span>" + esc(r.message || "") + "</span></li>";
        }).join("");
      })
      .catch(function (err) {
        activityStatus.textContent = "Could not load activity: " + (err.message || String(err));
        activityList.innerHTML = "";
      });
  }

  if (updateAllBtn) {
    updateAllBtn.addEventListener("click", function () {
      var pending = (allThemes || []).filter(function (t) { return t.installed && t.update_available; }).length;
      var msg = pending
        ? ("Update all " + pending + " theme(s) with available updates? This reinstalls packages from the catalog.")
        : "No updates are pending in the current catalog. Refresh and try Update all anyway?";
      if (!window.confirm(msg)) return;
      statusEl.textContent = "Updating themes...";
      updateAllBtn.disabled = true;
      cpnDesignFetchJson("/api/panel/themes/update-all", {
        method: "POST",
        headers: { "Content-Type": "application/json", "Accept": "application/json" },
        body: "{}"
      }).then(function (data) {
        statusEl.textContent = (data && data.message) ? data.message : "Update all finished.";
        load(false);
        loadActivity();
      }).catch(function (err) {
        statusEl.textContent = err.message || String(err);
        alert(err.message || String(err));
      }).then(function () {
        updateAllBtn.disabled = false;
      });
    });
  }
  loadActivity();
"#
}
