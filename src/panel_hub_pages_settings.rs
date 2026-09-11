//! Settings hub: Version Management, Design, Setup Wizard, Connect, and port.

use crate::manifest::detect_existing_install;
use crate::panel_hub_defs::settings_hub_sections;
use crate::panel_hubs::{feature_shell, hub_tiles_grid, section_heading};
use crate::panel_theme_chrome::design_settings_panel;

const RUNNING_VERSION: &str = env!("CARGO_PKG_VERSION");

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn settings_hub_main() -> String {
    let mut body = section_heading(
        "Settings",
        "Panel version, design, onboarding, community links, and listen port.",
    );
    for (title, tiles) in settings_hub_sections() {
        body.push_str(&hub_tiles_grid(title, &tiles));
    }
    body
}

/// Kept for callers that still import the stub name (parallel hub PRs).
pub fn settings_stub_page() -> String {
    settings_hub_main()
}

/// `can_manage`: panel admin only; non-admins get read-only version info.
pub fn version_management_page(can_manage: bool) -> String {
    let existing = detect_existing_install(RUNNING_VERSION);
    let installed = html_escape(&existing.package_version);
    let running = html_escape(RUNNING_VERSION);
    let manage_block = if can_manage {
        r#"<div id="cpn-version-ops" class="stack-form" style="margin-top:18px;max-width:640px;">
  <label for="cpn-version-select">Release / tag
    <select id="cpn-version-select" style="display:block;width:100%;margin-top:6px;"></select>
  </label>
  <div style="display:flex;flex-wrap:wrap;gap:8px;margin-top:12px;">
    <button type="button" class="btn-primary" id="cpn-version-upgrade-latest">Upgrade to latest</button>
    <button type="button" class="btn-primary" id="cpn-version-apply">Apply selected version</button>
    <button type="button" class="btn-primary" id="cpn-version-repair">Repair selected</button>
  </div>
  <div id="cpn-version-confirm" class="muted" style="display:none;margin-top:14px;padding:12px;border:1px solid var(--cpn-border, #334155);border-radius:8px;">
    <p id="cpn-version-confirm-text" style="margin:0 0 10px;"></p>
    <div style="display:flex;flex-wrap:wrap;gap:8px;">
      <button type="button" class="btn-primary" id="cpn-version-confirm-go">Confirm</button>
      <button type="button" id="cpn-version-confirm-cancel">Cancel</button>
    </div>
  </div>
  <div id="cpn-version-progress-wrap" style="display:none;margin-top:16px;">
    <div style="height:10px;background:rgba(148,163,184,0.25);border-radius:999px;overflow:hidden;">
      <div id="cpn-version-progress-bar" style="height:100%;width:0%;background:var(--cpn-accent, #2563eb);transition:width 0.2s ease;"></div>
    </div>
    <p id="cpn-version-progress-label" class="muted" style="margin-top:8px;" role="status"></p>
  </div>
  <p id="cpn-version-op-error" class="muted" style="margin-top:10px;color:#f87171;" role="alert"></p>
</div>"#
    } else {
        r#"<p class="muted" style="margin-top:16px;">Only the panel admin can upgrade, downgrade, or repair from this page.</p>"#
    };
    let body = format!(
        r#"<ul class="kv-list">
  <li><span>Running</span><strong id="cpn-version-running">{running}</strong></li>
  <li><span>Installed package</span><strong id="cpn-version-installed">{installed}</strong></li>
  <li><span>Latest</span><strong id="cpn-version-latest">-</strong></li>
  <li><span>Manifest</span><strong>{manifest}</strong></li>
</ul>
<p id="cpn-version-status" class="muted" role="status">Checking for updates...</p>
<div id="cpn-version-details" class="muted"></div>
<div class="stack-form" style="margin-top:16px;max-width:560px;">
  <button type="button" class="btn-primary" id="cpn-version-refresh">Check for updates</button>
</div>
{manage_block}
<p class="muted" style="margin-top:18px;">
  Package ops can run from this page when you are the panel admin and the installer service runs as root.
  CLI remains available: <code>sudo cpn-installer --upgrade</code> / <code>--repair</code> / <code>--downgrade --to X.Y.Z --yes</code>.
  Release assets: <a href="https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases" target="_blank" rel="noopener noreferrer">GitHub Releases</a>.
</p>
<script>
(function () {{
  var canManage = {can_manage_js};
  var statusEl = document.getElementById("cpn-version-status");
  var detailsEl = document.getElementById("cpn-version-details");
  var btn = document.getElementById("cpn-version-refresh");
  var latestEl = document.getElementById("cpn-version-latest");
  var installedEl = document.getElementById("cpn-version-installed");
  var selectEl = document.getElementById("cpn-version-select");
  var confirmBox = document.getElementById("cpn-version-confirm");
  var confirmText = document.getElementById("cpn-version-confirm-text");
  var confirmGo = document.getElementById("cpn-version-confirm-go");
  var confirmCancel = document.getElementById("cpn-version-confirm-cancel");
  var progressWrap = document.getElementById("cpn-version-progress-wrap");
  var progressBar = document.getElementById("cpn-version-progress-bar");
  var progressLabel = document.getElementById("cpn-version-progress-label");
  var opError = document.getElementById("cpn-version-op-error");
  var infoCache = null;
  var pending = null;
  var pollTimer = null;
  var busy = false;

  function esc(s) {{
    return String(s == null ? "" : s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  }}
  function norm(v) {{
    return String(v || "").replace(/^v/i, "").trim();
  }}
  function cmp(a, b) {{
    var as = norm(a).split(".");
    var bs = norm(b).split(".");
    var n = Math.max(as.length, bs.length);
    for (var i = 0; i < n; i++) {{
      var ai = parseInt(as[i] || "0", 10) || 0;
      var bi = parseInt(bs[i] || "0", 10) || 0;
      if (ai < bi) return -1;
      if (ai > bi) return 1;
    }}
    return 0;
  }}
  function setActionsEnabled(on) {{
    ["cpn-version-upgrade-latest", "cpn-version-apply", "cpn-version-repair", "cpn-version-refresh"].forEach(function (id) {{
      var el = document.getElementById(id);
      if (el) el.disabled = !on;
    }});
    if (selectEl) selectEl.disabled = !on;
  }}
  function clearConfirm() {{
    pending = null;
    if (confirmBox) confirmBox.style.display = "none";
  }}
  function armConfirm(action, version, label) {{
    if (!canManage || busy) return;
    pending = {{ action: action, version: version || null }};
    if (confirmText) confirmText.textContent = "About to " + label + ". Click Confirm to start, or Cancel.";
    if (confirmGo) confirmGo.textContent = "Confirm " + label;
    if (confirmBox) confirmBox.style.display = "block";
    if (opError) opError.textContent = "";
  }}
  function fillSelect(info) {{
    if (!selectEl) return;
    var installed = norm(info.installed_version);
    var latest = norm(info.latest_version || (info.latest_tag || ""));
    var releases = info.releases || [];
    selectEl.innerHTML = "";
    if (!releases.length) {{
      var empty = document.createElement("option");
      empty.value = "";
      empty.textContent = "No releases loaded";
      selectEl.appendChild(empty);
      return;
    }}
    releases.forEach(function (r) {{
      var opt = document.createElement("option");
      var ver = r.version || norm(r.tag_name);
      var tag = r.tag_name || ver;
      opt.value = tag;
      var marks = [];
      if (norm(ver) === installed || norm(tag) === installed) marks.push("installed");
      if (latest && (norm(ver) === latest || norm(tag) === latest)) marks.push("latest");
      opt.textContent = tag + (marks.length ? " (" + marks.join(", ") + ")" : "");
      selectEl.appendChild(opt);
    }});
    if (latest) {{
      for (var i = 0; i < selectEl.options.length; i++) {{
        if (norm(selectEl.options[i].value) === latest) {{
          selectEl.selectedIndex = i;
          break;
        }}
      }}
    }}
  }}
  function render(info) {{
    infoCache = info;
    if (!info) {{
      statusEl.textContent = "Could not load version information.";
      return;
    }}
    if (installedEl && info.installed_version) installedEl.textContent = info.installed_version;
    if (latestEl) latestEl.textContent = info.latest_version || info.latest_tag || "-";
    if (info.check_error) {{
      statusEl.textContent = "Update check failed: " + info.check_error;
    }} else if (info.update_available) {{
      statusEl.textContent = "Update available: " + (info.latest_version || info.latest_tag || "newer release");
    }} else {{
      statusEl.textContent = "You are on the latest known release" +
        (info.latest_version ? (" (" + info.latest_version + ")") : "") + ".";
    }}
    var lines = [];
    if (info.repo) lines.push("Repo: " + info.repo);
    if (info.source) lines.push("Source: " + info.source);
    if (info.latest_tag) lines.push("Latest tag: " + info.latest_tag);
    detailsEl.innerHTML = lines.map(function (l) {{
      return "<p>" + esc(l) + "</p>";
    }}).join("");
    fillSelect(info);
  }}
  function check() {{
    statusEl.textContent = "Checking for updates...";
    fetch("/api/version-check", {{
      credentials: "same-origin",
      headers: {{ "Accept": "application/json" }}
    }}).then(function (res) {{
      if (!res.ok) throw new Error("HTTP " + res.status);
      return res.json();
    }}).then(render).catch(function (err) {{
      statusEl.textContent = "Update check failed: " + (err && err.message ? err.message : String(err));
    }});
  }}
  function pollStatus() {{
    fetch("/api/maintenance/status", {{
      credentials: "same-origin",
      headers: {{ "Accept": "application/json" }}
    }}).then(function (res) {{
      if (!res.ok) throw new Error("HTTP " + res.status);
      return res.json();
    }}).then(function (st) {{
      if (progressBar) progressBar.style.width = Math.max(0, Math.min(100, Number(st.progress) || 0)) + "%";
      if (progressLabel) {{
        progressLabel.textContent = (st.phase || "") + (st.message ? (": " + st.message) : "");
      }}
      if (st.error) {{
        busy = false;
        setActionsEnabled(true);
        if (pollTimer) {{ clearInterval(pollTimer); pollTimer = null; }}
        if (opError) opError.textContent = st.error;
        if (progressLabel) progressLabel.textContent = "Failed: " + st.error;
        return;
      }}
      if (!st.busy && (st.phase === "completed" || st.phase === "ready" || st.phase === "failed")) {{
        busy = false;
        setActionsEnabled(true);
        if (pollTimer) {{ clearInterval(pollTimer); pollTimer = null; }}
        if (st.phase === "failed") {{
          if (opError) opError.textContent = st.error || "Maintenance failed";
        }} else {{
          if (progressBar) progressBar.style.width = "100%";
          if (progressLabel) progressLabel.textContent = "Completed.";
          check();
        }}
      }}
    }}).catch(function (err) {{
      if (opError) opError.textContent = "Status poll failed: " + (err && err.message ? err.message : String(err));
    }});
  }}
  function startJob(action, version) {{
    if (!canManage || busy) return;
    busy = true;
    setActionsEnabled(false);
    clearConfirm();
    if (opError) opError.textContent = "";
    if (progressWrap) progressWrap.style.display = "block";
    if (progressBar) progressBar.style.width = "1%";
    if (progressLabel) progressLabel.textContent = "Starting...";
    var installed = infoCache && infoCache.installed_version ? infoCache.installed_version : "";
    var isDown = action === "downgrade" || (version && installed && cmp(version, installed) < 0);
    var body = {{
      action: action,
      version: version || null,
      confirm_execute: true,
      confirm_downgrade: !!isDown,
      reset_data: false
    }};
    fetch("/api/maintenance", {{
      method: "POST",
      credentials: "same-origin",
      headers: {{ "Accept": "application/json", "Content-Type": "application/json" }},
      body: JSON.stringify(body)
    }}).then(function (res) {{
      return res.json().then(function (data) {{
        if (!res.ok) throw new Error((data && data.error) || ("HTTP " + res.status));
        return data;
      }});
    }}).then(function () {{
      if (pollTimer) clearInterval(pollTimer);
      pollTimer = setInterval(pollStatus, 500);
      pollStatus();
    }}).catch(function (err) {{
      busy = false;
      setActionsEnabled(true);
      if (opError) opError.textContent = err && err.message ? err.message : String(err);
      if (progressLabel) progressLabel.textContent = "Not started.";
    }});
  }}
  if (btn) btn.addEventListener("click", check);
  if (canManage) {{
    var upLatest = document.getElementById("cpn-version-upgrade-latest");
    var applyBtn = document.getElementById("cpn-version-apply");
    var repairBtn = document.getElementById("cpn-version-repair");
    if (upLatest) upLatest.addEventListener("click", function () {{
      var latest = infoCache && (infoCache.latest_tag || infoCache.latest_version);
      armConfirm("upgrade", latest || null, "upgrade to latest" + (latest ? (" (" + latest + ")") : ""));
    }});
    if (applyBtn) applyBtn.addEventListener("click", function () {{
      var tag = selectEl && selectEl.value;
      if (!tag) {{ if (opError) opError.textContent = "Select a release first."; return; }}
      var installed = infoCache && infoCache.installed_version ? infoCache.installed_version : "";
      var action = (installed && cmp(tag, installed) < 0) ? "downgrade" : "upgrade";
      armConfirm(action, tag, action + " to " + tag);
    }});
    if (repairBtn) repairBtn.addEventListener("click", function () {{
      var tag = selectEl && selectEl.value;
      if (!tag) {{ if (opError) opError.textContent = "Select a release first."; return; }}
      armConfirm("repair", tag, "repair with " + tag);
    }});
    if (confirmGo) confirmGo.addEventListener("click", function () {{
      if (!pending) return;
      startJob(pending.action, pending.version);
    }});
    if (confirmCancel) confirmCancel.addEventListener("click", clearConfirm);
  }}
  check();
}})();
</script>"#,
        running = running,
        installed = installed,
        manifest = if existing.has_manifest {
            "present"
        } else {
            "missing"
        },
        manage_block = manage_block,
        can_manage_js = if can_manage { "true" } else { "false" },
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Settings", Some("/settings")),
            ("Version Management", None),
        ],
        "Version Management",
        "Update CPN",
        &body,
        None,
        None,
    )
}

