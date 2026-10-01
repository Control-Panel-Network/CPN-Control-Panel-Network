//! Settings hub: Design, Setup Wizard, Connect, and port (Version Management is separate).

use crate::panel_hub_defs::settings_hub_sections;
use crate::panel_hubs::{feature_shell, hub_tiles_grid, section_heading};
use crate::panel_theme_chrome::design_settings_panel;

/// Settings pages that only the panel owner may open (tiles are hidden for other accounts).
const OWNER_ONLY_SETTINGS: &[&str] = &[
    "/settings/logs",
    "/settings/site-messages",
    "/settings/error-messages",
];

pub fn settings_hub_main() -> String {
    settings_hub_main_with(true, None, None)
}

/// Settings overview. `is_owner` hides owner-only tiles for other accounts; `notice` / `error`
/// carry the message of a redirect (for example "Admin only") so it is never dropped silently.
pub fn settings_hub_main_with(is_owner: bool, notice: Option<&str>, error: Option<&str>) -> String {
    let mut body = section_heading(
        "Settings",
        "Panel version, design, onboarding, community links, and listen port.",
    );
    if let Some(msg) = notice.filter(|m| !m.trim().is_empty()) {
        body.push_str(&format!(
            r#"<p class="panel-notice ok">{}</p>"#,
            escape_text(msg)
        ));
    }
    if let Some(msg) = error.filter(|m| !m.trim().is_empty()) {
        body.push_str(&format!(
            r#"<p class="panel-notice error">{}</p>"#,
            escape_text(msg)
        ));
    }
    for (title, tiles) in settings_hub_sections() {
        let tiles: Vec<_> = tiles
            .into_iter()
            .filter(|tile| is_owner || !OWNER_ONLY_SETTINGS.contains(&tile.href))
            .collect();
        if tiles.is_empty() {
            continue;
        }
        body.push_str(&hub_tiles_grid(title, &tiles));
    }
    body
}

fn escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Kept for callers that still import the stub name (parallel hub PRs).
pub fn settings_stub_page() -> String {
    settings_hub_main()
}

pub use crate::panel_hub_pages_version::version_management_page;

/// Normalize Design page tab (`?tab=`). Default is Design; `installed` / `store` mirror Plugins.
pub fn design_settings_tab(raw: Option<&str>) -> &'static str {
    match raw.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
        Some("store") | Some("theme-store") | Some("themes") => "store",
        Some("installed") | Some("theme-installed") | Some("my-themes") => "installed",
        _ => "design",
    }
}

fn design_view_tabs(active: &str) -> String {
    let design = if active == "design" { " active" } else { "" };
    let installed = if active == "installed" {
        " active"
    } else {
        ""
    };
    let store = if active == "store" { " active" } else { "" };
    format!(
        r#"<div class="plugin-tabs" role="tablist" aria-label="Design views">
  <a class="plugin-tab{design}" href="/settings/design?tab=design" role="tab" aria-selected="{design_sel}">Design</a>
  <a class="plugin-tab{installed}" href="/settings/design?tab=installed" role="tab" aria-selected="{installed_sel}">Installed</a>
  <a class="plugin-tab{store}" href="/settings/design?tab=store" role="tab" aria-selected="{store_sel}">Store</a>
</div>
<style>
  .plugin-tabs {{ display:flex; flex-wrap:wrap; gap:8px; margin:0 0 18px; }}
  .plugin-tab {{
    display:inline-flex; align-items:center; min-height:40px; padding:0 16px;
    border-radius:999px; border:1px solid var(--hairline); background:var(--canvas);
    color:var(--ink); font-size:14px; font-weight:600; text-decoration:none;
  }}
  .plugin-tab.active {{ background:#e7f1ff; color:#0b3d91; border-color:#93c5fd; }}
</style>"#,
        design = design,
        installed = installed,
        store = store,
        design_sel = if active == "design" { "true" } else { "false" },
        installed_sel = if active == "installed" {
            "true"
        } else {
            "false"
        },
        store_sel = if active == "store" { "true" } else { "false" },
    )
}

pub fn design_settings_page(username: &str) -> String {
    design_settings_page_with_tab(username, "design")
}

pub fn design_settings_page_with_tab(username: &str, tab: &str) -> String {
    let active = design_settings_tab(Some(tab));
    let tabs = design_view_tabs(active);
    if active == "store" || active == "installed" {
        let themes = crate::panel_theme_store::themes_catalog_panel(username, active);
        let (crumb, title, blurb, note) = if active == "installed" {
            (
                "Installed",
                "Installed themes",
                "Apply, update, or uninstall panel themes",
                r#"<p class="plugin-store-meta" style="margin-bottom:14px;">
  Manage themes already installed on this panel. Apply sets panel-wide chrome.
  Browse more packages on the Store tab. Light/Dark/Minimalist stay on the Design tab.
</p>"#,
            )
        } else {
            (
                "Store",
                "Theme Store",
                "Install themes from CPN-Themes",
                r#"<p class="plugin-store-meta" style="margin-bottom:14px;">
  Browse and install available theme packages from
  <a href="https://github.com/Control-Panel-Network/CPN-Themes" target="_blank" rel="noopener noreferrer">Control-Panel-Network/CPN-Themes</a>.
  After install, use the Installed tab to Apply, Update, or Uninstall. Light/Dark/Minimalist stay on the Design tab.
</p>"#,
            )
        };
        return feature_shell(
            &[
                ("Dashboard", Some("/dashboard")),
                ("Settings", Some("/settings")),
                ("Design", Some("/settings/design?tab=design")),
                (crumb, None),
            ],
            title,
            blurb,
            &format!("{tabs}{note}{themes}"),
            None,
            None,
        );
    }

    let panel = design_settings_panel(username);
    let note = r#"<p class="plugin-store-meta" style="margin-bottom:14px;">
  Minimalist mode is per signed-in user. Design Light/Dark presets update panel-wide tokens and also
  set your personal light/dark color mode so chrome surfaces change. Only the panel admin can change Design tokens.
  Install catalog themes from the Store tab; manage them on Installed.
</p>"#;
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Settings", Some("/settings")),
            ("Design", None),
        ],
        "Design",
        "Theme & custom CSS",
        &format!("{tabs}{note}{panel}"),
        None,
        None,
    )
}

