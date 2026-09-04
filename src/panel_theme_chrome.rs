//! Sidebar color-mode toggle script and Manage Design editor markup.

use crate::panel_admin::is_panel_admin;
use crate::panel_theme::{
    COLOR_MODE_STORAGE_KEY, ColorMode, DesignPreset, DesignTokens, PanelDesignFile, default_tokens,
    design_public_json, load_panel_design, resolve_tokens,
};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn sidebar_theme_toggle(color_mode: ColorMode) -> String {
    let next_label = match color_mode {
        ColorMode::Light => "Dark mode",
        ColorMode::Dark => "Light mode",
    };
    let icon = match color_mode {
        ColorMode::Light => "&#9790;",
        ColorMode::Dark => "&#9788;",
    };
    format!(
        r#"<button type="button" id="cpn-color-toggle" class="theme-toggle"
          aria-pressed="{pressed}" data-mode="{mode}" title="Toggle light and dark mode">
          <span class="theme-toggle-icon" aria-hidden="true">{icon}</span>
          <span class="theme-toggle-label">{label}</span>
        </button>"#,
        pressed = if color_mode == ColorMode::Dark {
            "true"
        } else {
            "false"
        },
        mode = color_mode.as_str(),
        icon = icon,
        label = next_label,
    )
}

pub fn color_mode_boot_script(color_mode: ColorMode) -> String {
    format!(
        r#"<script>
(function () {{
  var KEY = "{key}";
  var serverMode = "{mode}";
  try {{
    var stored = window.localStorage.getItem(KEY);
    if (stored === "light" || stored === "dark") {{
      document.documentElement.setAttribute("data-color-mode", stored);
      if (document.body) document.body.setAttribute("data-color-mode", stored);
    }} else {{
      document.documentElement.setAttribute("data-color-mode", serverMode);
      window.localStorage.setItem(KEY, serverMode);
    }}
  }} catch (e) {{
    document.documentElement.setAttribute("data-color-mode", serverMode);
  }}
}})();
</script>"#,
        key = COLOR_MODE_STORAGE_KEY,
        mode = color_mode.as_str(),
    )
}

pub fn color_mode_toggle_script() -> &'static str {
    r#"
<script>
(function () {
  var KEY = "cpn-color-mode";
  var btn = document.getElementById("cpn-color-toggle");
  if (!btn) return;

  function applyMode(mode) {
    document.documentElement.setAttribute("data-color-mode", mode);
    document.body.setAttribute("data-color-mode", mode);
    btn.setAttribute("data-mode", mode);
    btn.setAttribute("aria-pressed", mode === "dark" ? "true" : "false");
    var label = btn.querySelector(".theme-toggle-label");
    var icon = btn.querySelector(".theme-toggle-icon");
    if (label) label.textContent = mode === "dark" ? "Light mode" : "Dark mode";
    if (icon) icon.innerHTML = mode === "dark" ? "&#9788;" : "&#9790;";
    try { window.localStorage.setItem(KEY, mode); } catch (e) {}
  }

  function currentMode() {
    return document.body.getAttribute("data-color-mode") === "dark" ? "dark" : "light";
  }

  btn.addEventListener("click", function () {
    var next = currentMode() === "dark" ? "light" : "dark";
    applyMode(next);
    fetch("/api/panel/color-mode", {
      method: "POST",
      credentials: "same-origin",
      headers: { "Content-Type": "application/json", "Accept": "application/json" },
      body: JSON.stringify({ color_mode: next })
    }).catch(function () {});
  });
})();
</script>
"#
}

fn token_fields(tokens: &DesignTokens) -> String {
    format!(
        r#"<label>Accent <input id="cpn-design-accent" type="color" value="{accent}"></label>
<label>Accent focus <input id="cpn-design-accent-focus" type="color" value="{focus}"></label>
<label>Corner radius (px)
  <input id="cpn-design-radius" type="number" min="8" max="28" value="{radius}">
</label>
<label>Density
  <select id="cpn-design-density">
    <option value="comfortable"{c_sel}>Comfortable</option>
    <option value="compact"{k_sel}>Compact</option>
  </select>
</label>
<label>Font scale
  <input id="cpn-design-font-scale" type="number" min="0.9" max="1.25" step="0.01" value="{scale}">
</label>"#,
        accent = html_escape(&tokens.accent),
        focus = html_escape(&tokens.accent_focus),
        radius = tokens.radius_px,
        c_sel = if tokens.density == "comfortable" {
            " selected"
        } else {
            ""
        },
        k_sel = if tokens.density == "compact" {
            " selected"
        } else {
            ""
        },
        scale = tokens.font_scale,
    )
}

