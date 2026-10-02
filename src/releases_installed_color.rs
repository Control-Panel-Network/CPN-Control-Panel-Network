//! Installed-package color for Version Management (green / orange / red).

use crate::releases::{CpnRelease, compare_versions, normalize_version};
use serde::Serialize;
use std::cmp::Ordering;

/// ~6 calendar months in seconds (183 days).
pub const STALE_INSTALL_SECS: u64 = 183 * 24 * 60 * 60;

/// UI color for the Installed package value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstalledPackageColor {
    /// On latest for the active channel (no newer matching releases / tip).
    Current,
    /// Newer channel-matching release or stable tip is available; not critically stale.
    UpdateAvailable,
    /// Installed more than ~6 months ago and a newer stable (non-prerelease) exists.
    StaleBehind,
}

/// True for alpha / beta / rc / dev and similar pre-release labels.
pub fn is_prerelease_label(version: &str) -> bool {
    let lower = normalize_version(version).to_ascii_lowercase();
    [
        "alpha", "beta", "rc", "dev", "pre", "preview", "snapshot", "nightly",
    ]
    .iter()
    .any(|token| lower.contains(token))
}

fn release_is_prerelease(release: &CpnRelease) -> bool {
    release.prerelease
        || is_prerelease_label(&release.version)
        || is_prerelease_label(&release.tag_name)
}

fn installed_is_prerelease(installed_version: &str, releases: &[CpnRelease]) -> bool {
    if is_prerelease_label(installed_version) {
        return true;
    }
    let want = normalize_version(installed_version);
    releases.iter().any(|release| {
        release_is_prerelease(release)
            && (normalize_version(&release.version) == want
                || normalize_version(&release.tag_name) == want)
    })
}

/// Newer non-prerelease release than `installed_version`.
pub fn newer_stable_release_available(installed_version: &str, releases: &[CpnRelease]) -> bool {
    releases.iter().any(|release| {
        !release_is_prerelease(release)
            && compare_versions(installed_version, &release.version) == Ordering::Less
    })
}

/// Newer release in the active channel.
/// Stable installs ignore alpha/beta/dev/rc; pre-release installs include pre-releases
/// (and may also see newer stables).
pub fn newer_channel_release_available(installed_version: &str, releases: &[CpnRelease]) -> bool {
    let on_pre = installed_is_prerelease(installed_version, releases);
    releases.iter().any(|release| {
        let is_pre = release_is_prerelease(release);
        if !on_pre && is_pre {
            return false;
        }
        compare_versions(installed_version, &release.version) == Ordering::Less
    })
}

pub fn install_is_stale(installed_at_unix: Option<u64>, now_unix: u64) -> bool {
    match installed_at_unix {
        Some(ts) if ts > 0 => now_unix.saturating_sub(ts) > STALE_INSTALL_SECS,
        _ => false,
    }
}

/// Pick Installed package color from age, channel-aware updates, and tip updates.
pub fn installed_package_color(
    installed_version: &str,
    installed_at_unix: Option<u64>,
    now_unix: u64,
    releases: &[CpnRelease],
    stable_tip_update: bool,
) -> InstalledPackageColor {
    let newer_stable = newer_stable_release_available(installed_version, releases);
    let newer_channel = newer_channel_release_available(installed_version, releases);
    let update_available = newer_channel || stable_tip_update;
    let stale = install_is_stale(installed_at_unix, now_unix);

    if stale && newer_stable {
        InstalledPackageColor::StaleBehind
    } else if update_available {
        InstalledPackageColor::UpdateAvailable
    } else {
        InstalledPackageColor::Current
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::releases::CpnRelease;

    fn release(tag: &str, version: &str, prerelease: bool) -> CpnRelease {
        CpnRelease {
            tag_name: tag.into(),
            version: version.into(),
            name: tag.into(),
            published_at: String::new(),
            prerelease,
            draft: false,
            html_url: String::new(),
            assets: Vec::new(),
            rpm_asset: None,
            binary_asset: None,
            checksums_asset: None,
            checksums_asc_asset: None,
        }
    }

    #[test]
    fn detects_prerelease_labels() {
        assert!(is_prerelease_label("1.2.0-alpha.1"));
        assert!(is_prerelease_label("v0.2.6-beta.2"));
        assert!(is_prerelease_label("1.0.0-rc1"));
        assert!(is_prerelease_label("2.0.0-dev"));
        assert!(!is_prerelease_label("1.1.0"));
        assert!(!is_prerelease_label("v1.0.0"));
    }

    #[test]
    fn stable_install_ignores_newer_alpha() {
        let releases = vec![
            release("v1.2.0-alpha.1", "1.2.0-alpha.1", true),
            release("v1.0.0", "1.0.0", false),
        ];
        assert!(!newer_stable_release_available("1.0.0", &releases));
        assert!(!newer_channel_release_available("1.0.0", &releases));
        assert_eq!(
            installed_package_color("1.0.0", Some(1), 2, &releases, false),
            InstalledPackageColor::Current
        );
    }

    #[test]
    fn orange_when_newer_stable_and_fresh_install() {
        let now = 1_700_000_000u64;
        let releases = vec![
            release("v1.1.0", "1.1.0", false),
            release("v1.0.0", "1.0.0", false),
        ];
        assert_eq!(
            installed_package_color("1.0.0", Some(now - 86400), now, &releases, false),
            InstalledPackageColor::UpdateAvailable
        );
    }

    #[test]
    fn red_when_stale_and_newer_stable() {
        let now = 1_700_000_000u64;
        let old = now - STALE_INSTALL_SECS - 10;
        let releases = vec![
            release("v1.1.0", "1.1.0", false),
            release("v1.0.0", "1.0.0", false),
        ];
        assert_eq!(
            installed_package_color("1.0.0", Some(old), now, &releases, false),
            InstalledPackageColor::StaleBehind
        );
    }

    #[test]
    fn green_when_stale_but_no_newer_stable() {
        let now = 1_700_000_000u64;
        let old = now - STALE_INSTALL_SECS - 10;
        let releases = vec![release("v1.0.0", "1.0.0", false)];
        assert_eq!(
            installed_package_color("1.0.0", Some(old), now, &releases, false),
            InstalledPackageColor::Current
        );
    }

    #[test]
    fn prerelease_install_sees_newer_prerelease() {
        let releases = vec![
            release("v1.2.0-alpha.2", "1.2.0-alpha.2", true),
            release("v1.2.0-alpha.1", "1.2.0-alpha.1", true),
        ];
        assert!(newer_channel_release_available("1.2.0-alpha.1", &releases));
        assert!(!newer_stable_release_available("1.2.0-alpha.1", &releases));
    }

    #[test]
    fn tip_update_is_orange_not_red() {
        let now = 1_700_000_000u64;
        let old = now - STALE_INSTALL_SECS - 10;
        let releases = vec![release("v1.1.0", "1.1.0", false)];
        assert_eq!(
            installed_package_color("1.1.0", Some(old), now, &releases, true),
            InstalledPackageColor::UpdateAvailable
        );
    }
}
