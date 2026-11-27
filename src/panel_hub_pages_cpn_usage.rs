//! CPN panel resource usage page (Server → CPN usage).

use crate::panel_ops_process::truncate_command;
use crate::panel_ops_process_cpn::{
    CpnUsageSnapshot, format_rss_kb_for_user, snapshot_cpn_usage,
};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// CPN usage tab styles (shared with the processes hub).
pub(crate) fn cpn_usage_tab_styles() -> &'static str {
    r#"<style>
.cpu-toolbar{display:flex;flex-wrap:wrap;gap:12px;align-items:center;justify-content:space-between;margin:0 0 16px;}
.cpu-toolbar .muted{margin:0;max-width:70ch;}
.cpu-toolbar-actions{display:flex;flex-wrap:wrap;gap:10px;align-items:center;}
.cpu-toolbar .btn-secondary{
  display:inline-flex;align-items:center;justify-content:center;min-height:40px;padding:0 16px;
  border:0;border-radius:999px;font-weight:700;text-decoration:none;cursor:pointer;
  background:#e2e8f0;color:#0f172a;white-space:nowrap;
}
[data-color-mode="dark"] .cpu-toolbar .btn-secondary{background:#334155;color:#f8fafc;}
.cpu-refresh-label{display:inline-flex;align-items:center;gap:8px;font-size:0.88rem;color:var(--muted);font-weight:600;}
.cpu-refresh-select{
  min-height:40px;padding:0 12px;border-radius:10px;border:1px solid var(--hairline);
  background:transparent;color:inherit;font:inherit;font-weight:600;
}
.cpu-grid{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:14px;margin:0 0 18px;}
.cpu-card{border:1px solid var(--hairline);border-radius:14px;padding:14px 16px;background:var(--canvas);min-width:0;}
.cpu-card h3{margin:0 0 10px;font-size:0.95rem;}
.cpu-kv{display:grid;grid-template-columns:auto 1fr;gap:4px 12px;margin:0;font-size:0.9rem;}
.cpu-kv dt{color:var(--muted);font-weight:600;}
.cpu-kv dd{margin:0;font-variant-numeric:tabular-nums;text-align:right;}
.cpu-tip{
  margin:0 0 16px;padding:12px 14px;border-radius:12px;border:1px solid var(--hairline);
  background:rgba(59,130,246,.08);font-size:0.9rem;line-height:1.45;
}
.cpu-section{border:1px solid var(--hairline);border-radius:14px;padding:14px 16px;background:var(--canvas);}
.cpu-section h2{margin:0 0 12px;font-size:1.05rem;}
.cpu-table-wrap{overflow-x:auto;}
.cpu-table-wrap .data-table{width:100%;min-width:920px;}
.cpu-cmd{display:block;max-width:36ch;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;}
.cpu-kill-btn{
  display:inline-flex;align-items:center;justify-content:center;min-height:32px;padding:0 12px;
  border:0;border-radius:999px;font-weight:700;cursor:pointer;background:#dc2626;color:#fff;font-size:0.82rem;
}
.cpu-kill-btn:hover{filter:brightness(1.05);}
@media (max-width:1100px){.cpu-grid{grid-template-columns:repeat(2,minmax(0,1fr));}}
@media (max-width:719.98px){
  .cpu-grid{grid-template-columns:1fr;}
  .cpu-toolbar .btn-secondary{width:100%;}
  .cpu-refresh-label{width:100%;}
  .cpu-refresh-select{flex:1;}
}
</style>"#
}

fn toolbar_html() -> String {
    r#"<div class="cpu-toolbar">
  <p class="muted">What the CPN panel itself uses (panel binary and panel-owned webmail PHP-FPM). Site apps are separate. Auto-refresh defaults to Off.</p>
  <div class="cpu-toolbar-actions">
    <label class="cpu-refresh-label" for="cpu-refresh-interval">Auto-refresh
      <select id="cpu-refresh-interval" class="cpu-refresh-select" aria-label="Auto-refresh interval">
        <option value="0" selected>Off</option>
        <option value="5000">5 seconds</option>
        <option value="10000">10 seconds</option>
        <option value="30000">30 seconds</option>
        <option value="60000">60 seconds</option>
      </select>
    </label>
    <a class="btn-secondary" href="/server/processes?tab=cpn-usage">Refresh</a>
  </div>
</div>"#
        .to_string()
}

fn page_scripts() -> &'static str {
    r#"<script>
