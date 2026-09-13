//! Nameservers and Default Nameservers pages.

use crate::panel_hubs::feature_shell;
use crate::panel_ops_dns::{dns_csrf_token, load_default_nameservers, load_ns_hosts};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn ns_styles() -> &'static str {
    r#"<style>
.ns-table{width:100%;border-collapse:collapse;margin:12px 0 18px;}
.ns-table th,.ns-table td{text-align:left;padding:10px 8px;border-bottom:1px solid rgba(255,255,255,.08);}
.ns-table th{color:var(--muted,#9aa4b2);font-size:.85rem;}
.ns-form{display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));gap:10px;max-width:720px;}
.ns-form label{display:flex;flex-direction:column;gap:4px;font-size:.85rem;}
.ns-form input{padding:8px 10px;border-radius:8px;border:1px solid rgba(255,255,255,.12);background:rgba(0,0,0,.25);color:inherit;font:inherit;}
.ns-check{display:flex;flex-direction:column;gap:8px;margin:12px 0;}
.ns-check label{display:flex;gap:10px;align-items:center;}
.ns-links{display:flex;flex-wrap:wrap;gap:8px;margin-bottom:14px;}
.btn-secondary{display:inline-flex;align-items:center;gap:6px;padding:8px 14px;border-radius:999px;border:1px solid rgba(255,255,255,.14);background:transparent;color:inherit;text-decoration:none;font:inherit;cursor:pointer;}
</style>"#
}

pub fn nameservers_manage_page(
    username: &str,
    notice: Option<&str>,
    error: Option<&str>,
) -> String {
    let csrf = dns_csrf_token(username);
    let hosts = load_ns_hosts();
    let mut rows = String::new();
    for h in &hosts {
        let host = html_escape(&h.hostname);
        let v4 = html_escape(h.ipv4.as_deref().unwrap_or("-"));
        let v6 = html_escape(h.ipv6.as_deref().unwrap_or("-"));
        rows.push_str(&format!(
            r#"<tr><td><code>{host}</code></td><td>{v4}</td><td>{v6}</td>
              <td><form method="post" action="/server/dns/nameservers/delete" style="display:inline;">
                <input type="hidden" name="csrf" value="{csrf}">
                <input type="hidden" name="hostname" value="{host}">
                <button type="submit" class="btn-danger">Delete</button>
              </form></td></tr>"#,
            csrf = html_escape(&csrf),
        ));
    }
    if rows.is_empty() {
        rows = r#"<tr><td colspan="4">No nameserver hosts yet.</td></tr>"#.into();
    }
    let body = format!(
        r#"{styles}
        <div class="ns-links">
          <a class="btn-secondary" href="/server/dns/zones">DNS Zones</a>
          <a class="btn-secondary" href="/server/dns/defaults">Default Nameservers</a>
        </div>
        <p class="muted">Create nameserver hostnames with glue A/AAAA records. Assign them to new zones on Default Nameservers.</p>
        <table class="ns-table">
          <thead><tr><th>Hostname</th><th>A (IPv4)</th><th>AAAA (IPv6)</th><th></th></tr></thead>
          <tbody>{rows}</tbody>
        </table>
        <h3>Add nameserver</h3>
        <form method="post" action="/server/dns/nameservers/add" class="ns-form">
          <input type="hidden" name="csrf" value="{csrf}">
          <label>Hostname<input name="hostname" type="text" required placeholder="ns1.example.com"></label>
          <label>Glue A<input name="ipv4" type="text" placeholder="203.0.113.10"></label>
          <label>Glue AAAA<input name="ipv6" type="text" placeholder="2001:db8::1"></label>
          <div style="grid-column:1/-1;"><button type="submit" class="btn-primary">Add nameserver</button></div>
        </form>"#,
        styles = ns_styles(),
        rows = rows,
        csrf = html_escape(&csrf),
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("Nameservers", None),
        ],
        "Nameservers",
        "Create nameserver hostnames with glue records for your DNS zones.",
        &body,
        notice,
        error,
    )
}

pub fn default_nameservers_page(
    username: &str,
    notice: Option<&str>,
    error: Option<&str>,
) -> String {
    let csrf = dns_csrf_token(username);
    let hosts = load_ns_hosts();
    let defaults = load_default_nameservers();
    let mut checks = String::new();
    if hosts.is_empty() {
        checks.push_str(
            r#"<p class="muted">No nameserver hosts yet. Add some on the Nameservers page first, or type hostnames below.</p>"#,
        );
    } else {
        for h in &hosts {
            let host = html_escape(&h.hostname);
            let checked = if defaults.iter().any(|d| d.eq_ignore_ascii_case(&h.hostname)) {
                " checked"
            } else {
                ""
            };
            checks.push_str(&format!(
                r#"<label><input type="checkbox" name="ns" value="{host}"{checked}> <code>{host}</code></label>"#
            ));
        }
    }
    let joined = defaults.join("\n");
    let body = format!(
        r#"{styles}
        <div class="ns-links">
          <a class="btn-secondary" href="/server/dns/zones">DNS Zones</a>
          <a class="btn-secondary" href="/server/dns/nameservers">Nameservers</a>
        </div>
        <p class="muted">These nameservers are written into new zones (SOA primary + NS records).</p>
        <form id="cpn-dns-defaults-form" method="post" action="/server/dns/defaults/save">
          <input type="hidden" name="csrf" value="{csrf}">
          <input type="hidden" name="nameservers" id="cpn-dns-defaults-ns" value="">
          <div class="ns-check">{checks}</div>
          <label for="extra" style="display:block;margin:12px 0 6px;">Nameserver list (one per line; checkboxes sync into this field)</label>
          <textarea id="extra" name="extra" rows="4" style="width:100%;max-width:560px;font:inherit;">{extra}</textarea>
          <div style="margin-top:12px;"><button type="submit" class="btn-primary">Save default nameservers</button></div>
        </form>
        <script>
        (function(){{
          var form = document.getElementById("cpn-dns-defaults-form");
          var extra = document.getElementById("extra");
          var hidden = document.getElementById("cpn-dns-defaults-ns");
          if (!form || !extra) return;
          function syncFromChecks(){{
            var hostBoxes = form.querySelectorAll('input[type=checkbox][name=ns]');
            var known = {{}};
            hostBoxes.forEach(function(b){{ known[b.value.toLowerCase()] = true; }});
            var lines = [];
            hostBoxes.forEach(function(b){{ if (b.checked && b.value) lines.push(b.value); }});
            (extra.value || "").split(/\\r?\\n/).forEach(function(line){{
              line = (line || "").trim();
              if (!line) return;
              if (known[line.toLowerCase()]) return;
              if (lines.indexOf(line) < 0) lines.push(line);
            }});
            extra.value = lines.join("\\n");
            if (hidden) hidden.value = lines.join("\\n");
          }}
          form.querySelectorAll('input[type=checkbox][name=ns]').forEach(function(b){{
            b.addEventListener("change", syncFromChecks);
          }});
          form.addEventListener("submit", syncFromChecks);
        }})();
        </script>"#,
        styles = ns_styles(),
        csrf = html_escape(&csrf),
        checks = checks,
        extra = html_escape(&joined),
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("Default Nameservers", None),
        ],
        "Default Nameservers",
        "Configure which nameservers are assigned when a new DNS zone is created.",
        &body,
        notice,
        error,
    )
}
