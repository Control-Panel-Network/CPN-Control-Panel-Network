//! Prefer a guest-matching GitHub Release package when tip Actions artifacts
//! are missing but the tip commit does not need a new panel binary.
//!
//! Typical case: `stable` advances a few CI/docs/packaging commits after a
//! tagged release (same Cargo version). Labs should install `vX.Y.Z` RPM/DEB
//! instead of a full `cargo build --release`.

use crate::installer::AppState;
use crate::manifest::ManifestSource;
use crate::releases::{self, CpnRelease, RELEASE_LIST_LIMIT, normalize_version};
use crate::releases_cache;
use crate::upgrade_tip_log;
use std::path::Path;

fn installer_user_agent() -> String {
    format!("CPN-Installer/{}", env!("CARGO_PKG_VERSION"))
}

async fn curl_github_json(url: &str) -> Result<(u16, String), String> {
    let tmp = std::env::temp_dir().join(format!("cpn-gh-tip-fb-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    let mut args: Vec<String> = vec![
        "--silent".into(),
        "--show-error".into(),
        "--location".into(),
        "--max-time".into(),
        "25".into(),
        "-o".into(),
        tmp.to_string_lossy().into_owned(),
        "-w".into(),
        "%{http_code}".into(),
        "-H".into(),
        "Accept: application/vnd.github+json".into(),
        "-H".into(),
        format!("User-Agent: {}", installer_user_agent()),
    ];
    if let Some(token) = releases_cache::github_token() {
        args.push("-H".into());
        args.push(format!("Authorization: Bearer {token}"));
    }
    args.push(url.into());
    let output = tokio::process::Command::new("curl")
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("GitHub compare query failed: {error}"))?;
    let status = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u16>()
        .unwrap_or(0);
    let body = std::fs::read_to_string(&tmp).unwrap_or_default();
    let _ = std::fs::remove_file(&tmp);
    Ok((status, body))
}

/// Paths that do not require a new panel/installer binary for tip upgrades.
pub(crate) fn path_is_binary_neutral(path: &str) -> bool {
    let lower = path.replace('\\', "/").to_ascii_lowercase();
    let trimmed = lower.trim_start_matches("./");
    if trimmed.is_empty() {
        return false;
    }
    let prefixes = [
        ".github/",
        "docs/",
        "to-do/",
        "todo/",
        "packaging/",
        "docker/",
        "site/",
        "scripts/",
        "test/",
        "tests/",
    ];
    if prefixes.iter().any(|prefix| trimmed.starts_with(prefix)) {
        return true;
    }
    matches!(
        trimmed,
        "readme.md"
            | "changelog.md"
            | "license"
            | "license.md"
            | ".gitignore"
            | ".gitattributes"
            | ".dockerignore"
            | "code_of_conduct.md"
            | "contributing.md"
            | "security.md"
    ) || trimmed.ends_with(".md") && !trimmed.starts_with("src/") && !trimmed.starts_with("panel/")
}

pub(crate) fn tip_delta_allows_release_package(status: &str, files: &[String]) -> bool {
    match status {
        "identical" => true,
        "ahead" => !files.is_empty() && files.iter().all(|path| path_is_binary_neutral(path)),
        _ => false,
    }
}

async fn compare_tag_to_tip(
    repo: &str,
    tag_name: &str,
    tip_sha: &str,
) -> Result<(String, Vec<String>), String> {
    let url =
        format!("https://api.github.com/repos/{repo}/compare/{tag_name}...{tip_sha}?per_page=100");
    let (status, body) = curl_github_json(&url).await?;
    if status != 200 {
        return Err(format!(
            "GitHub compare {tag_name}...{} HTTP {status}",
            crate::build_meta::short_sha(tip_sha)
        ));
    }
    let value: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
    let compare_status = value
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let files = value
        .get("files")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|file| {
            file.get("filename")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect::<Vec<_>>();
    let total = value
        .get("total_commits")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    // Refuse when the file list is truncated or empty while commits exist.
    if compare_status == "ahead" && files.is_empty() && total > 0 {
        return Err(
            "GitHub compare returned commits but no file list; refusing package fallback".into(),
        );
    }
    if compare_status == "ahead" && total > 100 {
        return Err(
            "Tip is more than 100 commits ahead of the release tag; refusing package fallback"
                .into(),
        );
    }
    Ok((compare_status, files))
}

async fn fetch_tip_cargo_version(repo: &str, tip_sha: &str) -> Result<String, String> {
    let url = format!("https://api.github.com/repos/{repo}/contents/Cargo.toml?ref={tip_sha}");
    let (status, body) = curl_github_json(&url).await?;
    if status != 200 {
        return Err(format!(
            "Could not read tip Cargo.toml HTTP {status} for {}",
            crate::build_meta::short_sha(tip_sha)
        ));
    }
    let value: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
    let encoding = value.get("encoding").and_then(|v| v.as_str()).unwrap_or("");
    let content = value
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .replace('\n', "");
    if encoding != "base64" || content.is_empty() {
        return Err("Tip Cargo.toml response was not base64 content".into());
    }
    use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
    let decoded = B64
        .decode(content.as_bytes())
        .map_err(|error| format!("Tip Cargo.toml base64 decode failed: {error}"))?;
    let text = String::from_utf8_lossy(&decoded);
    parse_cargo_package_version(&text)
        .ok_or_else(|| "Tip Cargo.toml has no package version".to_string())
}

fn parse_cargo_package_version(toml: &str) -> Option<String> {
    let mut in_package = false;
    for line in toml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("version") {
            let rest = rest.trim().trim_start_matches('=').trim();
            let value = rest.trim_matches('"').trim_matches('\'').trim();
            if !value.is_empty() {
                return Some(normalize_version(value));
            }
        }
    }
    None
}

