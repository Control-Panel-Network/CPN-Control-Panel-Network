//! Tab chrome for Modify User (Account / Security / Other accounts).

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Normalize a tab id from query, hash, or server hint.
pub fn normalize_modify_tab(raw: &str) -> &'static str {
    match raw.trim().to_ascii_lowercase().as_str() {
        "security" | "passkeys" | "passkey" | "totp" | "password" | "mfa" => "security",
        "other" | "admin" | "others" | "other-accounts" => "other",
        _ => "account",
    }
}

pub fn modify_tabs_styles() -> &'static str {
    r#"
.modify-tabs {
  min-width:0; max-width:100%;
}
.modify-tablist {
  display:flex; flex-wrap:wrap; gap:8px; margin:0 0 18px;
  overflow-x:auto; -webkit-overflow-scrolling:touch; padding-bottom:2px;
  scrollbar-width:thin;
}
.modify-tab {
  display:inline-flex; align-items:center; justify-content:center;
  flex:0 0 auto; min-height:40px; min-width:44px; padding:8px 14px;
  border-radius:10px; border:1px solid var(--hairline,#334155);
  background:transparent; color:inherit; font:inherit; font-size:13px; font-weight:600;
  cursor:pointer; white-space:nowrap; touch-action:manipulation;
}
.modify-tab:hover { border-color:var(--blue,#2563eb); }
.modify-tab:focus-visible {
  outline:2px solid var(--blue,#2563eb); outline-offset:2px;
}
.modify-tab[aria-selected="true"] {
  background:var(--blue,#2563eb); border-color:transparent; color:#fff;
}
.modify-tabpanel[hidden] { display:none !important; }
.modify-tabpanel { min-width:0; max-width:100%; }
@media (max-width:719.98px) {
  .modify-tablist { flex-wrap:nowrap; }
  .modify-tab { min-height:44px; padding:10px 14px; }
}
[data-color-mode="dark"] .modify-tab {
  background:#1a1d26; border-color:#334155; color:#e2e8f0;
}
[data-color-mode="dark"] .modify-tab[aria-selected="true"] {
  background:var(--blue,#2563eb); color:#fff; border-color:transparent;
}
"#
}

pub fn modify_tabs_script() -> &'static str {
    r#"
(function(){
  var root=document.getElementById('modify-user-tabs');
  if(!root) return;
  var tabs=[].slice.call(root.querySelectorAll('[data-modify-tab]'));
  var panels=[].slice.call(root.querySelectorAll('.modify-tabpanel'));
  function known(id){
    return !!root.querySelector('[data-modify-tab="'+id+'"]');
  }
  function resolve(raw){
    var v=String(raw||'').toLowerCase().replace(/^#/,'');
    if(v==='passkeys'||v==='passkey'||v==='totp'||v==='password'||v==='mfa') return 'security';
    if(v==='admin'||v==='others'||v==='other-accounts') return 'other';
    if(v==='security'||v==='account'||v==='other') return v;
    return '';
  }
  function activate(id, pushHash){
    if(!known(id)) id='account';
    tabs.forEach(function(btn){
      var on=btn.getAttribute('data-modify-tab')===id;
      btn.setAttribute('aria-selected', on?'true':'false');
      btn.setAttribute('tabindex', on?'0':'-1');
    });
    panels.forEach(function(panel){
      var on=panel.id==='modify-panel-'+id;
      if(on) panel.removeAttribute('hidden'); else panel.setAttribute('hidden','');
    });
    if(pushHash){
      try{
        var u=new URL(location.href);
        u.searchParams.set('tab', id);
        u.hash=id;
        history.replaceState(null,'', u.pathname+u.search+u.hash);
      }catch(e){
        try{ history.replaceState(null,'','#'+id); }catch(_e){}
      }
    }
  }
  tabs.forEach(function(btn, idx){
    btn.addEventListener('click', function(){
      activate(btn.getAttribute('data-modify-tab'), true);
      btn.focus();
    });
    btn.addEventListener('keydown', function(ev){
      var key=ev.key;
      if(key!=='ArrowLeft' && key!=='ArrowRight' && key!=='Home' && key!=='End') return;
      ev.preventDefault();
      var next=idx;
      if(key==='ArrowLeft') next=(idx-1+tabs.length)%tabs.length;
      if(key==='ArrowRight') next=(idx+1)%tabs.length;
      if(key==='Home') next=0;
      if(key==='End') next=tabs.length-1;
      tabs[next].focus();
      activate(tabs[next].getAttribute('data-modify-tab'), true);
    });
  });
  var want='';
  try{
    var q=new URLSearchParams(location.search||'');
    want=resolve(q.get('tab')||'');
    if(!want && q.get('enroll')==='1') want='security';
  }catch(e){}
  if(!want) want=resolve((location.hash||'').replace(/^#/,''));
  if(!want) want=resolve(root.getAttribute('data-initial-tab')||'');
  if(!want || !known(want)) want='account';
  activate(want, false);
})();
"#
}

fn tab_button(id: &str, label: &str, selected: bool) -> String {
    let sel = if selected { "true" } else { "false" };
    let tab_index = if selected { "0" } else { "-1" };
    format!(
        r#"<button type="button" class="modify-tab" role="tab" id="modify-tab-{id}" data-modify-tab="{id}" aria-controls="modify-panel-{id}" aria-selected="{sel}" tabindex="{tab_index}">{label}</button>"#,
        id = html_escape(id),
        label = html_escape(label),
        sel = sel,
        tab_index = tab_index,
    )
}

fn tab_panel(id: &str, labelled_by: &str, hidden: bool, body: &str) -> String {
    let hide = if hidden { " hidden" } else { "" };
    format!(
        r#"<div class="modify-tabpanel" role="tabpanel" id="modify-panel-{id}" aria-labelledby="{labelled_by}"{hide}>{body}</div>"#,
        id = html_escape(id),
        labelled_by = html_escape(labelled_by),
        hide = hide,
        body = body,
    )
}

/// Wrap Account / Security / optional Other accounts panels in an accessible tabset.
pub fn wrap_modify_tabs(
    account_html: &str,
    security_html: &str,
    other_html: Option<&str>,
    initial_tab: &str,
) -> String {
    let mut initial = normalize_modify_tab(initial_tab);
    if initial == "other" && other_html.is_none() {
        initial = "account";
    }
    let mut buttons = vec![
        tab_button("account", "Account", initial == "account"),
        tab_button("security", "Security", initial == "security"),
    ];
    let mut panels = vec![
        tab_panel("account", "modify-tab-account", initial != "account", account_html),
        tab_panel(
            "security",
            "modify-tab-security",
            initial != "security",
            security_html,
        ),
    ];
    if let Some(other) = other_html {
        buttons.push(tab_button("other", "Other accounts", initial == "other"));
        panels.push(tab_panel(
            "other",
            "modify-tab-other",
            initial != "other",
            other,
        ));
    }
    format!(
        r#"
<style>{styles}</style>
<div class="modify-tabs" id="modify-user-tabs" data-initial-tab="{initial}">
  <div class="modify-tablist" role="tablist" aria-label="Modify User sections">
    {buttons}
  </div>
  {panels}
</div>
<script>{script}</script>"#,
        styles = modify_tabs_styles(),
        initial = html_escape(initial),
        buttons = buttons.join("\n    "),
        panels = panels.join("\n  "),
        script = modify_tabs_script(),
    )
}

#[cfg(test)]
mod tests {
    use super::{normalize_modify_tab, wrap_modify_tabs};

    #[test]
    fn normalize_aliases_map_to_security() {
        assert_eq!(normalize_modify_tab("passkeys"), "security");
        assert_eq!(normalize_modify_tab("TOTP"), "security");
        assert_eq!(normalize_modify_tab("other-accounts"), "other");
        assert_eq!(normalize_modify_tab("account"), "account");
    }

    #[test]
    fn wrap_includes_aria_and_deep_link_hooks() {
        let html = wrap_modify_tabs(
            "<p>account</p>",
            "<p>security</p>",
            Some("<p>other</p>"),
            "security",
        );
        assert!(html.contains("id=\"modify-user-tabs\""));
        assert!(html.contains("data-initial-tab=\"security\""));
        assert!(html.contains("role=\"tablist\""));
        assert!(html.contains("role=\"tab\""));
        assert!(html.contains("role=\"tabpanel\""));
        assert!(html.contains("data-modify-tab=\"account\""));
        assert!(html.contains("data-modify-tab=\"security\""));
        assert!(html.contains("data-modify-tab=\"other\""));
        assert!(html.contains("Other accounts"));
        assert!(html.contains("aria-selected=\"true\""));
    }

    #[test]
    fn wrap_omits_other_when_absent() {
        let html = wrap_modify_tabs("<p>a</p>", "<p>s</p>", None, "other");
        assert!(!html.contains("data-modify-tab=\"other\""));
        assert!(html.contains("data-initial-tab=\"account\""));
    }
}
