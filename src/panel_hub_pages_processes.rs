//! System Processes Monitor page (Server → Top Processes).

use crate::panel_ops_process::{
    ProcessRow, cpu_heat_class, snapshot_top_processes, truncate_command,
};
use crate::panel_ops_process_host::{HostProcessSummary, snapshot_host_process_summary};
use crate::panel_storage_fmt::{format_host_mib_for_user, format_kib_for_user};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Shared SPM tab styles (also used by the tabbed hub).
pub(crate) fn processes_tab_styles() -> &'static str {
    r#"<style>
.spm-toolbar{display:flex;flex-wrap:wrap;gap:12px;align-items:center;justify-content:space-between;margin:0 0 16px;}
.spm-toolbar .muted{margin:0;max-width:62ch;}
.spm-toolbar-actions{display:flex;flex-wrap:wrap;gap:10px;align-items:center;}
.spm-toolbar .btn-secondary,.spm-kill-btn{
  display:inline-flex;align-items:center;justify-content:center;min-height:40px;padding:0 16px;
  border:0;border-radius:999px;font-weight:700;text-decoration:none;cursor:pointer;
  background:#e2e8f0;color:#0f172a;white-space:nowrap;
}
[data-color-mode="dark"] .spm-toolbar .btn-secondary{background:#334155;color:#f8fafc;}
.spm-refresh-label{display:inline-flex;align-items:center;gap:8px;font-size:0.88rem;color:var(--muted);font-weight:600;}
.spm-refresh-select{
  min-height:40px;padding:0 12px;border-radius:10px;border:1px solid var(--hairline);
  background:transparent;color:inherit;font:inherit;font-weight:600;
}
.spm-grid{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:14px;margin:0 0 18px;}
.spm-card{border:1px solid var(--hairline);border-radius:14px;padding:14px 16px;background:var(--canvas);min-width:0;}
.spm-card h3{margin:0 0 10px;font-size:0.95rem;}
.spm-kv{display:grid;grid-template-columns:auto 1fr;gap:4px 12px;margin:0;font-size:0.9rem;}
.spm-kv dt{color:var(--muted);font-weight:600;}
.spm-kv dd{margin:0;font-variant-numeric:tabular-nums;text-align:right;word-break:break-word;}
.spm-load-card .spm-kv dt{min-width:7.5rem;}
.spm-kv dt.spm-load-5,.spm-kv dd.spm-load-5{
  background:rgba(59,130,246,.12);padding:4px 6px;margin:0 -6px;
}
.spm-kv dt.spm-load-5{border-radius:8px 0 0 8px;color:inherit;}
.spm-kv dd.spm-load-5{border-radius:0 8px 8px 0;font-size:1.1rem;font-weight:800;}
.spm-load-hint{margin:10px 0 0;font-size:0.8rem;color:var(--muted);}
.spm-good{color:#15803d;font-weight:700;}
.spm-bad{color:#b42318;font-weight:700;}
[data-color-mode="dark"] .spm-good{color:#4ade80;}
[data-color-mode="dark"] .spm-bad{color:#fda29b;}
.spm-tip{
  margin:0 0 16px;padding:12px 14px;border-radius:12px;border:1px solid var(--hairline);
  background:rgba(59,130,246,.08);font-size:0.9rem;line-height:1.45;
}
.spm-tip strong{font-weight:700;}
.spm-section{border:1px solid var(--hairline);border-radius:14px;padding:14px 16px;background:var(--canvas);}
.spm-section-head{display:flex;flex-wrap:wrap;gap:12px;align-items:center;justify-content:space-between;margin:0 0 8px;}
.spm-section-head h2{margin:0;font-size:1.05rem;}
.spm-cpu-caption{margin:0 0 12px;font-size:0.82rem;color:var(--muted);}
.spm-search{min-width:min(100%,240px);padding:8px 12px;border-radius:10px;border:1px solid var(--hairline);background:transparent;color:inherit;}
.spm-table-wrap{overflow-x:auto;}
.spm-table-wrap .data-table{width:100%;min-width:1080px;table-layout:auto;}
.spm-table-wrap .data-table th,.spm-table-wrap .data-table td{vertical-align:middle;}
.spm-col-pid{white-space:nowrap;width:1%;font-variant-numeric:tabular-nums;font-weight:600;}
.spm-col-user{white-space:nowrap;max-width:16ch;overflow:hidden;text-overflow:ellipsis;}
.spm-col-state{white-space:nowrap;}
.spm-col-cpu,.spm-col-mem{white-space:nowrap;min-width:9.5rem;}
.spm-col-virt,.spm-col-res,.spm-col-time{white-space:nowrap;font-variant-numeric:tabular-nums;}
.spm-cmd{display:block;max-width:28ch;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;}
.spm-metric{font-variant-numeric:tabular-nums;font-weight:600;min-width:3.6rem;}
.spm-bar{display:inline-flex;align-items:center;gap:8px;min-width:8.5rem;}
.spm-bar-track{flex:1;height:6px;border-radius:999px;background:rgba(148,163,184,.35);overflow:hidden;min-width:3.5rem;}
.spm-bar-fill{display:block;height:100%;border-radius:999px;background:#3b82f6;}
.spm-hot .spm-metric-cpu,.spm-hot .spm-bar-fill{color:#b42318;background:#ef4444;}
.spm-warm .spm-metric-cpu,.spm-warm .spm-bar-fill{color:#b54708;background:#f59e0b;}
[data-color-mode="dark"] .spm-hot .spm-metric-cpu{color:#fda29b;}
[data-color-mode="dark"] .spm-warm .spm-metric-cpu{color:#fec84b;}
.spm-cpu-all{display:block;font-size:0.72rem;color:var(--muted);font-weight:600;margin-top:2px;}
.spm-state{display:inline-flex;align-items:center;padding:2px 8px;border-radius:999px;font-size:0.78rem;font-weight:700;white-space:nowrap;}
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
@media (max-width:1100px){.spm-grid{grid-template-columns:repeat(2,minmax(0,1fr));}}
@media (max-width:719.98px){
  .spm-grid{grid-template-columns:1fr;}
  .spm-table-wrap{display:none;}
  .spm-list{display:grid;}
  .spm-toolbar .btn-secondary{width:100%;}
  .spm-refresh-label{width:100%;}
  .spm-refresh-select{flex:1;}
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

/// Bar width for memory % (0-100 of host RAM).
fn bar_width_mem(pct: &str) -> u32 {
    let v = pct.trim().parse::<f64>().unwrap_or(0.0).clamp(0.0, 100.0);
    v.round() as u32
}

/// Bar width for ps CPU%: scale against all cores so multi-core values stay readable.
/// Raw ps % is still shown as text; 100% of one core fills `100/cores` of the track.
fn bar_width_cpu(pct: &str, cores: u32) -> u32 {
    let cores = cores.max(1) as f64;
    let v = pct.trim().parse::<f64>().unwrap_or(0.0).max(0.0);
    let of_all = (v / (cores * 100.0)) * 100.0;
    of_all.clamp(0.0, 100.0).round() as u32
}

fn cpu_of_all_label(pct: &str, cores: u32) -> String {
    let cores = cores.max(1) as f64;
    let v = pct.trim().parse::<f64>().unwrap_or(0.0).max(0.0);
    let of_all = (v / (cores * 100.0)) * 100.0;
    format!("{of_all:.1}% of all cores")
}

fn summary_cards(s: &HostProcessSummary, viewer: &str) -> String {
    let cores = s.cpu.cores.max(1);
    let cache_kb: u64 = s.cpu.cache_kb.trim().parse().unwrap_or(0);
    let cache_label = html_escape(&format_kib_for_user(viewer, cache_kb));
    let mem_total = html_escape(&format_host_mib_for_user(viewer, s.memory.total_mb));
    let mem_free = html_escape(&format_host_mib_for_user(viewer, s.memory.free_mb));
    let mem_used = html_escape(&format_host_mib_for_user(viewer, s.memory.used_mb));
    let buff = html_escape(&format_host_mib_for_user(viewer, s.memory.buff_cache_mb));
    let swap_total = html_escape(&format_host_mib_for_user(viewer, s.swap.total_mb));
    let swap_free = html_escape(&format_host_mib_for_user(viewer, s.swap.free_mb));
    let swap_used = html_escape(&format_host_mib_for_user(viewer, s.swap.used_mb));
    let swap_avail = html_escape(&format_host_mib_for_user(viewer, s.swap.available_mb));
    format!(
        r#"<div class="spm-grid" aria-label="Host resource summary">
  <article class="spm-card"><h3>CPU Information</h3>
    <dl class="spm-kv">
      <dt>Cores</dt><dd>{cores}</dd>
      <dt>Frequency</dt><dd>{freq} MHz</dd>
      <dt>Model</dt><dd>{model}</dd>
      <dt>Cache</dt><dd>{cache}</dd>
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
  <article class="spm-card spm-load-card" id="spm-load-average">
    <h3>CPU Load Average</h3>
    <dl class="spm-kv">
      <dt>1 minute</dt><dd>{one}</dd>
      <dt class="spm-load-5">5 minutes</dt><dd class="spm-load-5" data-load="5min">{five}</dd>
      <dt>15 minutes</dt><dd>{fifteen}</dd>
      <dt>Current CPU</dt><dd>{current}%</dd>
    </dl>
    <p class="spm-load-hint">Load is runnable + waiting work over 1 / <strong>5</strong> / 15 minutes. On {cores} cores, sustained load near {cores}.0 means roughly full CPU.</p>
  </article>
  <article class="spm-card"><h3>Memory Usage</h3>
    <dl class="spm-kv">
      <dt>Total</dt><dd>{mem_total}</dd>
      <dt>Free</dt><dd class="spm-good">{mem_free}</dd>
      <dt>Used</dt><dd class="spm-bad">{mem_used}</dd>
      <dt>Buff/Cache</dt><dd>{buff}</dd>
    </dl>
  </article>
  <article class="spm-card"><h3>Swap Memory</h3>
    <dl class="spm-kv">
      <dt>Total</dt><dd>{swap_total}</dd>
      <dt>Free</dt><dd class="spm-good">{swap_free}</dd>
      <dt>Used</dt><dd class="spm-bad">{swap_used}</dd>
      <dt>Available</dt><dd>{swap_avail}</dd>
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
        cores = cores,
        freq = html_escape(&s.cpu.frequency_mhz),
        model = html_escape(&s.cpu.model),
        cache = cache_label,
        total = s.processes.total,
        running = s.processes.running,
        sleeping = s.processes.sleeping,
        zombie = s.processes.zombie,
        one = html_escape(&s.load.one),
        five = html_escape(&s.load.five),
        fifteen = html_escape(&s.load.fifteen),
        current = html_escape(&s.load.current_pct),
        mem_total = mem_total,
        mem_free = mem_free,
        mem_used = mem_used,
        buff = buff,
        swap_total = swap_total,
        swap_free = swap_free,
        swap_used = swap_used,
        swap_avail = swap_avail,
        iowait = html_escape(&s.cpu_time.iowait_pct),
        idle = html_escape(&s.cpu_time.idle_pct),
        hirq = html_escape(&s.cpu_time.hard_irq_pct),
        sirq = html_escape(&s.cpu_time.soft_irq_pct),
    )
}

fn metric_bar_cpu(pct: &str, cores: u32) -> String {
    let w = bar_width_cpu(pct, cores);
    let of_all = html_escape(&cpu_of_all_label(pct, cores));
    format!(
        r#"<span class="spm-bar" title="ps CPU% (can exceed 100% on multi-core hosts)"><span class="spm-bar-track"><span class="spm-bar-fill" style="width:{w}%;"></span></span><span class="spm-metric spm-metric-cpu">{pct}%</span></span><span class="spm-cpu-all">{of_all}</span>"#,
        w = w,
        pct = html_escape(pct),
        of_all = of_all,
    )
}

fn metric_bar_mem(pct: &str) -> String {
    let w = bar_width_mem(pct);
    format!(
        r#"<span class="spm-bar"><span class="spm-bar-track"><span class="spm-bar-fill" style="width:{w}%;"></span></span><span class="spm-metric">{pct}%</span></span>"#,
        w = w,
        pct = html_escape(pct),
    )
}

fn render_process_rows(rows: &[ProcessRow], show_kill: bool, cores: u32) -> (String, String) {
    let mut table = String::from(
        r#"<div class="table-wrap spm-table-wrap"><table class="data-table" id="spm-table"><thead><tr>
        <th scope="col">PID</th><th scope="col">USER</th><th scope="col">STATE</th>
        <th scope="col" title="ps %CPU: 100% ≈ one full core">CPU %</th>
        <th scope="col">MEMORY %</th><th scope="col">VIRT</th><th scope="col">RES</th>
        <th scope="col">TIME</th><th scope="col">COMMAND</th>"#,
    );
    if show_kill {
        table.push_str("<th scope=\"col\">ACTIONS</th>");
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
        let cpu_bar = metric_bar_cpu(&row.cpu, cores);
        let mem_bar = metric_bar_mem(&row.mem);
        let virt = html_escape(&row.virt);
        let res = html_escape(&row.res);
        let time = html_escape(&row.time);
        let kill_cell = if show_kill {
            format!(
                r#"<td data-label="ACTIONS"><form method="post" action="/server/processes/kill" class="spm-kill-form" onsubmit="return confirm('Send SIGTERM to PID {pid}?');">
              <input type="hidden" name="pid" value="{pid}">
              <input type="hidden" name="return_tab" value="top">
              <button type="submit" class="spm-kill-btn" title="Send SIGTERM">KILL</button>
            </form></td>"#
            )
        } else {
            String::new()
        };
        table.push_str(&format!(
            r#"<tr{row_class} data-proc-row data-search="{user} {pid} {state} {cmd_full}">
              <td data-label="PID" class="spm-col-pid"><code>{pid}</code></td>
              <td data-label="USER" class="spm-col-user" title="{user}">{user}</td>
              <td data-label="STATE" class="spm-col-state"><span class="spm-state {state_cls}">{state}</span></td>
              <td data-label="CPU %" class="spm-col-cpu">{cpu_bar}</td>
              <td data-label="MEMORY %" class="spm-col-mem">{mem_bar}</td>
              <td data-label="VIRT" class="spm-col-virt">{virt}</td>
              <td data-label="RES" class="spm-col-res">{res}</td>
              <td data-label="TIME" class="spm-col-time">{time}</td>
              <td data-label="COMMAND"><code class="spm-cmd" title="{cmd_full}">{cmd_short}</code></td>
              {kill_cell}
            </tr>"#
        ));
        cards.push_str(&format!(
            r#"<article class="spm-card-row" data-proc-row data-search="{user} {pid} {state} {cmd_full}">
              <div style="display:flex;justify-content:space-between;gap:8px;flex-wrap:wrap;">
                <strong title="{user}">{user}</strong><code>PID {pid}</code>
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
                  <input type="hidden" name="return_tab" value="top">
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

fn page_scripts() -> &'static str {
    r#"<script>
(function(){
  var STORAGE_KEY='cpn-spm-refresh-ms';
  var ALLOWED={0:true,5000:true,10000:true,30000:true,60000:true};
  var input=document.getElementById('spm-search');
  if(input){
    input.addEventListener('input', function(){
      var q=(input.value||'').toLowerCase().trim();
      document.querySelectorAll('[data-proc-row]').forEach(function(row){
        var hay=(row.getAttribute('data-search')||'').toLowerCase();
        row.style.display = (!q || hay.indexOf(q)!==-1) ? '' : 'none';
      });
    });
  }
  var sel=document.getElementById('spm-refresh-interval');
  var timer=null;
  function clearTimer(){ if(timer){ clearInterval(timer); timer=null; } }
  function applyInterval(ms){
    clearTimer();
    if(!ms) return;
    timer=setInterval(function(){ window.location.reload(); }, ms);
  }
  function readStored(){
    try{
      var raw=localStorage.getItem(STORAGE_KEY);
      var n=parseInt(raw||'0',10);
      return ALLOWED[n] ? n : 0;
    }catch(e){ return 0; }
  }
  function writeStored(ms){
    try{ localStorage.setItem(STORAGE_KEY, String(ms)); }catch(e){}
  }
  if(sel){
    var initial=readStored();
    sel.value=String(initial);
    applyInterval(initial);
    sel.addEventListener('change', function(){
      var ms=parseInt(sel.value||'0',10);
      if(!ALLOWED[ms]) ms=0;
      writeStored(ms);
      applyInterval(ms);
    });
  }
})();
</script>"#
}

fn toolbar_html() -> String {
    r#"<div class="spm-toolbar">
  <p class="muted">Snapshot from <code>ps</code> and <code>/proc</code> (CPU sorted). Auto-refresh defaults to Off.</p>
  <div class="spm-toolbar-actions">
    <label class="spm-refresh-label" for="spm-refresh-interval">Auto-refresh
      <select id="spm-refresh-interval" class="spm-refresh-select" aria-label="Auto-refresh interval">
        <option value="0" selected>Off</option>
        <option value="5000">5 seconds</option>
        <option value="10000">10 seconds</option>
        <option value="30000">30 seconds</option>
        <option value="60000">60 seconds</option>
      </select>
    </label>
    <a class="btn-secondary" href="/server/processes?tab=top" id="spm-refresh-btn">Refresh</a>
  </div>
</div>"#
        .to_string()
}

fn cpu_tip_html(cores: u32) -> String {
    let cores = cores.max(1);
    format!(
        r#"<p class="spm-tip" role="note"><strong>Why CPU% can exceed 100%:</strong> <code>ps</code> reports CPU as a sum across cores. On this host ({cores} cores), <strong>100% ≈ one full core</strong> and the theoretical max is about {max}%. Values like 109% mean a bit more than one core, not over 100% of the whole machine.</p>"#,
        cores = cores,
        max = cores.saturating_mul(100),
    )
}

/// Top Processes tab body (no outer feature shell).
/// `show_kill` is true only for panel admins.
pub fn processes_tab_body(viewer: &str, show_kill: bool) -> String {
    let summary = snapshot_host_process_summary();
    let cores = summary.cpu.cores.max(1);
    let cards = summary_cards(&summary, viewer);
    let tip = cpu_tip_html(cores);
    let toolbar = toolbar_html();
    match snapshot_top_processes(80, viewer) {
        Ok(rows) if rows.is_empty() => {
            format!("{toolbar}{tip}{cards}<p class=\"empty-state\">No processes returned.</p>")
        }
        Ok(rows) => {
            let (table, list) = render_process_rows(&rows, show_kill, cores);
            format!(
                r#"{toolbar}{tip}{cards}
<div class="spm-section">
  <div class="spm-section-head">
    <h2>Running Processes</h2>
    <input type="search" id="spm-search" class="spm-search" placeholder="Search processes..." autocomplete="off">
  </div>
  <p class="spm-cpu-caption">{cores} cores: CPU % bars are scaled to the whole host (100% of one core ≈ {one_core_bar:.1}% of the bar). Raw <code>ps</code> % stays in the label.</p>
  {table}{list}
</div>
{script}"#,
                cores = cores,
                one_core_bar = 100.0 / f64::from(cores),
                script = page_scripts()
            )
        }
        Err(err) => format!(
            "{toolbar}{tip}{cards}<p class=\"panel-notice error\">{e}</p>",
            e = html_escape(&err)
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{bar_width_cpu, cpu_of_all_label, processes_tab_body};

    #[test]
    fn processes_tab_includes_monitor_chrome() {
        let html = processes_tab_body("test-viewer", true);
        assert!(html.contains("System Processes Monitor") || html.contains("spm-toolbar"));
        assert!(html.contains("Refresh"));
        assert!(html.contains("spm-refresh-interval"));
        assert!(html.contains("5 minutes") || html.contains("spm-load-5"));
        assert!(html.contains("Why CPU% can exceed 100%") || html.contains("spm-tip"));
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
        assert!(html.contains("cpn-spm-refresh-ms") || html.contains("spm-refresh-interval"));
    }

    #[test]
    fn kill_hidden_for_non_admin() {
        let html = processes_tab_body("test-viewer", false);
        assert!(!html.contains("/server/processes/kill"));
        assert!(!html.contains(">KILL</button>"));
    }

    #[test]
    fn cpu_bar_scales_to_all_cores() {
        // 100% on 12 cores ≈ 8% of host capacity on the bar.
        assert_eq!(bar_width_cpu("100", 12), 8);
        assert_eq!(bar_width_cpu("1200", 12), 100);
        assert_eq!(bar_width_cpu("109", 12), 9);
        assert!(cpu_of_all_label("109", 12).contains("of all cores"));
    }
}
