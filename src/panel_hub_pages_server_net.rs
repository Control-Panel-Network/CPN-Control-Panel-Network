//! Server hub: Change Port page (DNS lives in panel_hub_pages_dns*).

use crate::panel_hubs::feature_shell;
use crate::panel_network::{network_public, preferred_listen_port_or_default};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn change_port_page(bind_port: u16, notice: Option<&str>, error: Option<&str>) -> String {
    let preferred = preferred_listen_port_or_default();
    let summary = network_public(bind_port, None);
    let public_url = summary.panel_public_url.as_deref().unwrap_or("");
    // Use r## so CSS/JS color literals like "#f87171" cannot terminate the raw string.
    let form = format!(
        r##"<ul class="kv-list">
          <li><span>Current bind</span><strong>{bind}</strong></li>
          <li><span>Preferred</span><strong>{pref}</strong></li>
          <li><span>Public base</span><strong>{base}</strong></li>
          <li><span>External URL</span><strong>{puburl}</strong></li>
        </ul>
        <form id="cpn-port-form" class="stack-form" style="max-width:420px;margin-top:16px;">
          <label for="port">New listen port</label>
          <input id="port" name="port" type="number" min="1" max="65535" value="{pref}" required>
          <label for="old_port_policy">Old port policy</label>
          <select id="old_port_policy" name="old_port_policy">
            <option value="redirect_1m">Redirect 1 month</option>
            <option value="redirect_3m">Redirect 3 months</option>
            <option value="deny">Deny old port</option>
          </select>
          <label for="panel_public_url">External panel URL (emails / NAT)</label>
          <input id="panel_public_url" name="panel_public_url" type="url" value="{puburl}" placeholder="http://127.0.0.1:2089" data-bind-port="{bind}">
          <p class="muted">Optional. Prefer this over hostname for password-reset links when DNS is private or you use VirtualBox host port forwards.</p>
          <p id="cpn-nat-hint" class="muted" role="note" style="display:none;color:#fbbf24;"></p>
          <button type="submit" class="btn-primary" id="cpn-port-save">Save port</button>
        </form>
        <p id="cpn-port-status" class="muted" role="status"></p>
        <script>
        (function(){{
          var form = document.getElementById("cpn-port-form");
          if (!form) return;
          var saveBtn = document.getElementById("cpn-port-save");
          var status = document.getElementById("cpn-port-status");
          var publicInput = document.getElementById("panel_public_url");
          var portInput = document.getElementById("port");
          var natHint = document.getElementById("cpn-nat-hint");
          function setStatus(text, isError) {{
            status.textContent = text || "";
            status.style.color = isError ? "#f87171" : "";
          }}
          function parseJsonSafe(text) {{
            if (!text || !String(text).trim()) return null;
            try {{ return JSON.parse(text); }} catch (e) {{ return null; }}
          }}
          function urlPort(raw) {{
            try {{
              var u = new URL(String(raw || "").trim());
              if (!u.port) {{
                return u.protocol === "https:" ? 443 : 80;
              }}
              return Number(u.port);
            }} catch (e) {{
              return null;
            }}
          }}
          function updateNatHint() {{
            if (!natHint || !publicInput) return;
            var listenPort = Number((portInput && portInput.value) || publicInput.getAttribute("data-bind-port") || 0);
            var external = String(publicInput.value || "").trim();
            if (!external) {{
              natHint.style.display = "none";
              natHint.textContent = "";
              return;
            }}
            var extPort = urlPort(external);
            if (!extPort || !listenPort) {{
              natHint.style.display = "none";
              natHint.textContent = "";
              return;
            }}
            if (extPort === listenPort) {{
              natHint.style.display = "block";
              natHint.style.color = "#fbbf24";
              natHint.textContent = "On VirtualBox NAT labs, the host port often differs from the guest listen port. Setting External URL to the guest port (for example 2087) can cause ERR_CONNECTION_REFUSED from Windows. Use the host forward port (for example http://127.0.0.1:2090 for clean2) unless you added a matching NAT rule.";
              return;
            }}
            natHint.style.display = "block";
            natHint.style.color = "";
            natHint.textContent = "External URL port (" + extPort + ") differs from listen port (" + listenPort + "). That is expected when VirtualBox/NAT forwards host:" + extPort + " to guest:" + listenPort + ".";
          }}
          if (publicInput) {{
            publicInput.addEventListener("input", updateNatHint);
            publicInput.addEventListener("change", updateNatHint);
          }}
          if (portInput) {{
            portInput.addEventListener("input", updateNatHint);
            portInput.addEventListener("change", updateNatHint);
          }}
          updateNatHint();
          function pollThenGo(url, attemptsLeft) {{
            if (attemptsLeft <= 0) {{
              setStatus("Restart still in progress. Open " + url + " manually when ready.", true);
              if (saveBtn) saveBtn.disabled = false;
              return;
            }}
            fetch(url, {{ method: "GET", credentials: "omit", cache: "no-store", mode: "cors" }})
              .then(function(r) {{
                if (r.ok || r.status === 401 || r.status === 302 || r.status === 303) {{
                  window.location.href = url;
                  return;
                }}
                setStatus("Waiting for panel on new port (" + attemptsLeft + ")...");
                setTimeout(function(){{ pollThenGo(url, attemptsLeft - 1); }}, 1000);
              }})
              .catch(function() {{
                setStatus("Waiting for panel restart (" + attemptsLeft + ")...");
                setTimeout(function(){{ pollThenGo(url, attemptsLeft - 1); }}, 1000);
              }});
          }}
          form.addEventListener("submit", function(ev){{
            ev.preventDefault();
            var port = Number(document.getElementById("port").value);
            var policy = document.getElementById("old_port_policy").value;
            var publicUrl = document.getElementById("panel_public_url").value;
            var extPort = urlPort(publicUrl);
            if (publicUrl && extPort && extPort === port) {{
              var okSame = window.confirm(
                "External URL uses the same port as the guest listen port (" + port + ").\\n\\n" +
                "On VirtualBox NAT, Windows often needs the host forward port instead (for example 2090 on clean2). Continue only if that host port is forwarded to the guest."
              );
              if (!okSame) {{
                setStatus("Save cancelled. Keep External URL on the host NAT port.", true);
                return;
              }}
            }}
            if (saveBtn) saveBtn.disabled = true;
            setStatus("Saving...");
            fetch("/api/listen-port", {{
              method: "POST",
              headers: {{ "Content-Type": "application/json", "Accept": "application/json" }},
              credentials: "same-origin",
              body: JSON.stringify({{ port: port, old_port_policy: policy, panel_public_url: publicUrl }})
            }}).then(function(r){{
              return r.text().then(function(t){{
                return {{ ok: r.ok, status: r.status, j: parseJsonSafe(t), raw: t }};
              }});
            }}).then(function(res){{
              var j = res.j || {{}};
              if (!res.ok) {{
                var err = (j && (j.error || j.message)) || ("Save failed (HTTP " + res.status + ")");
                if (!res.j && res.raw === "") {{
                  err = "Save failed: empty response (HTTP " + res.status + "). Sign in as panel admin and retry.";
                }}
                setStatus(err, true);
                if (saveBtn) saveBtn.disabled = false;
                return;
              }}
              var target = j.redirect_url || ((j.new_url || "").replace(/\/$/, "") + "/settings/port");
              if (!j.new_url && !j.redirect_url) {{
                setStatus(j.message || "Port preference saved.");
                if (saveBtn) saveBtn.disabled = false;
                return;
              }}
              if (j.restart_required || j.restart_scheduled) {{
                setStatus("Port saved. Restarting panel, then opening " + target + " ...");
                setTimeout(function(){{ pollThenGo(target, 45); }}, 1200);
              }} else {{
                setStatus("Saved. Opening " + target + " ...");
                setTimeout(function(){{ window.location.href = target; }}, 400);
              }}
            }}).catch(function(e){{
              setStatus(String(e && e.message ? e.message : e), true);
              if (saveBtn) saveBtn.disabled = false;
            }});
          }});
        }})();
        </script>
        <p class="muted">Uses the panel port migration API. When the listen port changes, the panel service restarts and this page opens on the new URL automatically.</p>"##,
        bind = bind_port,
        pref = preferred,
        base = html_escape(&summary.public_base_url),
        puburl = html_escape(public_url),
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Settings", Some("/settings")),
            ("Change Port", None),
        ],
        "Change Port",
        "Update the CPN panel listen port.",
        &form,
        notice,
        error,
    )
}
