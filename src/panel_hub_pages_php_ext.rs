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

    let host_default = load_php_default()
        .map(|r| format!("{} ({})", r.branch, r.stream))
        .unwrap_or_else(|| format!("{default_branch} (not persisted yet)"));

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

    let select_form = format!(
        r#"<form method="get" action="/server/php/extensions" class="stack-form" style="display:flex;flex-wrap:wrap;gap:12px;align-items:end;max-width:720px;">
      <div style="flex:1;min-width:220px;">
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
            r#"<form method="get" action="/server/php/extensions" class="stack-form" style="max-width:420px;margin-top:12px;">
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
            r#"<form method="post" action="/server/php/extensions/set-default" class="stack-form" style="margin-top:12px;max-width:520px;" onsubmit="return confirm('Apply PHP {php} as the host default (php-default.json, Remi/php-fpm, LiteSpeed lsphp when present)?');">
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

    let table = if !loaded {
        "<p class=\"muted\">Choose a PHP version and click Load Extensions.</p>".to_string()
    } else {
        match list_extensions(&selected, &search_q) {
            Ok(rows) if rows.is_empty() => {
                "<p class=\"empty-state\">No extension packages found for this version.</p>"
                    .to_string()
            }
            Ok(rows) => {
                let mut t = String::from(
                    r#"<div class="table-wrap" style="margin-top:16px;"><table class="data-table"><thead><tr>
                    <th>ID</th><th>PHP version</th><th>Extension</th><th>Description</th><th>Status</th><th>Actions</th>
                    </tr></thead><tbody>"#,
                );
                for row in rows {
                    let status = if row.installed {
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
                        status = status,
                        actions = actions,
                    ));
                }
                t.push_str("</tbody></table></div>");
                t
            }
            Err(err) => format!(
                "<p class=\"panel-notice error\">{e}</p>",
                e = html_escape(&err)
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

    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("PHP Extensions", None),
        ],
        "PHP Extensions",
        "Install or uninstall PHP extensions for each available PHP version.",
        &format!("{kv}{select_form}{set_default}{search_form}{table}{note}"),
        notice,
        error,
    )
}
