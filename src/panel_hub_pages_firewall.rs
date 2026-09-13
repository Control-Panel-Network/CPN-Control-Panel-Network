//! Firewall manager HTML (rules, banned IPs, trusted never-block).

use crate::panel_firewall_store::{BannedIp, FirewallRule, TrustedIp};
use crate::panel_hubs::{feature_shell, status_kv};
use crate::panel_ops_firewall::{
    firewall_csrf_token, panel_listen_port, prepare_manager, status_on, trusted_source_label,
};
use crate::panel_ops_security::firewall_status;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn fmt_ts(secs: u64) -> String {
    // Norwegian-facing dd/mm/yyyy HH:MM from unix seconds (UTC wall clock).
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let hours = rem / 3_600;
    let mins = (rem % 3_600) / 60;
    let (y, m, d) = civil_from_days(days as i64);
    format!("{d:02}/{m:02}/{y} {hours:02}:{mins:02}")
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    // Howard Hinnant civil_from_days (proleptic Gregorian).
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

fn tabs(active: &str) -> String {
    let items = [
        ("rules", "Firewall Rules", "/security/firewall?tab=rules"),
        ("banned", "Banned IPs", "/security/firewall?tab=banned"),
        (
            "trusted",
            "SSH trusted IPs",
            "/security/firewall?tab=trusted",
        ),
    ];
    let mut out = String::from(
        r#"<nav class="fw-tabs" style="display:flex;gap:8px;flex-wrap:wrap;margin:16px 0;">"#,
    );
    for (id, label, href) in items {
        let style = if id == active {
            "background:var(--accent,#7c3aed);color:#fff;border-color:transparent;"
        } else {
            "background:transparent;"
        };
        out.push_str(&format!(
            r#"<a class="btn-secondary" href="{href}" style="min-height:36px;padding:0 14px;border-radius:8px;{style}">{label}</a>"#,
            href = href,
            label = label,
            style = style,
        ));
    }
    out.push_str("</nav>");
    out
}

fn status_bar(is_admin: bool, csrf: &str) -> String {
    let on = status_on();
    let st = firewall_status();
    let badge = if on {
        r#"<span style="color:#12b76a;font-weight:700;">● ON</span>"#
    } else {
        r#"<span style="color:#f79009;font-weight:700;">● OFF</span>"#
    };
    let controls = if is_admin {
        format!(
            r#"
      <div style="display:flex;gap:8px;flex-wrap:wrap;margin-top:12px;">
        <form method="post" action="/security/firewall/start" style="margin:0;">
          <input type="hidden" name="csrf" value="{csrf}">
          <button type="submit" class="btn-primary" style="background:#12b76a;border:none;">Start</button>
        </form>
        <form method="post" action="/security/firewall/stop" style="margin:0;">
          <input type="hidden" name="csrf" value="{csrf}">
          <button type="submit" class="btn-warn" style="background:#f79009;color:#111;border:none;">Stop</button>
        </form>
        <form method="post" action="/security/firewall/reload" style="margin:0;">
          <input type="hidden" name="csrf" value="{csrf}">
          <button type="submit" class="btn-secondary" style="background:#2e90fa;color:#fff;border:none;">Reload</button>
        </form>
      </div>"#,
            csrf = html_escape(csrf),
        )
    } else {
        "<p class=\"muted\">Only the panel admin can start, stop, or change firewall rules.</p>"
            .into()
    };
    let kv = status_kv(&[
        ("Backend", &st.backend),
        ("Active", if st.active { "yes" } else { "no" }),
        ("Panel port", &panel_listen_port().to_string()),
    ]);
    format!(
        r#"<div class="fw-status" style="padding:14px;border:1px solid var(--border,#334155);border-radius:10px;margin-bottom:8px;">
      <div style="display:flex;justify-content:space-between;align-items:center;gap:12px;flex-wrap:wrap;">
        <strong>Firewall Status</strong>{badge}
      </div>
      {kv}{controls}
      <p class="muted" style="margin:10px 0 0;">CPN keeps the panel listen port open and never bans the server IP or first admin IP.</p>
    </div>"#
    )
}

fn rules_tab(rules: &[FirewallRule], is_admin: bool, csrf: &str) -> String {
    let mut body = String::from(
        r#"<div style="display:flex;justify-content:space-between;align-items:center;gap:8px;flex-wrap:wrap;margin:8px 0;">
      <h3 style="margin:0;">Firewall Rules</h3>
      <div style="display:flex;gap:8px;">
        <a class="btn-secondary" href="/security/firewall/export/rules">Export Rules</a>
      </div>
    </div>"#,
    );
    if is_admin {
        body.push_str(&format!(
            r#"
      <form method="post" action="/security/firewall/rules/add" class="stack-form" style="display:grid;grid-template-columns:repeat(auto-fit,minmax(140px,1fr));gap:10px;align-items:end;margin:12px 0;padding:12px;border:1px solid var(--border,#334155);border-radius:10px;">
        <input type="hidden" name="csrf" value="{csrf}">
        <label>Rule name<input name="name" required maxlength="64" placeholder="Allow SSH"></label>
        <label>Protocol<select name="protocol"><option value="tcp">TCP</option><option value="udp">UDP</option></select></label>
        <label>IP / CIDR<input name="source" value="0.0.0.0/0" maxlength="64"></label>
        <label>Port<input name="port" required maxlength="11" placeholder="80"></label>
        <button type="submit" class="btn-primary">+ Add Rule</button>
      </form>
      <form method="post" action="/security/firewall/rules/import" enctype="application/x-www-form-urlencoded" class="stack-form" style="margin:8px 0;">
        <input type="hidden" name="csrf" value="{csrf}">
        <label>Import rules JSON
          <textarea name="payload" rows="3" maxlength="100000" placeholder='[{{"id":"...","name":"http","protocol":"tcp","port":"80","source":"0.0.0.0/0"}}]'></textarea>
        </label>
        <button type="submit" class="btn-secondary">Import Rules</button>
      </form>"#,
            csrf = html_escape(csrf),
        ));
    }
    if rules.is_empty() {
        body.push_str("<p class=\"muted\">No CPN-managed rules yet.</p>");
        return body;
    }
    body.push_str(
        r#"<div class="table-wrap"><table class="data-table"><thead><tr>
      <th>Rule name</th><th>Protocol</th><th>IP address</th><th>Port</th><th>Actions</th>
    </tr></thead><tbody>"#,
    );
    for r in rules {
        let actions = if is_admin {
            format!(
                r#"<form method="post" action="/security/firewall/rules/delete" style="display:inline;" onsubmit="return confirm('Delete this rule?');">
              <input type="hidden" name="csrf" value="{csrf}">
              <input type="hidden" name="id" value="{id}">
              <button type="submit" class="btn-danger">Delete</button>
            </form>"#,
                csrf = html_escape(csrf),
                id = html_escape(&r.id),
            )
        } else {
            String::new()
        };
        body.push_str(&format!(
            r#"<tr><td>{name}</td><td><code>{proto}</code></td><td><code>{src}</code></td><td><code>{port}</code></td><td>{actions}</td></tr>"#,
            name = html_escape(&r.name),
            proto = html_escape(&r.protocol.to_ascii_uppercase()),
            src = html_escape(&r.source),
            port = html_escape(&r.port),
            actions = actions,
        ));
    }
    body.push_str("</tbody></table></div>");
    body
}

fn banned_tab(banned: &[BannedIp], is_admin: bool, csrf: &str, q: &str) -> String {
    let mut body = String::from(
        r#"<div style="display:flex;justify-content:space-between;align-items:center;gap:8px;flex-wrap:wrap;margin:8px 0;">
      <h3 style="margin:0;">Banned IP Addresses</h3>
      <a class="btn-secondary" href="/security/firewall/export/banned">Export Banned IPs</a>
    </div>"#,
    );
    if is_admin {
        body.push_str(&format!(
            r#"
      <form method="post" action="/security/firewall/banned/add" class="stack-form" style="display:grid;grid-template-columns:repeat(auto-fit,minmax(160px,1fr));gap:10px;align-items:end;margin:12px 0;padding:12px;border:1px solid var(--border,#334155);border-radius:10px;">
        <input type="hidden" name="csrf" value="{csrf}">
        <label>IP address<input name="ip" required maxlength="64" placeholder="192.0.2.10 or 192.0.2.0/24"></label>
        <label>Reason<input name="reason" maxlength="200" placeholder="Suspicious activity"></label>
        <label>Duration<select name="duration">
          <option value="3600">1 Hour</option>
          <option value="86400" selected>24 Hours</option>
          <option value="604800">7 Days</option>
          <option value="0">Never</option>
        </select></label>
        <button type="submit" class="btn-danger">Ban IP Address</button>
      </form>
      <p class="muted">Trusted IPs (server IP, first admin IP, and manual never-block entries) cannot be banned.</p>
      <form method="get" action="/security/firewall" style="display:flex;gap:8px;flex-wrap:wrap;margin:12px 0;">
        <input type="hidden" name="tab" value="banned">
        <input name="q" value="{q}" placeholder="Search by IP, reason or status..." maxlength="128" style="min-width:220px;flex:1;">
        <button type="submit" class="btn-secondary">Search</button>
      </form>"#,
            csrf = html_escape(csrf),
            q = html_escape(q),
        ));
    }
    let q_lower = q.trim().to_ascii_lowercase();
    let rows: Vec<&BannedIp> = banned
        .iter()
        .filter(|b| {
            if q_lower.is_empty() {
                return true;
            }
            b.ip.to_ascii_lowercase().contains(&q_lower)
                || b.reason.to_ascii_lowercase().contains(&q_lower)
                || b.status.to_ascii_lowercase().contains(&q_lower)
        })
        .collect();
    if rows.is_empty() {
        body.push_str("<p class=\"muted\">No banned IPs.</p>");
        return body;
    }
    body.push_str(
        r#"<div class="table-wrap"><table class="data-table"><thead><tr>
      <th>IP Address</th><th>Reason</th><th>Banned On</th><th>Expires</th><th>Status</th><th>Actions</th>
    </tr></thead><tbody>"#,
    );
    for b in rows {
        let expires = b.expires_at.map(fmt_ts).unwrap_or_else(|| "Never".into());
        let actions = if is_admin {
            format!(
                r#"<form method="post" action="/security/firewall/banned/unban" style="display:inline;">
              <input type="hidden" name="csrf" value="{csrf}">
              <input type="hidden" name="ip" value="{ip}">
              <button type="submit" class="btn-primary" style="background:#12b76a;border:none;">Unban</button>
            </form>
            <form method="post" action="/security/firewall/banned/delete" style="display:inline;">
              <input type="hidden" name="csrf" value="{csrf}">
              <input type="hidden" name="ip" value="{ip}">
              <button type="submit" class="btn-danger">Delete</button>
            </form>"#,
                csrf = html_escape(csrf),
                ip = html_escape(&b.ip),
            )
        } else {
            String::new()
        };
        body.push_str(&format!(
            r#"<tr><td><code>{ip}</code></td><td>{reason}</td><td>{on}</td><td>{exp}</td><td>{status}</td><td>{actions}</td></tr>"#,
            ip = html_escape(&b.ip),
            reason = html_escape(&b.reason),
            on = html_escape(&fmt_ts(b.banned_at)),
            exp = html_escape(&expires),
            status = html_escape(&b.status.to_ascii_uppercase()),
            actions = actions,
        ));
    }
    body.push_str("</tbody></table></div>");
    body
}

fn trusted_tab(trusted: &[TrustedIp], is_admin: bool, csrf: &str) -> String {
    let mut body = String::from(
        r#"<h3 style="margin:8px 0;">SSH trusted IPs (never block)</h3>
    <p class="muted">These addresses cannot be added under Banned IPs. The server IP and first admin IP are seeded automatically and are protected. Use your home or office public IP to avoid accidental lockouts.</p>"#,
    );
    if is_admin {
        body.push_str(&format!(
            r#"
      <form method="post" action="/security/firewall/trusted/add" class="stack-form" style="display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));gap:10px;align-items:end;margin:12px 0;padding:12px;border:1px solid var(--border,#334155);border-radius:10px;">
        <input type="hidden" name="csrf" value="{csrf}">
        <label>IP address<input name="ip" required maxlength="64" placeholder="Public IPv4 / IPv6"></label>
        <label>Label (optional)<input name="label" maxlength="64" placeholder="e.g. Home PC"></label>
        <button type="submit" class="btn-primary">+ Add trusted IP</button>
      </form>"#,
            csrf = html_escape(csrf),
        ));
    }
    if trusted.is_empty() {
        body.push_str(
            "<p class=\"muted\">No trusted IPs yet. Add at least the public IP you use to manage this server.</p>",
        );
        return body;
    }
    body.push_str(
        r#"<div class="table-wrap"><table class="data-table"><thead><tr>
      <th>IP Address</th><th>Label</th><th>Source</th><th>Protected</th><th>Actions</th>
    </tr></thead><tbody>"#,
    );
    for t in trusted {
        let can_remove =
            is_admin && matches!(t.source, crate::panel_firewall_store::TrustedSource::Manual);
        let actions = if can_remove {
            format!(
                r#"<form method="post" action="/security/firewall/trusted/delete" style="display:inline;" onsubmit="return confirm('Remove this trusted IP?');">
              <input type="hidden" name="csrf" value="{csrf}">
              <input type="hidden" name="ip" value="{ip}">
              <button type="submit" class="btn-danger">Remove</button>
            </form>"#,
                csrf = html_escape(csrf),
                ip = html_escape(&t.ip),
            )
        } else if t.protected {
            "<span class=\"muted\">locked</span>".into()
        } else {
            String::new()
        };
        body.push_str(&format!(
            r#"<tr><td><code>{ip}</code></td><td>{label}</td><td>{src}</td><td>{prot}</td><td>{actions}</td></tr>"#,
            ip = html_escape(&t.ip),
            label = html_escape(&t.label),
            src = html_escape(trusted_source_label(&t.source)),
            prot = if t.protected { "yes" } else { "no" },
            actions = actions,
        ));
    }
    body.push_str("</tbody></table></div>");
    body
}

/// Full firewall manager page.
pub fn firewall_manager_page(
    username: &str,
    tab: &str,
    notice: Option<&str>,
    error: Option<&str>,
    is_admin: bool,
    search: Option<&str>,
    peer_ip: Option<&str>,
) -> String {
    let store = prepare_manager(peer_ip);
    let csrf = firewall_csrf_token(username);
    let tab = match tab {
        "banned" | "trusted" => tab,
        _ => "rules",
    };
    let mut body = status_bar(is_admin, &csrf);
    body.push_str(&tabs(tab));
    let tab_body = match tab {
        "banned" => banned_tab(&store.banned, is_admin, &csrf, search.unwrap_or("")),
        "trusted" => trusted_tab(&store.trusted, is_admin, &csrf),
        _ => rules_tab(&store.rules, is_admin, &csrf),
    };
    body.push_str(&tab_body);
    let detail: String = firewall_status().detail.chars().take(800).collect();
    if !detail.is_empty() {
        body.push_str("<h3>Live backend status</h3>");
        body.push_str(&format!(
            r#"<pre class="manage-log-pre" style="white-space:pre-wrap;max-height:280px;overflow:auto;">{}</pre>"#,
            html_escape(&detail)
        ));
    }
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Security", Some("/security")),
            ("Firewall", None),
        ],
        "Firewall",
        "Manage firewalld rules, banned IPs, and trusted (never block) addresses.",
        &body,
        notice,
        error,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_has_tabs_not_foreign_brands() {
        let html =
            firewall_manager_page("admin", "rules", None, None, true, None, Some("192.0.2.1"));
        assert!(html.contains("Firewall Rules"));
        assert!(html.contains("Banned IPs"));
        assert!(html.contains("never block"));
        assert!(!html.contains("CyberPanel"));
        assert!(!html.contains("CSF"));
    }

    #[test]
    fn civil_date_smoke() {
        let (y, m, d) = civil_from_days(20_000);
        assert!(y > 1970);
        assert!((1..=12).contains(&m));
        assert!((1..=31).contains(&d));
    }
}
