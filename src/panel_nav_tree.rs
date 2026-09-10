//! Expandable sidebar groups with stacked child button rows for CPN Panel.

use crate::panel_icons::nav_icon_html;
use crate::panel_nav_catalog::{ACCOUNT, ADMINISTRATION, HOSTING, NavChild, NavEntry};

pub use crate::panel_nav_tree_chrome::{nav_tree_script, nav_tree_styles};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn chevron_svg() -> &'static str {
    r#"<svg class="nav-chevron" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="m9 18 6-6-6-6"/></svg>"#
}

fn flat_link(id: &str, href: &str, label: &str, active: &str) -> String {
    let class = if id == active {
        r#" class="nav-tile active""#
    } else {
        r#" class="nav-tile""#
    };
    // Leaf links have no children: never render an expand chevron.
    format!(
        r#"<a{class} href="{href}">{icon}<span>{label}</span></a>"#,
        icon = nav_icon_html(id),
        label = html_escape(label),
    )
}

fn child_button(label: &str, href: &str) -> String {
    format!(
        r#"<a class="nav-child-btn" href="{href}" data-nav-child="1"><span>{label}</span></a>"#,
        href = html_escape(href),
        label = html_escape(label),
    )
}

fn group_block(
    id: &str,
    href: &str,
    label: &str,
    children: &[NavChild],
    active: &str,
    feats: crate::panel_feature_gate::InstalledOptionalFeatures,
) -> String {
    let open = if id == active { " open" } else { "" };
    let parent_active = if id == active {
        " nav-parent-active"
    } else {
        ""
    };
    let visible: Vec<&NavChild> = children
        .iter()
        .filter(|child| feats.allows_href(child.href))
        .collect();
    let mut child_rows = Vec::with_capacity(visible.len() + 1);
    let has_hub_child = visible.iter().any(|c| c.href == href);
    if !has_hub_child {
        child_rows.push(child_button(&format!("{label} overview"), href));
    }
    let mut seen = std::collections::HashSet::new();
    for child in visible {
        let key = format!("{}|{}", child.href, child.label);
        if !seen.insert(key) {
            continue;
        }
        child_rows.push(child_button(child.label, child.href));
    }
    format!(
        r#"<details class="nav-group" data-nav-group="{id}"{open}>
  <summary class="nav-parent nav-tile{parent_active}">
    {icon}<span>{label}</span>{chevron}
  </summary>
  <div class="nav-children">
    {children}
  </div>
</details>"#,
        id = html_escape(id),
        open = open,
        parent_active = parent_active,
        icon = nav_icon_html(id),
        label = html_escape(label),
        chevron = chevron_svg(),
        children = child_rows.join("\n    "),
    )
}

