//! Users & Plans hub HTML: accounts, ACL grants, and honest scaffolds.

use crate::account_mgmt::list_accounts;
use crate::packages::is_panel_admin;
use crate::panel_hub_defs::users_plans_hub_sections;
use crate::panel_hub_pages_hosting::scaffold_feature;
use crate::panel_hubs::{feature_shell, hub_tiles_grid, section_heading};
use crate::site_acl::{SiteAclGrant, list_grants};

pub use crate::panel_hub_pages_profile::users_profile_page;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn users_plans_hub_main() -> String {
    let mut body = section_heading(
        "Users & Plans",
        "Accounts, hosting plans, site ACL, and admin tools.",
    );
    for (title, tiles) in users_plans_hub_sections() {
        body.push_str(&hub_tiles_grid(title, &tiles));
    }
    body
}

pub fn users_list_page(viewer: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let admin = is_panel_admin(viewer);
    let accounts = list_accounts().unwrap_or_default();
    let mut body = String::new();
    if !admin {
        body.push_str(
            r#"<p class="muted">Showing your account only. Panel admin can list every user.</p>"#,
        );
    }
    if accounts.is_empty() {
        body.push_str(r#"<p class="empty-state">No panel accounts found.</p>"#);
    } else {
        body.push_str(
            r#"<div class="table-wrap"><table class="data-table">
        <thead><tr><th>Username</th><th>Recovery email</th><th>Role</th></tr></thead><tbody>"#,
        );
        for acct in &accounts {
            if !admin && !acct.username.eq_ignore_ascii_case(viewer) {
                continue;
            }
            let role = if is_panel_admin(&acct.username) {
                "Admin"
            } else {
                "User"
            };
            body.push_str(&format!(
                r#"<tr><td><strong>{}</strong></td><td>{}</td><td>{}</td></tr>"#,
                html_escape(&acct.username),
                html_escape(&acct.recovery_email),
                role,
            ));
        }
        body.push_str("</tbody></table></div>");
    }
    if admin {
        body.push_str(
            r#"<p style="margin-top:16px;"><a class="btn-primary" href="/account/users/create">Create user</a>
            <a class="btn-secondary" href="/account/users/modify" style="margin-left:8px;">Modify user</a></p>"#,
        );
    }
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Users & Plans", Some("/account/users")),
            ("List Users", None),
        ],
        "List Users",
        "Panel accounts on this host.",
        &body,
        notice,
        error,
    )
}

pub fn users_create_page(notice: Option<&str>, error: Option<&str>) -> String {
    let body = r#"
      <form method="post" action="/account/users/create" class="stack-form" style="max-width:520px;display:grid;gap:12px;">
        <label>Username
          <input name="username" type="text" required autocomplete="username" maxlength="128">
        </label>
        <label>Recovery email
          <input name="recovery_email" type="email" required autocomplete="email" maxlength="254">
        </label>
        <label>Password (leave blank to generate)
          <input name="password" type="password" autocomplete="new-password" maxlength="256">
        </label>
        <label style="display:flex;align-items:center;gap:8px;">
          <input name="generate" type="checkbox" value="1">
          Generate a strong password
        </label>
        <button type="submit" class="btn-primary">Create user</button>
      </form>
      <p class="muted" style="margin-top:12px;">Generated passwords are shown once on the success page and never stored in the URL.</p>"#;
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Users & Plans", Some("/account/users")),
            ("Create User", None),
        ],
        "Create User",
        "Add a panel account.",
        body,
        notice,
        error,
    )
}

pub fn users_create_success_page(username: &str, generated_password: Option<&str>) -> String {
    let mut body = format!(
        r#"<p class="panel-notice ok" role="status">Account <strong>{}</strong> created.</p>"#,
        html_escape(username)
    );
    if let Some(password) = generated_password {
        body.push_str(&format!(
            r#"<p><strong>Generated password</strong> (copy now; it will not be shown again):</p>
            <p><code style="user-select:all;">{}</code></p>"#,
            html_escape(password)
        ));
    }
    body.push_str(
        r#"<p style="margin-top:16px;"><a class="btn-primary" href="/account/users/list">Back to list</a></p>"#,
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Users & Plans", Some("/account/users")),
            ("Create User", None),
        ],
        "Create User",
        "Account created.",
        &body,
        None,
        None,
    )
}

