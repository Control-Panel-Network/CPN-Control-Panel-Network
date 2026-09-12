//! Gate sidebar children and hub tiles on package-backed install state.
//!
//! Panel-native tools (MariaDB Manager, SFTP jail UI, scaffold stubs) stay visible.
//! Optional host packages such as phpMyAdmin and webmail appear only when installed.

use crate::apps::{AppId, AppStateKind, detect_app};
use crate::litespeed_stack::{
    any_litespeed_installed, litespeed_enterprise_installed, openlitespeed_installed,
};
use std::path::Path;

/// Detected optional software that backs specific nav/hub links.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstalledOptionalFeatures {
    pub phpmyadmin: bool,
    pub webmail: bool,
    pub openlitespeed: bool,
    pub litespeed_enterprise: bool,
    pub litespeed_any: bool,
}

impl InstalledOptionalFeatures {
    pub fn detect() -> Self {
        Self {
            phpmyadmin: phpmyadmin_installed(),
            webmail: webmail_installed(),
            openlitespeed: openlitespeed_installed(),
            litespeed_enterprise: litespeed_enterprise_installed(),
            litespeed_any: any_litespeed_installed(),
        }
    }

    /// Whether a nav/hub/dashboard href should be shown for this host.
    pub fn allows_href(self, href: &str) -> bool {
        match href.trim_end_matches('/') {
            "/databases/phpmyadmin" => self.phpmyadmin,
            "/email/webmail" => self.webmail,
            "/server/openlitespeed" => self.openlitespeed,
            "/server/litespeed-enterprise" => self.litespeed_enterprise,
            "/server/litespeed" => self.litespeed_any,
            _ => true,
        }
    }
}

/// True when phpMyAdmin packages or share path are present (Installed or Running).
pub fn phpmyadmin_installed() -> bool {
    let status = detect_app(AppId::Phpmyadmin);
    !matches!(status.state, AppStateKind::NotInstalled)
}

/// True when SnappyMail/Roundcube webmail files are present under /opt/cpn-webmail.
pub fn webmail_installed() -> bool {
    Path::new("/opt/cpn-webmail/snappymail").is_dir()
        || Path::new("/opt/cpn-webmail/roundcube").is_dir()
        || Path::new("/opt/cpn-webmail/current").exists()
}

/// Filter hub tiles, dropping package-backed entries that are not installed.
pub fn filter_hub_tiles<'a>(
    tiles: Vec<crate::panel_hubs::HubTile<'a>>,
    feats: InstalledOptionalFeatures,
) -> Vec<crate::panel_hubs::HubTile<'a>> {
    tiles
        .into_iter()
        .filter(|tile| feats.allows_href(tile.href))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::InstalledOptionalFeatures;

    fn feats(
        phpmyadmin: bool,
        webmail: bool,
        openlitespeed: bool,
        litespeed_enterprise: bool,
    ) -> InstalledOptionalFeatures {
        InstalledOptionalFeatures {
            phpmyadmin,
            webmail,
            openlitespeed,
            litespeed_enterprise,
            litespeed_any: openlitespeed || litespeed_enterprise,
        }
    }

    #[test]
    fn hides_phpmyadmin_when_not_installed() {
        let feats = feats(false, true, false, false);
        assert!(!feats.allows_href("/databases/phpmyadmin"));
        assert!(!feats.allows_href("/databases/phpmyadmin/"));
        assert!(feats.allows_href("/databases/manager"));
        assert!(feats.allows_href("/ftp/accounts"));
        assert!(feats.allows_href("/email/webmail"));
    }

    #[test]
    fn hides_webmail_when_not_installed() {
        let feats = feats(true, false, false, false);
        assert!(!feats.allows_href("/email/webmail"));
        assert!(feats.allows_href("/databases/phpmyadmin"));
        assert!(feats.allows_href("/email/accounts"));
    }

    #[test]
    fn gates_ols_and_olse_separately() {
        let ols_only = feats(false, false, true, false);
        assert!(ols_only.allows_href("/server/openlitespeed"));
        assert!(!ols_only.allows_href("/server/litespeed-enterprise"));
        assert!(ols_only.allows_href("/server/litespeed"));

        let lse_only = feats(false, false, false, true);
        assert!(!lse_only.allows_href("/server/openlitespeed"));
        assert!(lse_only.allows_href("/server/litespeed-enterprise"));
        assert!(lse_only.allows_href("/server/litespeed"));

        let none = feats(false, false, false, false);
        assert!(!none.allows_href("/server/openlitespeed"));
        assert!(!none.allows_href("/server/litespeed-enterprise"));
        assert!(!none.allows_href("/server/litespeed"));
    }

    #[test]
    fn keeps_panel_native_routes() {
        let feats = feats(false, false, false, false);
        for href in [
            "/databases",
            "/databases/all",
            "/databases/create",
            "/databases/manager",
            "/ftp/create",
            "/email/accounts",
            "/email/delivery",
            "/dashboard",
        ] {
            assert!(feats.allows_href(href), "should keep {href}");
        }
    }
}
