//! PHP Configurations UI: host default, basic settings, advanced php.ini editor.

use crate::panel_hubs::{feature_shell, status_kv};
use crate::panel_ops_php_ext::{list_php_versions, php_ext_csrf_token, selected_default_branch};
use crate::panel_ops_php_ini::{
    is_ini_truthy, read_basic_setting, read_php_ini_text, resolve_php_ini_target,
};
use crate::php_defaults::load_php_default;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn host_default_label() -> (String, bool) {
    match load_php_default() {
        Some(r) => (format!("{} ({})", r.branch, r.stream), true),
        None => {
            let fallback = selected_default_branch();
            (format!("{fallback} (not persisted yet)"), false)
        }
    }
}

/// Render PHP Configurations (basic settings or advanced php.ini editor).
pub fn php_configurations_page(
    username: &str,
    php: Option<&str>,
    tab: &str,
    notice: Option<&str>,
    error: Option<&str>,
    is_admin: bool,
) -> String {
    let versions = list_php_versions();
    let default_branch = selected_default_branch();
    let selected = php
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(default_branch.as_str())
        .to_string();
    let tab = if tab == "advanced" {
        "advanced"
    } else {
        "basic"
    };
    let csrf = php_ext_csrf_token(username);
    let (host_default, persisted) = host_default_label();

    let mut options = String::new();
    for v in &versions {
        let sel = if v.branch == selected {
            " selected"
        } else {
            ""
        };
        options.push_str(&format!(
            r#"<option value="{branch}"{sel}>{label}</option>"#,
            branch = html_escape(&v.branch),
            sel = sel,
            label = html_escape(&v.label),
        ));
    }

    let version_form = format!(
        r#"<form method="get" action="/server/php/configs" class="php-cfg-toolbar" id="php-cfg-version-form">
      <div class="php-cfg-field">
        <label for="php">Select PHP Version</label>
        <select id="php" name="php" data-initial="{php}">{options}</select>
      </div>
      <input type="hidden" name="tab" value="{tab}" id="php-cfg-tab">
      <noscript><button type="submit" class="btn-secondary">Load</button></noscript>
    </form>"#,
        options = options,
        tab = tab,
        php = html_escape(&selected),
    );

    let set_default = if is_admin {
        format!(
            r#"<form method="post" action="/server/php/configs/set-default" class="php-cfg-actions" onsubmit="return confirm('Apply PHP {php} as the host default used by system tools (php-fpm / phpMyAdmin) and new sites?');">
      <input type="hidden" name="csrf" value="{csrf}">
      <input type="hidden" name="php" value="{php}">
      <input type="hidden" name="tab" value="{tab}">
      <button type="submit" class="btn-secondary">Set PHP {php} as host default</button>
    </form>"#,
            php = html_escape(&selected),
            csrf = html_escape(&csrf),
            tab = tab,
        )
    } else {
        String::new()
    };

    let tabs = format!(
        r#"<div class="php-cfg-tabs" role="tablist">
      <a class="php-cfg-tab{basic}" href="/server/php/configs?php={php}&amp;tab=basic">Basic Settings</a>
      <a class="php-cfg-tab{adv}" href="/server/php/configs?php={php}&amp;tab=advanced">Advanced Editor</a>
    </div>"#,
        php = html_escape(&selected),
        basic = if tab == "basic" { " is-active" } else { "" },
        adv = if tab == "advanced" { " is-active" } else { "" },
    );

    let editor = match resolve_php_ini_target(&selected) {
        Ok(target) => match read_php_ini_text(&target.ini_path) {
            Ok(raw) => {
                if tab == "advanced" {
                    advanced_editor(
                        &target.ini_path.display().to_string(),
                        &raw,
                        &selected,
                        &csrf,
                        is_admin,
                    )
                } else {
                    basic_settings(&raw, &selected, &csrf, is_admin)
                }
            }
            Err(err) => format!(
                "<p class=\"panel-notice error\">{e}</p>",
                e = html_escape(&err)
            ),
        },
        Err(err) => format!(
            "<p class=\"panel-notice error\">{e}</p>",
            e = html_escape(&err)
        ),
    };

    let persist_note = if persisted {
        "Stored in /var/lib/cpn/php-default.json (used by phpMyAdmin / php-fpm and new sites)."
    } else {
        "Not written yet. Use Set as host default to persist for phpMyAdmin and system PHP."
    };

    let kv = status_kv(&[
        ("Host default", host_default.as_str()),
        ("Selected", selected.as_str()),
        ("Persistence", persist_note),
    ]);

    let cross = format!(
        r#"<p class="muted php-cfg-cross">Related: <a href="/server/php/extensions?php={php}&amp;load=1">PHP Extensions</a></p>"#,
        php = html_escape(&selected),
    );

    let styles = r#"<style>
.php-cfg-toolbar{display:flex;flex-wrap:wrap;gap:12px;align-items:end;margin-top:8px;max-width:720px;}
.php-cfg-field{flex:1 1 220px;min-width:180px;}
.php-cfg-field label{display:block;margin-bottom:6px;font-weight:600;font-size:.92rem;}
.php-cfg-field select{width:100%;box-sizing:border-box;}
.php-cfg-actions{margin-top:12px;display:flex;flex-wrap:wrap;gap:10px;}
.php-cfg-actions .btn-secondary,.php-cfg-toolbar .btn-secondary{width:auto;max-width:100%;}
.php-cfg-tabs{display:flex;flex-wrap:wrap;gap:8px;margin:18px 0 12px;border-bottom:1px solid var(--hairline);padding-bottom:8px;}
.php-cfg-tab{padding:8px 12px;border-radius:8px 8px 0 0;color:var(--muted);text-decoration:none;font-weight:600;}
.php-cfg-tab.is-active{color:var(--ink);box-shadow:inset 0 -2px 0 var(--blue);}
.php-cfg-row{display:flex;flex-wrap:wrap;justify-content:space-between;gap:12px;align-items:center;padding:12px 0;border-top:1px solid #eeeef0;}
.php-cfg-row code{color:#2563eb;}
.php-cfg-row .php-cfg-desc{color:var(--muted);font-size:.9rem;display:block;margin-top:2px;}
.php-cfg-ctrl{min-width:120px;max-width:100%;}
.php-cfg-ctrl input[type=text],.php-cfg-ctrl input[type=number]{width:140px;max-width:100%;}
.php-cfg-switch{display:inline-flex;align-items:center;gap:8px;}
.php-cfg-editor{width:100%;min-height:360px;font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:12px;box-sizing:border-box;}
.php-cfg-footer{display:flex;flex-wrap:wrap;gap:10px;margin-top:16px;}
.php-cfg-footer .btn-primary,.php-cfg-footer .btn-secondary{flex:1 1 160px;}
.php-cfg-cross{margin-top:18px;}
.php-cfg-modal-backdrop{position:fixed;inset:0;background:rgba(15,23,42,.55);display:none;align-items:center;justify-content:center;z-index:1200;padding:16px;}
.php-cfg-modal-backdrop.is-open{display:flex;}
.php-cfg-modal{background:var(--panel,#fff);color:var(--ink,#0f172a);border-radius:12px;max-width:420px;width:100%;padding:20px 22px;box-shadow:0 18px 50px rgba(15,23,42,.28);}
.php-cfg-modal h3{margin:0 0 8px;font-size:1.1rem;}
.php-cfg-modal p{margin:0 0 16px;color:var(--muted,#64748b);line-height:1.45;}
.php-cfg-modal-actions{display:flex;flex-wrap:wrap;gap:10px;justify-content:flex-end;}
.php-cfg-modal-actions .btn-primary,.php-cfg-modal-actions .btn-secondary{flex:1 1 auto;min-width:110px;}
@media (max-width:679.98px){
  .php-cfg-row{flex-direction:column;align-items:stretch;}
  .php-cfg-ctrl input[type=text],.php-cfg-ctrl input[type=number]{width:100%;}
  .php-cfg-footer .btn-primary,.php-cfg-footer .btn-secondary{flex:1 1 100%;}
}
</style>"#;

    let modal = r#"<div class="php-cfg-modal-backdrop" id="php-cfg-dirty-modal" role="dialog" aria-modal="true" aria-labelledby="php-cfg-dirty-title" hidden>
  <div class="php-cfg-modal">
    <h3 id="php-cfg-dirty-title">Unsaved changes</h3>
    <p>You have unsaved PHP settings. Save them before switching versions, abandon the changes, or cancel.</p>
    <div class="php-cfg-modal-actions">
      <button type="button" class="btn-secondary" id="php-cfg-dirty-cancel">Cancel</button>
      <button type="button" class="btn-secondary" id="php-cfg-dirty-abandon">Abandon changes</button>
      <button type="button" class="btn-primary" id="php-cfg-dirty-save">Save first</button>
    </div>
  </div>
</div>"#;

    let script = r#"<script>
(function () {
  var select = document.getElementById('php');
  var versionForm = document.getElementById('php-cfg-version-form');
  var modal = document.getElementById('php-cfg-dirty-modal');
  if (!select || !versionForm) return;

  var settingsForm = document.querySelector('form.php-cfg-basic, form[action="/server/php/configs/save-advanced"]');
  var initialSnapshot = settingsForm ? formSnapshot(settingsForm) : '';
  var pendingBranch = null;
  var PENDING_KEY = 'cpn-php-cfg-pending-branch';

  try {
    var resume = sessionStorage.getItem(PENDING_KEY);
    if (resume) {
      sessionStorage.removeItem(PENDING_KEY);
      if (resume !== (select.getAttribute('data-initial') || '')) {
        navigateTo(resume);
        return;
      }
    }
  } catch (e) {}

  function formSnapshot(form) {
    var data = new FormData(form);
    var parts = [];
    data.forEach(function (value, key) {
      if (key === 'csrf') return;
      parts.push(key + '=' + String(value));
    });
    // Unchecked boxes are absent from FormData; include explicit off state.
    form.querySelectorAll('input[type=checkbox]').forEach(function (el) {
      if (!el.name) return;
      if (!el.checked) parts.push(el.name + '=0');
    });
    parts.sort();
    return parts.join('&');
  }

  function isDirty() {
    if (!settingsForm) return false;
    return formSnapshot(settingsForm) !== initialSnapshot;
  }

  function navigateTo(branch) {
    var tabEl = document.getElementById('php-cfg-tab');
    var tab = tabEl ? tabEl.value : 'basic';
    var url = '/server/php/configs?php=' + encodeURIComponent(branch) + '&tab=' + encodeURIComponent(tab);
    window.location.assign(url);
  }

  function openModal(branch) {
    pendingBranch = branch;
    modal.hidden = false;
    modal.classList.add('is-open');
  }

  function closeModal() {
    pendingBranch = null;
    modal.classList.remove('is-open');
    modal.hidden = true;
    var initial = select.getAttribute('data-initial') || select.value;
    select.value = initial;
  }

  select.addEventListener('change', function () {
    var next = select.value;
    var current = select.getAttribute('data-initial') || '';
    if (next === current) return;
    if (isDirty()) {
      openModal(next);
      return;
    }
    navigateTo(next);
  });

  if (modal) {
    document.getElementById('php-cfg-dirty-cancel').addEventListener('click', closeModal);
    document.getElementById('php-cfg-dirty-abandon').addEventListener('click', function () {
      if (pendingBranch) navigateTo(pendingBranch);
    });
    document.getElementById('php-cfg-dirty-save').addEventListener('click', function () {
      if (!settingsForm) {
        closeModal();
        return;
      }
      try {
        if (pendingBranch) sessionStorage.setItem(PENDING_KEY, pendingBranch);
      } catch (e) {}
      closeModal();
      if (typeof settingsForm.requestSubmit === 'function') {
        settingsForm.requestSubmit();
      } else {
        settingsForm.submit();
      }
    });
    modal.addEventListener('click', function (ev) {
      if (ev.target === modal) closeModal();
    });
    document.addEventListener('keydown', function (ev) {
      if (ev.key === 'Escape' && modal.classList.contains('is-open')) closeModal();
    });
  }
})();
</script>"#;

    let note = if is_admin {
        "<p class=\"muted\">Saving creates a backup under <code>/var/lib/cpn/php-ini-backups/</code> before writing php.ini. Host default controls system php-fpm (phpMyAdmin), not only per-site handlers.</p>"
    } else {
        "<p class=\"muted\">Only the panel admin can edit php.ini or change the host default.</p>"
    };

    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("PHP Configurations", None),
        ],
        "PHP Configurations",
        "Configure PHP settings and choose the host default used by system tools like phpMyAdmin.",
        &format!(
            "{styles}{kv}{version_form}{set_default}{tabs}{editor}{note}{cross}{modal}{script}"
        ),
        notice,
        error,
    )
}

fn basic_settings(raw: &str, php: &str, csrf: &str, is_admin: bool) -> String {
    let mut rows = String::new();
    let bool_meta = [
        ("display_errors", "Show PHP errors to visitors."),
        ("file_uploads", "Allow file uploads."),
        ("allow_url_fopen", "Allow opening URLs as files."),
        ("allow_url_include", "Allow including URLs."),
    ];
    for (key, desc) in bool_meta {
        let on = read_basic_setting(raw, key)
            .map(|v| is_ini_truthy(&v))
            .unwrap_or(false);
        let checked = if on { " checked" } else { "" };
        let disabled = if is_admin { "" } else { " disabled" };
        rows.push_str(&format!(
            r#"<div class="php-cfg-row">
          <div><code>{key}</code><span class="php-cfg-desc">{desc}</span></div>
          <div class="php-cfg-ctrl"><label class="php-cfg-switch"><input type="checkbox" name="{key}" value="1"{checked}{disabled}> On</label></div>
        </div>"#,
            key = key,
            desc = desc,
            checked = checked,
            disabled = disabled,
        ));
    }
    let value_meta = [
        ("memory_limit", "Maximum memory per script.", "2048M"),
        (
            "max_execution_time",
            "Maximum execution time (seconds).",
            "240",
        ),
        ("upload_max_filesize", "Maximum upload file size.", "2048M"),
        ("post_max_size", "Maximum POST data size.", "2048M"),
        (
            "max_input_time",
            "Maximum input parsing time (seconds).",
            "300",
        ),
    ];
    for (key, desc, fallback) in value_meta {
        let val = read_basic_setting(raw, key).unwrap_or_else(|| fallback.to_string());
        let disabled = if is_admin { "" } else { " disabled" };
        rows.push_str(&format!(
            r#"<div class="php-cfg-row">
          <div><code>{key}</code><span class="php-cfg-desc">{desc}</span></div>
          <div class="php-cfg-ctrl"><input type="text" name="{key}" value="{val}"{disabled}></div>
        </div>"#,
            key = key,
            desc = desc,
            val = html_escape(&val),
            disabled = disabled,
        ));
    }
    if !is_admin {
        return format!("<div class=\"php-cfg-basic\">{rows}</div>");
    }
    format!(
        r#"<form method="post" action="/server/php/configs/save-basic" class="php-cfg-basic">
      <input type="hidden" name="csrf" value="{csrf}">
      <input type="hidden" name="php" value="{php}">
      {rows}
      <div class="php-cfg-footer">
        <button type="submit" class="btn-primary">Save Changes</button>
        <button type="submit" formaction="/server/php/configs/restart" class="btn-secondary">Restart PHP</button>
      </div>
    </form>"#,
        csrf = html_escape(csrf),
        php = html_escape(php),
        rows = rows,
    )
}

fn advanced_editor(path: &str, raw: &str, php: &str, csrf: &str, is_admin: bool) -> String {
    let disabled = if is_admin { "" } else { " disabled" };
    let save = if is_admin {
        r#"<div class="php-cfg-footer">
        <button type="submit" class="btn-primary">Save Changes</button>
        <button type="submit" formaction="/server/php/configs/restart" class="btn-secondary">Restart PHP</button>
      </div>"#
            .to_string()
    } else {
        String::new()
    };
    format!(
        r#"<form method="post" action="/server/php/configs/save-advanced">
      <input type="hidden" name="csrf" value="{csrf}">
      <input type="hidden" name="php" value="{php}">
      <p class="muted">Editing <code>{path}</code></p>
      <label for="ini"><strong>php.ini</strong></label>
      <textarea id="ini" name="ini" class="php-cfg-editor" spellcheck="false"{disabled}>{body}</textarea>
      {save}
    </form>"#,
        csrf = html_escape(csrf),
        php = html_escape(php),
        path = html_escape(path),
        body = html_escape(raw),
        disabled = disabled,
        save = save,
    )
}
