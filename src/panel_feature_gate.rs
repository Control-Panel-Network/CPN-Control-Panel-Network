//! Gate sidebar children and hub tiles on package-backed install state.
//!
//! Panel-native tools (MariaDB Manager, SFTP jail UI, scaffold stubs) stay visible.
//! Optional host packages such as phpMyAdmin, webmail, fail2ban, and malware scanners
//! appear only when installed or configured.
//! Email MTA-STS / BIMI appear only when their catalog plugins (or host feature flags)
//! are installed.

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
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Avoid re-running host probes (firewall-cmd, fail2ban, clam) on every HTML shell render.
const FEATURE_DETECT_TTL: Duration = Duration::from_secs(45);

/// Container-engine presence is probed at most this often (the sidebar asks on every render).
const DOCKER_PROBE_TTL: Duration = Duration::from_secs(45);

struct FeatureDetectCache {
    at: Instant,
    value: InstalledOptionalFeatures,
}

static FEATURE_DETECT_CACHE: Mutex<Option<FeatureDetectCache>> = Mutex::new(None);

/// True while a background refresh of `FEATURE_DETECT_CACHE` is running.
static FEATURE_REFRESHING: AtomicBool = AtomicBool::new(false);

static DOCKER_PROBE_CACHE: Mutex<Option<(Instant, bool)>> = Mutex::new(None);

/// Clears the cached host probes so the next panel render re-detects (call after install or
/// uninstall of a host package so the sidebar reflects the change immediately).
pub fn invalidate_feature_cache() {
    if let Ok(mut guard) = FEATURE_DETECT_CACHE.lock() {
        *guard = None;
    }
    if let Ok(mut guard) = DOCKER_PROBE_CACHE.lock() {
        *guard = None;
    }
}

fn store_feature_cache(value: InstalledOptionalFeatures) {
    if let Ok(mut guard) = FEATURE_DETECT_CACHE.lock() {
        *guard = Some(FeatureDetectCache {
            at: Instant::now(),
            value,
        });
    }
}

/// Resets `FEATURE_REFRESHING` even if the refresh thread panics.
struct RefreshGuard;

impl Drop for RefreshGuard {
    fn drop(&mut self) {
        FEATURE_REFRESHING.store(false, Ordering::Release);
    }
}

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
    pub docker: bool,
}

impl InstalledOptionalFeatures {
    /// Cached host-feature snapshot for sidebar and hub rendering.
    ///
    /// Panel pages render on Actix worker threads; a slow probe (stopped firewalld, wedged
    /// podman, busy package database) used to block the worker and, once every worker was
    /// busy, the browser saw `408 Request Timeout` on otherwise healthy pages such as
    /// `/account/security/enroll-2fa`. Only the very first call computes inline; once a
    /// snapshot exists a stale one is served immediately while a single background thread
    /// refreshes it.
    pub fn detect() -> Self {
        let cached = FEATURE_DETECT_CACHE
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(|c| (c.at, c.value)));
        match cached {
            Some((at, value)) if at.elapsed() < FEATURE_DETECT_TTL => value,
            Some((_, stale)) => {
                Self::spawn_refresh();
                stale
            }
            None => {
                let value = Self::detect_uncached();
                store_feature_cache(value);
                value
            }
        }
    }

    fn spawn_refresh() {
        if FEATURE_REFRESHING.swap(true, Ordering::AcqRel) {
            return;
        }
        let spawned = std::thread::Builder::new()
            .name("cpn-feature-refresh".into())
            .spawn(|| {
                let _guard = RefreshGuard;
                store_feature_cache(Self::detect_uncached());
            });
        if spawned.is_err() {
            FEATURE_REFRESHING.store(false, Ordering::Release);
        }
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
            docker: docker_installed(),
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
            "/server/litespeed/plans" => self.litespeed_any,
            "/security/fail2ban" => self.fail2ban,
            "/security/firewall" => self.firewall,
            "/security/malware-scan" => self.malware,
            "/docker" => self.docker,
            "/docker/list" => self.docker,
            "/docker/create" => self.docker,
            "/docker/stacks" => self.docker,
            "/docker/images" => self.docker,
            "/docker/logs" => self.docker,
            "/server/docker/apps" => self.docker,
            "/server/docker/containers" => self.docker,
            "/server/docker/images" => self.docker,
            "/apps" => false, // folded into Plugins Store (Host category)
            _ => true,
        }
    }
}

