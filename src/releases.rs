//! GitHub Releases lookup for CPN installer packages.

use crate::os_support::{GuestOs, PackageFamily};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

const DEFAULT_REPO: &str = "Control-Panel-Network/CPN-Control-Panel-Network";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    pub content_type: String,
    pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePackageKind {
    Rpm,
    Deb,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpnRelease {
    pub tag_name: String,
    pub version: String,
    pub name: String,
    pub published_at: String,
    pub prerelease: bool,
    pub draft: bool,
    pub html_url: String,
    pub assets: Vec<ReleaseAsset>,
    /// Legacy/UI summary field. Maintenance code must use compatible_package_asset().
    pub rpm_asset: Option<ReleaseAsset>,
    pub binary_asset: Option<ReleaseAsset>,
    pub checksums_asset: Option<ReleaseAsset>,
    pub checksums_asc_asset: Option<ReleaseAsset>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionCheck {
    pub running_version: String,
    pub installed_version: String,
    pub latest_version: Option<String>,
    pub latest_tag: Option<String>,
    pub update_available: bool,
    pub downgrade_possible: bool,
    pub repo: String,
    pub source: String,
    pub releases: Vec<CpnRelease>,
    pub error: Option<String>,
    #[serde(default)]
    pub from_cache: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_age_secs: Option<u64>,
    #[serde(default)]
    pub rate_limited: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after_secs: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_note: Option<String>,
}

pub fn github_repo() -> String {
    std::env::var("CPN_GITHUB_REPO").unwrap_or_else(|_| DEFAULT_REPO.into())
}

pub fn package_source_label() -> String {
    std::env::var("CPN_PACKAGE_SOURCE").unwrap_or_else(|_| "github-releases".into())
}

pub fn normalize_version(raw: &str) -> String {
    raw.trim().trim_start_matches('v').to_string()
}

/// Compare dotted numeric versions (`1.2.3` vs `1.2.10`). Non-numeric segments sort as 0.
pub fn compare_versions(left: &str, right: &str) -> Ordering {
    let parse = |value: &str| -> Vec<u64> {
        normalize_version(value)
            .split(|ch: char| !ch.is_ascii_digit())
            .filter(|part| !part.is_empty())
            .filter_map(|part| part.parse::<u64>().ok())
            .collect()
    };
    let left_parts = parse(left);
    let right_parts = parse(right);
    let len = left_parts.len().max(right_parts.len());
    for index in 0..len {
        let left_part = left_parts.get(index).copied().unwrap_or(0);
        let right_part = right_parts.get(index).copied().unwrap_or(0);
        match left_part.cmp(&right_part) {
            Ordering::Equal => continue,
            other => return other,
        }
    }
    Ordering::Equal
}

/// Retired CPN package identities that were retagged on GitHub as `0.2.x-alpha.*`.
/// RPM/semver still treat `1.0.0`/`1.0.1` as newer than `0.2.6`, which blocks upgrades.
pub fn is_retired_cpn_1_0_identity(version: &str) -> bool {
    matches!(normalize_version(version).as_str(), "1.0.0" | "1.0.1")
}

/// Current alpha product line after the 1.0.x retag (`0.2.x`, including prerelease tags).
pub fn is_active_0_2_line(version: &str) -> bool {
    normalize_version(version).starts_with("0.2.")
}

/// Official upgrade may replace leftover `1.0.0`/`1.0.1` installs with published `0.2.x`.
pub fn is_retag_migration(installed: &str, target: &str) -> bool {
    is_retired_cpn_1_0_identity(installed) && is_active_0_2_line(target)
}

/// Expand compact RPM Release prerelease tokens back toward Cargo form.
/// `alpha21` -> `alpha.21`, `beta3` -> `beta.3`, `rc1` -> `rc.1`.
pub fn expand_compact_prerelease(compact: &str) -> String {
    let compact = compact.trim();
    for prefix in ["alpha", "beta", "rc"] {
        if let Some(rest) = compact.strip_prefix(prefix)
            && !rest.is_empty()
            && rest.chars().all(|c| c.is_ascii_digit())
        {
            return format!("{prefix}.{rest}");
        }
    }
    compact.to_string()
}

/// Map RPM Version + Release (from `sync-version.sh`) back to Cargo package version.
/// Example: `0.2.6` + `0.alpha21.el9` -> `0.2.6-alpha.21`.
pub fn cargo_version_from_rpm(version: &str, release: &str) -> String {
    let version = normalize_version(version);
    let mut rel = release.trim().to_string();
    // Drop dist tags (.el9, .el10, .fc41, ...).
    if let Some(idx) = rel.find(".el") {
        rel.truncate(idx);
    } else if let Some(idx) = rel.find(".fc") {
        rel.truncate(idx);
    }
    if let Some(compact) = rel.strip_prefix("0.")
        && !compact.is_empty()
        && compact != "1"
    {
        return format!("{version}-{}", expand_compact_prerelease(compact));
    }
    version
}

fn pick_rpm_asset(assets: &[serde_json::Value]) -> Option<ReleaseAsset> {
    assets.iter().find_map(|asset| {
        let name = asset.get("name")?.as_str()?.to_string();
        let lower = name.to_ascii_lowercase();
        if !(lower.contains("cpn-installer") && lower.ends_with(".rpm")) {
            return None;
        }
        asset_from_json(asset)
    })
}

fn asset_from_json(asset: &serde_json::Value) -> Option<ReleaseAsset> {
    Some(ReleaseAsset {
        name: asset.get("name")?.as_str()?.into(),
        browser_download_url: asset.get("browser_download_url")?.as_str()?.into(),
        content_type: asset
            .get("content_type")
            .and_then(|value| value.as_str())
            .unwrap_or("application/octet-stream")
            .into(),
        size: asset
            .get("size")
            .and_then(|value| value.as_u64())
            .unwrap_or(0),
    })
}

fn pick_named_asset(assets: &[serde_json::Value], exact_name: &str) -> Option<ReleaseAsset> {
    assets.iter().find_map(|asset| {
        let name = asset.get("name")?.as_str()?;
        if name == exact_name {
            asset_from_json(asset)
        } else {
            None
        }
    })
}

fn pick_binary_asset(assets: &[serde_json::Value]) -> Option<ReleaseAsset> {
    // Release now includes RPM, DEB, ZIP, signatures and SBOM assets. Only the
    // exact extensionless Linux binary is a valid raw-binary maintenance fallback.
    pick_named_asset(assets, "cpn-installer")
}

fn release_arch_names() -> Option<(&'static str, &'static str)> {
    match std::env::consts::ARCH {
        "x86_64" => Some(("x86_64", "amd64")),
        "aarch64" => Some(("aarch64", "arm64")),
        _ => None,
    }
}

fn rpm_name_matches(name: &str, major: u32, rpm_arch: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("cpn-installer-")
        && lower.ends_with(&format!(".{rpm_arch}.rpm"))
        && lower.contains(&format!(".el{major}."))
}

fn deb_name_matches(name: &str, deb_arch: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("cpn-installer_") && lower.ends_with(&format!("_{deb_arch}.deb"))
}

/// Pick only a native package compatible with the detected Linux guest.
///
/// This prevents EL10 from consuming an EL8/EL9 RPM simply because GitHub
/// returned that asset first, and prevents apt-family guests from selecting RPMs.
pub fn compatible_package_asset(
    release: &CpnRelease,
    guest: &GuestOs,
) -> Option<(ReleaseAsset, NativePackageKind)> {
    let (rpm_arch, deb_arch) = release_arch_names()?;
    match guest.family {
        PackageFamily::Dnf => release
            .assets
            .iter()
            .find(|asset| rpm_name_matches(&asset.name, guest.major, rpm_arch))
            .cloned()
            .map(|asset| (asset, NativePackageKind::Rpm)),
        PackageFamily::Apt => release
            .assets
            .iter()
            .find(|asset| deb_name_matches(&asset.name, deb_arch))
            .cloned()
            .map(|asset| (asset, NativePackageKind::Deb)),
        PackageFamily::Windows => None,
    }
}

pub(crate) fn parse_release(value: &serde_json::Value) -> Option<CpnRelease> {
    let tag_name = value.get("tag_name")?.as_str()?.trim().to_string();
    if tag_name.is_empty() {
        return None;
    }
    let assets_json = value
        .get("assets")
        .and_then(|item| item.as_array())
        .cloned()
        .unwrap_or_default();
    let rpm_asset = pick_rpm_asset(&assets_json);
    let binary_asset = pick_binary_asset(&assets_json);
    let checksums_asset = pick_named_asset(&assets_json, "SHA256SUMS");
    let checksums_asc_asset = pick_named_asset(&assets_json, "SHA256SUMS.asc");
    let assets = assets_json.iter().filter_map(asset_from_json).collect();
    let published = value
        .get("published_at")
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .chars()
        .take(10)
        .collect::<String>();
    Some(CpnRelease {
        version: normalize_version(&tag_name),
        tag_name,
        name: value
            .get("name")
            .and_then(|item| item.as_str())
            .unwrap_or("")
            .into(),
        published_at: published,
        prerelease: value
            .get("prerelease")
            .and_then(|item| item.as_bool())
            .unwrap_or(false),
        draft: value
            .get("draft")
            .and_then(|item| item.as_bool())
            .unwrap_or(false),
        html_url: value
            .get("html_url")
            .and_then(|item| item.as_str())
            .unwrap_or("")
            .into(),
        assets,
        rpm_asset,
        binary_asset,
        checksums_asset,
        checksums_asc_asset,
    })
}

pub use crate::releases_fetch::{
    find_release, list_releases, list_releases_cached, version_check, version_check_with_options,
};

#[cfg(test)]
mod tests {
    use super::{
        cargo_version_from_rpm, compare_versions, deb_name_matches, expand_compact_prerelease,
        is_active_0_2_line, is_retag_migration, is_retired_cpn_1_0_identity, normalize_version,
        rpm_name_matches,
    };
    use std::cmp::Ordering;

    #[test]
    fn normalizes_v_prefix() {
        assert_eq!(normalize_version("v0.2.0"), "0.2.0");
        assert_eq!(normalize_version("0.2.0"), "0.2.0");
    }

    #[test]
    fn compares_semver_like() {
        assert_eq!(compare_versions("0.1.0", "0.2.0"), Ordering::Less);
        assert_eq!(compare_versions("0.2.0", "0.2.0"), Ordering::Equal);
        assert_eq!(compare_versions("0.2.10", "0.2.9"), Ordering::Greater);
        assert_eq!(compare_versions("v1.0.0", "0.9.9"), Ordering::Greater);
    }

    #[test]
    fn detects_retired_1_0_retag_migration() {
        assert!(is_retired_cpn_1_0_identity("1.0.0"));
        assert!(is_retired_cpn_1_0_identity("v1.0.1"));
        assert!(!is_retired_cpn_1_0_identity("1.0.2"));
        assert!(!is_retired_cpn_1_0_identity("0.2.6-alpha.21"));
        assert!(is_active_0_2_line("0.2.6-alpha.22"));
        assert!(is_retag_migration("1.0.0", "0.2.6-alpha.21"));
        assert!(!is_retag_migration("0.2.5-alpha.19", "0.2.6-alpha.21"));
        assert!(!is_retag_migration("1.0.0", "1.0.1"));
    }

    #[test]
    fn maps_rpm_nvr_back_to_cargo_prerelease() {
        assert_eq!(
            cargo_version_from_rpm("0.2.6", "0.alpha24.el9"),
            "0.2.6-alpha.24"
        );
        assert_eq!(
            cargo_version_from_rpm("0.2.6", "0.alpha21.el10"),
            "0.2.6-alpha.21"
        );
        assert_eq!(
            cargo_version_from_rpm("0.2.6", "0.alpha22.el10"),
            "0.2.6-alpha.22"
        );
        assert_eq!(cargo_version_from_rpm("0.2.6", "1.el9"), "0.2.6");
        assert_eq!(expand_compact_prerelease("alpha21"), "alpha.21");
        assert_eq!(expand_compact_prerelease("rc1"), "rc.1");
    }

    #[test]
    fn package_names_are_major_and_arch_specific() {
        assert!(rpm_name_matches(
            "cpn-installer-0.2.2-0.alpha7.el10.x86_64.rpm",
            10,
            "x86_64"
        ));
        assert!(!rpm_name_matches(
            "cpn-installer-0.2.2-0.alpha7.el9.x86_64.rpm",
            10,
            "x86_64"
        ));
        assert!(deb_name_matches(
            "cpn-installer_0.2.2~alpha.7_amd64.deb",
            "amd64"
        ));
        assert!(!deb_name_matches(
            "cpn-installer_0.2.2~alpha.7_arm64.deb",
            "amd64"
        ));
    }
}
