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
}
