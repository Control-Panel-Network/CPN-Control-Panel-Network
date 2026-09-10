//! Expandable sidebar groups with stacked child button rows for CPN Panel.

use crate::panel_icons::nav_icon_html;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[derive(Clone, Copy)]
struct NavChild {
    label: &'static str,
    href: &'static str,
}

#[derive(Clone, Copy)]
enum NavEntry {
    Link {
        id: &'static str,
        href: &'static str,
        label: &'static str,
    },
    Group {
        id: &'static str,
        href: &'static str,
        label: &'static str,
        children: &'static [NavChild],
    },
}

const WEBSITES_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "List Websites",
        href: "/websites",
    },
    NavChild {
        label: "Create Website",
        href: "/websites",
    },
];

const EMAIL_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "Email Accounts",
        href: "/email/accounts",
    },
    NavChild {
        label: "Create Email",
        href: "/email/create",
    },
    NavChild {
        label: "Forwarding",
        href: "/email/forwarding",
    },
    NavChild {
        label: "DKIM Manager",
        href: "/email/dkim",
    },
    NavChild {
        label: "Webmail",
        href: "/email/webmail",
    },
    NavChild {
        label: "Email Delivery",
        href: "/email/delivery",
    },
];

const DATABASES_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "All Databases",
        href: "/databases/all",
    },
    NavChild {
        label: "Create Database",
        href: "/databases/create",
    },
    NavChild {
        label: "phpMyAdmin",
        href: "/databases/phpmyadmin",
    },
    NavChild {
        label: "MariaDB Manager",
        href: "/databases/manager",
    },
    NavChild {
        label: "FTP Accounts",
        href: "/ftp/accounts",
    },
    NavChild {
        label: "Create SFTP Account",
        href: "/ftp/create",
    },
    NavChild {
        label: "Delete SFTP Account",
        href: "/ftp/delete",
    },
    NavChild {
        label: "Reset SFTP",
        href: "/ftp/reset",
    },
];

const BACKUPS_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "Create Backup",
        href: "/backups/create",
    },
    NavChild {
        label: "Restore Backup",
        href: "/backups/restore",
    },
    NavChild {
        label: "Schedule Backup",
        href: "/backups/schedule",
    },
    NavChild {
        label: "Destinations",
        href: "/backups/destinations",
    },
];

const USERS_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "View Profile",
        href: "/account/users/profile",
    },
    NavChild {
        label: "Create New User",
        href: "/account/users/create",
    },
    NavChild {
        label: "List Users",
        href: "/account/users/list",
    },
    NavChild {
        label: "Modify User",
        href: "/account/users/modify",
    },
    NavChild {
        label: "Create ACL",
        href: "/account/acl/create",
    },
    NavChild {
        label: "Modify ACL",
        href: "/account/acl/modify",
    },
];

const SERVER_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "Services Status",
        href: "/server/services",
    },
    NavChild {
        label: "PHP Extensions",
        href: "/server/php/extensions",
    },
    NavChild {
        label: "Top Processes",
        href: "/server/processes",
    },
    NavChild {
        label: "Root File Manager",
        href: "/server/files",
    },
    NavChild {
        label: "DNS Zones",
        href: "/server/dns/zones",
    },
    NavChild {
        label: "Cloudflare DNS",
        href: "/dns/cloudflare",
    },
];

const SECURITY_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "Firewall",
        href: "/security/firewall",
    },
    NavChild {
        label: "Secure SSH",
        href: "/security/ssh",
    },
    NavChild {
        label: "Fail2ban",
        href: "/security/fail2ban",
    },
    NavChild {
        label: "Manage SSL",
        href: "/security/ssl",
    },
    NavChild {
        label: "Hostname SSL",
        href: "/security/ssl/hostname",
    },
    NavChild {
        label: "Malware scan",
        href: "/security/malware-scan",
    },
];

const SETTINGS_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "Version Management",
        href: "/settings/version",
    },
    NavChild {
        label: "Design",
        href: "/settings/design",
    },
    NavChild {
        label: "Setup Wizard",
        href: "/settings/setup",
    },
    NavChild {
        label: "Connect",
        href: "/settings/connect",
    },
    NavChild {
        label: "Change Port",
        href: "/settings/port",
    },
];

