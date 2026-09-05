//! GitHub Releases lookup for CPN installer packages.

use crate::os_support::{GuestOs, PackageFamily};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::process::Stdio;
use tokio::process::Command;

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

async fn curl_json(url: &str) -> Result<String, String> {
    let output = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--max-time",
            "12",
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "User-Agent: cpn-installer",
            url,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("Could not query GitHub Releases: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "GitHub Releases request failed ({})",
            stderr.trim().chars().take(160).collect::<String>()
        ));
    }
    String::from_utf8(output.stdout).map_err(|error| error.to_string())
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

fn parse_release(value: &serde_json::Value) -> Option<CpnRelease> {
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

pub async fn list_releases(limit: usize) -> Result<Vec<CpnRelease>, String> {
    let repo = github_repo();
    let url = format!("https://api.github.com/repos/{repo}/releases?per_page=30");
    let body = curl_json(&url).await?;
    let value: serde_json::Value =
        serde_json::from_str(&body).map_err(|error| format!("Invalid releases JSON: {error}"))?;
    let items = value
        .as_array()
        .ok_or_else(|| "GitHub Releases response was not an array".to_string())?;
    let mut releases = items
        .iter()
        .filter_map(parse_release)
        .filter(|release| !release.draft)
        .collect::<Vec<_>>();
    releases.truncate(limit.max(1));
    Ok(releases)
}

pub async fn find_release(version_or_tag: &str) -> Result<CpnRelease, String> {
    let wanted = normalize_version(version_or_tag);
    let releases = list_releases(30).await?;
    releases
        .into_iter()
        .find(|release| {
            normalize_version(&release.version) == wanted
                || normalize_version(&release.tag_name) == wanted
                || release.tag_name == version_or_tag
                || release.tag_name == format!("v{wanted}")
        })
        .ok_or_else(|| format!("No GitHub release found for version {version_or_tag}"))
}

pub async fn version_check(running_version: &str, installed_version: &str) -> VersionCheck {
    let repo = github_repo();
    let source = package_source_label();
    match list_releases(20).await {
        Ok(releases) => {
            let latest = releases
                .iter()
                .find(|release| !release.prerelease)
                .or_else(|| releases.first());
            let latest_version = latest.map(|item| item.version.clone());
            let latest_tag = latest.map(|item| item.tag_name.clone());
            let update_available = latest_version
                .as_ref()
                .map(|latest| compare_versions(installed_version, latest) == Ordering::Less)
                .unwrap_or(false);
            let downgrade_possible = releases.iter().any(|release| {
                compare_versions(&release.version, installed_version) == Ordering::Less
            });
            VersionCheck {
                running_version: running_version.into(),
                installed_version: installed_version.into(),
                latest_version,
                latest_tag,
                update_available,
                downgrade_possible,
                repo,
                source,
                releases,
                error: None,
            }
        }
        Err(error) => VersionCheck {
            running_version: running_version.into(),
            installed_version: installed_version.into(),
            latest_version: None,
            latest_tag: None,
            update_available: false,
            downgrade_possible: false,
            repo,
            source,
            releases: Vec::new(),
            error: Some(error),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{compare_versions, deb_name_matches, normalize_version, rpm_name_matches};
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
