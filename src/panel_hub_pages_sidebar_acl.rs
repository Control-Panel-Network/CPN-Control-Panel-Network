//! ACL UI: per-user sidebar section visibility grants.

use crate::panel_hubs::feature_shell;
use crate::panel_website_manage_ui::html_escape;
use crate::sidebar_visibility::{controllable_nav_items, list_grants};

fn sidebar_grant_rows() -> String {
    let grants = list_grants();
    if grants.is_empty() {
        return r#"<p class="empty-state">No sidebar visibility grants yet. Everyone sees all sections allowed by their package.</p>"#.into();
    }
    let mut out = String::from(
        r#"<div class="table-wrap"><table class="data-table">
      <thead><tr>
        <th>Member</th><th>Hidden sections</th><th>Restrict admin</th><th></th>
      </tr></thead><tbody>"#,
    );
    for (idx, grant) in grants.iter().enumerate() {
        let hidden = if grant.hidden_nav_ids.is_empty() {
            "(none)".to_string()
        } else {
            grant.hidden_nav_ids.join(", ")
        };
        out.push_str(&format!(
            r#"<tr>
          <td><strong>{member}</strong></td>
          <td>{hidden}</td>
          <td>{restrict}</td>
          <td>
            <form method="post" action="/account/acl/sidebar/delete" class="inline-form" style="display:inline;"
                  onsubmit="return confirm('Remove this sidebar visibility grant?');">
              <input type="hidden" name="index" value="{idx}">
              <button type="submit" class="linkish" style="background:none;border:0;color:#d92d20;font-weight:600;cursor:pointer;padding:0;">Remove</button>
            </form>
          </td>
        </tr>"#,
            member = html_escape(&grant.member),
            hidden = html_escape(&hidden),
            restrict = if grant.restrict_admin { "Yes" } else { "No" },
            idx = idx,
        ));
    }
    out.push_str("</tbody></table></div>");
    out
}

fn sidebar_checkboxes(selected: &[String]) -> String {
    let mut out = String::from(
        r#"<fieldset style="border:1px solid var(--hairline,#d0d5dd);border-radius:8px;padding:12px;">
      <legend style="padding:0 6px;">Hide sidebar sections</legend>
      <p class="muted" style="margin:0 0 8px;">Checked sections are hidden for this member (and blocked with 403 if opened by URL). Dashboard always stays available.</p>
      <div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));gap:8px;">"#,
    );
    for item in controllable_nav_items() {
        let checked = if selected.iter().any(|id| id == item.id) {
            " checked"
        } else {
            ""
        };
        out.push_str(&format!(
            r#"<label style="display:flex;align-items:center;gap:8px;">
          <input type="checkbox" name="hidden_nav_ids" value="{id}"{checked}>
          {label}
        </label>"#,
            id = html_escape(item.id),
            checked = checked,
            label = html_escape(item.label),
        ));
    }
    out.push_str("</div></fieldset>");
    out
}

pub fn sidebar_acl_form() -> String {
    format!(
        r#"
      <form method="post" action="/account/acl/sidebar" class="stack-form" style="max-width:720px;display:grid;gap:12px;">
        <label>Member username
          <input name="member" type="text" required maxlength="128" placeholder="ops">
        </label>
        {checks}
        <label style="display:flex;align-items:center;gap:8px;">
          <input name="restrict_admin" type="checkbox" value="1">
          Also apply when this member is the panel owner/admin
        </label>
        <button type="submit" class="btn-primary">Save sidebar visibility</button>
      </form>
      <p class="muted" style="margin-top:12px;">Package plans can hide the same sections under Packages edit. ACL and package restrictions both apply (hidden if either hides the section). Owner/admin keeps full access unless the restrict-admin box is checked.</p>"#,
        checks = sidebar_checkboxes(&[]),
    )
}

/// Block appended on Modify ACL for sidebar visibility management.
pub fn sidebar_visibility_section() -> String {
    format!(
        r#"<section style="margin-top:28px;padding-top:18px;border-top:1px solid var(--hairline,#d0d5dd);">
  <h3 style="margin:0 0 8px;">Sidebar visibility (ACL)</h3>
  <p class="muted">Choose which left-sidebar sections a user can see and open. Hidden items disappear from the nav; direct URLs return 403 Forbidden.</p>
  {rows}
  <h4 style="margin:20px 0 10px;">Add or update grant</h4>
  {form}
</section>"#,
        rows = sidebar_grant_rows(),
        form = sidebar_acl_form(),
    )
}

pub fn sidebar_acl_standalone_page(notice: Option<&str>, error: Option<&str>) -> String {
    let body = format!(r#"{}{}"#, sidebar_grant_rows(), sidebar_acl_form());
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Users & Plans", Some("/account/users")),
            ("Modify ACL", Some("/account/acl/modify")),
            ("Sidebar visibility", None),
        ],
        "Sidebar visibility",
        "ACL grants for left-sidebar sections.",
        &body,
        notice,
        error,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_lists_controllable_items() {
        let html = sidebar_visibility_section();
        assert!(html.contains("Sidebar visibility"));
        assert!(html.contains("hidden_nav_ids"));
        assert!(html.contains("backups") || html.contains("Backups"));
        assert!(!html.to_lowercase().contains("cyberpanel"));
        assert!(!html.contains('\u{2014}'));
    }
}
