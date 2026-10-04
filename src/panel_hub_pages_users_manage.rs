//! Manage user modal (List Users) plus admin fragment for another account.

use crate::account_mgmt::find_account;
use crate::packages::is_panel_admin;
use crate::panel_hub_pages_profile::users_self_edit_body_with_tab;
use crate::panel_password_gen::{
    password_field_and_gen_html, password_gen_script, password_gen_styles,
};
use crate::panel_user_package::package_assign_section;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn manage_modal_shell() -> &'static str {
    r#"
<style>
.users-manage-dialog {
  border:0; padding:0; max-width:min(720px,calc(100vw - 24px)); width:100%;
  background:transparent;
}
.users-manage-dialog::backdrop { background:rgba(15,23,42,.72); }
.users-manage-card {
  background:var(--panel,#1a1d26); color:inherit; border:1px solid var(--hairline,#334155);
  border-radius:16px; padding:18px 16px 16px; max-height:min(88vh,900px); overflow:auto;
}
.users-manage-head {
  display:flex; flex-wrap:wrap; gap:10px; align-items:center; justify-content:space-between;
  margin:0 0 14px;
}
.users-manage-head h2 { margin:0; font-size:18px; }
#users-manage-body .modify-tabpanel[hidden] { display:none !important; }
#users-manage-body .modify-tabs { min-width:0; }
</style>
<dialog id="users-manage-dialog" class="users-manage-dialog" aria-labelledby="users-manage-title">
  <div class="users-manage-card">
    <div class="users-manage-head">
      <h2 id="users-manage-title">Manage user</h2>
      <button type="button" class="btn-secondary" id="users-manage-close">Close</button>
    </div>
    <div id="users-manage-body"><p class="muted">Loading…</p></div>
  </div>
</dialog>
<script>
(function(){
  var dlg=document.getElementById('users-manage-dialog');
  var body=document.getElementById('users-manage-body');
  var closeBtn=document.getElementById('users-manage-close');
  if(!dlg||!body) return;
  function runInlineScripts(root){
    var list=[].slice.call(root.querySelectorAll('script'));
    list.forEach(function(old){
      var s=document.createElement('script');
      if(old.type) s.type=old.type;
      if(old.src){ s.src=old.src; s.async=false; }
      else { s.text=old.textContent; }
      old.parentNode.replaceChild(s, old);
    });
  }
  function openFor(name){
    body.innerHTML='<p class="muted">Loading…</p>';
    if(dlg.showModal) dlg.showModal();
    var u='/account/users/manage-fragment?username='+encodeURIComponent(name);
    fetch(u,{credentials:'same-origin',headers:{'Accept':'text/html'}})
      .then(function(r){ if(!r.ok) throw new Error('Could not load user'); return r.text(); })
      .then(function(html){
        body.innerHTML=html;
        runInlineScripts(body);
      })
      .catch(function(err){ body.innerHTML='<p class="panel-notice error">'+String(err.message||err)+'</p>'; });
  }
  document.addEventListener('click', function(ev){
    var t=ev.target;
    if(!t||!t.closest) return;
    var btn=t.closest('.users-manage-open');
    if(btn){
      ev.preventDefault();
      openFor(btn.getAttribute('data-username')||'');
    }
  });
  if(closeBtn) closeBtn.addEventListener('click', function(){ dlg.close(); });
  dlg.addEventListener('click', function(ev){ if(ev.target===dlg) dlg.close(); });
  try{
    var q=new URLSearchParams(location.search||'');
    var reopen=q.get('manage');
    if(reopen) openFor(reopen);
  }catch(e){}
})();
</script>
"#
}

fn other_account_tab(target: &str, recovery_email: &str, lang: &str) -> String {
    let en_sel = if lang == "en" { " selected" } else { "" };
    let es_sel = if lang == "es" { " selected" } else { "" };
    let nb_sel = if lang == "nb" { " selected" } else { "" };
    format!(
        r#"
      <h3 style="margin:0 0 12px;">Account</h3>
      <form method="post" action="/account/users/admin-details" class="stack-form" style="max-width:520px;display:grid;gap:12px;">
        <input type="hidden" name="username" value="{user}">
        <label>Username
          <input type="text" value="{user}" disabled>
        </label>
        <label>Email
          <input name="recovery_email" type="email" required autocomplete="email" maxlength="254" value="{email}">
        </label>
        <label>Language
          <select name="language">
            <option value="en"{en_sel}>English</option>
            <option value="es"{es_sel}>Español</option>
            <option value="nb"{nb_sel}>Norsk</option>
          </select>
        </label>
        <button type="submit" class="btn-primary">Save details</button>
      </form>
      {pkg}"#,
        user = html_escape(target),
        email = html_escape(recovery_email),
        en_sel = en_sel,
        es_sel = es_sel,
        nb_sel = nb_sel,
        pkg = package_assign_section(target, true),
    )
}

