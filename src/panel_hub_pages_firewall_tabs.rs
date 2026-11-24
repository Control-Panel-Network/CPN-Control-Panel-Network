//! Tab bodies for the firewall manager (rules, banned IPs, trusted IPs).
//!
//! Each tab renders into its own `<section data-fw-panel>`; the page script shows one at a
//! time so switching tabs is instant (see `panel_hub_pages_firewall_ui`).

use crate::panel_firewall_store::{BannedIp, FirewallRule, TrustedIp, TrustedSource};
use crate::panel_hub_pages_firewall::{fmt_ts, html_escape};
use crate::panel_hub_pages_firewall_ui::{IP_CIDR_TIP, PROTOCOL_OPTIONS, info_tip};
use crate::panel_ops_firewall::trusted_source_label;
use crate::panel_ops_firewall_rules::protocol_label;

fn edit_button(kind: &str, fields: &[(&str, &str)]) -> String {
    let mut attrs = String::new();
    for (name, value) in fields {
        attrs.push_str(&format!(
            r#" data-field-{name}="{value}""#,
            value = html_escape(value)
        ));
    }
    format!(
        r#"<button type="button" class="btn-secondary fw-btn-sm" data-fw-edit="{kind}"{attrs}>Edit</button>"#
    )
}

fn delete_form(
    action: &str,
    csrf: &str,
    field: (&str, &str),
    title: &str,
    message: &str,
    label: &str,
) -> String {
    format!(
        r#"<form method="post" action="{action}" data-fw-confirm="{message}" data-fw-title="{title}" data-fw-label="{label}">
              <input type="hidden" name="csrf" value="{csrf}">
              <input type="hidden" name="{fname}" value="{fvalue}">
              <button type="submit" class="btn-danger fw-btn-sm">{label}</button>
            </form>"#,
        message = html_escape(message),
        title = html_escape(title),
        label = html_escape(label),
        csrf = html_escape(csrf),
        fname = field.0,
        fvalue = html_escape(field.1),
    )
}

/// Firewall rules tab.
pub fn rules_tab(rules: &[FirewallRule], is_admin: bool, csrf: &str) -> String {
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
      <form method="post" action="/security/firewall/rules/add" class="stack-form fw-form" style="grid-template-columns:repeat(auto-fit,minmax(140px,1fr));">
        <input type="hidden" name="csrf" value="{csrf}">
        <div class="fw-field"><div class="fw-field-head"><label for="fw-add-name">Rule name</label></div><input id="fw-add-name" name="name" required maxlength="64" placeholder="Allow SSH"></div>
        <div class="fw-field"><div class="fw-field-head"><label for="fw-add-protocol">Protocol</label></div><select id="fw-add-protocol" name="protocol">{protocols}</select></div>
        <div class="fw-field"><div class="fw-field-head"><label for="fw-add-source">IP / CIDR</label>{tip}</div><input id="fw-add-source" name="source" value="0.0.0.0/0" maxlength="64"></div>
        <div class="fw-field"><div class="fw-field-head"><label for="fw-add-port">Port</label></div><input id="fw-add-port" name="port" required maxlength="11" placeholder="80"></div>
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
            protocols = PROTOCOL_OPTIONS,
            tip = info_tip("fw-tip-add-source", "About IP / CIDR", IP_CIDR_TIP),
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
            let edit = edit_button(
                "rules",
                &[
                    ("id", &r.id),
                    ("name", &r.name),
                    ("protocol", &r.protocol),
                    ("source", &r.source),
                    ("port", &r.port),
                ],
            );
            let del = delete_form(
                "/security/firewall/rules/delete",
                csrf,
                ("id", &r.id),
                "Delete rule?",
                &format!(
                    "Are you sure you want to delete the rule \"{}\" ({} port {})? The port will be closed on the firewall.",
                    r.name,
                    protocol_label(&r.protocol),
                    r.port
                ),
                "Delete",
            );
            format!(r#"<div class="fw-actions">{edit}{del}</div>"#)
        } else {
            String::new()
        };
        body.push_str(&format!(
            r#"<tr><td>{name}</td><td><code>{proto}</code></td><td><code>{src}</code></td><td><code>{port}</code></td><td>{actions}</td></tr>"#,
            name = html_escape(&r.name),
            proto = html_escape(protocol_label(&r.protocol)),
            src = html_escape(&r.source),
            port = html_escape(&r.port),
            actions = actions,
        ));
    }
    body.push_str("</tbody></table></div>");
    body
}

