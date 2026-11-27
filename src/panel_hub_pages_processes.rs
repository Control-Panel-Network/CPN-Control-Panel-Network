//! System Processes Monitor page (Server → Top Processes).

use crate::panel_hubs::feature_shell;
use crate::panel_ops_process::{
    ProcessRow, cpu_heat_class, snapshot_top_processes, truncate_command,
};
use crate::panel_ops_process_host::{HostProcessSummary, snapshot_host_process_summary};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn processes_page_styles() -> &'static str {
    r#"<style>
.spm-toolbar{display:flex;flex-wrap:wrap;gap:12px;align-items:center;justify-content:space-between;margin:0 0 16px;}
.spm-toolbar .muted{margin:0;max-width:56ch;}
.spm-toolbar-actions{display:flex;flex-wrap:wrap;gap:8px;align-items:center;}
.spm-toolbar .btn-secondary,.spm-kill-btn{
  display:inline-flex;align-items:center;justify-content:center;min-height:40px;padding:0 16px;
  border:0;border-radius:999px;font-weight:700;text-decoration:none;cursor:pointer;
  background:#e2e8f0;color:#0f172a;white-space:nowrap;
}
[data-color-mode="dark"] .spm-toolbar .btn-secondary{background:#334155;color:#f8fafc;}
.spm-grid{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:14px;margin:0 0 18px;}
.spm-card{border:1px solid var(--hairline);border-radius:14px;padding:14px 16px;background:var(--canvas);min-width:0;}
.spm-card h3{margin:0 0 10px;font-size:0.95rem;}
.spm-kv{display:grid;grid-template-columns:auto 1fr;gap:4px 12px;margin:0;font-size:0.9rem;}
.spm-kv dt{color:var(--muted);font-weight:600;}
.spm-kv dd{margin:0;font-variant-numeric:tabular-nums;text-align:right;word-break:break-word;}
.spm-good{color:#15803d;font-weight:700;}
.spm-bad{color:#b42318;font-weight:700;}
[data-color-mode="dark"] .spm-good{color:#4ade80;}
[data-color-mode="dark"] .spm-bad{color:#fda29b;}
.spm-section{border:1px solid var(--hairline);border-radius:14px;padding:14px 16px;background:var(--canvas);}
.spm-section-head{display:flex;flex-wrap:wrap;gap:12px;align-items:center;justify-content:space-between;margin:0 0 12px;}
.spm-section-head h2{margin:0;font-size:1.05rem;}
.spm-search{min-width:min(100%,240px);padding:8px 12px;border-radius:10px;border:1px solid var(--hairline);background:transparent;color:inherit;}
.spm-table-wrap{overflow-x:auto;}
.spm-table-wrap .data-table{width:100%;min-width:920px;}
.spm-cmd{display:block;max-width:28ch;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;}
.spm-metric{font-variant-numeric:tabular-nums;font-weight:600;}
.spm-bar{display:inline-flex;align-items:center;gap:8px;min-width:7.5rem;}
.spm-bar-track{flex:1;height:6px;border-radius:999px;background:rgba(148,163,184,.35);overflow:hidden;}
.spm-bar-fill{display:block;height:100%;border-radius:999px;background:#3b82f6;}
.spm-hot .spm-metric-cpu,.spm-hot .spm-bar-fill{color:#b42318;background:#ef4444;}
.spm-warm .spm-metric-cpu,.spm-warm .spm-bar-fill{color:#b54708;background:#f59e0b;}
[data-color-mode="dark"] .spm-hot .spm-metric-cpu{color:#fda29b;}
[data-color-mode="dark"] .spm-warm .spm-metric-cpu{color:#fec84b;}
.spm-state{display:inline-flex;align-items:center;padding:2px 8px;border-radius:999px;font-size:0.78rem;font-weight:700;}
.spm-state-run{background:rgba(34,197,94,.18);color:#15803d;}
.spm-state-sleep{background:rgba(59,130,246,.18);color:#1d4ed8;}
.spm-state-zom{background:rgba(239,68,68,.18);color:#b42318;}
.spm-state-other{background:rgba(148,163,184,.2);color:inherit;}
[data-color-mode="dark"] .spm-state-run{color:#4ade80;}
[data-color-mode="dark"] .spm-state-sleep{color:#93c5fd;}
[data-color-mode="dark"] .spm-state-zom{color:#fda29b;}
.spm-kill-btn{background:#dc2626;color:#fff;min-height:32px;padding:0 12px;font-size:0.82rem;}
.spm-kill-btn:hover{filter:brightness(1.05);}
.spm-list{display:none;margin-top:8px;gap:12px;}
.spm-card-row{border:1px solid var(--hairline);border-radius:12px;padding:12px 14px;background:var(--canvas);min-width:0;}
.spm-auto{display:flex;align-items:center;gap:8px;font-size:0.88rem;color:var(--muted);}
@media (max-width:1100px){.spm-grid{grid-template-columns:repeat(2,minmax(0,1fr));}}
@media (max-width:719.98px){
  .spm-grid{grid-template-columns:1fr;}
  .spm-table-wrap{display:none;}
  .spm-list{display:grid;}
  .spm-toolbar .btn-secondary{width:100%;}
}
@media (min-width:720px){.spm-list{display:none !important;}}
</style>"#
}

fn state_class(state: &str) -> &'static str {
    match state {
        "Running" => "spm-state-run",
        "Sleeping" | "Disk sleep" => "spm-state-sleep",
        "Zombie" => "spm-state-zom",
        _ => "spm-state-other",
    }
}

fn bar_width(pct: &str) -> u32 {
    let v = pct.trim().parse::<f64>().unwrap_or(0.0).clamp(0.0, 100.0);
    v.round() as u32
}

fn summary_cards(s: &HostProcessSummary) -> String {
    format!(
        r#"<div class="spm-grid" aria-label="Host resource summary">
  <article class="spm-card"><h3>CPU Information</h3>
    <dl class="spm-kv">
      <dt>Cores</dt><dd>{cores}</dd>
      <dt>Frequency</dt><dd>{freq} MHz</dd>
      <dt>Model</dt><dd>{model}</dd>
      <dt>Cache</dt><dd>{cache} KB</dd>
    </dl>
  </article>
  <article class="spm-card"><h3>Process Statistics</h3>
    <dl class="spm-kv">
      <dt>Total</dt><dd>{total}</dd>
      <dt>Running</dt><dd class="spm-good">{running}</dd>
      <dt>Sleeping</dt><dd>{sleeping}</dd>
      <dt>Zombie</dt><dd class="spm-bad">{zombie}</dd>
    </dl>
  </article>
  <article class="spm-card"><h3>CPU Load Average</h3>
    <dl class="spm-kv">
      <dt>1 Min</dt><dd>{one}</dd>
      <dt>5 Min</dt><dd>{five}</dd>
      <dt>15 Min</dt><dd>{fifteen}</dd>
      <dt>Current</dt><dd>{current}%</dd>
    </dl>
  </article>
  <article class="spm-card"><h3>Memory Usage</h3>
    <dl class="spm-kv">
      <dt>Total</dt><dd>{mem_total}MB</dd>
      <dt>Free</dt><dd class="spm-good">{mem_free}MB</dd>
      <dt>Used</dt><dd class="spm-bad">{mem_used}MB</dd>
      <dt>Buff/Cache</dt><dd>{buff}MB</dd>
    </dl>
  </article>
  <article class="spm-card"><h3>Swap Memory</h3>
    <dl class="spm-kv">
      <dt>Total</dt><dd>{swap_total}MB</dd>
      <dt>Free</dt><dd class="spm-good">{swap_free}MB</dd>
      <dt>Used</dt><dd class="spm-bad">{swap_used}MB</dd>
      <dt>Available</dt><dd>{swap_avail}MB</dd>
    </dl>
  </article>
  <article class="spm-card"><h3>CPU Time Distribution</h3>
    <dl class="spm-kv">
      <dt>I/O Wait</dt><dd>{iowait}%</dd>
      <dt>Idle Time</dt><dd>{idle}%</dd>
      <dt>HW Interrupts</dt><dd>{hirq}%</dd>
      <dt>Soft IRQs</dt><dd>{sirq}%</dd>
    </dl>
  </article>
</div>"#,
        cores = s.cpu.cores,
        freq = html_escape(&s.cpu.frequency_mhz),
        model = html_escape(&s.cpu.model),
        cache = html_escape(&s.cpu.cache_kb),
        total = s.processes.total,
        running = s.processes.running,
        sleeping = s.processes.sleeping,
        zombie = s.processes.zombie,
        one = html_escape(&s.load.one),
        five = html_escape(&s.load.five),
        fifteen = html_escape(&s.load.fifteen),
        current = html_escape(&s.load.current_pct),
        mem_total = s.memory.total_mb,
        mem_free = s.memory.free_mb,
        mem_used = s.memory.used_mb,
        buff = s.memory.buff_cache_mb,
        swap_total = s.swap.total_mb,
        swap_free = s.swap.free_mb,
        swap_used = s.swap.used_mb,
        swap_avail = s.swap.available_mb,
        iowait = html_escape(&s.cpu_time.iowait_pct),
        idle = html_escape(&s.cpu_time.idle_pct),
        hirq = html_escape(&s.cpu_time.hard_irq_pct),
        sirq = html_escape(&s.cpu_time.soft_irq_pct),
    )
}

fn metric_bar(pct: &str, kind: &str) -> String {
    let w = bar_width(pct);
    format!(
        r#"<span class="spm-bar"><span class="spm-bar-track"><span class="spm-bar-fill" style="width:{w}%;"></span></span><span class="spm-metric {kind}">{pct}</span></span>"#,
        w = w,
        kind = kind,
        pct = html_escape(pct),
    )
}

fn render_process_rows(rows: &[ProcessRow], show_kill: bool) -> (String, String) {
    let mut table = String::from(
        r#"<div class="table-wrap spm-table-wrap"><table class="data-table" id="spm-table"><thead><tr>
        <th>PID</th><th>USER</th><th>STATE</th><th>CPU %</th><th>MEMORY %</th><th>VIRT</th><th>RES</th><th>TIME</th><th>COMMAND</th>"#,
    );
    if show_kill {
        table.push_str("<th>ACTIONS</th>");
    }
    table.push_str("</tr></thead><tbody>");
    let mut cards = String::from(r#"<div class="spm-list" aria-label="Running processes">"#);
    for row in rows {
        let heat = cpu_heat_class(&row.cpu);
        let row_class = if heat.is_empty() {
            String::new()
        } else if heat == "proc-hot" {
            r#" class="spm-hot""#.to_string()
        } else {
            r#" class="spm-warm""#.to_string()
        };
        let cmd_full = html_escape(&row.command);
        let cmd_short = html_escape(&truncate_command(&row.command, 64));
        let user = html_escape(&row.user);
        let pid = html_escape(&row.pid);
        let state = html_escape(&row.state);
        let state_cls = state_class(&row.state);
        let cpu_bar = metric_bar(&row.cpu, "spm-metric-cpu");
        let mem_bar = metric_bar(&row.mem, "");
        let virt = html_escape(&row.virt);
        let res = html_escape(&row.res);
        let time = html_escape(&row.time);
        let kill_cell = if show_kill {
            format!(
                r#"<td data-label="ACTIONS"><form method="post" action="/server/processes/kill" class="spm-kill-form" onsubmit="return confirm('Send SIGTERM to PID {pid}?');">
              <input type="hidden" name="pid" value="{pid}">
              <button type="submit" class="spm-kill-btn" title="Send SIGTERM">KILL</button>
            </form></td>"#
            )
        } else {
            String::new()
        };
        table.push_str(&format!(
            r#"<tr{row_class} data-proc-row data-search="{user} {pid} {state} {cmd_full}">
              <td data-label="PID"><code>{pid}</code></td>
              <td data-label="USER">{user}</td>
              <td data-label="STATE"><span class="spm-state {state_cls}">{state}</span></td>
              <td data-label="CPU %">{cpu_bar}</td>
              <td data-label="MEMORY %">{mem_bar}</td>
              <td data-label="VIRT">{virt}</td>
              <td data-label="RES">{res}</td>
              <td data-label="TIME">{time}</td>
              <td data-label="COMMAND"><code class="spm-cmd" title="{cmd_full}">{cmd_short}</code></td>
              {kill_cell}
            </tr>"#
        ));
        cards.push_str(&format!(
            r#"<article class="spm-card-row" data-proc-row data-search="{user} {pid} {state} {cmd_full}">
              <div style="display:flex;justify-content:space-between;gap:8px;flex-wrap:wrap;">
                <strong>{user}</strong><code>PID {pid}</code>
              </div>
              <p style="margin:8px 0;"><span class="spm-state {state_cls}">{state}</span>
                · CPU {cpu_bar} · MEM {mem_bar}</p>
              <p class="muted" style="margin:0;">VIRT {virt} · RES {res} · TIME {time}</p>
              <p style="margin:8px 0 0;"><code title="{cmd_full}">{cmd_short}</code></p>
              {kill_mobile}
            </article>"#,
            kill_mobile = if show_kill {
                format!(
                    r#"<form method="post" action="/server/processes/kill" style="margin-top:10px;" onsubmit="return confirm('Send SIGTERM to PID {pid}?');">
                  <input type="hidden" name="pid" value="{pid}">
                  <button type="submit" class="spm-kill-btn">KILL</button>
                </form>"#
                )
            } else {
                String::new()
            }
        ));
    }
    table.push_str("</tbody></table></div>");
    cards.push_str("</div>");
    (table, cards)
}

fn search_script() -> &'static str {
    r#"<script>
(function(){
  var input=document.getElementById('spm-search');
  if(!input) return;
  input.addEventListener('input', function(){
    var q=(input.value||'').toLowerCase().trim();
    document.querySelectorAll('[data-proc-row]').forEach(function(row){
      var hay=(row.getAttribute('data-search')||'').toLowerCase();
      row.style.display = (!q || hay.indexOf(q)!==-1) ? '' : 'none';
    });
  });
})();
</script>"#
}

/// Build the System Processes Monitor page.
/// `show_kill` is true only for panel admins.
pub fn processes_page(show_kill: bool) -> String {
    let styles = processes_page_styles();
    let summary = snapshot_host_process_summary();
    let cards = summary_cards(&summary);
    let toolbar = format!(
        r#"<div class="spm-toolbar">
  <p class="muted">One-shot snapshot from <code>ps</code> and <code>/proc</code> (CPU sorted). No auto-refresh by default.</p>
  <div class="spm-toolbar-actions">
    <label class="spm-auto"><input type="checkbox" id="spm-auto" disabled> Optional auto-refresh (off)</label>
    <a class="btn-secondary" href="/server/processes">Refresh</a>
  </div>
</div>"#
    );
    let body_main = match snapshot_top_processes(80) {
        Ok(rows) if rows.is_empty() => {
            format!("{toolbar}{cards}<p class=\"empty-state\">No processes returned.</p>")
        }
        Ok(rows) => {
            let (table, list) = render_process_rows(&rows, show_kill);
            format!(
                r#"{toolbar}{cards}
<div class="spm-section">
  <div class="spm-section-head">
    <h2>Running Processes</h2>
    <input type="search" id="spm-search" class="spm-search" placeholder="Search processes..." autocomplete="off">
  </div>
  {table}{list}
</div>
{script}"#,
                script = search_script()
            )
        }
        Err(err) => format!(
            "{toolbar}{cards}<p class=\"panel-notice error\">{e}</p>",
            e = html_escape(&err)
        ),
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("Top Processes", None),
        ],
        "System Processes Monitor",
        "CPU, memory, and running processes. Manual refresh only.",
        &format!("{styles}{body_main}"),
        None,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::processes_page;

    #[test]
    fn processes_page_includes_monitor_chrome() {
        let html = processes_page(true);
        assert!(html.contains("System Processes Monitor") || html.contains("spm-toolbar"));
        assert!(html.contains("Refresh"));
        assert!(html.contains("spm-grid") || html.contains("panel-notice error"));
        assert!(
            html.contains("Running Processes")
                || html.contains("empty-state")
                || html.contains("panel-notice error")
        );
        assert!(
            html.contains("spm-search")
                || html.contains("empty-state")
                || html.contains("panel-notice error")
        );
        assert!(!html.to_lowercase().contains("auto-refresh every"));
    }

    #[test]
    fn kill_hidden_for_non_admin() {
        let html = processes_page(false);
        assert!(!html.contains("/server/processes/kill"));
        assert!(!html.contains(">KILL</button>"));
    }
}
