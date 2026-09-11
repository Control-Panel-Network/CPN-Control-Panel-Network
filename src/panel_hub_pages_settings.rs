//! Settings hub: Design, Setup Wizard, Connect, and port (Version Management is separate).

use crate::panel_hub_defs::settings_hub_sections;
use crate::panel_hubs::{feature_shell, hub_tiles_grid, section_heading};
use crate::panel_theme_chrome::design_settings_panel;

pub fn settings_hub_main() -> String {
    let mut body = section_heading(
        "Settings",
        "Panel version, design, onboarding, community links, and listen port.",
    );
    for (title, tiles) in settings_hub_sections() {
        body.push_str(&hub_tiles_grid(title, &tiles));
    }
    body
}

/// Kept for callers that still import the stub name (parallel hub PRs).
pub fn settings_stub_page() -> String {
    settings_hub_main()
}

pub use crate::panel_hub_pages_version::version_management_page;

pub fn design_settings_page(username: &str) -> String {
    let panel = design_settings_panel(username);
    let themes = crate::panel_theme_store::themes_catalog_panel(username);
    let note = r#"<p class="plugin-store-meta" style="margin-bottom:14px;">
  Light/dark mode is per signed-in user (sidebar toggle). Built-in presets and catalog themes from
  <a href="https://github.com/Control-Panel-Network/CPN-Themes" target="_blank" rel="noopener noreferrer">Control-Panel-Network/CPN-Themes</a>
  apply panel-wide chrome. Only the panel admin can change Design.
</p>"#;
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Settings", Some("/settings")),
            ("Design", None),
        ],
        "Design",
        "Theme & custom CSS",
        &format!("{note}{panel}{themes}"),
        None,
        None,
    )
}

pub fn setup_wizard_page() -> String {
    let body = r#"<p>CPN does not ship a separate post-install wizard inside the signed-in panel yet.
  Use this checklist for first-run onboarding, then the installer UI when you need a fresh or maintenance install.</p>
<ol class="setup-checklist" style="margin:16px 0;padding-left:1.25rem;line-height:1.6;">
  <li>Confirm the first admin account can sign in at <a href="/login">/login</a> (no installer token required once bootstrap exists).</li>
  <li>Add a website under <a href="/websites">Websites</a> and open <strong>Manage</strong> for that site.</li>
  <li>Install or verify the web stack from the installer when needed (<code>/</code> with the installer token while <code>cpn-installer</code> is running).</li>
  <li>Configure mail under <a href="/email">Email</a> when you need mailboxes.</li>
  <li>Set the panel listen port under <a href="/settings/port">Change Port</a> (default <code>2087</code>).</li>
  <li>Review <a href="/settings/version">Version Management</a> after upgrades.</li>
</ol>
<p class="muted">Honest scope: this page is a guided checklist, not an interactive multi-step wizard. The full installer UI remains the path for server/mail recipe installs and upgrade/repair.</p>"#;
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Settings", Some("/settings")),
            ("Setup Wizard", None),
        ],
        "Setup Wizard",
        "Server onboarding",
        body,
        None,
        None,
    )
}

pub fn connect_page() -> String {
    let body = r#"<p>Community and documentation for Control Panel Network (CPN). No third-party control-panel branding.</p>
<ul class="kv-list" style="margin-top:14px;">
  <li><span>Repository</span><strong><a href="https://github.com/Control-Panel-Network/CPN-Control-Panel-Network" target="_blank" rel="noopener noreferrer">CPN-Control-Panel-Network</a></strong></li>
  <li><span>Organization</span><strong><a href="https://github.com/Control-Panel-Network" target="_blank" rel="noopener noreferrer">github.com/Control-Panel-Network</a></strong></li>
  <li><span>Issues</span><strong><a href="https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/issues" target="_blank" rel="noopener noreferrer">GitHub Issues</a></strong></li>
  <li><span>Releases</span><strong><a href="https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases" target="_blank" rel="noopener noreferrer">GitHub Releases</a></strong></li>
  <li><span>Contributing</span><strong><a href="https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/blob/stable/CONTRIBUTING.md" target="_blank" rel="noopener noreferrer">CONTRIBUTING.md</a></strong></li>
  <li><span>Security</span><strong><a href="https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/blob/stable/SECURITY.md" target="_blank" rel="noopener noreferrer">SECURITY.md</a></strong></li>
</ul>
<p class="muted" style="margin-top:16px;">CPN does not publish a Discord invite in-repo yet. Prefer GitHub Issues and Discussions on the org for community contact.</p>"#;
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Settings", Some("/settings")),
            ("Connect", None),
        ],
        "Connect",
        "Community & docs",
        body,
        None,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hub_lists_four_primary_tiles() {
        let html = settings_hub_main();
        assert!(html.contains("Version Management"));
        assert!(html.contains("Update CPN"));
        assert!(html.contains("Design"));
        assert!(html.contains("Theme &amp; custom CSS") || html.contains("Theme & custom CSS"));
        assert!(html.contains("Setup Wizard"));
        assert!(html.contains("Server onboarding"));
        assert!(html.contains("Connect"));
        assert!(html.contains("Community &amp; docs") || html.contains("Community & docs"));
        assert!(html.contains("Change Port"));
        assert!(!html.to_lowercase().contains("cyberpanel"));
        assert!(!html.to_lowercase().contains("cyberpersons"));
    }

    #[test]
    fn connect_is_cpn_native() {
        let html = connect_page();
        assert!(html.contains("Control-Panel-Network"));
        assert!(html.contains("Community & docs") || html.contains("Community &amp; docs"));
        assert!(html.contains("GitHub Releases"));
        assert!(!html.to_lowercase().contains("cyberpersons"));
    }

    #[test]
    fn version_page_uses_searchable_picker() {
        let html = version_management_page(true);
        assert!(html.contains("cpn-version-search"));
        assert!(html.contains("Type to search tags"));
        assert!(!html.contains("cpn-version-select"));
        assert!(html.contains("Upgrade to latest"));
        assert!(!html.contains('\u{2014}'));
        assert!(!html.contains('\u{2013}'));
        assert!(!html.to_lowercase().contains("cyberpanel"));
    }
}
