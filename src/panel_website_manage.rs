//! Website Manage dashboard HTML for CPN Panel.

use crate::panel_site_terminal::tab_terminal;
use crate::panel_website_manage_tabs::{
    tab_apps, tab_config, tab_domains, tab_files, tab_logs, tab_overview, tab_ssl,
};
use crate::panel_website_manage_tools::{tab_clone, tab_git};
use crate::panel_website_manage_ui::{
    html_escape, manage_banner, manage_styles, notice_block, quick_actions, tab_bar,
};
use crate::sites::SiteRecord;

pub use crate::panel_website_resources::{approx_dir_bytes, format_bytes};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManageTab {
    Overview,
    Domains,
    Logs,
    Config,
    Ssl,
    Files,
    Apps,
    Terminal,
    Git,
    Clone,
}

impl ManageTab {
    pub fn parse(raw: Option<&str>) -> Self {
        match raw.unwrap_or("").trim().to_ascii_lowercase().as_str() {
            "domains" => Self::Domains,
            "logs" => Self::Logs,
            "config" | "configurations" => Self::Config,
            "ssl" => Self::Ssl,
            "files" | "file" | "docroot" => Self::Files,
            "apps" | "applications" | "plugins" | "plugin" => Self::Apps,
            "terminal" | "term" | "shell" => Self::Terminal,
            "git" => Self::Git,
            "clone" | "staging" => Self::Clone,
            _ => Self::Overview,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Domains => "domains",
            Self::Logs => "logs",
            Self::Config => "config",
            Self::Ssl => "ssl",
            Self::Files => "files",
            Self::Apps => "plugins",
            Self::Terminal => "terminal",
            Self::Git => "git",
            Self::Clone => "clone",
        }
    }
}

fn tab_body(site: &SiteRecord, tab: ManageTab, username: &str) -> String {
    match tab {
        ManageTab::Overview => tab_overview(site, username),
        ManageTab::Domains => tab_domains(site),
        ManageTab::Logs => tab_logs(site),
        ManageTab::Config => tab_config(site),
        ManageTab::Ssl => tab_ssl(site),
        ManageTab::Files => tab_files(site),
        ManageTab::Apps => tab_apps(site),
        ManageTab::Terminal => tab_terminal(site, username),
        ManageTab::Git => tab_git(site, username),
        ManageTab::Clone => tab_clone(site, username),
    }
}

pub fn website_manage_main(
    site: &SiteRecord,
    username: &str,
    tab_raw: Option<&str>,
    notice: Option<&str>,
    error: Option<&str>,
) -> String {
    let tab = ManageTab::parse(tab_raw);
    let suspend = if site.enabled {
        format!(
            r#"<form method="post" action="/websites/suspend" class="inline-form" onsubmit="return confirm('Suspend {domain}?');">
          <input type="hidden" name="domain" value="{domain}">
          <button type="submit" class="btn-warn">Suspend</button>
        </form>"#,
            domain = html_escape(&site.domain),
        )
    } else {
        format!(
            r#"<form method="post" action="/websites/resume" class="inline-form">
          <input type="hidden" name="domain" value="{domain}">
          <button type="submit" class="btn-primary">Resume</button>
        </form>"#,
            domain = html_escape(&site.domain),
        )
    };
    let body = tab_body(site, tab, username);
    let styles = format!(
        "{}{}",
        manage_styles(),
        crate::panel_icons::icon_tone_styles()
    );
    format!(
        r##"<style>{styles}</style>
      <div class="site-manage">
        <p class="manage-muted"><a href="/websites">Back to Websites</a></p>
        {ok}
        {err}
        {banner}
        {quick}
        {tabs}
        <div class="manage-actions-row">
          {suspend}
          <form method="post" action="/websites/delete" class="inline-form" onsubmit="return confirm('Delete site {domain}? Document files under /home are kept.');">
            <input type="hidden" name="domain" value="{domain}">
            <button type="submit" class="btn-danger">Delete</button>
          </form>
        </div>
        <article>
          {body}
        </article>
      </div>"##,
        styles = styles,
        ok = notice_block("ok", notice),
        err = notice_block("error", error),
        banner = manage_banner(site, username),
        quick = quick_actions(site),
        tabs = tab_bar(&site.domain, tab.as_str()),
        suspend = suspend,
        domain = html_escape(&site.domain),
        body = body,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sites::SiteRecord;

    fn sample() -> SiteRecord {
        SiteRecord {
            schema_version: 1,
            domain: "cpn-lab-test.example".into(),
            owner: "Admin".into(),
            docroot: "/tmp/cpn-manage-missing".into(),
            enabled: true,
            engine: None,
            notes: String::new(),
            created_at_unix: 0,
            updated_at_unix: 0,
            vhost_wired: false,
            ssl: Default::default(),
            internal_ip: None,
            owner_suspend_message: String::new(),
            suspended_by: None,
            php_version: None,
        }
    }

    #[test]
    fn manage_dashboard_has_banner_tabs_and_preview() {
        let html = website_manage_main(&sample(), "Admin", Some("overview"), None, None);
        assert!(html.contains("manage-banner"));
        assert!(html.contains("Preview Website"));
        assert!(html.contains("File Manager"));
        assert!(html.contains("cpn-design-open"));
        assert!(html.contains("manage-tabs"));
        assert!(html.contains("Disk Usage"));
        assert!(html.contains("/preview/cpn-lab-test.example/"));
        assert!(!html.to_lowercase().contains("cyberpanel"));
        assert!(!html.to_lowercase().contains("email marketing"));
    }

    #[test]
    fn tab_parse_defaults_overview() {
        assert_eq!(ManageTab::parse(None), ManageTab::Overview);
        assert_eq!(ManageTab::parse(Some("SSL")), ManageTab::Ssl);
        assert_eq!(ManageTab::parse(Some("files")), ManageTab::Files);
        assert_eq!(ManageTab::parse(Some("apps")), ManageTab::Apps);
        assert_eq!(ManageTab::parse(Some("plugins")), ManageTab::Apps);
        assert_eq!(ManageTab::parse(Some("git")), ManageTab::Git);
        assert_eq!(ManageTab::parse(Some("terminal")), ManageTab::Terminal);
        assert_eq!(ManageTab::parse(Some("staging")), ManageTab::Clone);
        assert_eq!(ManageTab::Apps.as_str(), "plugins");
    }

    #[test]
    fn quick_actions_enable_terminal_git_clone() {
        let html = website_manage_main(&sample(), "Admin", Some("overview"), None, None);
        assert!(html.contains("tab=terminal"));
        assert!(html.contains("tab=git"));
        assert!(html.contains("tab=clone"));
        assert!(!html.contains("Web terminal ships later"));
        assert!(!html.contains("Git manager ships later"));
        assert!(!html.contains("Clone/staging ships later"));
    }
}
