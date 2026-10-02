//! Dashboard Activity Board markup (tabs for SSH, processes, traffic, disk, CPU).

use crate::packages::is_panel_admin;
use crate::panel_dashboard_activity_list::{activity_list_script, activity_list_styles};
use crate::panel_dashboard_activity_panels::{
    cpu_panel, disk_io_panel, panel_actions_panel, ssh_logins_panel, top_process_panel,
    traffic_panel,
};
use crate::panel_dashboard_activity_ssh::ssh_logs_panel;
use crate::panel_ops_activity::ssh_security_analysis;
use crate::panel_user_prefs::load_user_ui_prefs;

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
  max-width:1200px; margin:0; padding:22px 24px 24px;
  border-radius:8px; background:var(--canvas); border:1px solid var(--hairline);
  min-width:0;
}
.activity-board .eyebrow { margin:0 0 4px; font-size:11px; letter-spacing:.08em; font-weight:700; color:var(--muted); }
.activity-board-head { display:flex; flex-wrap:wrap; align-items:flex-start; justify-content:space-between; gap:10px; margin:0 0 14px; }
.activity-board-head h2 { margin:0; font-size:22px; letter-spacing:-.02em; }
.activity-board-toggle {
  min-height:36px; padding:0 14px; border-radius:999px; border:1px solid var(--hairline);
  background:transparent; color:inherit; font:inherit; font-size:13px; font-weight:700; cursor:pointer;
}
.activity-board.is-collapsed .activity-board-body { display:none; }
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
.activity-clip { display:block; max-width:100%; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; font-size:12px; }
.activity-proc-row td { padding-top:8px; padding-bottom:8px; vertical-align:top; }
.activity-more { margin:0; }
.activity-more > summary {
  list-style:none; cursor:pointer; display:inline-flex; align-items:center;
  min-height:30px; padding:0 10px; border-radius:999px; border:1px solid var(--hairline);
  font-size:12px; font-weight:700;
}
.activity-more > summary::-webkit-details-marker { display:none; }
.activity-more-body { margin:8px 0 0; padding:10px; border-radius:10px; border:1px solid var(--hairline); background:var(--surface-soft,#f8fafc); font-size:13px; overflow-wrap:anywhere; }
.activity-more-body pre { margin:8px 0 0; white-space:pre-wrap; word-break:break-word; font-size:12px; }
.activity-chart { margin:0 0 14px; padding:12px; border-radius:10px; border:1px solid var(--hairline); background:var(--surface-soft,#f8fafc); }
.activity-chart figcaption { margin:0 0 10px; font-weight:700; font-size:14px; }
.activity-chart-row { display:grid; gap:6px; margin:0 0 12px; }
.activity-chart-pair { display:grid; grid-template-columns:64px minmax(0,1fr) auto; gap:8px; align-items:center; font-size:12px; }
.activity-chart-n { font-variant-numeric:tabular-nums; font-weight:700; }
.activity-bar { height:10px; border-radius:999px; background:rgba(148,163,184,.28); overflow:hidden; }
.activity-bar i { display:block; height:100%; border-radius:999px; background:#2563eb; }
.activity-bar-b i { background:#12b76a; }
@media (max-width:719.98px) {
  .activity-board { padding:16px; }
  .activity-sec-meta, .activity-kpi { grid-template-columns:1fr; }
  .activity-tabs { flex-wrap:nowrap; }
  .activity-chart-pair { grid-template-columns:1fr; }
}
[data-color-mode="dark"] .activity-sec-box {
  background:#3b2a12; border-color:#93370d; color:#fecd89;
}
[data-color-mode="dark"] .activity-sec-card {
  background:#1a1d26; border-color:#93370d; color:var(--ink);
}
[data-color-mode="dark"] .activity-tip { background:#2a2f3a; }
[data-color-mode="dark"] .activity-kpi article,
[data-color-mode="dark"] .activity-chart,
[data-color-mode="dark"] .activity-more-body { background:#161922; }
"#,
    );
    css.push_str(activity_list_styles());
    css
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

fn activity_script() -> String {
    let mut js = String::from(
        r#"
(function(){
  var root=document.getElementById('activity-board');
  if(!root) return;
  var tabs=[].slice.call(root.querySelectorAll('[data-activity-tab]'));
  var panels=[].slice.call(root.querySelectorAll('.activity-panel'));
  var toggle=root.querySelector('[data-activity-toggle]');
  function setOpen(on){
    root.classList.toggle('is-collapsed', !on);
    if(toggle){
      toggle.setAttribute('aria-expanded', on?'true':'false');
      toggle.textContent=on?'Collapse':'Expand';
    }
  }
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
  setOpen(root.getAttribute('data-open')==='1');
  if(toggle){
    toggle.addEventListener('click', function(){
      var next=root.classList.contains('is-collapsed');
      setOpen(next);
      fetch('/api/panel/dashboard-layout',{
        method:'POST', credentials:'same-origin',
        headers:{'Content-Type':'application/json','Accept':'application/json'},
        body: JSON.stringify({ activity_board_open: next })
      }).catch(function(){});
    });
  }
})();
"#,
    );
    js.push_str(activity_list_script());
    js
}

/// Full Activity Board for panel admins; non-admins get a short placeholder.
pub fn activity_board_html(username: &str) -> String {
    let open = load_user_ui_prefs(username).activity_board_open;
    let collapsed = if open { "" } else { " is-collapsed" };
    let open_attr = if open { "1" } else { "0" };
    let toggle_label = if open { "Collapse" } else { "Expand" };
    let expanded = if open { "true" } else { "false" };

    if !is_panel_admin(username) {
        return format!(
            r#"
        <article class="activity-board{collapsed}" id="activity-board" data-open="{open_attr}">
          <div class="activity-board-head">
            <div>
              <p class="eyebrow">RECENT ACTIVITY</p>
              <h2>Activity Board</h2>
            </div>
            <button type="button" class="activity-board-toggle" data-activity-toggle aria-expanded="{expanded}">{toggle}</button>
          </div>
          <div class="activity-board-body">
            <p class="empty-state">SSH logs and host counters are available to the panel admin only.</p>
          </div>
        </article>
        <script>{script}</script>"#,
            collapsed = collapsed,
            open_attr = open_attr,
            expanded = expanded,
            toggle = toggle_label,
            script = activity_script(),
        );
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
    let minimalist = crate::panel_user_prefs::load_user_minimalist_mode(username);

    let tabs = [
        tab_btn("ssh-logins", "Recent SSH Logins", true, None),
        tab_btn("ssh-logs", "Recent SSH Logs", false, badge),
        tab_btn("panel-actions", "Panel Actions", false, None),
        tab_btn("top-process", "Top Process", false, None),
        tab_btn("traffic", "Traffic", false, None),
        tab_btn("disk-io", "Disk IO", false, None),
        tab_btn("cpu", "CPU Usage", false, None),
    ]
    .join("\n");

    let panels = [
        panel("ssh-logins", false, &ssh_logins_panel()),
        panel("ssh-logs", true, &ssh_logs_panel(username, &analysis)),
        panel("panel-actions", true, &panel_actions_panel(username)),
        panel("top-process", true, &top_process_panel()),
        panel("traffic", true, &traffic_panel(minimalist)),
        panel("disk-io", true, &disk_io_panel(minimalist)),
        panel("cpu", true, &cpu_panel()),
    ]
    .join("\n");

    format!(
        r#"
      <section class="activity-board{collapsed}" id="activity-board" aria-label="Activity Board" data-open="{open_attr}">
        <div class="activity-board-head">
          <div>
            <p class="eyebrow">RECENT ACTIVITY</p>
            <h2>Activity Board</h2>
          </div>
          <button type="button" class="activity-board-toggle" data-activity-toggle aria-expanded="{expanded}">{toggle}</button>
        </div>
        <div class="activity-board-body">
        <div class="activity-tabs" role="tablist" aria-label="Activity Board tabs">
          {tabs}
        </div>
        {panels}
        </div>
      </section>
      <script>{script}</script>"#,
        collapsed = collapsed,
        open_attr = open_attr,
        expanded = expanded,
        toggle = toggle_label,
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
        assert!(html.contains("is-collapsed"));
        assert!(html.contains("data-activity-toggle"));
    }

    #[test]
    fn wrap_helper_exports_list_controls_markup() {
        use crate::panel_dashboard_activity_list::wrap_activity_table;
        let table = r#"<div class="table-wrap"><table class="data-table"><tbody><tr><td>ok</td></tr></tbody></table></div>"#;
        let html = wrap_activity_table("demo", "Filter", table);
        assert!(html.contains("Go to page"));
        assert!(html.contains("activity-list-search"));
        assert!(html.contains("data-page-size=\"5\""));
    }
}
