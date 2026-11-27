//! System Processes Monitor hub (Top Processes + CPN usage tabs).

use crate::panel_hub_pages_cpn_usage::{cpn_usage_tab_body, cpn_usage_tab_styles};
use crate::panel_hub_pages_processes::{processes_tab_body, processes_tab_styles};
use crate::panel_hubs::feature_shell;

fn hub_styles() -> &'static str {
    r#"<style>
#spm-hub-root .spm-hub-tabs{display:flex;flex-wrap:wrap;gap:8px;margin:0 0 18px;}
#spm-hub-root .spm-hub-tab{
  display:inline-flex;align-items:center;min-height:36px;padding:0 14px;border-radius:8px;
  border:1px solid var(--hairline);background:transparent;color:inherit;font-weight:700;
  text-decoration:none;cursor:pointer;
}
#spm-hub-root .spm-hub-tab.is-active{background:var(--accent,#7c3aed);color:#fff;border-color:transparent;}
#spm-hub-root .spm-hub-tab:focus-visible{outline:2px solid var(--blue,#2e90fa);outline-offset:2px;}
#spm-hub-root .spm-hub-panel[hidden]{display:none !important;}
#spm-hub-root .spm-card,#spm-hub-root .cpu-card{min-height:8.75rem;}
@media (max-width:719.98px){#spm-hub-root .spm-hub-tabs{flex-wrap:nowrap;overflow-x:auto;}}
</style>"#
}

fn hub_script() -> &'static str {
    r#"<script>
(function(){
  var root=document.getElementById('spm-hub-root');
  if(!root) return;
  var tabs=[].slice.call(root.querySelectorAll('[data-spm-tab]'));
  var panels=[].slice.call(root.querySelectorAll('[data-spm-panel]'));
  function showTab(id, pushUrl){
    tabs.forEach(function(t){
      var on=t.getAttribute('data-spm-tab')===id;
      t.classList.toggle('is-active', on);
      t.setAttribute('aria-selected', on?'true':'false');
      t.setAttribute('tabindex', on?'0':'-1');
    });
    panels.forEach(function(p){
      var on=p.getAttribute('data-spm-panel')===id;
      if(on) p.removeAttribute('hidden'); else p.setAttribute('hidden','');
    });
    if(pushUrl){
      try{
        var q=new URLSearchParams(location.search||'');
        q.set('tab', id);
        history.replaceState(null,'',location.pathname+'?'+q.toString());
      }catch(e){}
    }
  }
  tabs.forEach(function(tab){
    tab.addEventListener('click', function(ev){
      var id=tab.getAttribute('data-spm-tab');
      if(!id) return;
      if(ev.metaKey||ev.ctrlKey||ev.shiftKey||ev.altKey) return;
      ev.preventDefault();
      showTab(id, true);
    });
  });
  root.addEventListener('click', function(ev){
    var tab=ev.target.closest?ev.target.closest('[data-spm-tab]'):null;
    if(!tab||!root.contains(tab)) return;
  });
})();
</script>"#
}

/// Normalize `?tab=` query values.
pub fn normalize_spm_tab(raw: &str) -> &'static str {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" | "top" | "processes" | "spm" => "top",
        "cpn" | "cpn-usage" | "usage" | "panel" => "cpn-usage",
        _ => "top",
    }
}

pub fn spm_nav_id(active: &str) -> &'static str {
    if active == "cpn-usage" {
        "cpn-usage"
    } else {
        "processes"
    }
}

fn tabs_nav(active: &str) -> String {
    let items = [("top", "Top Processes"), ("cpn-usage", "CPN usage")];
    let mut out = String::from(
        r#"<nav class="spm-hub-tabs" role="tablist" aria-label="System Processes Monitor">"#,
    );
    for (id, label) in items {
        let on = id == active;
        out.push_str(&format!(
            r#"<a class="spm-hub-tab{cls}" role="tab" id="spm-tab-{id}" href="/server/processes?tab={id}" data-spm-tab="{id}" aria-controls="spm-panel-{id}" aria-selected="{sel}" tabindex="{ti}">{label}</a>"#,
            cls = if on { " is-active" } else { "" },
            sel = if on { "true" } else { "false" },
            ti = if on { "0" } else { "-1" },
        ));
    }
    out.push_str("</nav>");
    out
}

fn tab_panel(id: &str, active: &str, inner: &str) -> String {
    format!(
        r#"<section class="spm-hub-panel" id="spm-panel-{id}" role="tabpanel" aria-labelledby="spm-tab-{id}" data-spm-panel="{id}"{hidden}>{inner}</section>"#,
        hidden = if id == active { "" } else { " hidden" },
    )
}

/// Combined System Processes Monitor page with tabs.
pub fn system_processes_hub_page(active: &str, show_kill: bool) -> String {
    let active = normalize_spm_tab(active);
    let top_body = processes_tab_body(show_kill);
    let cpn_body = cpn_usage_tab_body(show_kill);
    let body = format!(
        r#"<div id="spm-hub-root">{tabs}{top_panel}{cpn_panel}{script}</div>"#,
        tabs = tabs_nav(active),
        top_panel = tab_panel("top", active, &top_body),
        cpn_panel = tab_panel("cpn-usage", active, &cpn_body),
        script = hub_script(),
    );
    let styles = format!(
        "{}{}{}",
        hub_styles(),
        processes_tab_styles(),
        cpn_usage_tab_styles()
    );
    let (last_crumb, subtitle) = if active == "cpn-usage" {
        (
            "CPN usage",
            "What the CPN panel itself uses (RSS/CPU) and host-wide top processes on separate tabs.",
        )
    } else {
        (
            "Top Processes",
            "CPU, memory, and running processes. Choose an auto-refresh interval or refresh manually.",
        )
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            (last_crumb, None),
        ],
        "System Processes Monitor",
        subtitle,
        &format!("{styles}{body}"),
        None,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::{normalize_spm_tab, system_processes_hub_page};

    #[test]
    fn tab_query_aliases() {
        assert_eq!(normalize_spm_tab(""), "top");
        assert_eq!(normalize_spm_tab("cpn-usage"), "cpn-usage");
        assert_eq!(normalize_spm_tab("USAGE"), "cpn-usage");
    }

    #[test]
    fn hub_has_both_tabs() {
        let html = system_processes_hub_page("top", true);
        assert!(html.contains("data-spm-tab=\"top\""));
        assert!(html.contains("data-spm-tab=\"cpn-usage\""));
        assert!(html.contains("spm-hub-root"));
        assert!(html.contains("System Processes Monitor"));
    }

    #[test]
    fn cpn_tab_admin_kill_or_empty() {
        let html = system_processes_hub_page("cpn-usage", true);
        assert!(
            html.contains("CPN-related processes")
                || html.contains("empty-state")
                || html.contains("panel-notice error")
        );
        if html.contains("/server/processes/kill") {
            assert!(html.contains("return_tab") && html.contains("cpn-usage"));
        }
    }
}