fn other_security_tab(target: &str) -> String {
    format!(
        r#"
      <form method="post" action="/account/users/password" class="stack-form" style="max-width:520px;display:grid;gap:12px;">
        <h3 style="margin:0;">Reset password</h3>
        <input type="hidden" name="username" value="{user}">
        {pw_gen}
        <button type="submit" class="btn-primary">Reset password</button>
      </form>
      {styles}
      {script}"#,
        user = html_escape(target),
        pw_gen = password_field_and_gen_html(
            "New password (leave blank to generate)",
            crate::account::default_password_policy().min_length,
        ),
        styles = password_gen_styles(),
        script = password_gen_script(),
    )
}

fn other_admin_tab(target: &str) -> String {
    let admin_target = is_panel_admin(target);
    let delete = if admin_target {
        String::new()
    } else {
        format!(
            r#"
      <form method="post" action="/account/users/delete" class="stack-form" style="max-width:520px;display:grid;gap:12px;"
            onsubmit="return confirm('Delete this panel account? This cannot be undone.');">
        <h4 style="margin:0;">Delete user</h4>
        <input type="hidden" name="username" value="{user}">
        <button type="submit" class="btn-secondary">Delete user</button>
      </form>"#,
            user = html_escape(target),
        )
    };
    format!(
        r#"
      <h3 style="margin:0 0 12px;">Other</h3>
      <form method="post" action="/account/users/rename" class="stack-form" style="max-width:520px;display:grid;gap:12px;margin-bottom:28px;">
        <h4 style="margin:0;">Rename user</h4>
        <input type="hidden" name="username" value="{user}">
        <label>New username
          <input name="new_username" type="text" required autocomplete="username" maxlength="128">
        </label>
        <button type="submit" class="btn-primary">Rename user</button>
      </form>
      <form method="post" action="/account/users/status" class="stack-form" style="max-width:520px;display:grid;gap:12px;margin-bottom:28px;">
        <h4 style="margin:0;">Deactivate / Enable</h4>
        <input type="hidden" name="username" value="{user}">
        <label>Action
          <select name="action" required>
            <option value="deactivate">Deactivate</option>
            <option value="enable">Enable</option>
          </select>
        </label>
        <label style="display:flex;align-items:center;gap:8px;">
          <input name="force" type="checkbox" value="1">
          Force deactivate last active admin (locks admin UI until CLI re-enable)
        </label>
        <button type="submit" class="btn-secondary">Apply status</button>
      </form>
      {delete}"#,
        user = html_escape(target),
        delete = delete,
    )
}

/// HTML fragment for the List Users manage modal.
pub fn users_manage_fragment(viewer: &str, target_raw: &str) -> Result<String, String> {
    let target = target_raw.trim();
    if target.is_empty() {
        return Err("Username is required".into());
    }
    let self_edit = viewer.eq_ignore_ascii_case(target);
    if !self_edit && !is_panel_admin(viewer) {
        return Err("Only the panel admin can manage other accounts".into());
    }
    if self_edit {
        return Ok(users_self_edit_body_with_tab(
            viewer,
            None,
            None,
            None,
            None,
            "account",
            is_panel_admin(viewer),
        ));
    }
    let (boot, _) = find_account(target)?;
    let account = other_account_tab(&boot.username, &boot.recovery_email, &boot.language);
    let security = other_security_tab(&boot.username);
    let other = other_admin_tab(&boot.username);
    Ok(crate::panel_hub_pages_profile_tabs::wrap_modify_tabs(
        &account,
        &security,
        Some(&other),
        "account",
    ))
}

#[cfg(test)]
mod tests {
    use super::manage_modal_shell;

    #[test]
    fn modal_reexecutes_injected_scripts() {
        let html = manage_modal_shell();
        assert!(html.contains("runInlineScripts"), "{html}");
        assert!(html.contains("users-manage-dialog"), "{html}");
        assert!(html.contains("q.get('manage')"), "{html}");
    }
}