pub fn users_modify_page(notice: Option<&str>, error: Option<&str>) -> String {
    let accounts = list_accounts().unwrap_or_default();
    let mut options = String::new();
    for acct in &accounts {
        if is_panel_admin(&acct.username) {
            continue;
        }
        options.push_str(&format!(
            r#"<option value="{u}">{u}</option>"#,
            u = html_escape(&acct.username),
        ));
    }
    let select_inner = if options.is_empty() {
        r#"<option value="">No non-admin accounts</option>"#.to_string()
    } else {
        options
    };
    let body = format!(
        r#"
      <form method="post" action="/account/users/password" class="stack-form" style="max-width:520px;display:grid;gap:12px;margin-bottom:28px;">
        <h3 style="margin:0;">Reset password</h3>
        <label>Username
          <select name="username" required>{select_inner}</select>
        </label>
        <label>New password (leave blank to generate)
          <input name="password" type="password" autocomplete="new-password" maxlength="256">
        </label>
        <label style="display:flex;align-items:center;gap:8px;">
          <input name="generate" type="checkbox" value="1">
          Generate a strong password
        </label>
        <button type="submit" class="btn-primary">Reset password</button>
      </form>
      <form method="post" action="/account/users/delete" class="stack-form" style="max-width:520px;display:grid;gap:12px;"
            onsubmit="return confirm('Delete this panel account? This cannot be undone.');">
        <h3 style="margin:0;">Delete user</h3>
        <label>Username
          <select name="username" required>{select_inner}</select>
        </label>
        <button type="submit" class="btn-secondary">Delete user</button>
      </form>
      <p class="muted">The bootstrap admin account cannot be deleted from this screen.</p>"#
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Users & Plans", Some("/account/users")),
            ("Modify User", None),
        ],
        "Modify User",
        "Reset password or remove a panel account.",
        &body,
        notice,
        error,
    )
}

pub fn users_password_success_page(username: &str, generated_password: Option<&str>) -> String {
    let mut body = format!(
        r#"<p class="panel-notice ok" role="status">Password updated for <strong>{}</strong>.</p>"#,
        html_escape(username)
    );
    if let Some(password) = generated_password {
        body.push_str(&format!(
            r#"<p><strong>Generated password</strong> (copy now; it will not be shown again):</p>
            <p><code style="user-select:all;">{}</code></p>"#,
            html_escape(password)
        ));
    }
    body.push_str(
        r#"<p style="margin-top:16px;"><a class="btn-primary" href="/account/users/modify">Back to modify</a></p>"#,
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Users & Plans", Some("/account/users")),
            ("Modify User", None),
        ],
        "Modify User",
        "Password updated.",
        &body,
        None,
        None,
    )
}

pub fn users_reseller_page() -> String {
    scaffold_feature(
        "Users & Plans",
        "/account/users",
        "Reseller Center",
        "Reseller settings",
        "CPN does not ship a reseller hierarchy yet. This tile is reserved for future multi-tenant reseller quotas and branding.",
    )
}

pub fn api_access_page() -> String {
    scaffold_feature(
        "Users & Plans",
        "/account/users",
        "API Access",
        "API tokens",
        "Panel API tokens are not issued yet. Use the signed-in session for panel routes until token auth ships.",
    )
}

