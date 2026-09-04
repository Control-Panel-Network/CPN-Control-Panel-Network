//! Self-hosted Lucide-style SVG icons for hub tiles and sidebar nav.
//! No CDN dependency; works offline in lab VMs.

use crate::panel_icons_svg;

/// Accent tone class suffix (`tone-{name}`) for light/dark friendly icon chips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconTone {
    Blue,
    Green,
    Amber,
    Violet,
    Rose,
    Cyan,
    Slate,
}

impl IconTone {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Blue => "blue",
            Self::Green => "green",
            Self::Amber => "amber",
            Self::Violet => "violet",
            Self::Rose => "rose",
            Self::Cyan => "cyan",
            Self::Slate => "slate",
        }
    }
}

/// Resolve icon key + tone from a hub/manage tile href. Longest path wins; unknown -> fallback.
pub fn resolve_href(href: &str) -> (&'static str, IconTone) {
    let path = href.split('?').next().unwrap_or(href);
    let rules: &[(&str, &str, IconTone)] = &[
        ("/security/ssl/hostname", "certificate", IconTone::Green),
        ("/security/ssl/mail", "mail", IconTone::Green),
        ("/security/ssl", "lock", IconTone::Green),
        ("/security/malware-scan", "scan", IconTone::Rose),
        ("/security/rule-packs", "boxes", IconTone::Violet),
        ("/security/modsec-rules", "file", IconTone::Violet),
        ("/security/modsecurity", "shield-alert", IconTone::Violet),
        ("/security/fail2ban", "shield", IconTone::Rose),
        ("/security/ssh", "terminal", IconTone::Amber),
        ("/security/firewall", "shield", IconTone::Rose),
        ("/security", "shield", IconTone::Rose),
        ("/settings/version", "rocket", IconTone::Blue),
        ("/settings/design", "palette", IconTone::Violet),
        ("/settings/setup", "wand", IconTone::Amber),
        ("/settings/connect", "link", IconTone::Cyan),
        ("/settings/port", "plug", IconTone::Amber),
        ("/settings", "settings", IconTone::Slate),
        ("/account/users/profile", "user-cog", IconTone::Blue),
        ("/account/users/list", "users", IconTone::Blue),
        ("/account/users/create", "user-plus", IconTone::Green),
        ("/account/users/modify", "user-cog", IconTone::Amber),
        ("/account/users/reseller", "users", IconTone::Violet),
        ("/account/acl/create", "key", IconTone::Green),
        ("/account/acl/modify", "key-round", IconTone::Amber),
        ("/account/api-access", "key", IconTone::Cyan),
        ("/account/users", "users", IconTone::Blue),
        ("/email/accounts", "mail", IconTone::Blue),
        ("/email/create", "user-plus", IconTone::Green),
        ("/email/forwarding", "forward", IconTone::Cyan),
        ("/email/catchall", "inbox", IconTone::Amber),
        ("/email/pattern-forwarding", "filter", IconTone::Violet),
        ("/email/limits", "gauge", IconTone::Amber),
        ("/email/password", "key-round", IconTone::Rose),
        ("/email/dkim", "key", IconTone::Green),
        ("/email/webmail", "external-link", IconTone::Blue),
        ("/email/delivery", "send", IconTone::Cyan),
        ("/email/debugger", "bug", IconTone::Rose),
        ("/email/queue", "list-ordered", IconTone::Amber),
        ("/email/spamassassin", "shield-alert", IconTone::Rose),
        ("/email/rspamd", "shield", IconTone::Violet),
        ("/email/mailscanner", "scan", IconTone::Cyan),
        ("/email/marketing", "megaphone", IconTone::Amber),
        ("/email/plus-addressing", "tags", IconTone::Green),
        ("/email", "mail", IconTone::Blue),
        ("/databases/all", "database", IconTone::Blue),
        ("/databases/create", "plus", IconTone::Green),
        ("/databases/delete", "trash-2", IconTone::Rose),
        ("/databases/phpmyadmin", "table", IconTone::Violet),
        ("/databases/manager", "database", IconTone::Cyan),
        ("/databases", "database", IconTone::Blue),
        ("/ftp/accounts", "folder", IconTone::Blue),
        ("/ftp/create", "folder-plus", IconTone::Green),
        ("/ftp/delete", "trash-2", IconTone::Rose),
        ("/ftp/reset", "refresh-cw", IconTone::Amber),
        ("/ftp", "folder-sync", IconTone::Blue),
        ("/backups/create", "hard-drive", IconTone::Green),
        ("/backups/restore", "refresh-cw", IconTone::Blue),
        ("/backups/schedule", "clock", IconTone::Amber),
        ("/backups/destinations", "folder", IconTone::Cyan),
        ("/backups/google-drive", "cloud", IconTone::Violet),
        ("/backups/remote", "network", IconTone::Slate),
        ("/backups", "hard-drive", IconTone::Blue),
        ("/server/services", "activity", IconTone::Green),
        ("/server/php/extensions", "plug", IconTone::Violet),
        ("/server/php/configs", "file", IconTone::Slate),
        ("/server/php/tuning", "gauge", IconTone::Amber),
        ("/server/processes", "activity", IconTone::Rose),
        ("/server/packages", "package", IconTone::Blue),
        ("/server/docker/apps", "boxes", IconTone::Cyan),
        ("/server/docker/containers", "container", IconTone::Blue),
        ("/server/docker/images", "hard-drive", IconTone::Violet),
        ("/server/files", "folder", IconTone::Amber),
        ("/server/dns/zones", "dns", IconTone::Cyan),
        ("/server/dns/nameservers", "globe", IconTone::Blue),
        ("/server/dns/defaults", "settings", IconTone::Slate),
        ("/server", "server", IconTone::Blue),
        ("/packages/new", "plus", IconTone::Green),
        ("/packages", "package", IconTone::Blue),
        ("/plugins", "puzzle", IconTone::Violet),
        ("/apps", "boxes", IconTone::Cyan),
        ("/websites", "globe", IconTone::Blue),
        ("/dashboard", "layout-dashboard", IconTone::Blue),
    ];
    let mut best: Option<(&str, &str, IconTone)> = None;
    for &(prefix, key, tone) in rules {
        if path == prefix || path.starts_with(&format!("{prefix}/")) {
            let better = match best {
                None => true,
                Some((prev, _, _)) => prefix.len() > prev.len(),
            };
            if better {
                best = Some((prefix, key, tone));
            }
        }
    }
    match best {
        Some((_, key, tone)) => (key, tone),
        None => ("layout-dashboard", IconTone::Slate),
    }
}