pub fn design_settings_page(username: &str) -> String {
    let panel = design_settings_panel(username);
    let themes = crate::panel_theme_store::themes_catalog_panel(username);
    let note = r#"<p class="plugin-store-meta" style="margin-bottom:14px;">
  Light/dark mode is per signed-in user (sidebar toggle). Built-in presets and catalog themes from
  <a href="https://github.com/Control-Panel-Network/CPN-Themes" target="_blank" rel="noopener noreferrer">Control-Panel-Network/CPN-Themes</a>
  apply panel-wide chrome. Only the panel admin can change Design.
</p>"#;
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Settings", Some("/settings")),
            ("Design", None),
        ],
        "Design",
        "Theme & custom CSS",
        &format!("{note}{panel}{themes}"),
        None,
        None,
    )
}

pub fn setup_wizard_page() -> String {
    let body = r#"<p>CPN does not ship a separate post-install wizard inside the signed-in panel yet.
  Use this checklist for first-run onboarding, then the installer UI when you need a fresh or maintenance install.</p>
<ol class="setup-checklist" style="margin:16px 0;padding-left:1.25rem;line-height:1.6;">
  <li>Confirm the first admin account can sign in at <a href="/login">/login</a> (no installer token required once bootstrap exists).</li>
  <li>Add a website under <a href="/websites">Websites</a> and open <strong>Manage</strong> for that site.</li>
  <li>Install or verify the web stack from the installer when needed (<code>/</code> with the installer token while <code>cpn-installer</code> is running).</li>
  <li>Configure mail under <a href="/email">Email</a> when you need mailboxes.</li>
  <li>Set the panel listen port under <a href="/settings/port">Change Port</a> (default <code>2087</code>).</li>
  <li>Review <a href="/settings/version">Version Management</a> after upgrades.</li>
</ol>
<p class="muted">Honest scope: this page is a guided checklist, not an interactive multi-step wizard. The full installer UI remains the path for server/mail recipe installs and upgrade/repair.</p>"#;
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Settings", Some("/settings")),
            ("Setup Wizard", None),
        ],
        "Setup Wizard",
        "Server onboarding",
        body,
        None,
        None,
    )
}