/// Inline Design editor for Settings > Design (same APIs as Manage modal).
pub fn design_settings_panel(username: &str) -> String {
    let design = load_panel_design();
    let tokens = resolve_tokens(&design);
    let can_edit = is_panel_admin(username);
    let preset = design.preset.as_str();
    let fields = token_fields(&tokens);
    let edit_disabled = if can_edit { "" } else { " disabled" };
    let save_row = if can_edit {
        r#"<div class="cpn-design-actions">
  <button type="button" class="manage-btn primary" id="cpn-design-save">Save custom</button>
  <button type="button" class="manage-btn" id="cpn-design-restore">Restore Default</button>
</div>
<p class="manage-muted">Custom saves under /var/lib/cpn/panel-design.json. Default stays an immutable built-in baseline. Optional free-form custom CSS is not stored yet; use token presets above.</p>"#
            .to_string()
    } else {
        r#"<p class="manage-muted">Only the panel admin can change Design. You can still use the sidebar light/dark toggle for your own session.</p>"#
            .to_string()
    };

    format!(
        r#"<article class="section-card cpn-design-inline" id="cpn-design-dialog">
  <div class="cpn-design-panel">
    <header>
      <h2>Panel Design</h2>
    </header>
    <p class="manage-muted">Active preset: <strong id="cpn-design-active-preset">{preset}</strong></p>
    <div class="cpn-design-presets" role="group" aria-label="Design presets">
      <button type="button" class="manage-btn{def_active}" data-preset="default"{edit_disabled}>Default</button>
      <button type="button" class="manage-btn{light_active}" data-preset="light"{edit_disabled}>Light</button>
      <button type="button" class="manage-btn{dark_active}" data-preset="dark"{edit_disabled}>Dark</button>
      <button type="button" class="manage-btn{custom_active}" data-preset="custom"{edit_disabled}>Custom</button>
    </div>
    <div class="cpn-design-fields">{fields}</div>
    {save_row}
  </div>
</article>
<style>
.cpn-design-inline {{ max-width:520px; }}
.cpn-design-panel {{ padding:4px 0 8px; display:grid; gap:12px; }}
.cpn-design-panel header {{ display:flex; align-items:center; justify-content:space-between; gap:12px; }}
.cpn-design-panel h2 {{ margin:0; font-size:18px; }}
.cpn-design-presets {{ display:flex; flex-wrap:wrap; gap:8px; }}
.cpn-design-presets .manage-btn.active {{ outline:2px solid var(--cpn-accent, var(--blue)); }}
.cpn-design-fields {{ display:grid; gap:10px; }}
.cpn-design-fields label {{ display:grid; gap:4px; font-size:13px; font-weight:600; }}
.cpn-design-fields input, .cpn-design-fields select {{
  min-height:40px; border-radius:10px; border:1px solid var(--hairline);
  background:var(--canvas); color:inherit; padding:0 10px; font:inherit;
}}
.cpn-design-actions {{ display:flex; flex-wrap:wrap; gap:8px; }}
.manage-btn {{
  min-height:36px; padding:0 12px; border-radius:10px; border:1px solid var(--hairline);
  background:var(--canvas); color:inherit; font:inherit; cursor:pointer;
}}
.manage-btn.primary {{ background:var(--blue); color:#fff; border-color:transparent; }}
.manage-muted {{ color:var(--muted); font-size:13px; line-height:1.45; margin:0; }}
</style>
<script>
(function () {{
  var root = document.getElementById("cpn-design-dialog");
  if (!root) return;
  var canEdit = {can_edit};

  function readTokens() {{
    return {{
      accent: document.getElementById("cpn-design-accent").value,
      accent_focus: document.getElementById("cpn-design-accent-focus").value,
      radius_px: Number(document.getElementById("cpn-design-radius").value),
      density: document.getElementById("cpn-design-density").value,
      font_scale: Number(document.getElementById("cpn-design-font-scale").value)
    }};
  }}

  function setActivePreset(preset) {{
    var label = document.getElementById("cpn-design-active-preset");
    if (label) label.textContent = preset;
    root.querySelectorAll("[data-preset]").forEach(function (el) {{
      el.classList.toggle("active", el.getAttribute("data-preset") === preset);
    }});
  }}

  function applyServerTokens(payload) {{
    if (!payload || !payload.tokens) return;
    var t = payload.tokens;
    document.getElementById("cpn-design-accent").value = t.accent;
    document.getElementById("cpn-design-accent-focus").value = t.accent_focus;
    document.getElementById("cpn-design-radius").value = t.radius_px;
    document.getElementById("cpn-design-density").value = t.density;
    document.getElementById("cpn-design-font-scale").value = t.font_scale;
    setActivePreset(payload.preset || "default");
  }}

  function postJson(url, body) {{
    return fetch(url, {{
      method: "POST",
      credentials: "same-origin",
      headers: {{ "Content-Type": "application/json", "Accept": "application/json" }},
      body: JSON.stringify(body || {{}})
    }}).then(function (res) {{
      return res.json().then(function (data) {{
        if (!res.ok) throw new Error((data && data.error) || ("HTTP " + res.status));
        return data;
      }});
    }});
  }}

  root.querySelectorAll("[data-preset]").forEach(function (btn) {{
    btn.addEventListener("click", function () {{
      if (!canEdit) return;
      var preset = btn.getAttribute("data-preset");
      postJson("/api/panel/design/preset", {{ preset: preset }})
        .then(function (data) {{ applyServerTokens(data); window.location.reload(); }})
        .catch(function (err) {{ alert(err.message || String(err)); }});
    }});
  }});

  var saveBtn = document.getElementById("cpn-design-save");
  if (saveBtn) {{
    saveBtn.addEventListener("click", function () {{
      postJson("/api/panel/design", {{ tokens: readTokens() }})
        .then(function () {{ window.location.reload(); }})
        .catch(function (err) {{ alert(err.message || String(err)); }});
    }});
  }}

  var restoreBtn = document.getElementById("cpn-design-restore");
  if (restoreBtn) {{
    restoreBtn.addEventListener("click", function () {{
      if (!window.confirm("Restore immutable Default design and clear saved Custom?")) return;
      postJson("/api/panel/design/restore", {{}})
        .then(function () {{ window.location.reload(); }})
        .catch(function (err) {{ alert(err.message || String(err)); }});
    }});
  }}

  setActivePreset("{preset}");
}})();
</script>"#,
        preset = html_escape(preset),
        fields = fields,
        save_row = save_row,
        edit_disabled = edit_disabled,
        can_edit = if can_edit { "true" } else { "false" },
        def_active = if design.preset == DesignPreset::Default {
            " active"
        } else {
            ""
        },
        light_active = if design.preset == DesignPreset::Light {
            " active"
        } else {
            ""
        },
        dark_active = if design.preset == DesignPreset::Dark {
            " active"
        } else {
            ""
        },
        custom_active = if design.preset == DesignPreset::Custom {
            " active"
        } else {
            ""
        },
    )
}

/// Design button + modal for Manage (admin can edit; others see read-only status).
pub fn manage_design_controls(username: &str) -> String {
    let design = load_panel_design();
    let tokens = resolve_tokens(&design);
    let can_edit = is_panel_admin(username);
    let preset = design.preset.as_str();
    let fields = token_fields(&tokens);
    let edit_disabled = if can_edit { "" } else { " disabled" };
    let save_row = if can_edit {
        r#"<div class="cpn-design-actions">
  <button type="button" class="manage-btn primary" id="cpn-design-save">Save custom</button>
  <button type="button" class="manage-btn" id="cpn-design-restore">Restore Default</button>
</div>
<p class="manage-muted">Custom saves under /var/lib/cpn/panel-design.json. Default stays an immutable built-in baseline. Scope: panel-wide chrome and Manage (not per-site branding).</p>"#
            .to_string()
    } else {
        r#"<p class="manage-muted">Only the panel admin can change Design. You can still use the sidebar light/dark toggle for your own session.</p>"#
            .to_string()
    };

    format!(
        r#"<button type="button" class="manage-btn" id="cpn-design-open" aria-haspopup="dialog">Design</button>
<dialog id="cpn-design-dialog" class="cpn-design-dialog">
  <form method="dialog" class="cpn-design-panel">
    <header>
      <h2>Panel Design</h2>
      <button type="submit" class="manage-btn" value="cancel" aria-label="Close">Close</button>
    </header>
    <p class="manage-muted">Active preset: <strong id="cpn-design-active-preset">{preset}</strong></p>
    <div class="cpn-design-presets" role="group" aria-label="Design presets">
      <button type="button" class="manage-btn{def_active}" data-preset="default"{edit_disabled}>Default</button>
      <button type="button" class="manage-btn{light_active}" data-preset="light"{edit_disabled}>Light</button>
      <button type="button" class="manage-btn{dark_active}" data-preset="dark"{edit_disabled}>Dark</button>
      <button type="button" class="manage-btn{custom_active}" data-preset="custom"{edit_disabled}>Custom</button>
    </div>
    <div class="cpn-design-fields">{fields}</div>
    {save_row}
  </form>
</dialog>
<style>
.cpn-design-dialog {{ border:1px solid var(--m-line,#2a2f3a); border-radius:16px; padding:0;
  background:var(--m-card,#1b1e27); color:var(--m-ink,#f2f4f7); max-width:440px; width:calc(100% - 24px); }}
.cpn-design-panel {{ padding:18px 18px 16px; display:grid; gap:12px; }}
.cpn-design-panel header {{ display:flex; align-items:center; justify-content:space-between; gap:12px; }}
.cpn-design-panel h2 {{ margin:0; font-size:18px; }}
.cpn-design-presets {{ display:flex; flex-wrap:wrap; gap:8px; }}
.cpn-design-presets .manage-btn.active {{ outline:2px solid var(--m-accent,#3b82f6); }}
.cpn-design-fields {{ display:grid; gap:10px; }}
.cpn-design-fields label {{ display:grid; gap:4px; font-size:13px; font-weight:600; }}
.cpn-design-fields input, .cpn-design-fields select {{
  min-height:40px; border-radius:10px; border:1px solid var(--m-line,#2a2f3a);
  background:#0b0d12; color:inherit; padding:0 10px; font:inherit;
}}
.cpn-design-actions {{ display:flex; flex-wrap:wrap; gap:8px; }}
</style>
<script>
(function () {{
  var openBtn = document.getElementById("cpn-design-open");
  var dialog = document.getElementById("cpn-design-dialog");
  if (!openBtn || !dialog) return;
  var canEdit = {can_edit};
  openBtn.addEventListener("click", function () {{ dialog.showModal(); }});

  function readTokens() {{
    return {{
      accent: document.getElementById("cpn-design-accent").value,
      accent_focus: document.getElementById("cpn-design-accent-focus").value,
      radius_px: Number(document.getElementById("cpn-design-radius").value),
      density: document.getElementById("cpn-design-density").value,
      font_scale: Number(document.getElementById("cpn-design-font-scale").value)
    }};
  }}

  function setActivePreset(preset) {{
    var label = document.getElementById("cpn-design-active-preset");
    if (label) label.textContent = preset;
    dialog.querySelectorAll("[data-preset]").forEach(function (el) {{
      el.classList.toggle("active", el.getAttribute("data-preset") === preset);
    }});
  }}

  function applyServerTokens(payload) {{
    if (!payload || !payload.tokens) return;
    var t = payload.tokens;
    document.getElementById("cpn-design-accent").value = t.accent;
    document.getElementById("cpn-design-accent-focus").value = t.accent_focus;
    document.getElementById("cpn-design-radius").value = t.radius_px;
    document.getElementById("cpn-design-density").value = t.density;
    document.getElementById("cpn-design-font-scale").value = t.font_scale;
    setActivePreset(payload.preset || "default");
  }}

  function postJson(url, body) {{
    return fetch(url, {{
      method: "POST",
      credentials: "same-origin",
      headers: {{ "Content-Type": "application/json", "Accept": "application/json" }},
      body: JSON.stringify(body || {{}})
    }}).then(function (res) {{
      return res.json().then(function (data) {{
        if (!res.ok) throw new Error((data && data.error) || ("HTTP " + res.status));
        return data;
      }});
    }});
  }}

  dialog.querySelectorAll("[data-preset]").forEach(function (btn) {{
    btn.addEventListener("click", function () {{
      if (!canEdit) return;
      var preset = btn.getAttribute("data-preset");
      postJson("/api/panel/design/preset", {{ preset: preset }})
        .then(function (data) {{ applyServerTokens(data); window.location.reload(); }})
        .catch(function (err) {{ alert(err.message || String(err)); }});
    }});
  }});

  var saveBtn = document.getElementById("cpn-design-save");
  if (saveBtn) {{
    saveBtn.addEventListener("click", function () {{
      postJson("/api/panel/design", {{ tokens: readTokens() }})
        .then(function () {{ window.location.reload(); }})
        .catch(function (err) {{ alert(err.message || String(err)); }});
    }});
  }}

  var restoreBtn = document.getElementById("cpn-design-restore");
  if (restoreBtn) {{
    restoreBtn.addEventListener("click", function () {{
      if (!window.confirm("Restore immutable Default design and clear saved Custom?")) return;
      postJson("/api/panel/design/restore", {{}})
        .then(function () {{ window.location.reload(); }})
        .catch(function (err) {{ alert(err.message || String(err)); }});
    }});
  }}

  setActivePreset("{preset}");
}})();
</script>"#,
        preset = html_escape(preset),
        fields = fields,
        save_row = save_row,
        edit_disabled = edit_disabled,
        can_edit = if can_edit { "true" } else { "false" },
        def_active = if design.preset == DesignPreset::Default {
            " active"
        } else {
            ""
        },
        light_active = if design.preset == DesignPreset::Light {
            " active"
        } else {
            ""
        },
        dark_active = if design.preset == DesignPreset::Dark {
            " active"
        } else {
            ""
        },
        custom_active = if design.preset == DesignPreset::Custom {
            " active"
        } else {
            ""
        },
    )
}

#[allow(dead_code)]
pub fn design_snapshot_for_tests() -> serde_json::Value {
    let design = PanelDesignFile::default();
    assert_eq!(resolve_tokens(&design), default_tokens());
    design_public_json(&design)
}