pub fn resolve_nav(id: &str) -> (&'static str, IconTone) {
    if id.starts_with("plugin-") {
        return ("puzzle", IconTone::Violet);
    }
    match id {
        "dashboard" => ("layout-dashboard", IconTone::Blue),
        "websites" => ("globe", IconTone::Blue),
        "email" => ("mail", IconTone::Cyan),
        "databases" => ("database", IconTone::Violet),
        "backups" => ("hard-drive", IconTone::Amber),
        "apps" => ("boxes", IconTone::Cyan),
        "users" | "packages" => ("users", IconTone::Blue),
        "server" => ("server", IconTone::Slate),
        "security" => ("shield", IconTone::Rose),
        "settings" => ("settings", IconTone::Slate),
        "plugins" => ("puzzle", IconTone::Violet),
        _ => ("layout-dashboard", IconTone::Slate),
    }
}

pub fn hub_icon_html(href: &str) -> String {
    let (key, tone) = resolve_href(href);
    format!(
        r#"<span class="hub-tile-icon tone-{tone}" aria-hidden="true">{svg}</span>"#,
        tone = tone.as_str(),
        svg = panel_icons_svg::svg_owned(key),
    )
}

pub fn nav_icon_html(id: &str) -> String {
    let (key, tone) = resolve_nav(id);
    format!(
        r#"<span class="nav-icon tone-{tone}" aria-hidden="true">{svg}</span>"#,
        tone = tone.as_str(),
        svg = panel_icons_svg::svg_owned(key),
    )
}

pub fn manage_icon_html(href: &str) -> String {
    let (key, tone) = resolve_href(href);
    format!(
        r#"<span class="manage-tile-icon tone-{tone}" aria-hidden="true">{svg}</span>"#,
        tone = tone.as_str(),
        svg = panel_icons_svg::svg_owned(key),
    )
}

pub fn icon_tone_styles() -> &'static str {
    r#"