/// Banned IPs tab (search runs on the client when scripting is available).
pub fn banned_tab(banned: &[BannedIp], is_admin: bool, csrf: &str, q: &str) -> String {
    let mut body = String::from(
        r#"<div style="display:flex;justify-content:space-between;align-items:center;gap:8px;flex-wrap:wrap;margin:8px 0;">
      <h3 style="margin:0;">Banned IP Addresses</h3>
      <a class="btn-secondary" href="/security/firewall/export/banned">Export Banned IPs</a>
    </div>"#,
    );
    if is_admin {
        body.push_str(&format!(
            r#"
      <form method="post" action="/security/firewall/banned/add" class="stack-form fw-form" style="grid-template-columns:repeat(auto-fit,minmax(160px,1fr));">
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
      <p class="muted">Trusted IPs (server IP, first admin IP, and manual never-block entries) cannot be banned.</p>"#,
            csrf = html_escape(csrf),
        ));
    }
    body.push_str(&format!(
        r#"
      <form method="get" action="/security/firewall" style="display:flex;gap:8px;flex-wrap:wrap;margin:12px 0;">
        <input type="hidden" name="tab" value="banned">
        <input name="q" value="{q}" placeholder="Search by IP, reason or status..." maxlength="128" style="min-width:220px;flex:1;" data-fw-search-input aria-label="Search banned IPs">
        <button type="submit" class="btn-secondary">Search</button>
      </form>"#,
        q = html_escape(q),
    ));
    let q_lower = q.trim().to_ascii_lowercase();
    if banned.is_empty() {
        body.push_str("<p class=\"muted\">No banned IPs.</p>");
        return body;
    }
    let matches = |b: &BannedIp| {
        q_lower.is_empty()
            || b.ip.to_ascii_lowercase().contains(&q_lower)
            || b.reason.to_ascii_lowercase().contains(&q_lower)
            || b.status.to_ascii_lowercase().contains(&q_lower)
    };
    let any_match = banned.iter().any(matches);
    body.push_str(&format!(
        r#"<p class="muted fw-nomatch" data-fw-nomatch{hide}>No banned IPs match your search.</p>
      <div class="table-wrap"><table class="data-table"><thead><tr>
      <th>IP Address</th><th>Reason</th><th>Banned On</th><th>Expires</th><th>Status</th><th>Actions</th>
    </tr></thead><tbody>"#,
        hide = if any_match { " hidden" } else { "" },
    ));
    for b in banned {
        let expires = b.expires_at.map(fmt_ts).unwrap_or_else(|| "Never".into());
        let actions = if is_admin {
            let edit = edit_button(
                "banned",
                &[("old_ip", &b.ip), ("ip", &b.ip), ("reason", &b.reason)],
            );
            let unban = format!(
                r#"<form method="post" action="/security/firewall/banned/unban">
              <input type="hidden" name="csrf" value="{csrf}">
              <input type="hidden" name="ip" value="{ip}">
              <button type="submit" class="btn-primary fw-btn-sm" style="background:#12b76a;border:none;">Unban</button>
            </form>"#,
                csrf = html_escape(csrf),
                ip = html_escape(&b.ip),
            );
            let del = delete_form(
                "/security/firewall/banned/delete",
                csrf,
                ("ip", &b.ip),
                "Delete banned entry?",
                &format!(
                    "Are you sure you want to delete the ban for {}? The address will be unblocked.",
                    b.ip
                ),
                "Delete",
            );
            format!(r#"<div class="fw-actions">{edit}{unban}{del}</div>"#)
        } else {
            String::new()
        };
        let haystack = format!("{} {} {}", b.ip, b.reason, b.status).to_ascii_lowercase();
        body.push_str(&format!(
            r#"<tr data-fw-search="{hay}"{hidden}><td><code>{ip}</code></td><td>{reason}</td><td>{on}</td><td>{exp}</td><td>{status}</td><td>{actions}</td></tr>"#,
            hay = html_escape(&haystack),
            hidden = if matches(b) { "" } else { " hidden" },
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

/// SSH trusted (never block) IPs tab.
pub fn trusted_tab(trusted: &[TrustedIp], is_admin: bool, csrf: &str) -> String {
    let mut body = String::from(
        r#"<h3 style="margin:8px 0;">SSH trusted IPs (never block)</h3>
    <p class="muted">These addresses cannot be added under Banned IPs. The server IP and first admin IP are seeded automatically and are protected. Use your home or office public IP to avoid accidental lockouts.</p>"#,
    );
    if is_admin {
        body.push_str(&format!(
            r#"
      <form method="post" action="/security/firewall/trusted/add" class="stack-form fw-form" style="grid-template-columns:repeat(auto-fit,minmax(180px,1fr));">
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
        let editable = is_admin && matches!(t.source, TrustedSource::Manual);
        let actions = if editable {
            let edit = edit_button(
                "trusted",
                &[("old_ip", &t.ip), ("ip", &t.ip), ("label", &t.label)],
            );
            let del = delete_form(
                "/security/firewall/trusted/delete",
                csrf,
                ("ip", &t.ip),
                "Remove trusted IP?",
                &format!(
                    "Are you sure you want to remove {} from the trusted (never block) list? It could then be banned.",
                    t.ip
                ),
                "Remove",
            );
            format!(r#"<div class="fw-actions">{edit}{del}</div>"#)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(proto: &str) -> FirewallRule {
        FirewallRule {
            id: "r1".into(),
            name: "dns \"x\"".into(),
            protocol: proto.into(),
            port: "53".into(),
            source: "0.0.0.0/0".into(),
        }
    }

    #[test]
    fn rules_tab_has_edit_confirm_and_both_label() {
        let html = rules_tab(&[rule("both")], true, "tok");
        assert!(html.contains("data-fw-edit=\"rules\""));
        assert!(html.contains("data-field-protocol=\"both\""));
        assert!(html.contains("data-fw-confirm="));
        assert!(html.contains("TCP + UDP"));
        assert!(html.contains("value=\"both\""));
        assert!(html.contains("&quot;x&quot;"));
        assert!(!html.contains("onsubmit=\"return confirm("));
    }

    #[test]
    fn non_admin_sees_no_edit_controls() {
        let html = rules_tab(&[rule("tcp")], false, "tok");
        assert!(!html.contains("data-fw-edit"));
        assert!(!html.contains("fw-tip"));
    }

    #[test]
    fn banned_rows_carry_search_text_and_edit() {
        let b = BannedIp {
            ip: "192.0.2.9".into(),
            reason: "Scan".into(),
            banned_at: 1_700_000_000,
            expires_at: None,
            status: "active".into(),
        };
        let html = banned_tab(&[b], true, "tok", "nomatch");
        assert!(html.contains("data-fw-search=\"192.0.2.9 scan active\""));
        assert!(html.contains("data-fw-edit=\"banned\""));
        assert!(html.contains("data-fw-nomatch"));
    }

    #[test]
    fn trusted_protected_rows_are_locked_manual_rows_editable() {
        let manual = TrustedIp {
            ip: "198.51.100.5".into(),
            label: "Home".into(),
            protected: true,
            source: TrustedSource::Manual,
        };
        let server = TrustedIp {
            ip: "203.0.113.1".into(),
            label: "Server".into(),
            protected: true,
            source: TrustedSource::Server,
        };
        let html = trusted_tab(&[manual, server], true, "tok");
        assert!(html.contains("data-fw-edit=\"trusted\""));
        assert!(html.contains("locked"));
    }
}
