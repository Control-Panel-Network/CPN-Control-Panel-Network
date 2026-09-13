//! Settings > Design: Minimalist mode toggle (per-user).

use crate::panel_user_prefs::load_user_minimalist_mode;

/// Card + save script for Minimalist mode on `/settings/design`.
pub fn minimalist_settings_card(username: &str) -> String {
    let minimalist = load_user_minimalist_mode(username);
    let checked = if minimalist { " checked" } else { "" };
    format!(
        r#"<article class="section-card cpn-minimalist-inline" id="cpn-minimalist-card">
  <h2>Minimalist mode</h2>
  <p class="manage-muted">Static UI with less CPU and RAM. Live host graphs on site Overview update only when you refresh the page (no 10s polling).</p>
  <label class="cpn-minimalist-toggle">
    <input type="checkbox" id="cpn-minimalist-mode"{checked}>
    Enable Minimalist mode
  </label>
  <p class="manage-muted" id="cpn-minimalist-status" aria-live="polite"></p>
</article>
<style>
.cpn-minimalist-inline {{ max-width:520px; margin-bottom:16px; display:grid; gap:10px; }}
.cpn-minimalist-inline h2 {{ margin:0; font-size:18px; }}
.cpn-minimalist-toggle {{ display:flex; align-items:center; gap:10px; font-weight:600; cursor:pointer; }}
</style>
<script>
(function () {{
  var mini = document.getElementById("cpn-minimalist-mode");
  var miniStatus = document.getElementById("cpn-minimalist-status");
  if (!mini) return;
  mini.addEventListener("change", function () {{
    var enabled = !!mini.checked;
    if (miniStatus) miniStatus.textContent = "Saving...";
    fetch("/api/panel/minimalist-mode", {{
      method: "POST",
      credentials: "same-origin",
      headers: {{ "Content-Type": "application/json", "Accept": "application/json" }},
      body: JSON.stringify({{ minimalist_mode: enabled }})
    }}).then(function (res) {{
      return res.json().then(function (data) {{
        if (!res.ok) throw new Error((data && data.error) || ("HTTP " + res.status));
        return data;
      }});
    }}).then(function (data) {{
      if (miniStatus) {{
        miniStatus.textContent = data.minimalist_mode
          ? "Minimalist mode on. Site Overview charts stay as a snapshot until you refresh."
          : "Minimalist mode off. Live Overview metrics resume on the next page load.";
      }}
    }}).catch(function (err) {{
      mini.checked = !enabled;
      if (miniStatus) miniStatus.textContent = err.message || String(err);
    }});
  }});
}})();
</script>"#,
        checked = checked,
    )
}