.hub-tile-icon, .manage-tile-icon, .nav-icon {
  display:inline-grid; place-items:center; color:#2563eb;
  background:#eef4ff; border:1px solid #d7e6ff;
}
.hub-tile-icon svg, .manage-tile-icon svg, .nav-icon svg { display:block; }
.hub-tile-icon.tone-green, .manage-tile-icon.tone-green, .nav-icon.tone-green { color:#067647; background:#ecfdf3; border-color:#abefc6; }
.hub-tile-icon.tone-amber, .manage-tile-icon.tone-amber, .nav-icon.tone-amber { color:#b54708; background:#fffaeb; border-color:#fedf89; }
.hub-tile-icon.tone-violet, .manage-tile-icon.tone-violet, .nav-icon.tone-violet { color:#6941c6; background:#f4f3ff; border-color:#d9d6fe; }
.hub-tile-icon.tone-rose, .manage-tile-icon.tone-rose, .nav-icon.tone-rose { color:#c01048; background:#fff1f3; border-color:#fecdd6; }
.hub-tile-icon.tone-cyan, .manage-tile-icon.tone-cyan, .nav-icon.tone-cyan { color:#0e7490; background:#ecfeff; border-color:#a5f3fc; }
.hub-tile-icon.tone-slate, .manage-tile-icon.tone-slate, .nav-icon.tone-slate { color:#344054; background:#f2f4f7; border-color:#d0d5dd; }
.hub-tile-icon.tone-blue, .manage-tile-icon.tone-blue, .nav-icon.tone-blue { color:#175cd3; background:#eff8ff; border-color:#b2ddff; }
[data-color-mode="dark"] .hub-tile-icon,
[data-color-mode="dark"] .manage-tile-icon,
[data-color-mode="dark"] .nav-icon {
  color:#93c5fd; background:rgba(59,130,246,.16); border-color:rgba(59,130,246,.35);
}
[data-color-mode="dark"] .hub-tile-icon.tone-green,
[data-color-mode="dark"] .manage-tile-icon.tone-green,
[data-color-mode="dark"] .nav-icon.tone-green { color:#6ce9a6; background:rgba(6,118,71,.22); border-color:rgba(108,233,166,.35); }
[data-color-mode="dark"] .hub-tile-icon.tone-amber,
[data-color-mode="dark"] .manage-tile-icon.tone-amber,
[data-color-mode="dark"] .nav-icon.tone-amber { color:#fdb022; background:rgba(181,71,8,.22); border-color:rgba(253,176,34,.35); }
[data-color-mode="dark"] .hub-tile-icon.tone-violet,
[data-color-mode="dark"] .manage-tile-icon.tone-violet,
[data-color-mode="dark"] .nav-icon.tone-violet { color:#bdb4fe; background:rgba(105,65,198,.22); border-color:rgba(189,180,254,.35); }
[data-color-mode="dark"] .hub-tile-icon.tone-rose,
[data-color-mode="dark"] .manage-tile-icon.tone-rose,
[data-color-mode="dark"] .nav-icon.tone-rose { color:#fda29b; background:rgba(192,16,72,.22); border-color:rgba(253,162,155,.35); }
[data-color-mode="dark"] .hub-tile-icon.tone-cyan,
[data-color-mode="dark"] .manage-tile-icon.tone-cyan,
[data-color-mode="dark"] .nav-icon.tone-cyan { color:#67e8f9; background:rgba(14,116,144,.22); border-color:rgba(103,232,249,.35); }
[data-color-mode="dark"] .hub-tile-icon.tone-slate,
[data-color-mode="dark"] .manage-tile-icon.tone-slate,
[data-color-mode="dark"] .nav-icon.tone-slate { color:#d0d5dd; background:rgba(52,64,84,.35); border-color:rgba(208,213,221,.25); }
[data-color-mode="dark"] .hub-tile-icon.tone-blue,
[data-color-mode="dark"] .manage-tile-icon.tone-blue,
[data-color-mode="dark"] .nav-icon.tone-blue { color:#93c5fd; background:rgba(23,92,211,.22); border-color:rgba(147,197,253,.35); }
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_email_and_fallback() {
        let (k, _) = resolve_href("/email/dkim");
        assert_eq!(k, "key");
        let (k2, _) = resolve_href("/unknown/path");
        assert_eq!(k2, "layout-dashboard");
        let html = hub_icon_html("/email/accounts");
        assert!(html.contains("<svg"));
        assert!(html.contains("tone-blue"));
    }

    #[test]
    fn maps_nav_and_security() {
        assert_eq!(resolve_nav("security").0, "shield");
        assert_eq!(resolve_href("/security/firewall").0, "shield");
        assert_eq!(resolve_href("/settings/design").0, "palette");
        assert_eq!(resolve_href("/account/users/create").0, "user-plus");
    }
}
