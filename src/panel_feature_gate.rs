//! Gate sidebar children and hub tiles on package-backed install state.
//!
//! Panel-native tools (MariaDB Manager, SFTP jail UI, scaffold stubs) stay visible.
//! Optional host packages such as phpMyAdmin, webmail, fail2ban, and malware scanners
//! appear only when installed or configured.
//! Email MTA-STS / BIMI appear only when their catalog plugins (or host feature flags)
//! are installed.

use crate::apps::{AppId, AppStateKind, detect_app};
use crate::litespeed_stack::{
    any_litespeed_installed, litespeed_enterprise_installed, openlitespeed_installed,
};
use crate::panel_feature_flags::host_feature_enabled;
use crate::panel_hubs::feature_shell;
use crate::panel_ops_security::{fail2ban_status, firewall_status};
use crate::panel_ops_security_ssl::malware_scan_status;
use crate::plugins::plugin_id_enabled_anywhere;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Avoid re-running host probes (firewall-cmd, fail2ban, clam) on every HTML shell render.
const FEATURE_DETECT_TTL: Duration = Duration::from_secs(45);

struct FeatureDetectCache {
    at: Instant,
    value: InstalledOptionalFeatures,
}

static FEATURE_DETECT_CACHE: Mutex<Option<FeatureDetectCache>> = Mutex::new(None);

/// Detected optional software that backs specific nav/hub links.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstalledOptionalFeatures {
    pub phpmyadmin: bool,
    pub webmail: bool,
    pub openlitespeed: bool,
    pub litespeed_enterprise: bool,
    pub litespeed_any: bool,
    pub fail2ban: bool,
    pub firewall: bool,
    pub malware: bool,
    pub mta_sts: bool,
    pub bimi: bool,
}

impl InstalledOptionalFeatures {
    pub fn detect() -> Self {
        if let Ok(guard) = FEATURE_DETECT_CACHE.lock() {
            if let Some(cached) = guard.as_ref() {
                if cached.at.elapsed() < FEATURE_DETECT_TTL {
                    return cached.value;
                }
            }
        }
        let value = Self::detect_uncached();
        if let Ok(mut guard) = FEATURE_DETECT_CACHE.lock() {
            *guard = Some(FeatureDetectCache {
                at: Instant::now(),
                value,
            });
        }
        value
    }

    fn detect_uncached() -> Self {
        let fw = firewall_status();
        let f2b = fail2ban_status();
        let mal = malware_scan_status();
        Self {
            phpmyadmin: phpmyadmin_installed(),
            webmail: webmail_installed(),
            openlitespeed: openlitespeed_installed(),
            litespeed_enterprise: litespeed_enterprise_installed(),
            litespeed_any: any_litespeed_installed(),
            fail2ban: f2b.installed,
            firewall: fw.backend != "none",
            malware: mal.installed || mal.engine == "nt-api",
            mta_sts: mta_sts_unlocked(),
            bimi: bimi_unlocked(),
        }
    }

    /// Whether a nav/hub/dashboard href should be shown for this host.
    pub fn allows_href(self, href: &str) -> bool {
        match href.trim_end_matches('/') {
            "/databases/phpmyadmin" => self.phpmyadmin,
            "/email/webmail" => self.webmail,
            "/email/mta-sts" => self.mta_sts,
            "/email/bimi" => self.bimi,
            "/server/openlitespeed" => self.openlitespeed,
            "/server/litespeed-enterprise" => self.litespeed_enterprise,
            "/server/litespeed" => self.litespeed_any,
            "/security/fail2ban" => self.fail2ban,
            "/security/firewall" => self.firewall,
            "/security/malware-scan" => self.malware,
            "/apps" => false, // folded into Plugins Store (Host category)
            _ => true,
        }
    }
}

/// True when phpMyAdmin packages or share path are present (Installed or Running).
pub fn phpmyadmin_installed() -> bool {
    let status = detect_app(AppId::Phpmyadmin);
    !matches!(status.state, AppStateKind::NotInstalled)
}

/// True when a panel-proxied or Nextcloud webmail client is present.
pub fn webmail_installed() -> bool {
    Path::new("/opt/cpn-webmail/snappymail").is_dir()
        || Path::new("/opt/cpn-webmail/tachyon").is_dir()
        || Path::new("/opt/cpn-webmail/roundcube").is_dir()
        || Path::new("/opt/cpn-webmail/current").exists()
        || crate::apps_nextcloud::nextsnapmail_app_present()
}

pub fn mta_sts_unlocked() -> bool {
    host_feature_enabled("mta-sts") || plugin_id_enabled_anywhere("mtaSts")
}

pub fn bimi_unlocked() -> bool {
    host_feature_enabled("bimi") || plugin_id_enabled_anywhere("bimi")
}

