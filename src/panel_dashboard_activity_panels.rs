//! Activity Board tab bodies (SSH tables, processes, traffic, disk, CPU).

use crate::panel_dashboard_activity_list::wrap_activity_table;
use crate::panel_hub_pages_server_logs::panel_actions_table_sized;
use crate::panel_ops_activity::ActivityLogRow;
use crate::panel_ops_activity_host::{
    cpu_activity, disk_io_snapshot, format_bytes, format_grouped_u64, network_traffic,
};
use crate::panel_ops_process::{snapshot_top_processes, truncate_command};

pub(crate) fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub(crate) fn more_details(summary: &str, body_html: &str) -> String {
    format!(
        r#"<details class="activity-more"><summary>{sum}</summary><div class="activity-more-body">{body}</div></details>"#,
        sum = html_escape(summary),
        body = body_html,
    )
}

fn clip_code(full: &str, max: usize) -> String {
    let shown = truncate_command(full, max);
    format!(
        r#"<code class="activity-clip" title="{title}">{shown}</code>"#,
        title = html_escape(full),
        shown = html_escape(&shown),
    )
}

pub(crate) fn log_table(rows: &[ActivityLogRow], empty: &str) -> String {
    if rows.is_empty() {
        return format!(r#"<p class="empty-state">{e}</p>"#, e = html_escape(empty));
    }
    let mut t = String::from(
        r#"<div class="table-wrap"><table class="data-table"><thead><tr><th>Timestamp</th><th>Message</th><th>More</th></tr></thead><tbody>"#,
    );
    for row in rows {
        t.push_str(&format!(
            r#"<tr><td><time>{ts}</time></td><td>{msg}</td><td>{more}</td></tr>"#,
            ts = html_escape(&row.timestamp),
            msg = clip_code(&row.message, 96),
            more = more_details(
                "More",
                &format!(
                    "<p><strong>Timestamp</strong> {ts}</p><pre>{msg}</pre>",
                    ts = html_escape(&row.timestamp),
                    msg = html_escape(&row.message)
                ),
            ),
        ));
    }
    t.push_str("</tbody></table></div>");
    t
}

fn compare_chart(title: &str, caption: &str, rows: &[(String, String, u64, String, u64)]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let max = rows.iter().map(|r| r.2.max(r.4)).max().unwrap_or(1).max(1);
    let mut bars = String::new();
    for (name, a_label, a_val, b_label, b_val) in rows {
        let a_pct = ((*a_val as f64 / max as f64) * 100.0).clamp(0.0, 100.0);
        let b_pct = ((*b_val as f64 / max as f64) * 100.0).clamp(0.0, 100.0);
        bars.push_str(&format!(
            r#"<div class="activity-chart-row">
  <strong>{name}</strong>
  <div class="activity-chart-pair">
    <span>{a_lab}</span>
    <div class="activity-bar" title="{a_lab}"><i style="width:{a_pct:.1}%"></i></div>
    <span class="activity-chart-n">{a_n}</span>
  </div>
  <div class="activity-chart-pair">
    <span>{b_lab}</span>
    <div class="activity-bar activity-bar-b" title="{b_lab}"><i style="width:{b_pct:.1}%"></i></div>
    <span class="activity-chart-n">{b_n}</span>
  </div>
</div>"#,
            name = html_escape(name),
            a_lab = html_escape(a_label),
            b_lab = html_escape(b_label),
            a_pct = a_pct,
            b_pct = b_pct,
            a_n = html_escape(&format_grouped_u64(*a_val)),
            b_n = html_escape(&format_grouped_u64(*b_val)),
        ));
    }
    format!(
        r#"<figure class="activity-chart" data-activity-chart>
  <figcaption>{title}<span class="muted"> {cap}</span></figcaption>
  {bars}
</figure>"#,
        title = html_escape(title),
        cap = html_escape(caption),
        bars = bars,
    )
}

pub fn ssh_logins_panel() -> String {
    let rows = crate::panel_ops_activity::recent_ssh_logins(200);
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
        {table}"#
    )
}

