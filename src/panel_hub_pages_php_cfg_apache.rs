//! Apache / httpd PHP controls for the PHP Configurations page.

use crate::php_apache_default::{
    apache_php_ui_status, list_apache_php_versions, load_apache_php_default,
};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Runtime selector + Apache PHP default card (separate from LiteSpeed host default).
pub fn apache_php_section(
    selected_php: &str,
    runtime: &str,
    csrf: &str,
    tab: &str,
    is_admin: bool,
) -> String {
    let (httpd_ok, apache_label, _) = apache_php_ui_status();
    let apache_versions = list_apache_php_versions();
    let apache_selected = load_apache_php_default()
        .map(|r| r.branch)
        .filter(|b| apache_versions.iter().any(|v| &v.branch == b))
        .unwrap_or_else(|| {
            apache_versions
                .first()
                .map(|v| v.branch.clone())
                .unwrap_or_else(|| selected_php.to_string())
        });

    let runtime_ls = if runtime == "apache" { "" } else { " selected" };
    let runtime_ap = if runtime == "apache" { " selected" } else { "" };

    let runtime_form = format!(
        r#"<form method="get" action="/server/php/configs" class="php-cfg-toolbar" id="php-cfg-runtime-form">
      <div class="php-cfg-field">
        <label for="runtime">Edit settings for</label>
        <select id="runtime" name="runtime">
          <option value="litespeed"{runtime_ls}>LiteSpeed lsphp (host sites / OLS)</option>
          <option value="apache"{runtime_ap}>Apache PHP (httpd / system handler)</option>
        </select>
      </div>
      <input type="hidden" name="php" value="{php}">
      <input type="hidden" name="tab" value="{tab}">
      <noscript><button type="submit" class="btn-secondary">Load</button></noscript>
    </form>"#,
        runtime_ls = runtime_ls,
        runtime_ap = runtime_ap,
        php = html_escape(selected_php),
        tab = html_escape(tab),
    );

    let mut options = String::new();
    for v in &apache_versions {
        let sel = if v.branch == apache_selected {
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

    let disabled_attr = if httpd_ok && is_admin {
        ""
    } else {
        " disabled"
    };
    let set_btn = if is_admin {
        if httpd_ok {
            format!(
                r#"<button type="submit" class="btn-secondary"{disabled}>Set PHP <span id="php-apache-branch-label">{branch}</span> as Apache PHP default</button>"#,
                disabled = disabled_attr,
                branch = html_escape(&apache_selected),
            )
        } else {
            r#"<button type="button" class="btn-secondary" disabled>Set Apache PHP default (httpd required)</button>"#
                .to_string()
        }
    } else {
        String::new()
    };

    let heal = if httpd_ok {
        String::new()
    } else {
        r#"<p class="muted php-cfg-apache-heal">Apache httpd is not installed. Install it from <a href="/server/system-repair?filter=httpd">System Repair</a> (heal id <code>httpd</code>), then return here to set the Apache PHP default.</p>"#
            .to_string()
    };

    let form = if is_admin {
        format!(
            r#"<form method="post" action="/server/php/configs" class="php-cfg-actions php-cfg-apache-form" onsubmit="return confirm('Apply the selected version as the Apache / httpd PHP default? This does not change the LiteSpeed lsphp host default.');">
      <input type="hidden" name="csrf" value="{csrf}">
      <input type="hidden" name="op" value="set-apache-default">
      <input type="hidden" name="tab" value="{tab}">
      <input type="hidden" name="runtime" value="apache">
      <div class="php-cfg-field">
        <label for="apache_php">Apache PHP version</label>
        <select id="apache_php" name="php"{disabled}>{options}</select>
      </div>
      {set_btn}
    </form>"#,
            csrf = html_escape(csrf),
            tab = html_escape(tab),
            disabled = disabled_attr,
            options = options,
            set_btn = set_btn,
        )
    } else {
        format!(
            r#"<div class="php-cfg-field"><label>Apache PHP version</label>
      <select disabled>{options}</select></div>"#,
            options = options,
        )
    };

    format!(
        r#"<div class="php-cfg-apache card-block">
  <h3 class="php-cfg-apache-title">Apache PHP</h3>
  <p class="muted">Separate from LiteSpeed lsphp host default. Controls the system Apache handler / Remi PHP used with httpd. Stored in <code>/var/lib/cpn/php-apache-default.json</code>.</p>
  <p><strong>Apache PHP default:</strong> {label}</p>
  {runtime_form}
  {form}
  {heal}
</div>
<style>
.php-cfg-apache{{margin:18px 0;padding:14px 16px;border:1px solid var(--hairline);border-radius:12px;max-width:720px;}}
.php-cfg-apache-title{{margin:0 0 8px;font-size:1.05rem;}}
.php-cfg-apache-form .php-cfg-field{{margin-bottom:10px;}}
.php-cfg-apache-heal{{margin-top:10px;}}
</style>
<script>
(function () {{
  var rt = document.getElementById('runtime');
  if (rt) {{
    rt.addEventListener('change', function () {{
      var f = document.getElementById('php-cfg-runtime-form');
      if (f) f.submit();
    }});
  }}
  var ap = document.getElementById('apache_php');
  var lab = document.getElementById('php-apache-branch-label');
  if (ap && lab) {{
    ap.addEventListener('change', function () {{ lab.textContent = ap.value; }});
  }}
}})();
</script>"#,
        label = html_escape(&apache_label),
        runtime_form = runtime_form,
        form = form,
        heal = heal,
    )
}
