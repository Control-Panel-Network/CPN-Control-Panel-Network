//! Per-user dashboard overview widget order and edit-mode chrome.

use crate::panel_user_prefs::{UserUiPrefs, load_user_ui_prefs, save_user_ui_prefs};

pub const DEFAULT_DASH_WIDGETS: [&str; 5] = ["sites", "gauges", "tools", "health", "activity"];

pub fn is_known_widget(id: &str) -> bool {
    DEFAULT_DASH_WIDGETS.contains(&id)
}

pub fn normalize_dash_widgets(ids: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for id in ids {
        let key = id.trim().to_ascii_lowercase();
        if is_known_widget(&key) && !out.iter().any(|x| x == &key) {
            out.push(key);
        }
    }
    for def in DEFAULT_DASH_WIDGETS {
        if !out.iter().any(|x| x == def) {
            out.push(def.to_string());
        }
    }
    out
}

pub fn load_dashboard_widgets(username: &str) -> Vec<String> {
    normalize_dash_widgets(&load_user_ui_prefs(username).dashboard_widgets)
}

pub fn save_dashboard_layout(
    username: &str,
    widgets: &[String],
    activity_board_open: Option<bool>,
) -> Result<UserUiPrefs, String> {
    let mut prefs = load_user_ui_prefs(username);
    if !widgets.is_empty() {
        prefs.dashboard_widgets = normalize_dash_widgets(widgets);
    }
    if let Some(open) = activity_board_open {
        prefs.activity_board_open = open;
    }
    save_user_ui_prefs(username, &prefs)?;
    Ok(prefs)
}