/// True when the phpMyAdmin share path is present.
///
/// Path-only: the `/phpmyadmin` proxy and sidebar call this often. Do not run
/// `rpm -q` or a :8081 probe here (those belong to Apps detect, not Open).
pub fn phpmyadmin_installed() -> bool {
    crate::apps_phpmyadmin::phpmyadmin_share_dir().is_some()
}

/// True when Docker Engine or Podman CLI is present (Host package installed).
///
/// The sidebar, search catalog and nav tree ask this on every panel render, so it only checks
/// that a CLI answers `--version` (bounded) and caches the answer. It deliberately does not run
/// `info`, `ps` or `images`; those belong to the `/docker` pages.
pub fn docker_installed() -> bool {
    if let Ok(guard) = DOCKER_PROBE_CACHE.lock()
        && let Some((at, value)) = *guard
        && at.elapsed() < DOCKER_PROBE_TTL
    {
        return value;
    }
    let value = crate::panel_ops_docker::docker_bin().is_some();
    if let Ok(mut guard) = DOCKER_PROBE_CACHE.lock() {
        *guard = Some((Instant::now(), value));
    }
    value
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
    // Prefer O(1) host flag / host-plugin path. Scanning every site plugin tree
    // (list_installed_all) can stall under heavy disk I/O and used to pin workers.
    if host_feature_enabled("mta-sts") {
        return true;
    }
    if crate::plugin_activation::host_plugin_installed("mtaSts") {
        return true;
    }
    plugin_id_enabled_anywhere("mtaSts")
}

pub fn bimi_unlocked() -> bool {
    if host_feature_enabled("bimi") {
        return true;
    }
    if crate::plugin_activation::host_plugin_installed("bimi") {
        return true;
    }
    plugin_id_enabled_anywhere("bimi")
}

/// Soft-gate body when MTA-STS / BIMI plugins are not installed.
pub fn email_auth_plugin_required_page(feature_label: &str, plugin_id: &str) -> String {
    let body = format!(
        r#"<p><strong>{label} is available as a free Plugin Store package.</strong></p>
        <p>Install <code>{id}</code> from the Plugin Store (Host scope). After install, this page and the Email sidebar entry unlock automatically.</p>
        <p style="margin-top:16px;">
          <a class="btn primary" href="/plugins?view=store&amp;category=Host">Open Plugin Store</a>
          <a class="btn" href="/plugins?view=store&amp;q={id_q}" style="margin-left:8px;">Search for {id}</a>
        </p>
        <p class="muted" style="margin-top:12px;">CLI: <code>sudo cpn plugin install --host --id {id}</code></p>"#,
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
            docker: false,
        }
    }

    #[test]
    fn invalidate_clears_feature_snapshot() {
        let value = feats(
            false, false, false, false, false, false, false, false, false,
        );
        super::store_feature_cache(value);
        super::invalidate_feature_cache();
        let cleared = super::FEATURE_DETECT_CACHE
            .lock()
            .map(|guard| guard.is_none())
            .unwrap_or(false);
        assert!(cleared);
    }

    #[test]
    fn docker_installed_answers_are_stable_between_calls() {
        super::invalidate_feature_cache();
        let first = super::docker_installed();
        assert_eq!(first, super::docker_installed());
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
        assert!(!none.allows_href("/docker"));
        assert!(!none.allows_href("/server/docker/containers"));

        let all = feats(true, true, true, true, true, true, true, true, true);
        assert!(all.allows_href("/security/fail2ban"));
        assert!(all.allows_href("/security/firewall"));
        assert!(all.allows_href("/security/malware-scan"));
    }

    #[test]
    fn gates_docker_until_installed() {
        let none = feats(
            false, false, false, false, false, false, false, false, false,
        );
        assert!(!none.allows_href("/docker"));
        assert!(!none.allows_href("/docker/images"));
        let mut with_docker = none;
        with_docker.docker = true;
        assert!(with_docker.allows_href("/docker"));
        assert!(with_docker.allows_href("/docker/images"));
        assert!(with_docker.allows_href("/server/docker/apps"));
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
            "/databases/password",
            "/databases/delete",
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