const PLUGINS_CHILDREN: &[NavChild] = &[
    NavChild {
        label: "Installed",
        href: "/plugins",
    },
    NavChild {
        label: "Plugin Store",
        href: "/plugins?view=store",
    },
];

const HOSTING: &[NavEntry] = &[
    NavEntry::Link {
        id: "dashboard",
        href: "/dashboard",
        label: "Dashboard",
    },
    NavEntry::Group {
        id: "websites",
        href: "/websites",
        label: "Websites",
        children: WEBSITES_CHILDREN,
    },
    NavEntry::Group {
        id: "email",
        href: "/email",
        label: "Email",
        children: EMAIL_CHILDREN,
    },
    NavEntry::Group {
        id: "databases",
        href: "/databases",
        label: "Databases & FTP",
        children: DATABASES_CHILDREN,
    },
    NavEntry::Group {
        id: "backups",
        href: "/backups",
        label: "Backups",
        children: BACKUPS_CHILDREN,
    },
    NavEntry::Link {
        id: "apps",
        href: "/apps",
        label: "Apps",
    },
    NavEntry::Group {
        id: "plugins",
        href: "/plugins",
        label: "Plugins",
        children: PLUGINS_CHILDREN,
    },
];

const ACCOUNT: &[NavEntry] = &[
    NavEntry::Group {
        id: "users",
        href: "/account/users",
        label: "Users & Plans",
        children: USERS_CHILDREN,
    },
    NavEntry::Link {
        id: "packages",
        href: "/packages",
        label: "Packages",
    },
];

const ADMINISTRATION: &[NavEntry] = &[
    NavEntry::Group {
        id: "server",
        href: "/server",
        label: "Server",
        children: SERVER_CHILDREN,
    },
    NavEntry::Group {
        id: "security",
        href: "/security",
        label: "Security",
        children: SECURITY_CHILDREN,
    },
    NavEntry::Group {
        id: "settings",
        href: "/settings",
        label: "Settings",
        children: SETTINGS_CHILDREN,
    },
];

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

fn group_block(id: &str, href: &str, label: &str, children: &[NavChild], active: &str) -> String {
    let open = if id == active { " open" } else { "" };
    let parent_active = if id == active {
        " nav-parent-active"
    } else {
        ""
    };
    let mut child_rows = Vec::with_capacity(children.len() + 1);
    let has_hub_child = children.iter().any(|c| c.href == href);
    if !has_hub_child {
        child_rows.push(child_button(&format!("{label} overview"), href));
    }
    let mut seen = std::collections::HashSet::new();
    for child in children {
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

fn render_section(title: &str, entries: &[NavEntry], active: &str) -> Vec<String> {
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
                parts.push(group_block(id, href, label, children, active));
            }
        }
    }
    parts.push(r#"</div>"#.to_string());
    parts
}

