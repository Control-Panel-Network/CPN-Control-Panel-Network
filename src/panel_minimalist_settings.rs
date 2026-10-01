//! Settings > Design: Minimalist mode toggle (per-user).

use crate::panel_user_prefs::load_user_minimalist_mode;

/// Shared fetch helper fragment for Design page scripts (relative same-origin APIs).
pub fn design_fetch_js_helpers() -> &'static str {
    r#"
  function cpnDesignNetworkError(err) {
    var msg = (err && err.message) ? String(err.message) : String(err || "Request failed");
    if (/failed to fetch|networkerror|load failed|abort/i.test(msg)) {
      return "Cannot reach the panel API on " + window.location.origin +
        ". If the listen port changed, open the new panel URL and try again.";
    }
    return msg;
  }
  function cpnDesignFetchJson(url, options, attempt) {
    attempt = attempt || 0;
    var opts = options || {};
    opts.credentials = "same-origin";
    opts.headers = opts.headers || {};
    if (!opts.headers.Accept) opts.headers.Accept = "application/json";
    var ctrl = null;
    if (typeof AbortController !== "undefined" && !opts.signal) {
      ctrl = new AbortController();
      opts.signal = ctrl.signal;
      setTimeout(function () { try { ctrl.abort(); } catch (e) {} }, 20000);
    }
    return fetch(url, opts).then(function (res) {
      return res.text().then(function (text) {
        var data = null;
        if (text) {
          try { data = JSON.parse(text); } catch (e) {
            throw new Error(res.ok ? "Invalid JSON from panel API" : ("HTTP " + res.status));
          }
        }
        if (!res.ok) throw new Error((data && data.error) || ("HTTP " + res.status));
        return data || {};
      });
    }).catch(function (err) {
      if (attempt < 1 && /failed to fetch|networkerror|load failed|abort/i.test(String(err && err.message || err))) {
        return cpnDesignFetchJson(url, options, attempt + 1);
      }
      throw new Error(cpnDesignNetworkError(err));
    });
  }
"#
}

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
{helpers}
  var mini = document.getElementById("cpn-minimalist-mode");
  var miniStatus = document.getElementById("cpn-minimalist-status");
  if (!mini) return;
  mini.addEventListener("change", function () {{
    var enabled = !!mini.checked;
    if (miniStatus) miniStatus.textContent = "Saving...";
    cpnDesignFetchJson("/api/panel/minimalist-mode", {{
      method: "POST",
      headers: {{ "Content-Type": "application/json", "Accept": "application/json" }},
      body: JSON.stringify({{ minimalist_mode: enabled }})
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
        helpers = design_fetch_js_helpers(),
    )
}