fn release_matches_tip_version(release: &CpnRelease, tip_version: &str) -> bool {
    normalize_version(&release.version) == normalize_version(tip_version)
}

/// When Actions tip artifacts are missing, install the newest publishable release
/// package if tip Cargo version matches and tip-only changes are binary-neutral.
pub async fn try_apply_matching_release_package(
    state: &AppState,
    repo: &str,
    tip_sha: &str,
) -> Result<Option<(ManifestSource, String)>, String> {
    let tip_version = match fetch_tip_cargo_version(repo, tip_sha).await {
        Ok(v) => v,
        Err(error) => {
            upgrade_tip_log::log_info(format!("Release package fallback skipped ({error})"));
            return Ok(None);
        }
    };
    let fetched =
        crate::releases_fetch::list_releases_for_repo_opts(repo, RELEASE_LIST_LIMIT, true, true)
            .await
            .map_err(|error| format!("Could not list releases for tip fallback: {error}"))?;
    let Some(release) = releases::pick_newest_publishable_release(&fetched.releases)
        .cloned()
        .filter(|item| release_matches_tip_version(item, &tip_version))
    else {
        upgrade_tip_log::log_info(format!(
            "No publishable GitHub Release matches tip Cargo version {tip_version}; building from source"
        ));
        return Ok(None);
    };
    let guest = match crate::os_support::detect_guest_os() {
        Ok(guest) => guest,
        Err(_) => {
            upgrade_tip_log::log_info(
                "Could not detect guest OS for release package fallback; building from source",
            );
            return Ok(None);
        }
    };
    if releases::compatible_package_asset(&release, &guest).is_none() {
        upgrade_tip_log::log_info(format!(
            "Release {} has no guest-matching package for this host; building from source",
            release.tag_name
        ));
        return Ok(None);
    }

    let (compare_status, files) = match compare_tag_to_tip(repo, &release.tag_name, tip_sha).await {
        Ok(pair) => pair,
        Err(error) => {
            upgrade_tip_log::log_info(format!("Release package fallback skipped ({error})"));
            return Ok(None);
        }
    };
    if !tip_delta_allows_release_package(&compare_status, &files) {
        upgrade_tip_log::log_info(format!(
            "Tip {} vs {} is {compare_status} with non-package-safe changes; building from source",
            crate::build_meta::short_sha(tip_sha),
            release.tag_name
        ));
        return Ok(None);
    }

    upgrade_tip_log::log_info(format!(
        "Tip {} has no Actions artifacts; installing matching release {} (Cargo {tip_version}; delta is binary-neutral)",
        crate::build_meta::short_sha(tip_sha),
        release.tag_name
    ));
    state
        .progress(
            "downloading",
            18,
            format!(
                "Installing release {} package for tip {}",
                release.tag_name,
                crate::build_meta::short_sha(tip_sha)
            ),
        )
        .await;
    let source = crate::upgrade_apply::apply_release(state, &release, false, false).await?;
    let installer = Path::new(crate::manifest::installer_bin());
    crate::build_meta::overwrite_embedded_sha(installer, tip_sha)?;
    crate::upgrade_tip_verify::require_built_sha(installer, tip_sha)?;
    upgrade_tip_log::log_info(format!(
        "Installed release {} package and stamped tip SHA {}",
        release.tag_name,
        crate::build_meta::short_sha(tip_sha)
    ));
    Ok(Some((source, tip_version)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_neutral_paths() {
        assert!(path_is_binary_neutral(".github/workflows/docker-hub.yml"));
        assert!(path_is_binary_neutral("docs/CHANGELOG.md"));
        assert!(path_is_binary_neutral("packaging/cpn-installer.spec"));
        assert!(path_is_binary_neutral("README.md"));
        assert!(!path_is_binary_neutral("src/upgrade_tip.rs"));
        assert!(!path_is_binary_neutral("Cargo.toml"));
        assert!(!path_is_binary_neutral("Cargo.lock"));
        assert!(!path_is_binary_neutral("build.rs"));
    }

    #[test]
    fn delta_rules() {
        assert!(tip_delta_allows_release_package("identical", &[]));
        assert!(tip_delta_allows_release_package(
            "ahead",
            &[
                ".github/workflows/docker-hub.yml".into(),
                "docs/CHANGELOG.md".into(),
                "packaging/cpn-installer.spec".into(),
            ]
        ));
        assert!(!tip_delta_allows_release_package(
            "ahead",
            &["src/auth_api.rs".into()]
        ));
        assert!(!tip_delta_allows_release_package("diverged", &[]));
        assert!(!tip_delta_allows_release_package("ahead", &[]));
    }

    #[test]
    fn parses_cargo_version() {
        let toml = "[package]\nname = \"cpn-installer\"\nversion = \"1.4.3\"\n";
        assert_eq!(parse_cargo_package_version(toml).as_deref(), Some("1.4.3"));
    }
}