pub fn top_process_panel() -> String {
    let body = match snapshot_top_processes(50) {
        Ok(rows) if rows.is_empty() => "<p class=\"empty-state\">No processes returned.</p>".into(),
        Ok(rows) => {
            let mut t = String::from(
                r#"<div class="table-wrap"><table class="data-table activity-proc-table"><thead><tr><th>User</th><th>PID</th><th>CPU%</th><th>MEM%</th><th>Command</th><th>Action</th></tr></thead><tbody>"#,
            );
            for r in rows {
                let full = html_escape(&r.command);
                t.push_str(&format!(
                    r#"<tr class="activity-proc-row"><td>{user}</td><td>{pid}</td><td>{cpu}</td><td>{mem}</td><td>{cmd}</td><td>{act}</td></tr>"#,
                    user = html_escape(&r.user),
                    pid = html_escape(&r.pid),
                    cpu = html_escape(&r.cpu),
                    mem = html_escape(&r.mem),
                    cmd = clip_code(&r.command, 64),
                    act = more_details(
                        "Manage",
                        &format!(
                            "<p><strong>User</strong> {user} · <strong>PID</strong> {pid}</p><p>CPU {cpu}% · MEM {mem}%</p><pre>{cmd}</pre><p><a href=\"/server/processes\">Open full process list</a></p>",
                            user = html_escape(&r.user),
                            pid = html_escape(&r.pid),
                            cpu = html_escape(&r.cpu),
                            mem = html_escape(&r.mem),
                            cmd = full,
                        ),
                    ),
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
        <p class="muted" style="margin:0 0 10px;">Live snapshot from ps (CPU sorted). Commands are truncated here. Use Manage for the full line, or Server → Top Processes.</p>
        {body}"#
    )
}

pub fn traffic_panel(minimalist: bool) -> String {
    let rows = network_traffic();
    let chart_note = if minimalist {
        "Snapshot until you refresh (Minimalist mode)."
    } else {
        "Cumulative counters since boot (not live bandwidth)."
    };
    let table = if rows.is_empty() {
        r#"<p class="empty-state">Network counters unavailable (Linux /proc/net/dev required).</p>"#
            .to_string()
    } else {
        let chart_rows: Vec<(String, String, u64, String, u64)> = rows
            .iter()
            .map(|r| {
                (
                    r.name.clone(),
                    "RX B".into(),
                    r.rx_bytes,
                    "TX B".into(),
                    r.tx_bytes,
                )
            })
            .collect();
        let chart = compare_chart("Traffic share", chart_note, &chart_rows);
        let mut t = String::from(
            r#"<div class="table-wrap"><table class="data-table"><thead><tr><th>Interface</th><th>RX</th><th>TX</th><th>RX pkts</th><th>TX pkts</th><th>More</th></tr></thead><tbody>"#,
        );
        for r in &rows {
            t.push_str(&format!(
                r#"<tr><td><code>{name}</code></td><td>{rx}</td><td>{tx}</td><td>{rp}</td><td>{tp}</td><td>{more}</td></tr>"#,
                name = html_escape(&r.name),
                rx = html_escape(&format_bytes(r.rx_bytes)),
                tx = html_escape(&format_bytes(r.tx_bytes)),
                rp = html_escape(&format_grouped_u64(r.rx_packets)),
                tp = html_escape(&format_grouped_u64(r.tx_packets)),
                more = more_details(
                    "More",
                    &format!(
                        "<p><strong>{name}</strong></p><p>RX {rx} ({rp} packets)</p><p>TX {tx} ({tp} packets)</p>",
                        name = html_escape(&r.name),
                        rx = html_escape(&format_bytes(r.rx_bytes)),
                        tx = html_escape(&format_bytes(r.tx_bytes)),
                        rp = html_escape(&format_grouped_u64(r.rx_packets)),
                        tp = html_escape(&format_grouped_u64(r.tx_packets)),
                    ),
                ),
            ));
        }
        t.push_str("</tbody></table></div>");
        format!(
            "{chart}{table}",
            chart = chart,
            table = wrap_activity_table("traffic", "Filter interface name", &t)
        )
    };
    format!(
        r#"<div class="activity-panel-head"><h3>Traffic</h3>
        <p class="muted" style="margin:0;">{note}</p></div>
        {table}"#,
        note = html_escape(chart_note),
        table = table,
    )
}

pub fn disk_io_panel(minimalist: bool) -> String {
    let rows = disk_io_snapshot();
    let chart_note = if minimalist {
        "Kernel counters since boot. Snapshot until you refresh (Minimalist mode)."
    } else {
        "Kernel counters since boot from /proc/diskstats."
    };
    let table = if rows.is_empty() {
        r#"<p class="empty-state">Disk IO stats unavailable (Linux /proc/diskstats required).</p>"#
            .to_string()
    } else {
        let chart_rows: Vec<(String, String, u64, String, u64)> = rows
            .iter()
            .map(|r| {
                (
                    r.device.clone(),
                    "Reads".into(),
                    r.reads,
                    "Writes".into(),
                    r.writes,
                )
            })
            .collect();
        let chart = compare_chart("Disk IO share", chart_note, &chart_rows);
        let mut t = String::from(
            r#"<div class="table-wrap"><table class="data-table"><thead><tr><th>Device</th><th>Reads</th><th>Writes</th><th>Read sectors</th><th>Write sectors</th><th>More</th></tr></thead><tbody>"#,
        );
        for r in &rows {
            t.push_str(&format!(
                r#"<tr><td><code>{dev}</code></td><td>{reads}</td><td>{writes}</td><td>{rs}</td><td>{ws}</td><td>{more}</td></tr>"#,
                dev = html_escape(&r.device),
                reads = html_escape(&format_grouped_u64(r.reads)),
                writes = html_escape(&format_grouped_u64(r.writes)),
                rs = html_escape(&format_grouped_u64(r.read_sectors)),
                ws = html_escape(&format_grouped_u64(r.write_sectors)),
                more = more_details(
                    "More",
                    &format!(
                        "<p><strong>{dev}</strong></p><p>Reads {reads} · Writes {writes}</p><p>Read sectors {rs} · Write sectors {ws}</p>",
                        dev = html_escape(&r.device),
                        reads = html_escape(&format_grouped_u64(r.reads)),
                        writes = html_escape(&format_grouped_u64(r.writes)),
                        rs = html_escape(&format_grouped_u64(r.read_sectors)),
                        ws = html_escape(&format_grouped_u64(r.write_sectors)),
                    ),
                ),
            ));
        }
        t.push_str("</tbody></table></div>");
        format!(
            "{chart}{table}",
            chart = chart,
            table = wrap_activity_table("disk-io", "Filter device name", &t)
        )
    };
    format!(
        r#"<div class="activity-panel-head"><h3>Disk IO</h3>
        <p class="muted" style="margin:0;">{note}</p></div>
        {table}"#,
        note = html_escape(chart_note),
        table = table,
    )
}

pub fn cpu_panel() -> String {
    let cpu = cpu_activity();
    let pct_n = cpu.percent.unwrap_or(0);
    let pct = cpu
        .percent
        .map(|n| format!("{n}%"))
        .unwrap_or_else(|| "n/a".into());
    let more = more_details(
        "More",
        "<p>This figure is the same host sample as the dashboard gauges (since-boot average from /proc/stat). It is not a 1-second live poll.</p>",
    );
    format!(
        r#"<div class="activity-panel-head"><h3>CPU Usage</h3>
        <p class="muted" style="margin:0;">Same host sample as the dashboard gauges (since-boot average from /proc/stat).</p></div>
        <div class="activity-kpi">
          <article><span>CPU sample</span><strong>{pct}</strong>
            <div class="activity-bar" style="margin-top:8px;" title="CPU sample"><i style="width:{pct_n}%"></i></div>
            {more}
          </article>
          <article><span>Detail</span><strong style="font-size:16px;">{detail}</strong></article>
          <article><span>Load average</span><strong style="font-size:16px;">{load}</strong></article>
        </div>
        <p class="muted">Gauges above the Activity Board update on each page load.</p>"#,
        pct = html_escape(&pct),
        pct_n = pct_n.min(100),
        more = more,
        detail = html_escape(&cpu.detail),
        load = html_escape(&cpu.loadavg),
    )
}

pub fn panel_actions_panel(username: &str) -> String {
    format!(
        r#"<div class="activity-panel-head"><h3>Panel Actions</h3>
        <p class="muted" style="margin:0;">Plugin and host-package actions for this node.</p></div>
        {table}"#,
        table = panel_actions_table_sized(username, 5),
    )
}
