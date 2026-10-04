//! Account tab markup for Modify User / Manage user.

use crate::packages::is_panel_admin;
use crate::panel_user_package::package_assign_section;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn account_tab_html(
    boot_username: &str,
    recovery_email: &str,
    lang: &str,
    storage_unit: crate::panel_user_prefs::StorageUnitPref,
) -> String {
    let en_sel = if lang == "en" { " selected" } else { "" };
    let es_sel = if lang == "es" { " selected" } else { "" };
    let nb_sel = if lang == "nb" { " selected" } else { "" };
    let can_assign = is_panel_admin(boot_username);
    format!(
        r#"
      <p class="modify-back-profile" style="margin:0 0 16px;"><a class="btn-secondary" href="/account/users/profile">Back to profile</a></p>
      <h3 style="margin:0 0 12px;">Your account</h3>
      <form method="post" action="/account/users/profile/details" class="stack-form" style="max-width:520px;display:grid;gap:12px;margin-bottom:8px;">
        <label>Username
          <input name="username" type="text" required autocomplete="username" maxlength="128" value="{username}">
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
        <label>Storage display unit
          <select name="storage_unit">{unit_opts}</select>
        </label>
        <p class="muted" style="margin:0;">Auto picks KB, MB, GB, or TB from the real size. Forced units keep that unit for disk and bandwidth.</p>
        <button type="submit" class="btn-primary">Save details</button>
      </form>
      {pkg}"#,
        username = html_escape(boot_username),
        email = html_escape(recovery_email),
        en_sel = en_sel,
        es_sel = es_sel,
        nb_sel = nb_sel,
        unit_opts = crate::panel_storage_fmt::storage_unit_options_html(storage_unit),
        pkg = package_assign_section(boot_username, can_assign),
    )
}