pub fn connect_page() -> String {
    let body = r#"<p>Community and documentation for Control Panel Network (CPN). No third-party control-panel branding.</p>
<ul class="kv-list" style="margin-top:14px;">
  <li><span>Repository</span><strong><a href="https://github.com/Control-Panel-Network/CPN-Control-Panel-Network" target="_blank" rel="noopener noreferrer">CPN-Control-Panel-Network</a></strong></li>
  <li><span>Organization</span><strong><a href="https://github.com/Control-Panel-Network" target="_blank" rel="noopener noreferrer">github.com/Control-Panel-Network</a></strong></li>
  <li><span>Issues</span><strong><a href="https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/issues" target="_blank" rel="noopener noreferrer">GitHub Issues</a></strong></li>
  <li><span>Releases</span><strong><a href="https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases" target="_blank" rel="noopener noreferrer">GitHub Releases</a></strong></li>
  <li><span>Contributing</span><strong><a href="https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/blob/stable/CONTRIBUTING.md" target="_blank" rel="noopener noreferrer">CONTRIBUTING.md</a></strong></li>
  <li><span>Security</span><strong><a href="https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/blob/stable/SECURITY.md" target="_blank" rel="noopener noreferrer">SECURITY.md</a></strong></li>
</ul>
<p class="muted" style="margin-top:16px;">CPN does not publish a Discord invite in-repo yet. Prefer GitHub Issues and Discussions on the org for community contact.</p>"#;
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Settings", Some("/settings")),
            ("Connect", None),
        ],
        "Connect",
        "Community & docs",
        body,
        None,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hub_lists_four_primary_tiles() {
        let html = settings_hub_main();
        assert!(html.contains("Version Management"));
        assert!(html.contains("Update CPN"));
        assert!(html.contains("Design"));
        assert!(html.contains("Theme &amp; custom CSS") || html.contains("Theme & custom CSS"));
        assert!(html.contains("Setup Wizard"));
        assert!(html.contains("Server onboarding"));
        assert!(html.contains("Connect"));
        assert!(html.contains("Community &amp; docs") || html.contains("Community & docs"));
        assert!(html.contains("Change Port"));
        assert!(!html.to_lowercase().contains("cyberpanel"));
        assert!(!html.to_lowercase().contains("cyberpersons"));
    }

    #[test]
    fn connect_is_cpn_native() {
        let html = connect_page();
        assert!(html.contains("Control-Panel-Network"));
        assert!(html.contains("Community & docs") || html.contains("Community &amp; docs"));
        assert!(html.contains("GitHub Releases"));
        assert!(!html.to_lowercase().contains("cyberpersons"));
    }
}