fn grant_rows() -> String {
    let grants = list_grants();
    if grants.is_empty() {
        return r#"<p class="empty-state">No site ACL grants yet.</p>"#.into();
    }
    let mut out = String::from(
        r#"<div class="table-wrap"><table class="data-table">
      <thead><tr>
        <th>Member</th><th>Scope</th><th>Install</th><th>Uninstall</th><th>Enable</th><th></th>
      </tr></thead><tbody>"#,
    );
    for (idx, grant) in grants.iter().enumerate() {
        let scope = if !grant.domain.trim().is_empty() {
            format!("Domain: {}", grant.domain)
        } else {
            format!("All owned by: {}", grant.all_owned_by)
        };
        out.push_str(&format!(
            r#"<tr>
          <td><strong>{member}</strong></td>
          <td>{scope}</td>
          <td>{install}</td><td>{uninstall}</td><td>{enable}</td>
          <td>
            <form method="post" action="/account/acl/delete" class="inline-form" style="display:inline;"
                  onsubmit="return confirm('Remove this ACL grant?');">
              <input type="hidden" name="index" value="{idx}">
              <button type="submit" class="linkish" style="background:none;border:0;color:#d92d20;font-weight:600;cursor:pointer;padding:0;">Remove</button>
            </form>
          </td>
        </tr>"#,
            member = html_escape(&grant.member),
            scope = html_escape(&scope),
            install = if grant.can_install { "Yes" } else { "No" },
            uninstall = if grant.can_uninstall { "Yes" } else { "No" },
            enable = if grant.can_enable { "Yes" } else { "No" },
            idx = idx,
        ));
    }
    out.push_str("</tbody></table></div>");
    out
}

fn acl_form(action: &str) -> String {
    format!(
        r#"
      <form method="post" action="{action}" class="stack-form" style="max-width:560px;display:grid;gap:12px;">
        <label>Member username
          <input name="member" type="text" required maxlength="128" placeholder="ops">
        </label>
        <label>Domain FQDN (leave blank when using all-owned-by)
          <input name="domain" type="text" maxlength="253" placeholder="example.com">
        </label>
        <label>All sites owned by (leave blank when using domain)
          <input name="all_owned_by" type="text" maxlength="128" placeholder="admin">
        </label>
        <label style="display:flex;align-items:center;gap:8px;">
          <input name="can_install" type="checkbox" value="1" checked> Can install apps/plugins
        </label>
        <label style="display:flex;align-items:center;gap:8px;">
          <input name="can_uninstall" type="checkbox" value="1" checked> Can uninstall
        </label>
        <label style="display:flex;align-items:center;gap:8px;">
          <input name="can_enable" type="checkbox" value="1" checked> Can enable/disable
        </label>
        <button type="submit" class="btn-primary">Save grant</button>
      </form>
      <p class="muted" style="margin-top:12px;">Grants map to the existing site ACL store used by Apps and Plugins.</p>"#,
        action = html_escape(action),
    )
}

pub fn acl_create_page(notice: Option<&str>, error: Option<&str>) -> String {
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Users & Plans", Some("/account/users")),
            ("Create ACL", None),
        ],
        "Create ACL",
        "Add a site permission grant for a panel account.",
        &acl_form("/account/acl/create"),
        notice,
        error,
    )
}

pub fn acl_modify_page(notice: Option<&str>, error: Option<&str>) -> String {
    let body = format!(
        r#"{}
      <h3 style="margin:24px 0 12px;">Add another grant</h3>
      {}"#,
        grant_rows(),
        acl_form("/account/acl/create")
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Users & Plans", Some("/account/users")),
            ("Modify ACL", None),
        ],
        "Modify ACL",
        "Review and remove site ACL grants.",
        &body,
        notice,
        error,
    )
}

/// Build a grant from form fields (shared by create route).
pub fn grant_from_form_fields(
    member: &str,
    domain: &str,
    all_owned_by: &str,
    can_install: bool,
    can_uninstall: bool,
    can_enable: bool,
) -> SiteAclGrant {
    SiteAclGrant {
        member: member.to_string(),
        domain: domain.to_string(),
        all_owned_by: all_owned_by.to_string(),
        can_install,
        can_uninstall,
        can_enable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hub_lists_packages_and_users_tiles() {
        let html = users_plans_hub_main();
        assert!(html.contains("Users &amp; Plans") || html.contains("Users & Plans"));
        assert!(html.contains("/packages"));
        assert!(html.contains("/account/users/list"));
        assert!(html.contains("hub-tile"));
        assert!(!html.contains("Not configured yet"));
        assert!(!html.contains("CyberPanel"));
    }
}