pub fn setup_wizard_page() -> String {
    setup_wizard_page_with(None, None)
}

pub fn setup_wizard_page_with(notice: Option<&str>, error: Option<&str>) -> String {
    use crate::panel_ops_mail_onboarding::{MailMode, load_mail_onboarding};
    let cfg = load_mail_onboarding();
    let local_sel = if cfg.mail_mode == MailMode::Local {
        " selected"
    } else {
        ""
    };
    let ext_sel = if cfg.mail_mode == MailMode::External {
        " selected"
    } else {
        ""
    };
    let skip_checked = if cfg.skip_rdns { " checked" } else { "" };
    let notice_html = notice
        .map(|n| format!(r#"<p class="notice ok">{}</p>"#, html_escape_setup(n)))
        .unwrap_or_default();
    let error_html = error
        .map(|n| format!(r#"<p class="notice error">{}</p>"#, html_escape_setup(n)))
        .unwrap_or_default();
    let body = format!(
        r#"{notice_html}{error_html}
<article class="section-card" style="max-width:720px;">
  <h2>SERVER CONFIGURATION</h2>
  <div class="notice info" style="margin:12px 0;padding:12px 14px;">
    <p><strong>Choose wisely:</strong> If you are not going to use email service on this server, skip rDNS checks.</p>
    <p>Ensure that the hostname you provide below is set as rDNS (reverse DNS, also called PTR record) against your IP address. (Only required if you want to use email services on the same server).</p>
    <p>Make sure that the provided hostname also has an A record pointing to your server's IP address.</p>
    <p>If the above conditions fail, your server may not function as expected, especially for email services.</p>
  </div>
  <form method="post" action="/settings/setup" class="stack-form">
    <label for="hostname">Hostname</label>
    <input id="hostname" name="hostname" type="text" placeholder="mail.example.com" value="{hostname}">
    <label for="mail_mode">Mail system</label>
    <select id="mail_mode" name="mail_mode">
      <option value="local"{local_sel}>Local mail on this server (default client settings use each site domain)</option>
      <option value="external"{ext_sel}>External mail system (custom IMAP/SMTP hosts)</option>
    </select>
    <label for="external_imap_host">External IMAP host (optional)</label>
    <input id="external_imap_host" name="external_imap_host" type="text" value="{imap}" placeholder="imap.provider.com">
    <label for="external_smtp_host">External SMTP host (optional)</label>
    <input id="external_smtp_host" name="external_smtp_host" type="text" value="{smtp}" placeholder="smtp.provider.com">
    <label><input type="checkbox" name="skip_rdns" value="1"{skip_checked}> Skip rDNS/PTR check</label>
    <p class="muted">Check this if you do not want to use email service on this server. New sites will skip SPF/DKIM/DMARC auto-DNS.</p>
    <button type="submit" class="btn-primary">Save configuration</button>
  </form>
</article>
<ol class="setup-checklist" style="margin:16px 0;padding-left:1.25rem;line-height:1.6;">
  <li>Confirm the first admin account can sign in at <a href="/login">/login</a>.</li>
  <li>Add a website under <a href="/websites">Websites</a> (auto Let's Encrypt + mail DNS by default).</li>
  <li>Review <a href="/email/accounts">Email Accounts</a> mail client settings.</li>
  <li>Set the panel listen port under <a href="/settings/port">Change Port</a> (default <code>2087</code>).</li>
</ol>
<p class="muted">CPN branding only. This onboarding covers hostname/rDNS guidance and local vs external mail.</p>"#,
        notice_html = notice_html,
        error_html = error_html,
        hostname = html_escape_setup(&cfg.hostname),
        local_sel = local_sel,
        ext_sel = ext_sel,
        imap = html_escape_setup(&cfg.external_imap_host),
        smtp = html_escape_setup(&cfg.external_smtp_host),
        skip_checked = skip_checked,
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Settings", Some("/settings")),
            ("Setup Wizard", None),
        ],
        "Setup Wizard",
        "Server onboarding",
        &body,
        None,
        None,
    )
}

fn html_escape_setup(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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
    fn non_owner_hub_hides_owner_only_tiles_and_shows_message() {
        let owner = settings_hub_main_with(true, None, None);
        assert!(owner.contains("/settings/logs"));
        let other = settings_hub_main_with(false, None, Some("Admin only <b>"));
        assert!(!other.contains("/settings/logs"));
        assert!(!other.contains("/settings/site-messages"));
        assert!(other.contains("/settings/design"));
        assert!(other.contains("Admin only &lt;b&gt;"));
    }

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
        assert!(html.contains("Site messages"));
        assert!(html.contains("Error messages"));
        assert!(html.contains("/settings/error-messages"));
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
        assert!(!html.contains("id=\"cpn-version-select\""));
        assert!(html.contains("Upgrade to latest"));
        assert!(!html.contains('\u{2014}'));
        assert!(!html.contains('\u{2013}'));
        assert!(!html.to_lowercase().contains("cyberpanel"));
    }

    #[test]
    fn design_tab_normalizes_store_aliases() {
        assert_eq!(design_settings_tab(None), "design");
        assert_eq!(design_settings_tab(Some("")), "design");
        assert_eq!(design_settings_tab(Some("Design")), "design");
        assert_eq!(design_settings_tab(Some("store")), "store");
        assert_eq!(design_settings_tab(Some("Theme-Store")), "store");
        assert_eq!(design_settings_tab(Some("themes")), "store");
        assert_eq!(design_settings_tab(Some("installed")), "installed");
        assert_eq!(design_settings_tab(Some("My-Themes")), "installed");
    }

    #[test]
    fn design_page_splits_appearance_installed_and_store_tabs() {
        let design = design_settings_page_with_tab("owner", "design");
        assert!(design.contains("href=\"/settings/design?tab=design\""));
        assert!(design.contains("href=\"/settings/design?tab=installed\""));
        assert!(design.contains("href=\"/settings/design?tab=store\""));
        assert!(design.contains(">Installed</a>"));
        assert!(design.contains(">Store</a>"));
        assert!(design.contains("cpn-minimalist-card") || design.contains("Minimalist mode"));
        assert!(!design.contains("id=\"cpn-themes-catalog\""));
        assert!(!design.to_lowercase().contains("cyberpanel"));

        let store = design_settings_page_with_tab("owner", "store");
        assert!(store.contains("id=\"cpn-themes-catalog\""));
        assert!(store.contains("data-view=\"store\""));
        assert!(store.contains("cpn-themes-q"));
        assert!(store.contains("Refresh catalog"));
        assert!(store.contains("/api/panel/themes/install"));
        assert!(!store.contains("cpn-minimalist-card"));
        assert!(!store.to_lowercase().contains("cyberpanel"));
        assert!(!store.contains('\u{2014}'));
        assert!(!store.contains('\u{2013}'));

        let installed = design_settings_page_with_tab("owner", "installed");
        assert!(installed.contains("id=\"cpn-themes-catalog\""));
        assert!(installed.contains("data-view=\"installed\""));
        assert!(installed.contains("cpn-themes-q"));
        assert!(installed.contains("Installed themes"));
        assert!(!installed.contains("id=\"cpn-themes-refresh\""));
        assert!(installed.contains("/api/panel/themes/apply"));
        assert!(!installed.to_lowercase().contains("cyberpanel"));
    }
}