(function(){
  var STORAGE_KEY='cpn-usage-refresh-ms';
  var ALLOWED={0:true,5000:true,10000:true,30000:true,60000:true};
  var sel=document.getElementById('cpu-refresh-interval');
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

fn summary_cards(snap: &CpnUsageSnapshot, viewer: &str) -> String {
    format!(
        r#"<div class="cpu-grid" aria-label="CPN resource summary">
  <article class="cpu-card"><h3>Panel service</h3>
    <dl class="cpu-kv">
      <dt>RSS</dt><dd>{panel_rss}</dd>
      <dt>CPU (sum)</dt><dd>{panel_cpu:.1}%</dd>
      <dt>Processes</dt><dd>{panel_n}</dd>
    </dl>
  </article>
  <article class="cpu-card"><h3>Webmail PHP-FPM</h3>
    <dl class="cpu-kv">
      <dt>RSS</dt><dd>{web_rss}</dd>
      <dt>CPU (sum)</dt><dd>{web_cpu:.1}%</dd>
      <dt>Processes</dt><dd>{web_n}</dd>
    </dl>
  </article>
  <article class="cpu-card"><h3>Other CPN</h3>
    <dl class="cpu-kv">
      <dt>RSS</dt><dd>{other_rss}</dd>
      <dt>CPU (sum)</dt><dd>{other_cpu:.1}%</dd>
      <dt>Processes</dt><dd>{other_n}</dd>
    </dl>
  </article>
  <article class="cpu-card"><h3>CPN total</h3>
    <dl class="cpu-kv">
      <dt>RSS</dt><dd><strong>{total_rss}</strong></dd>
      <dt>CPU (sum)</dt><dd><strong>{total_cpu:.1}%</strong></dd>
      <dt>Processes</dt><dd>{total_n}</dd>
    </dl>
  </article>
</div>"#,
        panel_rss = html_escape(&format_rss_kb_for_user(viewer, snap.panel_rss_kb)),
        panel_cpu = snap.panel_cpu,
        panel_n = snap.panel_count,
        web_rss = html_escape(&format_rss_kb_for_user(viewer, snap.webmail_rss_kb)),
        web_cpu = snap.webmail_cpu,
        web_n = snap.webmail_count,
        other_rss = html_escape(&format_rss_kb_for_user(viewer, snap.other_rss_kb)),
        other_cpu = snap.other_cpu,
        other_n = snap.other_count,
        total_rss = html_escape(&format_rss_kb_for_user(viewer, snap.total_rss_kb())),
        total_cpu = snap.total_cpu(),
        total_n = snap.total_count(),
    )
}

fn rows_table(snap: &CpnUsageSnapshot, show_kill: bool) -> String {
    if snap.rows.is_empty() {
        return "<p class=\"empty-state\">No CPN panel processes matched on this host.</p>"
            .to_string();
    }
    let mut html = String::from(
        r#"<div class="table-wrap cpu-table-wrap"><table class="data-table">
<thead><tr>
  <th scope="col">Role</th><th scope="col">PID</th><th scope="col">USER</th>
  <th scope="col">STATE</th><th scope="col">CPU %</th><th scope="col">MEM %</th>
  <th scope="col">RES</th><th scope="col">TIME</th><th scope="col">COMMAND</th>"#,
    );
    if show_kill {
        html.push_str(r#"<th scope="col">ACTIONS</th>"#);
    }
    html.push_str("</tr></thead><tbody>");
    for item in &snap.rows {
        let role = html_escape(&item.role);
        let pid = html_escape(&item.row.pid);
        let user = html_escape(&item.row.user);
        let state = html_escape(&item.row.state);
        let cpu = html_escape(&item.row.cpu);
        let mem = html_escape(&item.row.mem);
        let res = html_escape(&item.row.res);
        let time = html_escape(&item.row.time);
        let cmd_full = html_escape(&item.row.command);
        let cmd_short = html_escape(&truncate_command(&item.row.command, 72));
        let kill_cell = if show_kill {
            format!(
                r#"<td data-label="ACTIONS"><form method="post" action="/server/processes/kill" class="cpu-kill-form" onsubmit="return confirm('Send SIGTERM to PID {pid}?');">
  <input type="hidden" name="pid" value="{pid}">
  <input type="hidden" name="return_tab" value="cpn-usage">
  <button type="submit" class="cpu-kill-btn" title="Send SIGTERM">KILL</button>
</form></td>"#
            )
        } else {
            String::new()
        };
        html.push_str(&format!(
            r#"<tr>
  <td>{role}</td>
  <td><code>{pid}</code></td>
  <td title="{user}">{user}</td>
  <td>{state}</td>
  <td>{cpu}%</td>
  <td>{mem}%</td>
  <td>{res}</td>
  <td>{time}</td>
  <td><code class="cpu-cmd" title="{cmd_full}">{cmd_short}</code></td>
  {kill_cell}
</tr>"#
        ));
    }
    html.push_str("</tbody></table></div>");
    html
}

/// CPN usage tab body (no outer feature shell).
pub fn cpn_usage_tab_body(viewer: &str, show_kill: bool) -> String {
    let toolbar = toolbar_html();
    match snapshot_cpn_usage(viewer) {
        Ok(snap) => {
            let tip = format!(
                r#"<p class="cpu-tip" role="note"><strong>Tip:</strong> {note} High host load or swap usually comes from site workloads, databases, queues, or browsers, not from this panel total.</p>"#,
                note = html_escape(&snap.note),
            );
            let cards = summary_cards(&snap, viewer);
            let table = rows_table(&snap, show_kill);
            format!(
                r#"{toolbar}{tip}{cards}
<div class="cpu-section">
  <h2>CPN-related processes</h2>
  {table}
</div>
{script}"#,
                script = page_scripts()
            )
        }
        Err(err) => format!(
            r#"{toolbar}<p class="panel-notice error">{e}</p>"#,
            e = html_escape(&err)
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::cpn_usage_tab_body;

    #[test]
    fn cpn_usage_tab_chrome() {
        let html = cpn_usage_tab_body("test-viewer", true);
        assert!(html.contains("cpu-toolbar") || html.contains("panel-notice error"));
        assert!(html.contains("Refresh") || html.contains("panel-notice error"));
        assert!(html.contains("cpu-refresh-interval") || html.contains("panel-notice error"));
        assert!(html.contains("tab=cpn-usage") || html.contains("panel-notice error"));
        assert!(!html.contains('\u{2014}'));
    }

    #[test]
    fn cpn_kill_gated() {
        let html = cpn_usage_tab_body("test-viewer", false);
        assert!(!html.contains(">KILL</button>"));
    }
}
