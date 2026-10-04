//! Hosting package picker for Manage user / Modify User Account tab.

use crate::packages::{
    DEFAULT_PACKAGE_ID, is_panel_admin, list_packages, package_for_account,
};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Current assignment plus an assign form for panel admins.
pub fn package_assign_section(target_username: &str, viewer_can_assign: bool) -> String {
    let assigned = package_for_account(target_username).ok();
    let pkg_id = assigned
        .as_ref()
        .map(|p| p.id.as_str())
        .unwrap_or(DEFAULT_PACKAGE_ID);
    let pkg_name = assigned
        .as_ref()
        .map(|p| p.name.as_str())
        .unwrap_or("Default");
    let summary = format!(
        "<p class=\"muted\" style=\"margin:0;\">Dashboard Statistics uses this assigned package for used/limit rows. Limit -1 shows as ∞. 0 means none allowed. Current: <strong>{name}</strong> (<code>{id}</code>).</p>",
        name = html_escape(pkg_name),
        id = html_escape(pkg_id),
    );
    if !viewer_can_assign {
        return format!(
            r#"
      <div class="stack-form" style="max-width:520px;display:grid;gap:8px;margin:16px 0 8px;">
        <h3 style="margin:0;">Package</h3>
        {summary}
      </div>"#,
            summary = summary,
        );
    }
    let mut opts = String::new();
    for pkg in list_packages().unwrap_or_default() {
        let selected = if pkg.id == pkg_id { " selected" } else { "" };
        opts.push_str(&format!(
            r#"<option value="{id}"{selected}>{name} ({id})</option>"#,
            id = html_escape(&pkg.id),
            name = html_escape(&pkg.name),
            selected = selected,
        ));
    }
    if opts.is_empty() {
        opts.push_str(&format!(
            r#"<option value="{id}" selected>Default ({id})</option>"#,
            id = html_escape(DEFAULT_PACKAGE_ID),
        ));
    }
    format!(
        r#"
      <form method="post" action="/account/users/assign-package" class="stack-form" style="max-width:520px;display:grid;gap:12px;margin:16px 0 8px;">
        <h3 style="margin:0;">Package</h3>
        {summary}
        <input type="hidden" name="username" value="{user}">
        <label>Assigned package
          <select name="package_id" required>{opts}</select>
        </label>
        <button type="submit" class="btn-primary">Save package</button>
      </form>"#,
        summary = summary,
        user = html_escape(target_username),
        opts = opts,
    )
}

pub fn viewer_can_assign_package(viewer: &str) -> bool {
    is_panel_admin(viewer)
}

#[cfg(test)]
mod tests {
    use super::package_assign_section;
    use crate::account::with_test_data_dir;
    use crate::account_mgmt::create_account;
    use crate::packages::ensure_default_package;

    #[test]
    fn admin_section_lists_packages() {
        with_test_data_dir(|| {
            unsafe {
                std::env::set_var("CPN_RESERVED_USERNAMES_OFFLINE", "1");
            }
            create_account(
                "panelowner",
                None,
                true,
                "owner@example.com",
                crate::account::default_password_policy(),
                "en",
            )
            .expect("create");
            let _ = ensure_default_package();
            let html = package_assign_section("panelowner", true);
            assert!(html.contains("Assigned package"), "{html}");
            assert!(html.contains("pkg-default"), "{html}");
            assert!(html.contains("/account/users/assign-package"), "{html}");
            unsafe {
                std::env::remove_var("CPN_RESERVED_USERNAMES_OFFLINE");
            }
        });
    }
}
