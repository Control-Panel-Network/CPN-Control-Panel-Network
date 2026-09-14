//! Dashboard Activity Board markup (tabs for SSH, processes, traffic, disk, CPU).

use crate::packages::is_panel_admin;
use crate::panel_dashboard_activity_list::{
    activity_list_script, activity_list_styles, wrap_activity_table,
};
use crate::panel_dashboard_activity_ssh::ssh_logs_panel;
use crate::panel_ops_activity::{ActivityLogRow, recent_ssh_logins, ssh_security_analysis};
use crate::panel_ops_activity_host::{
    cpu_activity, disk_io_snapshot, format_bytes, network_traffic,
};
use crate::panel_ops_process::snapshot_top_processes;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn activity_board_styles() -> String {
    let mut css = String::from(
        r#"
.activity-board {
  max-width:1200px; margin:22px auto 0; padding:22px 24px 24px;
  border-radius:8px; background:var(--canvas); border:1px solid var(--hairline);
  min-width:0;
}
.activity-board .eyebrow { margin:0 0 4px; font-size:11px; letter-spacing:.08em; font-weight:700; color:var(--muted); }
.activity-board > h2 { margin:0 0 14px; font-size:22px; letter-spacing:-.02em; }
.activity-tabs {
  display:flex; flex-wrap:wrap; gap:8px; margin:0 0 16px;
  overflow-x:auto; -webkit-overflow-scrolling:touch; padding-bottom:2px;
}
.activity-tab {
  display:inline-flex; align-items:center; gap:8px; flex:0 0 auto;
  min-height:36px; padding:6px 12px; border-radius:999px; border:1px solid var(--hairline);
  background:transparent; color:inherit; font:inherit; font-size:13px; font-weight:600;
  cursor:pointer; white-space:nowrap; position:relative;
}
.activity-tab[aria-selected="true"] {
  background:var(--blue); border-color:transparent; color:#fff;
}
.activity-tab .tab-badge {
  display:inline-grid; place-items:center; min-width:18px; height:18px; padding:0 5px;
  border-radius:999px; background:#f04438; color:#fff; font-size:11px; font-weight:700;
}
.activity-tab[aria-selected="true"] .tab-badge { background:#fff; color:#b42318; }
.activity-panel[hidden] { display:none !important; }
.activity-panel-head {
  display:flex; flex-wrap:wrap; align-items:center; justify-content:space-between; gap:10px;
  margin:0 0 12px;
}
.activity-panel-head h3 { margin:0; font-size:16px; }
.activity-sec-box {
  margin:0 0 14px; padding:12px 14px; border-radius:10px;
  border:1px solid #fecd89; background:#fffaeb; color:#93370d;
}
.activity-sec-box h4 { margin:0 0 6px; font-size:14px; display:flex; align-items:center; gap:8px; }
.activity-sec-card {
  margin-top:10px; padding:12px; border-radius:8px; background:#fff; color:#1d1d1f;
  border:1px solid #f5d0a9;
}
.activity-sec-card .badge-info {
  display:inline-block; margin-left:8px; padding:2px 8px; border-radius:999px;
  background:#ecfdf3; color:#027a48; font-size:11px; font-weight:700;
}
.activity-sec-meta {
  display:grid; grid-template-columns:repeat(3,minmax(0,1fr)); gap:8px; margin:10px 0 0; font-size:13px;
}
.activity-sec-meta span { color:var(--muted); display:block; font-size:11px; }
.activity-tip {
  margin:10px 0 0; padding:8px 10px; border-radius:8px; background:#f2f4f7; font-size:13px;
}
.activity-sec-actions {
  display:flex; flex-wrap:wrap; align-items:center; gap:10px; margin:12px 0 0;
}
.activity-sec-actions form { margin:0; }
.activity-sec-actions button {
  min-height:34px; padding:0 12px; border-radius:999px; border:1px solid var(--hairline);
  background:transparent; color:inherit; font:inherit; font-size:13px; font-weight:700; cursor:pointer;
}
.activity-sec-actions .muted { margin:0; font-size:12px; }
.activity-sec-snoozed {
  margin:0 0 14px; padding:10px 12px; border-radius:10px; border:1px dashed var(--hairline);
  font-size:13px; color:var(--muted);
}
.activity-board .data-table th {
  background:rgba(37,99,235,.12); color:var(--ink); border-top:0;
}
.activity-kpi {
  display:grid; grid-template-columns:repeat(3,minmax(0,1fr)); gap:12px; margin:0 0 14px;
}
.activity-kpi article {
  padding:12px; border-radius:10px; border:1px solid var(--hairline); background:var(--surface-soft,#f8fafc);
}
.activity-kpi strong { display:block; font-size:22px; margin-top:4px; }
.activity-kpi span { color:var(--muted); font-size:12px; }
@media (max-width:719.98px) {
  .activity-board { padding:16px; }
  .activity-sec-meta, .activity-kpi { grid-template-columns:1fr; }
  .activity-tabs { flex-wrap:nowrap; }
}
[data-color-mode="dark"] .activity-sec-box {
  background:#3b2a12; border-color:#93370d; color:#fecd89;
}
[data-color-mode="dark"] .activity-sec-card {
  background:#1a1d26; border-color:#93370d; color:var(--ink);
}
[data-color-mode="dark"] .activity-tip { background:#2a2f3a; }
[data-color-mode="dark"] .activity-kpi article { background:#161922; }
"#,
    );
    css.push_str(activity_list_styles());
    css
}

fn log_table(rows: &[ActivityLogRow], empty: &str) -> String {
    if rows.is_empty() {
        return format!(r#"<p class="empty-state">{e}</p>"#, e = html_escape(empty));
    }
    let mut t = String::from(
        r#"<div class="table-wrap"><table class="data-table"><thead><tr><th>Timestamp</th><th>Message</th></tr></thead><tbody>"#,
    );
    for row in rows {
        t.push_str(&format!(
            r#"<tr><td><time>{ts}</time></td><td><code>{msg}</code></td></tr>"#,
            ts = html_escape(&row.timestamp),
            msg = html_escape(&row.message),
        ));
    }
    t.push_str("</tbody></table></div>");
    t
}

fn tab_btn(id: &str, label: &str, selected: bool, badge: Option<usize>) -> String {
    let sel = if selected { "true" } else { "false" };
    let badge_html = match badge {
        Some(n) if n > 0 => format!(
            r#"<span class="tab-badge" aria-label="{n} alerts">{n}</span>"#,
            n = n.min(99)
        ),
        _ => String::new(),
    };
    format!(
        r#"<button type="button" class="activity-tab" role="tab" id="activity-tab-{id}" data-activity-tab="{id}" aria-controls="activity-panel-{id}" aria-selected="{sel}">{label}{badge}</button>"#,
        id = html_escape(id),
        label = html_escape(label),
        sel = sel,
        badge = badge_html,
    )
}

fn panel(id: &str, hidden: bool, body: &str) -> String {
    let hide = if hidden { " hidden" } else { "" };
    format!(
        r#"<div class="activity-panel" role="tabpanel" id="activity-panel-{id}" aria-labelledby="activity-tab-{id}"{hide}>{body}</div>"#,
        id = html_escape(id),
        hide = hide,
        body = body,
    )
}

fn ssh_logins_panel() -> String {
    let rows = recent_ssh_logins(200);
    let table = wrap_activity_table(
        "ssh-logins",
        "Filter timestamp or message",
        &log_table(
            &rows,
            "No recent SSH logins found (log files may be empty or unreadable).",
        ),
    );
    format!(
        r#"<div class="activity-panel-head"><h3>Recent SSH Logins</h3>
        <p class="muted" style="margin:0;">Accepted sessions from auth logs / journal.</p></div>
        {table}"#,
        table = table,
    )
}

fn top_process_panel() -> String {
    let body = match snapshot_top_processes(50) {
        Ok(rows) if rows.is_empty() => "<p class=\"empty-state\">No processes returned.</p>".into(),
        Ok(rows) => {
            let mut t = String::from(
                r#"<div class="table-wrap"><table class="data-table"><thead><tr><th>User</th><th>PID</th><th>CPU%</th><th>MEM%</th><th>Command</th></tr></thead><tbody>"#,
            );
            for r in rows {
                t.push_str(&format!(
                    r#"<tr><td>{user}</td><td>{pid}</td><td>{cpu}</td><td>{mem}</td><td><code>{cmd}</code></td></tr>"#,
                    user = html_escape(&r.user),
                    pid = html_escape(&r.pid),
                    cpu = html_escape(&r.cpu),
                    mem = html_escape(&r.mem),
                    cmd = html_escape(&r.command),
                ));
            }
            t.push_str("</tbody></table></div>");
            wrap_activity_table("top-process", "Filter user, PID, or command", &t)
        }
        Err(err) => format!(
            r#"<p class="panel-notice error">{e}</p>"#,
            e = html_escape(&err)
        ),
    };
    format!(
        r#"<div class="activity-panel-head">
          <h3>Top Process</h3>
          <a href="/server/processes" style="font-size:13px;font-weight:700;">Open full process list</a>
        </div>
        <p class="muted" style="margin:0 0 10px;">Live snapshot from ps (CPU sorted). Full page: Server → Top Processes.</p>
        {body}"#,
        body = body,
    )
}

fn traffic_panel() -> String {
    let rows = network_traffic();
    let table = if rows.is_empty() {
        r#"<p class="empty-state">Network counters unavailable (Linux /proc/net/dev required).</p>"#
            .to_string()
    } else {
        let mut t = String::from(
            r#"<div class="table-wrap"><table class="data-table"><thead><tr><th>Interface</th><th>RX</th><th>TX</th><th>RX pkts</th><th>TX pkts</th></tr></thead><tbody>"#,
        );
        for r in rows {
            t.push_str(&format!(
                r#"<tr><td><code>{name}</code></td><td>{rx}</td><td>{tx}</td><td>{rp}</td><td>{tp}</td></tr>"#,
                name = html_escape(&r.name),
                rx = html_escape(&format_bytes(r.rx_bytes)),
                tx = html_escape(&format_bytes(r.tx_bytes)),
                rp = r.rx_packets,
                tp = r.tx_packets,
            ));
        }
        t.push_str("</tbody></table></div>");
        wrap_activity_table("traffic", "Filter interface name", &t)
    };
    format!(
        r#"<div class="activity-panel-head"><h3>Traffic</h3>
        <p class="muted" style="margin:0;">Cumulative counters since boot (not live bandwidth).</p></div>
        {table}"#,
        table = table,
    )
}

fn disk_io_panel() -> String {
    let rows = disk_io_snapshot();
    let table = if rows.is_empty() {
        r#"<p class="empty-state">Disk IO stats unavailable (Linux /proc/diskstats required).</p>"#
            .to_string()
    } else {
        let mut t = String::from(
            r#"<div class="table-wrap"><table class="data-table"><thead><tr><th>Device</th><th>Reads</th><th>Writes</th><th>Read sectors</th><th>Write sectors</th></tr></thead><tbody>"#,
        );
        for r in rows {
            t.push_str(&format!(
                r#"<tr><td><code>{dev}</code></td><td>{reads}</td><td>{writes}</td><td>{rs}</td><td>{ws}</td></tr>"#,
                dev = html_escape(&r.device),
                reads = r.reads,
                writes = r.writes,
                rs = r.read_sectors,
                ws = r.write_sectors,
            ));
        }
        t.push_str("</tbody></table></div>");
        wrap_activity_table("disk-io", "Filter device name", &t)
    };
    format!(
        r#"<div class="activity-panel-head"><h3>Disk IO</h3>
        <p class="muted" style="margin:0;">Kernel counters since boot from /proc/diskstats.</p></div>
        {table}"#,
        table = table,
    )
}

fn cpu_panel() -> String {
    let cpu = cpu_activity();
    let pct = cpu
        .percent
        .map(|n| format!("{n}%"))
        .unwrap_or_else(|| "n/a".into());
    format!(
        r#"<div class="activity-panel-head"><h3>CPU Usage</h3>
        <p class="muted" style="margin:0;">Same host sample as the dashboard gauges (since-boot average from /proc/stat).</p></div>
        <div class="activity-kpi">
          <article><span>CPU sample</span><strong>{pct}</strong></article>
          <article><span>Detail</span><strong style="font-size:16px;">{detail}</strong></article>
          <article><span>Load average</span><strong style="font-size:16px;">{load}</strong></article>
        </div>
        <p class="muted">Gauges above the Activity Board update on each page load.</p>"#,
        pct = html_escape(&pct),
        detail = html_escape(&cpu.detail),
        load = html_escape(&cpu.loadavg),
    )
}

fn activity_script() -> String {
    let mut js = String::from(
        r#"
(function(){
  var root=document.getElementById('activity-board');
  if(!root) return;
  var tabs=[].slice.call(root.querySelectorAll('[data-activity-tab]'));
  var panels=[].slice.call(root.querySelectorAll('.activity-panel'));
  function activate(id){
    tabs.forEach(function(btn){
      var on=btn.getAttribute('data-activity-tab')===id;
      btn.setAttribute('aria-selected', on?'true':'false');
    });
    panels.forEach(function(panel){
      var on=panel.id==='activity-panel-'+id;
      if(on) panel.removeAttribute('hidden'); else panel.setAttribute('hidden','');
    });
    try{ history.replaceState(null,'','#activity-'+id); }catch(e){}
  }
  tabs.forEach(function(btn){
    btn.addEventListener('click', function(){ activate(btn.getAttribute('data-activity-tab')); });
  });
  var hash=(location.hash||'').replace(/^#/,'');
  var want=null;
  if(hash.indexOf('activity-')===0) want=hash.slice('activity-'.length);
  try{
    var q=new URLSearchParams(location.search||'');
    var qAct=q.get('activity');
    if(qAct) want=qAct;
  }catch(e){}
  if(want && root.querySelector('[data-activity-tab="'+want+'"]')) activate(want);
})();
"#,
    );
    js.push_str(activity_list_script());
    js
}

/// Full Activity Board for panel admins; non-admins get a short placeholder.
pub fn activity_board_html(username: &str) -> String {
    if !is_panel_admin(username) {
        return r#"
        <article class="activity-card" id="activity-board">
          <p class="eyebrow">RECENT ACTIVITY</p>
          <h2>Activity Board</h2>
          <p class="empty-state">SSH logs and host counters are available to the panel admin only.</p>
        </article>"#
            .into();
    }

    let analysis = ssh_security_analysis();
    let badge = if analysis.alert_count > 0 || analysis.failed_logins > 0 {
        Some(if analysis.alert_count > 0 {
            analysis.alert_count
        } else {
            1
        })
    } else {
        None
    };

    let tabs = [
        tab_btn("ssh-logins", "Recent SSH Logins", true, None),
        tab_btn("ssh-logs", "Recent SSH Logs", false, badge),
        tab_btn("top-process", "Top Process", false, None),
        tab_btn("traffic", "Traffic", false, None),
        tab_btn("disk-io", "Disk IO", false, None),
        tab_btn("cpu", "CPU Usage", false, None),
    ]
    .join("\n");

    let panels = [
        panel("ssh-logins", false, &ssh_logins_panel()),
        panel("ssh-logs", true, &ssh_logs_panel(username, &analysis)),
        panel("top-process", true, &top_process_panel()),
        panel("traffic", true, &traffic_panel()),
        panel("disk-io", true, &disk_io_panel()),
        panel("cpu", true, &cpu_panel()),
    ]
    .join("\n");

    format!(
        r#"
      <section class="activity-board" id="activity-board" aria-label="Activity Board">
        <p class="eyebrow">RECENT ACTIVITY</p>
        <h2>Activity Board</h2>
        <div class="activity-tabs" role="tablist" aria-label="Activity Board tabs">
          {tabs}
        </div>
        {panels}
      </section>
      <script>{script}</script>"#,
        tabs = tabs,
        panels = panels,
        script = activity_script(),
    )
}

#[cfg(test)]
mod tests {
    use super::activity_board_html;

    #[test]
    fn non_admin_sees_restricted_notice() {
        let html = activity_board_html("not-an-admin-user-xyz");
        assert!(html.contains("Activity Board"));
        assert!(html.contains("panel admin only"));
        assert!(!html.contains("data-activity-tab=\"ssh-logins\""));
    }

    #[test]
    fn wrap_helper_exports_list_controls_markup() {
        use crate::panel_dashboard_activity_list::wrap_activity_table;
        let table = r#"<div class="table-wrap"><table class="data-table"><tbody><tr><td>ok</td></tr></tbody></table></div>"#;
        let html = wrap_activity_table("demo", "Filter", table);
        assert!(html.contains("Go to page"));
        assert!(html.contains("activity-list-search"));
        assert!(html.contains("data-page-size=\"10\""));
    }
}
