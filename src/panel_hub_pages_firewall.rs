//! Firewall manager HTML (rules, banned IPs, trusted never-block).

use crate::panel_hub_pages_firewall_tabs::{banned_tab, rules_tab, trusted_tab};
use crate::panel_hub_pages_firewall_ui::{
    ban_edit_dialog_html, confirm_dialog_html, firewall_css, firewall_script,
    rule_edit_dialog_html, trusted_edit_dialog_html,
};
use crate::panel_hubs::{feature_shell, status_kv};
use crate::panel_ops_firewall::{firewall_csrf_token, panel_listen_port, prepare_manager};
use crate::panel_ops_firewall_status::firewall_status_cached;
use crate::panel_ops_security::FirewallStatus;
pub(crate) fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub(crate) fn fmt_ts(secs: u64) -> String {
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
        ("rules", "Firewall Rules"),
        ("banned", "Banned IPs"),
        ("trusted", "SSH trusted IPs"),
    ];
    let mut out =
        String::from(r#"<nav class="fw-tabs" role="tablist" aria-label="Firewall sections">"#);
    for (id, label) in items {
        let on = id == active;
        out.push_str(&format!(
            r#"<a class="fw-tab{cls}" role="tab" id="fw-tab-{id}" href="/security/firewall?tab={id}" data-fw-tab="{id}" aria-controls="fw-panel-{id}" aria-selected="{sel}" tabindex="{ti}">{label}</a>"#,
            cls = if on { " is-active" } else { "" },
            sel = if on { "true" } else { "false" },
            ti = if on { "0" } else { "-1" },
        ));
    }
    out.push_str("</nav>");
    out
}

fn panel(id: &str, active: &str, inner: &str) -> String {
    format!(
        r#"<section id="fw-panel-{id}" role="tabpanel" aria-labelledby="fw-tab-{id}" data-fw-panel="{id}"{hidden}>{inner}</section>"#,
        hidden = if id == active { "" } else { " hidden" },
    )
}

fn status_bar(st: &FirewallStatus, is_admin: bool, csrf: &str) -> String {
    let badge = if st.active {
        r#"<span style="color:#12b76a;font-weight:700;">● ON</span>"#
    } else {
        r#"<span style="color:#f79009;font-weight:700;">● OFF</span>"#
    };
    let controls = if is_admin {
        format!(
            r#"
      <div class="fw-controls">
        <form method="post" action="/security/firewall/start" style="margin:0;">
          <input type="hidden" name="csrf" value="{csrf}">
          <button type="submit" class="fw-btn fw-btn-start">Start</button>
        </form>
        <form method="post" action="/security/firewall/stop" style="margin:0;">
          <input type="hidden" name="csrf" value="{csrf}">
          <button type="submit" class="fw-btn fw-btn-stop">Stop</button>
        </form>
        <form method="post" action="/security/firewall/reload" style="margin:0;">
          <input type="hidden" name="csrf" value="{csrf}">
          <button type="submit" class="fw-btn fw-btn-reload">Reload</button>
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
    // One probe per render (cached), shared by the status bar and the live detail block.
    let st = firewall_status_cached();
    let mut body = String::from(firewall_css());
    body.push_str(r#"<div id="fw-root">"#);
    body.push_str(&status_bar(&st, is_admin, &csrf));
    body.push_str(&tabs(tab));
    // All tabs are rendered up front and switched on the client, so tab clicks never
    // reload the page or re-run the host firewall probes.
    body.push_str(&panel(
        "rules",
        tab,
        &rules_tab(&store.rules, is_admin, &csrf),
    ));
    body.push_str(&panel(
        "banned",
        tab,
        &banned_tab(&store.banned, is_admin, &csrf, search.unwrap_or("")),
    ));
    body.push_str(&panel(
        "trusted",
        tab,
        &trusted_tab(&store.trusted, is_admin, &csrf),
    ));
    let detail: String = st.detail.chars().take(800).collect();
    if !detail.is_empty() {
        body.push_str("<h3>Live backend status</h3>");
        body.push_str(&format!(
            r#"<pre class="manage-log-pre" style="white-space:pre-wrap;max-height:280px;overflow:auto;">{}</pre>"#,
            html_escape(&detail)
        ));
    }
    body.push_str("</div>");
    if is_admin {
        body.push_str(&rule_edit_dialog_html(&csrf));
        body.push_str(&ban_edit_dialog_html(&csrf));
        body.push_str(&trusted_edit_dialog_html(&csrf));
        body.push_str(confirm_dialog_html());
    }
    body.push_str(firewall_script());
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
    fn all_tabs_render_up_front_for_instant_client_switching() {
        let html = firewall_manager_page("admin", "banned", None, None, true, None, None);
        for id in ["rules", "banned", "trusted"] {
            assert!(html.contains(&format!("data-fw-panel=\"{id}\"")));
            assert!(html.contains(&format!("data-fw-tab=\"{id}\"")));
        }
        // Only the requested tab starts visible; the others stay hidden until selected.
        assert!(html.contains("data-fw-panel=\"banned\">"));
        assert!(html.contains("data-fw-panel=\"rules\" hidden"));
        assert!(html.contains("data-fw-panel=\"trusted\" hidden"));
        assert!(html.contains("href=\"/security/firewall?tab=trusted\""));
    }

    #[test]
    fn stop_button_is_round_red_and_not_the_old_orange_square() {
        let html = firewall_manager_page("admin", "rules", None, None, true, None, None);
        assert!(html.contains("fw-btn fw-btn-stop"));
        assert!(html.contains(".fw-btn-stop{background:#d92d20;}"));
        assert!(html.contains("border-radius:999px"));
        assert!(!html.contains("btn-warn"));
        assert!(!html.contains("background:#f79009;color:#111"));
    }

    #[test]
    fn page_offers_both_protocol_edit_dialogs_and_ip_tip() {
        let html = firewall_manager_page("admin", "rules", None, None, true, None, None);
        assert!(html.contains("value=\"both\""));
        assert!(html.contains("id=\"fw-edit-rules\""));
        assert!(html.contains("id=\"fw-edit-banned\""));
        assert!(html.contains("id=\"fw-edit-trusted\""));
        assert!(html.contains("id=\"fw-confirm\""));
        assert!(html.contains("0.0.0.0/0 means all sources"));
    }

    #[test]
    fn non_admin_page_has_no_mutating_dialogs() {
        let html = firewall_manager_page("user", "rules", None, None, false, None, None);
        assert!(!html.contains("id=\"fw-edit-rules\""));
        assert!(!html.contains("/security/firewall/stop"));
    }

    #[test]
    fn civil_date_smoke() {
        let (y, m, d) = civil_from_days(20_000);
        assert!(y > 1970);
        assert!((1..=12).contains(&m));
        assert!((1..=31).contains(&d));
    }
}
