//! Server hub: Change Port and DNS feature pages.

use crate::panel_hubs::feature_shell;
use crate::panel_network::{network_public, preferred_listen_port_or_default};
use crate::panel_ops_dns::{
    delete_zone, list_zones, load_nameservers, save_nameservers, write_zone,
};

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
    let form = format!(
        r#"<ul class="kv-list">
          <li><span>Current bind</span><strong>{bind}</strong></li>
          <li><span>Preferred</span><strong>{pref}</strong></li>
          <li><span>Public base</span><strong>{base}</strong></li>
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
            var status = document.getElementById("cpn-port-status");
            status.textContent = "Saving...";
            fetch("/api/listen-port", {{
              method: "POST",
              headers: {{ "Content-Type": "application/json" }},
              credentials: "same-origin",
              body: JSON.stringify({{ port: port, old_port_policy: policy }})
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

pub fn dns_zones_page(notice: Option<&str>, error: Option<&str>) -> String {
    let zones = list_zones().unwrap_or_default();
    let mut list = String::from("<ul>");
    if zones.is_empty() {
        list = "<p class=\"empty-state\">No zones yet.</p>".into();
    } else {
        for z in &zones {
            list.push_str(&format!(
                r#"<li><code>{z}</code>
              <form method="post" action="/server/dns/zones/delete" class="inline-form" style="display:inline;margin-left:8px;">
                <input type="hidden" name="name" value="{z}">
                <button type="submit" class="btn-danger">Delete</button>
              </form></li>"#,
                z = html_escape(z),
            ));
        }
        list.push_str("</ul>");
    }
    let form = r#"<form method="post" action="/server/dns/zones/save" class="stack-form" style="max-width:640px;margin-top:16px;">
      <label for="name">Zone name</label>
      <input id="name" name="name" type="text" required placeholder="example.com">
      <label for="content">Zone file</label>
      <textarea id="content" name="content" rows="8" style="width:100%;font:inherit;" placeholder="example.com. IN A 203.0.113.10"></textarea>
      <button type="submit" class="btn-primary">Save zone</button>
    </form>"#;
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("DNS Zones", None),
        ],
        "DNS Zones",
        "Zone files stored under the CPN data directory.",
        &format!("{list}{form}"),
        notice,
        error,
    )
}

pub fn save_dns_zone(name: &str, content: &str) -> Result<String, String> {
    write_zone(name, content)?;
    Ok(format!("Saved zone {}", name.trim()))
}

pub fn remove_dns_zone(name: &str) -> Result<String, String> {
    delete_zone(name)?;
    Ok(format!("Deleted zone {}", name.trim()))
}

pub fn nameservers_page(notice: Option<&str>, error: Option<&str>, defaults: bool) -> String {
    let ns = load_nameservers();
    let joined = ns.join("\n");
    let title = if defaults {
        "Default Nameservers"
    } else {
        "Nameservers"
    };
    let form = format!(
        r#"<form method="post" action="/server/dns/nameservers/save" class="stack-form" style="max-width:560px;">
      <label for="nameservers">One nameserver per line</label>
      <textarea id="nameservers" name="nameservers" rows="6" style="width:100%;font:inherit;">{joined}</textarea>
      <button type="submit" class="btn-primary">Save nameservers</button>
    </form>
    <p class="muted">Stored as JSON under the CPN data dir. Wire to PowerDNS or BIND in a later release.</p>"#,
        joined = html_escape(&joined),
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            (title, None),
        ],
        title,
        "Configure nameserver defaults for this node.",
        &form,
        notice,
        error,
    )
}

pub fn save_ns_lines(raw: &str) -> Result<String, String> {
    let values: Vec<String> = raw
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    save_nameservers(&values)?;
    Ok(format!("Saved {} nameserver(s)", values.len()))
}
