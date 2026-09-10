//! Theme Store UI for Settings > Design (CPN-Themes catalog).

use crate::panel_admin::is_panel_admin;

/// Theme Store cards sourced from Control-Panel-Network/CPN-Themes.
pub fn themes_catalog_panel(username: &str) -> String {
    let can_edit = is_panel_admin(username);
    format!(
        r##"<article class="section-card cpn-themes-catalog" id="cpn-themes-catalog">
  <header class="cpn-themes-head">
    <div>
      <h2>Theme Store</h2>
      <p class="plugin-store-meta">Catalog: <a href="https://github.com/Control-Panel-Network/CPN-Themes" target="_blank" rel="noopener noreferrer">https://github.com/Control-Panel-Network/CPN-Themes</a></p>
    </div>
    <button type="button" class="manage-btn" id="cpn-themes-refresh">Refresh catalog</button>
  </header>
  <p id="cpn-themes-status" class="plugin-store-meta" role="status">Loading themes...</p>
  <div id="cpn-themes-grid" class="plugin-grid" aria-live="polite"></div>
</article>
<style>
.cpn-themes-catalog {{ margin-top:18px; }}
.cpn-themes-head {{ display:flex; flex-wrap:wrap; gap:12px; align-items:flex-start; justify-content:space-between; margin-bottom:12px; }}
.cpn-themes-catalog h2 {{ margin:0 0 6px; font-size:18px; color:var(--ink); }}
.cpn-theme-swatch {{
  width:100%; height:48px; border-radius:12px; border:1px solid var(--hairline);
  background:linear-gradient(90deg, var(--swatch-a, #2563eb), var(--swatch-b, #1d4ed8));
}}
.cpn-themes-catalog .plugin-grid {{
  display:grid; grid-template-columns:repeat(auto-fill,minmax(240px,1fr)); gap:16px; margin-top:10px;
}}
.cpn-themes-catalog .plugin-card {{
  display:flex; flex-direction:column; gap:10px; padding:16px;
  border:1px solid var(--hairline); border-radius:16px; background:var(--canvas); color:var(--ink);
}}
.cpn-themes-catalog .plugin-card h3 {{ margin:0; font-size:16px; color:var(--ink); }}
.cpn-themes-catalog .plugin-desc {{ margin:0; font-size:14px; line-height:1.45; color:var(--ink); opacity:.9; }}
.cpn-themes-catalog .plugin-meta {{ margin:0; font-size:13px; color:var(--ink); opacity:.8; }}
.cpn-themes-catalog .plugin-badges {{ display:flex; flex-wrap:wrap; gap:6px; }}
.cpn-themes-catalog .plugin-badge {{
  display:inline-flex; align-items:center; min-height:24px; padding:0 8px; border-radius:999px;
  font-size:11px; font-weight:700; background:#dbeafe; color:#1e3a8a;
}}
.cpn-themes-catalog .plugin-actions {{ margin-top:auto; }}
.cpn-themes-catalog .plugin-store-meta {{ color:var(--ink); font-size:.92rem; }}
.cpn-themes-catalog .plugin-store-meta a {{ color:var(--blue); font-weight:600; }}
.cpn-themes-catalog .manage-btn {{
  min-height:36px; padding:0 12px; border-radius:10px; border:1px solid var(--hairline);
  background:var(--canvas); color:inherit; font:inherit; cursor:pointer;
}}
</style>
<script>
(function () {{
  var statusEl = document.getElementById("cpn-themes-status");
  var grid = document.getElementById("cpn-themes-grid");
  var refreshBtn = document.getElementById("cpn-themes-refresh");
  var canEdit = {can_edit};
  if (!statusEl || !grid) return;

  function esc(s) {{
    return String(s == null ? "" : s)
      .replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
  }}

  function render(themes, meta) {{
    if (!themes || !themes.length) {{
      statusEl.textContent = "No themes found in the catalog.";
      grid.innerHTML = "";
      return;
    }}
    statusEl.textContent = themes.length + " themes - " + (meta || "CPN-Themes");
    grid.innerHTML = themes.map(function (t) {{
      var a = (t.tokens && t.tokens.accent) || "#2563eb";
      var b = (t.tokens && t.tokens.accent_focus) || a;
      var btn = canEdit
        ? ('<button type="button" class="btn-primary cpn-theme-apply" data-id="' + esc(t.id) + '">Apply</button>')
        : '<span class="plugin-badge">Admin only</span>';
      return '<article class="plugin-card">' +
        '<div class="cpn-theme-swatch" style="--swatch-a:' + esc(a) + ';--swatch-b:' + esc(b) + ';"></div>' +
        '<h3>' + esc(t.name) + '</h3>' +
        '<div class="plugin-badges"><span class="plugin-badge">v' + esc(t.version) + '</span></div>' +
        '<p class="plugin-desc">' + esc(t.description) + '</p>' +
        '<p class="plugin-meta">Author: ' + esc(t.author) + '</p>' +
        '<div class="plugin-actions">' + btn + '</div></article>';
    }}).join("");
    grid.querySelectorAll(".cpn-theme-apply").forEach(function (btn) {{
      btn.addEventListener("click", function () {{
        var id = btn.getAttribute("data-id");
        fetch("/api/panel/themes/apply", {{
          method: "POST",
          credentials: "same-origin",
          headers: {{ "Content-Type": "application/json", "Accept": "application/json" }},
          body: JSON.stringify({{ id: id }})
        }}).then(function (res) {{
          return res.json().then(function (data) {{
            if (!res.ok) throw new Error((data && data.error) || ("HTTP " + res.status));
            window.location.reload();
          }});
        }}).catch(function (err) {{ alert(err.message || String(err)); }});
      }});
    }});
  }}

  function load(force) {{
    statusEl.textContent = "Loading themes...";
    var url = "/api/panel/themes/catalog" + (force ? "?refresh=1" : "");
    fetch(url, {{ credentials: "same-origin", headers: {{ "Accept": "application/json" }} }})
      .then(function (res) {{
        return res.json().then(function (data) {{
          if (!res.ok) throw new Error((data && data.error) || ("HTTP " + res.status));
          render(data.themes || [], (data.repo || "") + (data.fetched_at_local ? (" - " + data.fetched_at_local) : ""));
        }});
      }})
      .catch(function (err) {{
        statusEl.textContent = "Could not load themes: " + (err.message || String(err));
        grid.innerHTML = "";
      }});
  }}

  if (refreshBtn) refreshBtn.addEventListener("click", function () {{ load(true); }});
  load(false);
}})();
</script>"##,
        can_edit = if can_edit { "true" } else { "false" },
    )
}
