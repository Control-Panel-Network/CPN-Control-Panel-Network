//! MariaDB Manager list, password change, and confirmed delete pages.

use crate::panel_dashboard_activity_list::wrap_activity_table;
use crate::panel_db_acl::{ManagedDatabase, list_managed_databases};
use crate::panel_hubs::{feature_shell, status_kv};
use crate::panel_ops_db::{
    is_protected_db_user, is_system_database, list_databases, list_usernames_for_database,
};
use crate::uninstall_confirm::{impacts_attr, uninstall_dialog_bundle};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn db_delete_impacts(db: &ManagedDatabase, users: &[String]) -> Vec<String> {
    let mut impacts = vec![
        format!(
            "Permanently drops MariaDB database `{}` and all of its tables/data",
            db.name
        ),
        "This cannot be undone without a backup restore".into(),
    ];
    if db.in_registry {
        impacts.push(format!(
            "Removes the CPN registry entry for `{}` (quota counting)",
            db.name
        ));
    }
    let droppable: Vec<&String> = users.iter().filter(|u| !is_protected_db_user(u)).collect();
    if droppable.is_empty() {
        impacts.push(
            "No dedicated MariaDB users with schema grants on this database were found to remove"
                .into(),
        );
    } else {
        impacts.push(format!(
            "May remove MariaDB users that only had grants on this database: {}",
            droppable
                .iter()
                .map(|u| format!("`{u}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        impacts.push(
            "Users that still have grants on other databases, and protected accounts, are kept"
                .into(),
        );
    }
    impacts
}

fn managed_rows(dbs: &[ManagedDatabase]) -> String {
    if dbs.is_empty() {
        return r#"<p class="empty-state">No manageable databases yet. Create one, or ask the panel owner for Host access.</p>"#.into();
    }
    let mut rows = String::from(
        r#"<div class="table-wrap"><table class="data-table" id="databases-table">
      <thead><tr>
        <th>Database</th><th>Owner</th><th>Domain</th><th>Status</th><th>Actions</th>
      </tr></thead><tbody>"#,
    );
    for db in dbs {
        let status = match (db.in_mariadb, db.in_registry) {
            (true, true) => "MariaDB + registry",
            (true, false) => "MariaDB only",
            (false, true) => "Registry only",
            (false, false) => "Unknown",
        };
        let owner = if db.owner.is_empty() {
            "<span class=\"muted\">-</span>".into()
        } else {
            html_escape(&db.owner)
        };
        let domain = if db.domain.is_empty() {
            "<span class=\"muted\">Host</span>".into()
        } else {
            html_escape(&db.domain)
        };
        rows.push_str(&format!(
            r#"<tr>
          <td data-label="Database"><code>{name}</code></td>
          <td data-label="Owner">{owner}</td>
          <td data-label="Domain">{domain}</td>
          <td data-label="Status">{status}</td>
          <td data-label="Actions">
            <div class="db-actions">
              <a class="btn-secondary" href="/databases/password?name={name_q}">Change password</a>
              <a class="btn-danger" href="/databases/delete?name={name_q}">Delete</a>
            </div>
          </td>
        </tr>"#,
            name = html_escape(&db.name),
            name_q = urlencoding_path(&db.name),
            owner = owner,
            domain = domain,
            status = html_escape(status),
        ));
    }
    rows.push_str("</tbody></table></div>");
    format!(
        r#"<style>
.db-actions {{ display:flex; flex-wrap:wrap; gap:8px; }}
.db-actions .btn-secondary, .db-actions .btn-danger {{
  display:inline-block; text-decoration:none; padding:6px 10px; font-size:13px; border-radius:8px;
}}
.db-actions .btn-danger {{
  background:#fee2e2; color:#991b1b; border:1px solid #fecaca;
}}
[data-color-mode="dark"] .db-actions .btn-danger {{
  background:#7f1d1d; color:#fecaca; border-color:#991b1b;
}}
</style>{}"#,
        wrap_activity_table("databases", "Filter database, owner, or domain", &rows)
    )
}

fn urlencoding_path(value: &str) -> String {
    let mut out = String::new();
    for b in value.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn databases_all_page_for(username: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let status = list_databases();
    let managed = list_managed_databases(username);
    let kv = status_kv(&[
        ("Engine", &status.engine_label),
        ("TCP 3306", if status.listening { "Open" } else { "Closed" }),
        ("Visible", &managed.len().to_string()),
    ]);
    let help = format!(
        "<p class=\"muted\">{} Site users see domain-jailed / owned registry databases. Panel owner can manage Host MariaDB schemas. Passwords are never logged.</p>
        <p style=\"display:flex;flex-wrap:wrap;gap:8px;margin-top:12px;\">
          <a class=\"btn-primary\" href=\"/databases/create\">Create database</a>
          <a class=\"btn-secondary\" href=\"/databases/password\">Change password</a>
          <a class=\"btn-secondary\" href=\"/databases/delete\">Delete database</a>
          <a class=\"btn-secondary\" href=\"/databases/manager\">MariaDB Manager</a>
        </p>",
        html_escape(&status.detail)
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Databases & FTP", Some("/databases")),
            ("All Databases", None),
        ],
        "All Databases",
        "Manage MariaDB databases, passwords, and deletions.",
        &format!("{kv}{}{help}", managed_rows(&managed)),
        notice,
        error,
    )
}

pub fn databases_password_page_for(
    username: &str,
    selected_name: Option<&str>,
    notice: Option<&str>,
    error: Option<&str>,
) -> String {
    let managed = list_managed_databases(username);
    let selected = selected_name
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    let options = if managed.is_empty() {
        "<option value=\"\">No manageable databases</option>".to_string()
    } else {
        managed
            .iter()
            .map(|db| {
                let sel = if db.name.eq_ignore_ascii_case(selected) {
                    " selected"
                } else {
                    ""
                };
                format!(
                    "<option value=\"{n}\"{sel}>{n}</option>",
                    n = html_escape(&db.name),
                    sel = sel
                )
            })
            .collect::<Vec<_>>()
            .join("")
    };

    let mut user_options =
        String::from("<option value=\"\">Enter username below if empty</option>");
    if !selected.is_empty() {
        if let Ok(users) = list_usernames_for_database(selected) {
            for u in users {
                if is_protected_db_user(&u) {
                    continue;
                }
                user_options.push_str(&format!(
                    "<option value=\"{u}\">{u}</option>",
                    u = html_escape(&u)
                ));
            }
        }
    }

    let form = format!(
        r#"<form method="post" action="/databases/password" class="stack-form" style="max-width:460px;">
      <label for="name">Database</label>
      <select id="name" name="name" required onchange="if(this.value){{location='/databases/password?name='+encodeURIComponent(this.value);}}">{options}</select>
      <label for="db_user_pick">MariaDB user (from grants)</label>
      <select id="db_user_pick" name="db_user_pick">{user_options}</select>
      <label for="db_user">Or type MariaDB username</label>
      <input id="db_user" name="db_user" type="text" pattern="[A-Za-z0-9_]+" maxlength="64" placeholder="app_user" autocomplete="off">
      <label for="password">New password</label>
      <input id="password" name="password" type="password" required minlength="8" maxlength="128" autocomplete="new-password">
      <label for="password_confirm">Confirm password</label>
      <input id="password_confirm" name="password_confirm" type="password" required minlength="8" maxlength="128" autocomplete="new-password">
      <button type="submit" class="btn-primary" {disabled}>Update database password</button>
    </form>
    <p class="muted">Updates <code>localhost</code>, <code>127.0.0.1</code>, and <code>%</code> hosts when those accounts exist. Protected panel users (for example <code>root</code>, <code>cpn_pma</code>) cannot be changed here. The new password is never written to logs.</p>
    <script>
    (function(){{
      var pick=document.getElementById('db_user_pick');
      var typed=document.getElementById('db_user');
      if(!pick||!typed) return;
      pick.addEventListener('change', function(){{
        if(pick.value) typed.value = pick.value;
      }});
    }})();
    </script>"#,
        options = options,
        user_options = user_options,
        disabled = if managed.is_empty() { "disabled" } else { "" },
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Databases & FTP", Some("/databases")),
            ("Change password", None),
        ],
        "Change database password",
        "Reset a MariaDB user password for a database you manage.",
        &form,
        notice,
        error,
    )
}

pub fn databases_delete_page_for(
    username: &str,
    selected_name: Option<&str>,
    notice: Option<&str>,
    error: Option<&str>,
) -> String {
    let managed = list_managed_databases(username);
    let selected = selected_name
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    let selected_db = managed
        .iter()
        .find(|d| d.name.eq_ignore_ascii_case(selected));

    let options = if managed.is_empty() {
        "<option value=\"\">No manageable databases</option>".to_string()
    } else {
        managed
            .iter()
            .map(|db| {
                let sel = if db.name.eq_ignore_ascii_case(selected) {
                    " selected"
                } else {
                    ""
                };
                format!(
                    "<option value=\"{n}\"{sel}>{n}</option>",
                    n = html_escape(&db.name),
                    sel = sel
                )
            })
            .collect::<Vec<_>>()
            .join("")
    };

    let (impact_list, impacts_data, confirm_name_hint) = if let Some(db) = selected_db {
        let users = list_usernames_for_database(&db.name).unwrap_or_default();
        let impacts = db_delete_impacts(db, &users);
        let list = impacts
            .iter()
            .map(|i| format!("<li>{}</li>", html_escape(i)))
            .collect::<Vec<_>>()
            .join("");
        (
            format!(
                r#"<div class="db-delete-impact">
          <p class="cpn-uninstall-warn">If you continue, these changes apply:</p>
          <ul>{list}</ul>
        </div>"#
            ),
            impacts_attr(&impacts),
            html_escape(&db.name),
        )
    } else {
        (
            r#"<p class="muted">Select a database to preview delete impact.</p>"#.to_string(),
            impacts_attr(&[
                "Permanently drops the MariaDB database and its data".into(),
                "May remove dedicated MariaDB users with grants only on that database".into(),
                "Removes the CPN registry entry when present".into(),
            ]),
            String::new(),
        )
    };

    let form = format!(
        r#"<form method="get" action="/databases/delete" class="stack-form" style="max-width:520px;margin-bottom:16px;">
      <label for="name_pick">Database</label>
      <select id="name_pick" name="name" required onchange="this.form.submit()">{options}</select>
      <noscript><button type="submit" class="btn-secondary">Preview impact</button></noscript>
    </form>
    <script>
    (function(){{
      document.addEventListener('submit', function(ev){{
        var form=ev.target;
        if(!form||!form.classList||!form.classList.contains('db-delete-form')) return;
        var expected=form.querySelector('input[name="name"]');
        var typed=form.querySelector('input[name="confirm_name"]');
        if(!expected||!typed) return;
        if(String(typed.value||'').trim() !== String(expected.value||'').trim()) {{
          ev.preventDefault();
          ev.stopImmediatePropagation();
          alert('Type the exact database name to confirm delete.');
          typed.focus();
          return false;
        }}
        var flag=form.querySelector('input[name="drop_users_flag"]');
        var hidden=form.querySelector('input[name="drop_users"]');
        if(hidden) hidden.value = (flag && flag.checked) ? '1' : '0';
      }}, true);
    }})();
    </script>
    {dialog}
    <form method="post" action="/databases/delete" class="stack-form cpn-uninstall-form db-delete-form"
          style="max-width:520px;"
          data-uninstall-name="database {confirm_esc}"
          data-uninstall-impacts="{impacts_data}">
      <input type="hidden" name="name" value="{confirm_esc}">
      <input type="hidden" name="confirm" value="0">
      <input type="hidden" name="drop_users" value="1">
      {impact_list}
      <label for="confirm_name">Type the database name to confirm</label>
      <input id="confirm_name" name="confirm_name" type="text" required autocomplete="off"
             pattern="[A-Za-z0-9_]+" maxlength="64"
             placeholder="{confirm_esc}" aria-describedby="confirm_hint">
      <p id="confirm_hint" class="muted">Type <code>{confirm_esc}</code> exactly, then Confirm delete. System schemas cannot be dropped.</p>
      <label class="db-drop-users"><input type="checkbox" name="drop_users_flag" value="1" checked>
        Also remove MariaDB users that only had grants on this database
      </label>
      <button type="submit" class="btn-danger" {disabled}>Delete database</button>
    </form>
    <p class="muted">Protected accounts such as <code>root</code> and <code>cpn_pma</code> are never deleted.</p>
    <style>
    .db-delete-impact {{ margin:0 0 14px; }}
    .db-delete-impact ul {{ margin:0; padding-left:1.2em; font-size:14px; line-height:1.45; }}
    .db-delete-impact li {{ margin:0 0 6px; }}
    .db-drop-users {{ display:flex; gap:8px; align-items:flex-start; font-size:14px; }}
    .db-delete-form .btn-danger {{
      background:#dc2626; color:#fff; border:0; padding:10px 14px; border-radius:10px; font-weight:700; cursor:pointer;
    }}
    </style>"#,
        dialog = uninstall_dialog_bundle(),
        options = options,
        impact_list = impact_list,
        impacts_data = impacts_data,
        confirm_esc = confirm_name_hint,
        disabled = if selected_db.is_none() || is_system_database(selected) {
            "disabled"
        } else {
            ""
        },
    );

    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Databases & FTP", Some("/databases")),
            ("Delete Database", None),
        ],
        "Delete Database",
        "Remove a MariaDB database with typed-name confirmation.",
        &form,
        notice,
        error,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delete_impacts_mention_data_and_users() {
        let db = ManagedDatabase {
            name: "shop_db".into(),
            owner: "alice".into(),
            domain: "shop.example".into(),
            in_mariadb: true,
            in_registry: true,
        };
        let impacts = db_delete_impacts(&db, &["shop_db".into(), "root".into()]);
        assert!(impacts.iter().any(|i| i.contains("shop_db")));
        assert!(impacts.iter().any(|i| i.contains("registry")));
        assert!(impacts.iter().any(|i| i.contains("`shop_db`")));
        assert!(
            !impacts
                .iter()
                .any(|i| i.contains("`root`") && i.contains("May remove"))
        );
    }

    #[test]
    fn all_page_uses_activity_list_cards() {
        let html = databases_all_page_for("nobody", None, None);
        assert!(html.contains("activity-list"));
        assert!(html.contains("data-label=\"Database\"") || html.contains("empty-state"));
    }
}