fn render_section(
    title: &str,
    entries: &[NavEntry],
    active: &str,
    feats: crate::panel_feature_gate::InstalledOptionalFeatures,
) -> Vec<String> {
    let mut parts = Vec::new();
    parts.push(format!(
        r#"<div class="nav-section">{}</div>"#,
        html_escape(title)
    ));
    parts.push(r#"<div class="nav-tile-grid">"#.to_string());
    for entry in entries {
        match *entry {
            NavEntry::Link { id, href, label } => {
                parts.push(flat_link(id, href, label, active));
            }
            NavEntry::Group {
                id,
                href,
                label,
                children,
            } => {
                parts.push(group_block(id, href, label, children, active, feats));
            }
        }
    }
    parts.push(r#"</div>"#.to_string());
    parts
}

/// Primary sidebar navigation HTML (sections, expandable groups, child buttons).
pub fn nav_links_html(active: &str, username: &str) -> String {
    let feats = crate::panel_feature_gate::InstalledOptionalFeatures::detect();
    let mut parts = Vec::new();
    parts.extend(render_section("Hosting", HOSTING, active, feats));
    parts.extend(render_section("Account", ACCOUNT, active, feats));
    parts.extend(render_section("Administration", ADMINISTRATION, active, feats));

    // Installed plugin shortcuts (Hosting already has Plugins + Plugin Store).
    let plugin_links = crate::plugins_settings::sidebar_plugin_links(username);
    if !plugin_links.is_empty() {
        parts.push(r#"<div class="nav-section">Installed plugins</div>"#.to_string());
        parts.push(r#"<div class="nav-tile-grid">"#.to_string());
        let mut child_html = Vec::new();
        let mut domains: Vec<&str> = plugin_links.iter().map(|l| l.domain.as_str()).collect();
        domains.sort_unstable();
        domains.dedup();
        let need_domain_hint = domains.len() > 1;
        for link in &plugin_links {
            let label = if need_domain_hint {
                format!("{} ({})", link.name, link.domain)
            } else {
                link.name.clone()
            };
            child_html.push(format!(
                r#"<a class="nav-child-btn" href="{href}" data-nav-child="1"><span>{label}</span></a>"#,
                href = html_escape(&link.href),
                label = html_escape(&label),
            ));
        }
        parts.push(child_html.join("\n          "));
        parts.push(r#"</div>"#.to_string());
    }

    parts.join("\n          ")
}

#[cfg(test)]
mod tests {
    use super::{nav_links_html, nav_tree_styles};

    #[test]
    fn nested_users_group_renders_child_buttons() {
        let html = nav_links_html("users", "admin");
        assert!(html.contains("nav-group"));
        assert!(html.contains("nav-tile-grid"));
        assert!(html.contains("nav-tile"));
        assert!(html.contains("View Profile"));
        assert!(html.contains("Create New User"));
        assert!(html.contains("List Users"));
        assert!(html.contains("/account/users/profile"));
        assert!(html.contains("nav-child-btn"));
        assert!(html.contains(" open"));
        assert!(html.contains("Plugin Store") || html.contains("Plugins"));
        assert!(
            html.contains("/plugins?view=store") || html.contains("data-nav-group=\"plugins\"")
        );
    }

    #[test]
    fn leaf_links_have_no_expand_chevron() {
        let html = nav_links_html("dashboard", "admin");
        assert!(html.contains("href=\"/dashboard\""));
        assert!(html.contains(">Dashboard</span></a>") || html.contains(">Dashboard</span>"));
        let dash_idx = html.find("href=\"/dashboard\"").expect("dashboard link");
        let dash_snip = &html[dash_idx..dash_idx + 180.min(html.len() - dash_idx)];
        assert!(
            !dash_snip.contains("nav-chevron"),
            "Dashboard leaf must not render chevron: {dash_snip}"
        );
        if let Some(apps_idx) = html.find("href=\"/apps\"") {
            let apps_snip = &html[apps_idx..apps_idx + 160.min(html.len() - apps_idx)];
            assert!(
                !apps_snip.contains("nav-chevron"),
                "Apps leaf must not render chevron: {apps_snip}"
            );
        }
        assert!(html.contains("nav-chevron"));
        assert!(html.contains("data-nav-group=\"websites\""));
    }

    #[test]
    fn styles_include_stacked_child_buttons() {
        let css = nav_tree_styles();
        assert!(css.contains(".nav-child-btn"));
        assert!(css.contains(".nav-children"));
        assert!(css.contains(".nav-chevron"));
        assert!(css.contains("flex-direction:column"));
        assert!(css.contains(".nav-tile-grid"));
        assert!(!css.contains("grid-template-columns:repeat(2"));
    }

    #[test]
    fn package_backed_links_follow_install_detection() {
        let html = nav_links_html("databases", "admin");
        if crate::panel_feature_gate::phpmyadmin_installed() {
            assert!(
                html.contains("/databases/phpmyadmin"),
                "phpMyAdmin must appear when installed"
            );
        } else {
            assert!(
                !html.contains("/databases/phpmyadmin"),
                "phpMyAdmin must be hidden when not installed"
            );
            assert!(!html.contains(">phpMyAdmin</span>"));
        }
        if crate::panel_feature_gate::webmail_installed() {
            assert!(html.contains("/email/webmail"));
        } else {
            assert!(!html.contains("/email/webmail"));
        }
        assert!(html.contains("/databases/manager"));
        assert!(html.contains("/databases/all"));
    }
}