/// Primary sidebar navigation HTML (sections, expandable groups, child buttons).
pub fn nav_links_html(active: &str, username: &str) -> String {
    let mut parts = Vec::new();
    parts.extend(render_section("Hosting", HOSTING, active));
    parts.extend(render_section("Account", ACCOUNT, active));
    parts.extend(render_section("Administration", ADMINISTRATION, active));

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

/// CSS for single-column nav tiles, expandable parents, and stacked child buttons.
pub fn nav_tree_styles() -> &'static str {
    r#"
.nav-tile-grid {
  display:flex;
  flex-direction:column;
  gap:8px;
  margin:0 0 10px;
}
.nav-section {
  margin:14px 4px 8px;
  color:var(--muted); font-size:11px; font-weight:700;
  letter-spacing:.08em; text-transform:uppercase;
}
.sidebar nav > .nav-section:first-child { margin-top:4px; }
.nav-group { margin:0; border:0; min-width:0; width:100%; }
.nav-group > summary {
  list-style:none; cursor:pointer;
}
.nav-group > summary::-webkit-details-marker { display:none; }
.sidebar nav a.nav-tile,
.nav-parent.nav-tile {
  display:flex; align-items:center; gap:10px; min-height:44px; width:100%; min-width:0;
  padding:8px 12px; border-radius:10px; color:var(--ink);
  font-size:14px; font-weight:600; line-height:1.25; user-select:none;
  background:#fff; border:1px solid var(--hairline);
  box-shadow:0 1px 2px rgba(29,29,31,.05);
}
.sidebar nav a.nav-tile > span:not(.nav-icon),
.nav-parent.nav-tile > span:not(.nav-icon) {
  flex:1 1 auto; min-width:0;
  overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
}
.sidebar nav a.nav-tile:hover,
.nav-parent.nav-tile:hover {
  border-color:#c9d8ef; background:#f8fbff; color:var(--blue);
}
.sidebar nav a.nav-tile.active,
.nav-parent-active {
  border-color:#9ec2f0; background:#e7f1ff; color:var(--blue);
}
.nav-parent .nav-chevron,
.sidebar nav a.nav-tile .nav-chevron {
  margin-left:auto; flex:0 0 auto; color:var(--muted);
  transition:transform .18s ease;
}
.nav-group[open] > .nav-parent .nav-chevron { transform:rotate(90deg); color:var(--blue); }
.nav-children {
  display:flex;
  flex-direction:column;
  gap:6px;
  margin:8px 0 2px;
  padding:0 0 0 8px;
}
.nav-child-btn {
  display:flex; align-items:center; min-height:40px; width:100%; min-width:0; padding:8px 12px;
  border-radius:10px; background:#fff; border:1px solid var(--hairline);
  color:var(--ink); font-size:13px; font-weight:500;
  box-shadow:0 1px 2px rgba(29,29,31,.04);
}
.nav-child-btn span {
  overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
}
.nav-child-btn:hover { border-color:#c9d8ef; background:#f8fbff; color:var(--blue); }
.nav-child-btn.active {
  border-color:#9ec2f0; background:#e7f1ff; color:var(--blue); font-weight:600;
}
.sidebar nav a.nav-child { padding-left:22px; font-size:14px; min-height:40px; }
[data-color-mode="dark"] .sidebar nav a.nav-tile,
[data-color-mode="dark"] .nav-parent.nav-tile,
[data-color-mode="dark"] .nav-child-btn {
  background:#1c212b; border-color:#2a3140; color:#e5e7eb;
  box-shadow:none;
}
[data-color-mode="dark"] .sidebar nav a.nav-tile:hover,
[data-color-mode="dark"] .nav-parent.nav-tile:hover,
[data-color-mode="dark"] .nav-child-btn:hover {
  background:#232a36; border-color:#3b82f6; color:#93c5fd;
}
[data-color-mode="dark"] .sidebar nav a.nav-tile.active,
[data-color-mode="dark"] .nav-parent-active,
[data-color-mode="dark"] .nav-child-btn.active {
  background:rgba(59,130,246,.2); border-color:#3b82f6; color:#93c5fd;
}
[data-color-mode="dark"] .nav-parent .nav-chevron,
[data-color-mode="dark"] .sidebar nav a.nav-tile .nav-chevron { color:#9ca3af; }
"#
}

/// Highlight the child button that best matches the current path.
pub fn nav_tree_script() -> &'static str {
    r#"
<script>
(function () {
  var path = window.location.pathname || '/';
  var best = null;
  var bestLen = -1;
  document.querySelectorAll('a.nav-child-btn[href]').forEach(function (a) {
    var href = a.getAttribute('href') || '';
    if (!href || href.charAt(0) !== '/') return;
    var exact = path === href;
    var prefix = href !== '/' && (path === href || path.indexOf(href + '/') === 0);
    if (!exact && !prefix) return;
    if (href.length > bestLen) {
      best = a;
      bestLen = href.length;
    }
  });
  if (best) {
    best.classList.add('active');
    var group = best.closest('details.nav-group');
    if (group) group.open = true;
  }
})();
</script>
"#
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
        // Dashboard and Apps are leaf NavEntry::Link items.
        assert!(html.contains("href=\"/dashboard\""));
        assert!(html.contains(">Dashboard</span></a>") || html.contains(">Dashboard</span>"));
        let dash_idx = html
            .find("href=\"/dashboard\"")
            .expect("dashboard link");
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
        // Groups still get a chevron on the summary parent.
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
}
