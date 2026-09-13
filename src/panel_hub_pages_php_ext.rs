//! PHP Extensions manager UI (version select, load, install/uninstall table).

use crate::panel_hubs::{feature_shell, status_kv};
use crate::panel_ops_php_ext::{
    list_extensions, list_php_versions, php_ext_csrf_token, selected_default_branch,
};
use crate::php_defaults::load_php_default;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn host_default_label() -> String {
    match load_php_default() {
        Some(r) => format!("{} ({})", r.branch, r.stream),
        None => {
            let fallback = selected_default_branch();
            format!("{fallback} (not persisted yet)")
        }
    }
}

/// Render the PHP Extensions manager.
pub fn php_extensions_page(
    username: &str,
    php: Option<&str>,
    search: Option<&str>,
    loaded: bool,
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
    let search_q = search.unwrap_or("").trim().to_string();
    let csrf = php_ext_csrf_token(username);
    let host_default = host_default_label();

    let mut options = String::new();
    for v in &versions {
        let sel = if v.branch == selected {
            " selected"
        } else {
            ""
        };
        let mark = if v.installed { "" } else { " (not installed)" };
        options.push_str(&format!(
            r#"<option value="{branch}"{sel}>{label}{mark}</option>"#,
            branch = html_escape(&v.branch),
            sel = sel,
            label = html_escape(&v.label),
            mark = mark,
        ));
    }

    let styles = r#"<style>
.php-ext-toolbar{display:flex;flex-wrap:wrap;gap:12px;align-items:end;margin-top:8px;max-width:720px;}
.php-ext-field{flex:1 1 220px;min-width:160px;}
.php-ext-field label{display:block;margin-bottom:6px;font-weight:600;font-size:.92rem;}
.php-ext-field select,.php-ext-field input{width:100%;box-sizing:border-box;}
.php-ext-toolbar .btn-primary,.php-ext-search .btn-secondary,.php-ext-default .btn-secondary{
  width:auto;max-width:100%;white-space:normal;
}
.php-ext-default,.php-ext-search{margin-top:12px;max-width:520px;}
.php-ext-cross{margin-top:14px;}
.php-ext-list{display:none;margin-top:16px;gap:12px;}
.php-ext-card{border:1px solid var(--hairline);border-radius:12px;padding:12px 14px;background:var(--canvas);}
.php-ext-card-top{display:flex;flex-wrap:wrap;gap:8px;align-items:center;justify-content:space-between;}
.php-ext-card-meta{color:var(--muted);font-size:.88rem;margin:8px 0;}
.php-ext-card-actions{margin-top:10px;}
.php-ext-table-wrap{margin-top:16px;}
@media (max-width:719.98px){
  .php-ext-table-wrap{display:none;}
  .php-ext-list{display:grid;}
  .php-ext-toolbar{max-width:100%;}
  .php-ext-default,.php-ext-search{max-width:100%;}
  .php-ext-toolbar .btn-primary,.php-ext-search .btn-secondary,.php-ext-default .btn-secondary{
    width:100%;
  }
}
@media (min-width:720px){
  .php-ext-list{display:none !important;}
}
</style>"#;

    let select_form = format!(
        r#"<form method="get" action="/server/php/extensions" class="php-ext-toolbar">
      <div class="php-ext-field">
        <label for="php">Select PHP Version</label>
        <select id="php" name="php">{options}</select>
      </div>
      <input type="hidden" name="load" value="1">
      <button type="submit" class="btn-primary">Load Extensions</button>
    </form>"#,
        options = options,
    );

    let search_form = if loaded {
        format!(
            r#"<form method="get" action="/server/php/extensions" class="php-ext-search stack-form">
      <input type="hidden" name="php" value="{php}">
      <input type="hidden" name="load" value="1">
      <label for="q">Search extensions</label>
      <input id="q" name="q" type="text" value="{q}" placeholder="Search extensions...">
      <button type="submit" class="btn-secondary">Search</button>
    </form>"#,
            php = html_escape(&selected),
            q = html_escape(&search_q),
        )
    } else {
        String::new()
    };

    let set_default = if is_admin {
        format!(
            r#"<form method="post" action="/server/php/extensions/set-default" class="php-ext-default stack-form" onsubmit="return confirm('Apply PHP {php} as the host default (php-default.json, Remi/php-fpm, LiteSpeed lsphp when present)?');">
      <input type="hidden" name="csrf" value="{csrf}">
      <input type="hidden" name="php" value="{php}">
      <button type="submit" class="btn-secondary">Set PHP {php} as host default</button>
    </form>"#,
            php = html_escape(&selected),
            csrf = html_escape(&csrf),
        )
    } else {
        String::new()
    };

    let (table, cards) = if !loaded {
        (
            "<p class=\"muted\">Choose a PHP version and click Load Extensions.</p>".to_string(),
            String::new(),
        )
    } else {
        match list_extensions(&selected, &search_q) {
            Ok(rows) if rows.is_empty() => (
                "<p class=\"empty-state\">No extension packages found for this version.</p>"
                    .to_string(),
                String::new(),
            ),
            Ok(rows) => {
                let mut t = String::from(
                    r#"<div class="table-wrap php-ext-table-wrap"><table class="data-table"><thead><tr>
                    <th>ID</th><th>PHP version</th><th>Extension</th><th>Description</th><th>Status</th><th>Actions</th>
                    </tr></thead><tbody>"#,
                );
                let mut cards =
                    String::from(r#"<div class="php-ext-list" aria-label="Extensions">"#);
                for row in rows {
                    let status_html = if row.installed {
                        r#"<span class="badge ok">Installed</span>"#
                    } else {
                        r#"<span class="badge">Available</span>"#
                    };
                    let actions = if !is_admin {
                        "<span class=\"muted\">Admin only</span>".to_string()
                    } else if row.installed && row.protected {
                        "<span class=\"muted\">Protected</span>".to_string()
                    } else if row.installed {
                        format!(
                            r#"<form method="post" action="/server/php/extensions/uninstall" class="inline-form" onsubmit="return confirm('Uninstall {pkg}?');">
                          <input type="hidden" name="csrf" value="{csrf}">
                          <input type="hidden" name="php" value="{php}">
                          <input type="hidden" name="package" value="{pkg}">
                          <input type="hidden" name="q" value="{q}">
                          <button type="submit" class="btn-danger">Uninstall</button>
                        </form>"#,
                            csrf = html_escape(&csrf),
                            php = html_escape(&selected),
                            pkg = html_escape(&row.package),
                            q = html_escape(&search_q),
                        )
                    } else {
                        format!(
                            r#"<form method="post" action="/server/php/extensions/install" class="inline-form">
                          <input type="hidden" name="csrf" value="{csrf}">
                          <input type="hidden" name="php" value="{php}">
                          <input type="hidden" name="package" value="{pkg}">
                          <input type="hidden" name="q" value="{q}">
                          <button type="submit" class="btn-primary">Install</button>
                        </form>"#,
                            csrf = html_escape(&csrf),
                            php = html_escape(&selected),
                            pkg = html_escape(&row.package),
                            q = html_escape(&search_q),
                        )
                    };
                    t.push_str(&format!(
                        r#"<tr>
                          <td>#{id}</td>
                          <td><span class="badge">{badge}</span></td>
                          <td><code>{name}</code></td>
                          <td>{desc}</td>
                          <td>{status}</td>
                          <td>{actions}</td>
                        </tr>"#,
                        id = row.id,
                        badge = html_escape(&row.branch),
                        name = html_escape(&row.name),
                        desc = html_escape(&row.description),
                        status = status_html,
                        actions = actions,
                    ));
                    cards.push_str(&format!(
                        r#"<article class="php-ext-card">
                          <div class="php-ext-card-top">
                            <strong><code>{name}</code></strong>
                            {status}
                          </div>
                          <p class="php-ext-card-meta">#{id} · PHP {badge}<br>{desc}</p>
                          <div class="php-ext-card-actions">{actions}</div>
                        </article>"#,
                        id = row.id,
                        badge = html_escape(&row.branch),
                        name = html_escape(&row.name),
                        desc = html_escape(&row.description),
                        status = status_html,
                        actions = actions,
                    ));
                }
                t.push_str("</tbody></table></div>");
                cards.push_str("</div>");
                (t, cards)
            }
            Err(err) => (
                format!(
                    "<p class=\"panel-notice error\">{e}</p>",
                    e = html_escape(&err)
                ),
                String::new(),
            ),
        }
    };

    let kv = status_kv(&[
        ("Host default", host_default.as_str()),
        ("Selected", selected.as_str()),
        (
            "Package surface",
            if versions.iter().any(|v| {
                v.branch == selected
                    && matches!(
                        v.family,
                        crate::panel_ops_php_ext::PhpPackageFamily::LiteSpeed
                    )
            }) {
                "LiteSpeed lsphp"
            } else {
                "System PHP module"
            },
        ),
    ]);

    let note = if is_admin {
        "<p class=\"muted\">Install and uninstall use dnf allowlisted packages only (LiteSpeed <code>lsphpXX-*</code> when OpenLiteSpeed is present). CSRF tokens bind each action to your session.</p>"
    } else {
        "<p class=\"muted\">Only the panel admin can install or uninstall PHP extensions.</p>"
    };

    let cross = format!(
        r#"<p class="muted php-ext-cross">Related: <a href="/server/php/configs?php={php}">PHP Configurations</a> (host default, php.ini, Restart PHP)</p>"#,
        php = html_escape(&selected),
    );

    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("PHP Extensions", None),
        ],
        "PHP Extensions",
        "Install or uninstall PHP extensions for each available PHP version.",
        &format!("{styles}{kv}{select_form}{set_default}{search_form}{table}{cards}{note}{cross}"),
        notice,
        error,
    )
}
