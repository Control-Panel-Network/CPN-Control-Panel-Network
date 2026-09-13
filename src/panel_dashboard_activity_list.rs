//! Client-side search and pagination for Activity Board tables.

pub fn activity_list_styles() -> &'static str {
    r#"
.activity-list { margin:0 0 4px; min-width:0; }
.activity-list-controls {
  display:flex; flex-wrap:wrap; gap:10px 14px; align-items:center;
  margin:0 0 12px; padding:10px 12px; border:1px solid var(--hairline);
  border-radius:10px; background:var(--surface-soft,#f8fafc);
}
.activity-list-search-wrap {
  display:flex; flex-wrap:wrap; gap:8px; align-items:center; flex:1 1 200px; min-width:0;
}
.activity-list-search-wrap label {
  font-size:13px; font-weight:600; color:var(--ink); white-space:nowrap;
}
.activity-list-search {
  flex:1 1 160px; min-width:0; min-height:34px; max-width:100%;
  border:1px solid #94a3b8; border-radius:8px; padding:4px 10px;
  font:inherit; color:var(--ink); background:var(--canvas);
}
.activity-list-pager {
  display:flex; flex-wrap:wrap; gap:8px; align-items:center; margin-left:auto;
}
.activity-list-pager label {
  font-size:13px; font-weight:600; color:var(--ink); display:inline-flex;
  flex-wrap:wrap; gap:6px; align-items:center;
}
.activity-list-pager select,
.activity-list-pager input[type="number"] {
  min-height:34px; border:1px solid #94a3b8; border-radius:8px; padding:4px 8px;
  font:inherit; color:var(--ink); background:var(--canvas); max-width:88px;
}
.activity-list-status { font-size:13px; font-weight:700; color:var(--ink); white-space:nowrap; }
.activity-list-match { font-size:12px; color:var(--muted); white-space:nowrap; }
.activity-list-goto {
  display:inline-flex; flex-wrap:wrap; gap:6px; align-items:center; margin:0;
}
.activity-list-pager .btn-secondary,
.activity-list-pager .btn-primary {
  min-height:34px; padding:0 12px; border-radius:999px; font-weight:700; font-size:13px;
}
.activity-list-empty { margin:8px 0 0; }
@media (max-width:719.98px) {
  .activity-list-controls { padding:10px; }
  .activity-list-pager { margin-left:0; width:100%; }
  .activity-list-search { max-width:none; }
}
[data-color-mode="dark"] .activity-list-controls { background:#161922; }
[data-color-mode="dark"] .activity-list-search,
[data-color-mode="dark"] .activity-list-pager select,
[data-color-mode="dark"] .activity-list-pager input[type="number"] {
  background:#1a1d26; border-color:#475569; color:#f1f5f9;
}
"#
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Wrap a rendered data table with search + pagination controls (default 10 per page).
/// Pass `None` / empty inner when there is no table (caller shows empty-state instead).
pub fn wrap_activity_table(list_id: &str, search_placeholder: &str, table_html: &str) -> String {
    if table_html.trim().is_empty() || table_html.contains("empty-state") {
        return table_html.to_string();
    }
    let id = html_escape(list_id);
    let ph = html_escape(search_placeholder);
    format!(
        r#"<div class="activity-list" data-activity-list id="activity-list-{id}" data-page-size="10">
  <div class="activity-list-controls" role="group" aria-label="Table search and pagination">
    <div class="activity-list-search-wrap">
      <label for="activity-search-{id}">Search</label>
      <input type="search" class="activity-list-search" id="activity-search-{id}" placeholder="{ph}" autocomplete="off" aria-controls="activity-list-{id}">
      <span class="activity-list-match" data-activity-match>0 matching</span>
    </div>
    <div class="activity-list-pager">
      <label for="activity-per-page-{id}">Show
        <select class="activity-list-per-page" id="activity-per-page-{id}" aria-label="Rows per page">
          <option value="5">5</option>
          <option value="10" selected>10</option>
          <option value="20">20</option>
          <option value="50">50</option>
        </select>
        per page
      </label>
      <span class="activity-list-status" data-activity-status>Page 1 / 1</span>
      <button type="button" class="btn-secondary" data-activity-prev>Prev</button>
      <button type="button" class="btn-secondary" data-activity-next>Next</button>
      <form class="activity-list-goto" data-activity-goto>
        <label for="activity-goto-{id}">Go to page
          <input id="activity-goto-{id}" class="activity-list-goto-input" name="goto_page" type="number" min="1" value="1" inputmode="numeric">
        </label>
        <button type="submit" class="btn-primary">Go</button>
      </form>
    </div>
  </div>
  {table}
  <p class="empty-state activity-list-empty" data-activity-empty hidden>No matching rows.</p>
</div>"#,
        id = id,
        ph = ph,
        table = table_html,
    )
}

pub fn activity_list_script() -> &'static str {
    r#"
(function(){
  function initList(root){
    var table=root.querySelector('table.data-table');
    if(!table) return;
    var tbody=table.tBodies[0];
    if(!tbody) return;
    var rows=[].slice.call(tbody.rows);
    var search=root.querySelector('.activity-list-search');
    var perSel=root.querySelector('.activity-list-per-page');
    var statusEl=root.querySelector('[data-activity-status]');
    var matchEl=root.querySelector('[data-activity-match]');
    var prevBtn=root.querySelector('[data-activity-prev]');
    var nextBtn=root.querySelector('[data-activity-next]');
    var gotoForm=root.querySelector('[data-activity-goto]');
    var gotoInput=root.querySelector('.activity-list-goto-input');
    var emptyEl=root.querySelector('[data-activity-empty]');
    var wrap=root.querySelector('.table-wrap');
    var page=1;
    var perPage=parseInt(root.getAttribute('data-page-size')||'10',10)||10;
    if(perSel){ perSel.value=String(perPage); }

    function q(){ return (search&&search.value||'').trim().toLowerCase(); }
    function filtered(){
      var needle=q();
      if(!needle) return rows.slice();
      return rows.filter(function(tr){
        return (tr.textContent||'').toLowerCase().indexOf(needle)!==-1;
      });
    }
    function render(){
      var all=filtered();
      var total=all.length;
      var pages=Math.max(1, Math.ceil(total/perPage)||1);
      if(page>pages) page=pages;
      if(page<1) page=1;
      var start=(page-1)*perPage;
      var end=start+perPage;
      rows.forEach(function(tr){ tr.hidden=true; });
      all.forEach(function(tr,i){ tr.hidden=!(i>=start&&i<end); });
      if(statusEl) statusEl.textContent='Page '+page+' / '+pages;
      if(matchEl) matchEl.textContent=total+' matching';
      if(gotoInput){ gotoInput.max=String(pages); gotoInput.value=String(page); }
      if(prevBtn) prevBtn.disabled=page<=1;
      if(nextBtn) nextBtn.disabled=page>=pages;
      var showEmpty=total===0;
      if(emptyEl) emptyEl.hidden=!showEmpty;
      if(wrap) wrap.hidden=showEmpty;
      table.hidden=showEmpty;
    }
    if(search){
      search.addEventListener('input', function(){ page=1; render(); });
    }
    if(perSel){
      perSel.addEventListener('change', function(){
        var n=parseInt(perSel.value,10);
        perPage=(n===5||n===10||n===20||n===50)?n:10;
        root.setAttribute('data-page-size', String(perPage));
        page=1;
        render();
      });
    }
    if(prevBtn){
      prevBtn.addEventListener('click', function(){ if(page>1){ page-=1; render(); } });
    }
    if(nextBtn){
      nextBtn.addEventListener('click', function(){ page+=1; render(); });
    }
    if(gotoForm){
      gotoForm.addEventListener('submit', function(ev){
        ev.preventDefault();
        var n=parseInt(gotoInput&&gotoInput.value,10);
        if(!isFinite(n)||n<1) n=1;
        page=n;
        render();
      });
    }
    render();
  }
  function boot(){
    [].slice.call(document.querySelectorAll('[data-activity-list]')).forEach(initList);
  }
  if(document.readyState==='loading') document.addEventListener('DOMContentLoaded', boot);
  else boot();
})();
"#
}

#[cfg(test)]
mod tests {
    use super::wrap_activity_table;

    #[test]
    fn wrap_skips_empty_state() {
        let out = wrap_activity_table("x", "Find", r#"<p class="empty-state">None</p>"#);
        assert!(out.contains("empty-state"));
        assert!(!out.contains("data-activity-list"));
    }

    #[test]
    fn wrap_adds_controls_around_table() {
        let table = r#"<div class="table-wrap"><table class="data-table"><tbody><tr><td>a</td></tr></tbody></table></div>"#;
        let out = wrap_activity_table("ssh-logins", "Filter timestamp or message", table);
        assert!(out.contains("data-activity-list"));
        assert!(out.contains("Go to page"));
        assert!(out.contains("activity-list-search"));
        assert!(out.contains("value=\"10\" selected"));
    }
}