/// Soft-gate body when MTA-STS / BIMI plugins are not installed.
pub fn email_auth_plugin_required_page(feature_label: &str, plugin_id: &str) -> String {
    let body = format!(
        r#"<p><strong>{label} is available as a free Plugin Store package.</strong></p>
        <p>Install <code>{id}</code> for a website from the Plugin Store. After install, this page and the Email sidebar entry unlock automatically.</p>
        <p style="margin-top:16px;">
          <a class="btn primary" href="/plugins?view=store">Open Plugin Store</a>
          <a class="btn" href="/plugins?view=store&amp;q={id_q}" style="margin-left:8px;">Search for {id}</a>
        </p>
        <p class="muted" style="margin-top:12px;">CLI: <code>sudo cpn plugin install --domain example.com --id {id}</code></p>"#,
        label = html_escape(feature_label),
        id = html_escape(plugin_id),
        id_q = urlencoding_attr(plugin_id),
    );
    feature_shell(
        &[("Email", Some("/email")), (feature_label, None)],
        feature_label,
        "Optional email authentication plugin (not installed yet).",
        &body,
        None,
        None,
    )
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn urlencoding_attr(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
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
        fail2ban: bool,
        firewall: bool,
        malware: bool,
        mta_sts: bool,
        bimi: bool,
    ) -> InstalledOptionalFeatures {
        InstalledOptionalFeatures {
            phpmyadmin,
            webmail,
            openlitespeed,
            litespeed_enterprise,
            litespeed_any: openlitespeed || litespeed_enterprise,
            fail2ban,
            firewall,
            malware,
            mta_sts,
            bimi,
        }
    }

    #[test]
    fn hides_phpmyadmin_when_not_installed() {
        let feats = feats(false, true, false, false, false, true, false, false, false);
        assert!(!feats.allows_href("/databases/phpmyadmin"));
        assert!(!feats.allows_href("/databases/phpmyadmin/"));
        assert!(feats.allows_href("/databases/manager"));
        assert!(feats.allows_href("/ftp/accounts"));
        assert!(feats.allows_href("/email/webmail"));
    }

    #[test]
    fn hides_webmail_when_not_installed() {
        let feats = feats(true, false, false, false, false, true, false, false, false);
        assert!(!feats.allows_href("/email/webmail"));
        assert!(feats.allows_href("/databases/phpmyadmin"));
        assert!(feats.allows_href("/email/accounts"));
    }

    #[test]
    fn hides_mta_sts_and_bimi_until_plugins() {
        let none = feats(
            false, false, false, false, false, false, false, false, false,
        );
        assert!(!none.allows_href("/email/mta-sts"));
        assert!(!none.allows_href("/email/bimi"));
        assert!(none.allows_href("/email/accounts"));
        assert!(none.allows_href("/email/delivery"));

        let both = feats(false, false, false, false, false, false, false, true, true);
        assert!(both.allows_href("/email/mta-sts"));
        assert!(both.allows_href("/email/bimi"));
    }

    #[test]
    fn gates_ols_and_olse_separately() {
        let ols_only = feats(false, false, true, false, false, true, false, false, false);
        assert!(ols_only.allows_href("/server/openlitespeed"));
        assert!(!ols_only.allows_href("/server/litespeed-enterprise"));
        assert!(ols_only.allows_href("/server/litespeed"));

        let lse_only = feats(false, false, false, true, false, true, false, false, false);
        assert!(!lse_only.allows_href("/server/openlitespeed"));
        assert!(lse_only.allows_href("/server/litespeed-enterprise"));
        assert!(lse_only.allows_href("/server/litespeed"));

        let none = feats(
            false, false, false, false, false, false, false, false, false,
        );
        assert!(!none.allows_href("/server/openlitespeed"));
        assert!(!none.allows_href("/server/litespeed-enterprise"));
        assert!(!none.allows_href("/server/litespeed"));
    }

    #[test]
    fn gates_security_optionals() {
        let none = feats(
            false, false, false, false, false, false, false, false, false,
        );
        assert!(!none.allows_href("/security/fail2ban"));
        assert!(!none.allows_href("/security/firewall"));
        assert!(!none.allows_href("/security/malware-scan"));
        assert!(none.allows_href("/security/ssh"));
        assert!(!none.allows_href("/apps"));

        let all = feats(true, true, true, true, true, true, true, true, true);
        assert!(all.allows_href("/security/fail2ban"));
        assert!(all.allows_href("/security/firewall"));
        assert!(all.allows_href("/security/malware-scan"));
    }

    #[test]
    fn keeps_panel_native_routes() {
        let feats = feats(
            false, false, false, false, false, false, false, false, false,
        );
        for href in [
            "/databases",
            "/databases/all",
            "/databases/create",
            "/databases/manager",
            "/ftp/create",
            "/email/accounts",
            "/email/delivery",
            "/dashboard",
            "/plugins",
            "/plugins?view=store",
        ] {
            assert!(feats.allows_href(href), "should keep {href}");
        }
    }
}
