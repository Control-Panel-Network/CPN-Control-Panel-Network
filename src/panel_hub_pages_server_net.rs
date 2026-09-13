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
    let form = format!(
        r#"<ul class="kv-list">
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
          <input id="panel_public_url" name="panel_public_url" type="url" value="{puburl}" placeholder="http://127.0.0.1:2089">
          <p class="muted">Optional. Prefer this over hostname for password-reset links when DNS is private or you use VirtualBox host port forwards.</p>
          <button type="submit" class="btn-primary">Save port</button>
        </form>
        <p id="cpn-port-status" class="muted" role="status"></p>
        <script>
        (function(){{
          var form = document.getElementById("cpn-port-form");
          if (!form) return;
          form.addEventListener("submit", function(ev){{
            ev.preventDefault();
            var port = Number(document.getElementById("port").value);
            var policy = document.getElementById("old_port_policy").value;
            var publicUrl = document.getElementById("panel_public_url").value;
            var status = document.getElementById("cpn-port-status");
            status.textContent = "Saving...";
            fetch("/api/listen-port", {{
              method: "POST",
              headers: {{ "Content-Type": "application/json" }},
              credentials: "same-origin",
              body: JSON.stringify({{ port: port, old_port_policy: policy, panel_public_url: publicUrl }})
            }}).then(function(r){{ return r.json().then(function(j){{ return {{ok:r.ok, j:j}}; }}); }})
              .then(function(res){{
                if (res.ok) {{
                  status.textContent = "Port preference saved. Reopen the panel on the new port if the process rebound.";
                }} else {{
                  status.textContent = (res.j && (res.j.error || res.j.message)) || "Save failed";
                }}
              }}).catch(function(e){{ status.textContent = String(e); }});
          }});
        }})();
        </script>
        <p class="muted">Uses the existing panel port migration API. Restart may be required depending on how the service is supervised.</p>"#,
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