pub fn restore_dashboard_layout(username: &str) -> Result<UserUiPrefs, String> {
    let mut prefs = load_user_ui_prefs(username);
    prefs.dashboard_widgets.clear();
    prefs.activity_board_open = false;
    save_user_ui_prefs(username, &prefs)?;
    Ok(prefs)
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn wrap_dash_widget(id: &str, label: &str, inner: &str) -> String {
    format!(
        r#"<article class="dash-widget" data-dash-widget="{id}" aria-label="{label}">
  <div class="dash-widget-chrome" hidden>
    <span class="dash-widget-handle" draggable="true" title="Drag to reorder">{label}</span>
  </div>
  {inner}
</article>"#,
        id = html_escape(id),
        label = html_escape(label),
        inner = inner,
    )
}

pub fn dashboard_layout_toolbar() -> String {
    r#"<div class="dash-layout-toolbar" id="dash-layout-toolbar">
  <button type="button" class="btn-secondary" id="dash-layout-edit">Edit overview</button>
  <button type="button" class="btn-primary" id="dash-layout-save" disabled>Save</button>
  <button type="button" class="btn-secondary" id="dash-layout-restore">Restore default</button>
  <p class="muted" id="dash-layout-status" aria-live="polite">Drag widgets on a wide screen, then Save. Restore default resets the stock order.</p>
</div>"#
        .into()
}

pub fn dashboard_layout_styles() -> &'static str {
    r#"
.dash-layout { min-width:0; }
.dash-layout-toolbar {
  display:flex; flex-wrap:wrap; gap:8px; align-items:center;
  margin:0 0 14px;
}
.dash-layout-toolbar .muted { margin:0; flex:1 1 220px; font-size:12px; }
.dash-layout-stack { display:flex; flex-direction:column; gap:18px; min-width:0; }
.dash-widget { min-width:0; position:relative; }
.dash-widget-chrome {
  display:flex; align-items:center; gap:8px; margin:0 0 8px;
}
.dash-widget-handle {
  display:inline-flex; align-items:center; min-height:32px; padding:0 10px;
  border-radius:999px; border:1px dashed var(--hairline); font-size:12px; font-weight:700;
  cursor:grab; user-select:none; background:var(--surface-soft,#f8fafc); color:var(--ink);
}
.dash-widget-handle:active { cursor:grabbing; }
.dash-layout[data-editing="1"] .dash-widget {
  outline:1px dashed var(--hairline); outline-offset:6px; border-radius:12px;
}
.dash-layout[data-editing="1"] .dash-widget-chrome { display:flex; }
.dash-widget.is-dragging { opacity:.55; }
.dash-widget.is-drop { outline-color:var(--blue,#2563eb); }
@media (max-width:719.98px) {
  .dash-widget-handle { cursor:default; }
}
"#
}

pub fn dashboard_layout_script() -> &'static str {
    r#"
(function(){
  var root=document.getElementById('dash-layout');
  if(!root) return;
  var stack=root.querySelector('.dash-layout-stack');
  var editBtn=document.getElementById('dash-layout-edit');
  var saveBtn=document.getElementById('dash-layout-save');
  var restoreBtn=document.getElementById('dash-layout-restore');
  var status=document.getElementById('dash-layout-status');
  var editing=false;
  var dirty=false;
  var dragEl=null;
  function widgets(){ return [].slice.call(stack.querySelectorAll('[data-dash-widget]')); }
  function order(){ return widgets().map(function(el){ return el.getAttribute('data-dash-widget'); }); }
  function setStatus(msg){ if(status) status.textContent=msg; }
  function setEditing(on){
    editing=!!on;
    root.setAttribute('data-editing', editing?'1':'0');
    if(editBtn) editBtn.textContent=editing?'Done editing':'Edit overview';
    widgets().forEach(function(el){
      var chrome=el.querySelector('.dash-widget-chrome');
      if(chrome) chrome.hidden=!editing;
    });
  }
  function setDirty(on){
    dirty=!!on;
    if(saveBtn) saveBtn.disabled=!dirty;
  }
  function canDrag(){
    return editing && window.matchMedia && window.matchMedia('(pointer:fine)').matches && window.innerWidth>=720;
  }
  function post(url, body){
    return fetch(url,{
      method:'POST',
      credentials:'same-origin',
      headers:{'Content-Type':'application/json','Accept':'application/json'},
      body: JSON.stringify(body||{})
    }).then(function(res){
      return res.text().then(function(text){
        var data={};
        try{ data=text?JSON.parse(text):{}; }catch(e){ throw new Error('HTTP '+res.status); }
        if(!res.ok) throw new Error(data.error||('HTTP '+res.status));
        return data;
      });
    });
  }
  widgets().forEach(function(el){
    var handle=el.querySelector('.dash-widget-handle');
    if(!handle) return;
    handle.addEventListener('dragstart', function(ev){
      if(!canDrag()){ ev.preventDefault(); return; }
      dragEl=el;
      el.classList.add('is-dragging');
      try{ ev.dataTransfer.setData('text/plain', el.getAttribute('data-dash-widget')||''); ev.dataTransfer.effectAllowed='move'; }catch(e){}
    });
    handle.addEventListener('dragend', function(){
      if(dragEl) dragEl.classList.remove('is-dragging');
      widgets().forEach(function(w){ w.classList.remove('is-drop'); });
      dragEl=null;
    });
    el.addEventListener('dragover', function(ev){
      if(!dragEl||dragEl===el) return;
      ev.preventDefault();
      el.classList.add('is-drop');
    });
    el.addEventListener('dragleave', function(){ el.classList.remove('is-drop'); });
    el.addEventListener('drop', function(ev){
      ev.preventDefault();
      el.classList.remove('is-drop');
      if(!dragEl||dragEl===el) return;
      var list=widgets();
      var from=list.indexOf(dragEl);
      var to=list.indexOf(el);
      if(from<0||to<0) return;
      if(from<to) stack.insertBefore(dragEl, el.nextSibling);
      else stack.insertBefore(dragEl, el);
      setDirty(true);
      setStatus('Layout changed. Click Save to keep this order.');
    });
  });
  if(editBtn){
    editBtn.addEventListener('click', function(){ setEditing(!editing); });
  }
  if(saveBtn){
    saveBtn.addEventListener('click', function(){
      setStatus('Saving...');
      post('/api/panel/dashboard-layout', { widgets: order() }).then(function(){
        setDirty(false);
        setStatus('Overview layout saved.');
      }).catch(function(err){
        setStatus(err.message||String(err));
      });
    });
  }
  if(restoreBtn){
    restoreBtn.addEventListener('click', function(){
      if(!window.confirm('Restore the default dashboard overview layout? This resets widget order and collapses the Activity Board.')) return;
      setStatus('Restoring...');
      post('/api/panel/dashboard-layout/restore', {}).then(function(){
        window.location.reload();
      }).catch(function(err){
        setStatus(err.message||String(err));
      });
    });
  }
  setEditing(false);
  setDirty(false);
})();
"#
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn normalize_fills_missing_and_drops_unknown() {
        let got = normalize_dash_widgets(&[
            "activity".into(),
            "nope".into(),
            "Activity".into(),
            "gauges".into(),
        ]);
        assert_eq!(
            got,
            vec![
                "activity".to_string(),
                "gauges".into(),
                "sites".into(),
                "tools".into(),
                "health".into()
            ]
        );
    }

    #[test]
    fn layout_persists_and_restores() {
        with_test_data_dir(|| {
            save_dashboard_layout("Admin", &["health".into(), "activity".into()], Some(true))
                .unwrap();
            let prefs = load_user_ui_prefs("Admin");
            assert_eq!(prefs.dashboard_widgets[0], "health");
            assert!(prefs.activity_board_open);
            restore_dashboard_layout("Admin").unwrap();
            let prefs = load_user_ui_prefs("Admin");
            assert!(prefs.dashboard_widgets.is_empty());
            assert!(!prefs.activity_board_open);
            assert_eq!(load_dashboard_widgets("Admin")[0], "sites");
        });
    }
}
