//! Server > Logs page and the shared panel-actions table (also used by the Activity Board).

use crate::panel_action_log::{ActionRecord, action_label, recent};
use crate::panel_admin::is_panel_admin;
use crate::panel_dashboard_activity_list::wrap_activity_table;
use crate::panel_hubs::{HubTile, feature_shell, hub_tiles_grid};
use crate::panel_ops_ssl_inspect::format_dd_mm_yyyy;

/// Rows shown in the table (newest first).
pub const TABLE_LIMIT: usize = 200;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// `dd/mm/yyyy HH:MM:SS` in UTC; the page script swaps it for the browser-local time.
fn utc_stamp(ts: u64) -> String {
    let secs = ts % 86_400;
    format!(
        "{} {:02}:{:02}:{:02}",
        format_dd_mm_yyyy(ts),
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

fn rows_table(rows: &[ActionRecord]) -> String {
    let mut table = String::from(
        r#"<div class="table-wrap"><table class="data-table"><thead><tr><th>Time</th><th>User</th><th>Action</th><th>Scope</th><th>Result</th><th>Details</th></tr></thead><tbody>"#,
    );
    for row in rows {
        let scope = if row.target.is_empty() {
            "Host".to_string()
        } else {
            html_escape(&row.target)
        };
        let result = if row.ok {
            r#"<span class="plugin-badge installed">OK</span>"#
        } else {
            r#"<span class="plugin-badge paid">Failed</span>"#
        };
        table.push_str(&format!(
            r#"<tr><td><time data-ts="{ts}" title="UTC">{stamp} UTC</time></td><td>{actor}</td><td>{label}</td><td>{scope}</td><td>{result}</td><td>{msg}</td></tr>"#,
            ts = row.ts,
            stamp = utc_stamp(row.ts),
            actor = html_escape(&row.actor),
            label = html_escape(action_label(&row.action)),
            scope = scope,
            result = result,
            msg = html_escape(&row.message),
        ));
    }
    table.push_str("</tbody></table></div>");
    table
}

fn local_time_script() -> &'static str {
    r#"<script>(function(){function p(n){return String(n).padStart(2,'0');}
document.querySelectorAll('time[data-ts]').forEach(function(t){var d=new Date(parseInt(t.getAttribute('data-ts'),10)*1000);if(isNaN(d.getTime()))return;t.textContent=p(d.getDate())+'/'+p(d.getMonth()+1)+'/'+d.getFullYear()+' '+p(d.getHours())+':'+p(d.getMinutes())+':'+p(d.getSeconds());t.title='Local time';});})();</script>"#
}

/// Searchable, paginated table of plugin and host-package actions. Panel admins see every
/// account; other users only see their own actions.
pub fn panel_actions_table(username: &str) -> String {
    let filter = if is_panel_admin(username) {
        None
    } else {
        Some(username)
    };
    let rows = recent(TABLE_LIMIT, filter);
    if rows.is_empty() {
        return r#"<p class="empty-state">No plugin or host-package actions recorded yet. Install, activate, deactivate and uninstall actions from the Plugins Store appear here.</p>"#
            .into();
    }
    format!(
        "{}{}",
        wrap_activity_table(
            "panel-actions",
            "Filter user, action, site or details",
            &rows_table(&rows)
        ),
        local_time_script()
    )
}

pub fn server_logs_page(username: &str) -> String {
    let tiles = [
        HubTile {
            title: "Website access and error logs",
            subtitle: "Per site, jailed to each home (Manage > Logs)",
            href: "/websites",
            live: true,
        },
        HubTile {
            title: "SSH logins and auth logs",
            subtitle: "Activity Board (panel admin)",
            href: "/dashboard?activity=ssh-logs",
            live: true,
        },
        HubTile {
            title: "Log retention",
            subtitle: "Keep window and size cap for site logs",
            href: "/settings/logs",
            live: true,
        },
    ];
    let body = format!(
        r#"{tiles}
<h2 style="margin:20px 0 6px;">Panel activity</h2>
<p class="muted">Plugin and host-package actions: install, activate, deactivate, attach to a site, start, stop and uninstall. Times are shown in your browser time zone.</p>
{table}"#,
        tiles = hub_tiles_grid("Log sources", &tiles),
        table = panel_actions_table(username),
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("Logs", None),
        ],
        "Logs",
        "Panel activity and where to find every other log on this node.",
        &body,
        None,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::panel_action_log::record;

    #[test]
    fn empty_log_shows_hint() {
        with_test_data_dir(|| {
            let html = panel_actions_table("someone");
            assert!(html.contains("No plugin or host-package actions"));
        });
    }

    #[test]
    fn rows_render_with_scope_and_result() {
        with_test_data_dir(|| {
            record("alice", "host.attach", "a.example.com", true, "Attached").unwrap();
            record("alice", "plugin.install-host", "", false, "Boom <b>").unwrap();
            let html = panel_actions_table("alice");
            assert!(html.contains("Host package: attach to site"));
            assert!(html.contains("a.example.com"));
            assert!(html.contains("Failed"));
            assert!(html.contains("Boom &lt;b&gt;"));
            assert!(html.contains("data-ts="));
            // A different non-admin user sees nothing of alice's actions.
            assert!(panel_actions_table("bob").contains("No plugin or host-package actions"));
        });
    }

    #[test]
    fn page_lists_log_sources() {
        with_test_data_dir(|| {
            let html = server_logs_page("alice");
            assert!(html.contains("Website access and error logs"));
            assert!(html.contains("/settings/logs"));
        });
    }
}
