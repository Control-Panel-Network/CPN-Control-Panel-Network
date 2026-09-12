//! Direct GitHub Release asset fallback (bypasses api.github.com rate limits).
//!
//! When the Releases API returns HTTP 403/429 and no disk cache exists, CPN can still
//! resolve a tip by downloading `SHA256SUMS` from the CDN download URL:
//! `https://github.com/<repo>/releases/download/<tag>/SHA256SUMS`
//!
//! Optional env: `CPN_RELEASE_TAG` (exact tip), `CPN_GITHUB_REPO`.

use crate::releases::{CpnRelease, ReleaseAsset, github_repo, normalize_version, parse_release};
use std::process::Stdio;
use tokio::process::Command;

const TIP_CARGO_VERSION: &str = env!("CARGO_PKG_VERSION");

fn tag_candidates(wanted: Option<&str>) -> Vec<String> {
    let mut out = Vec::new();
    let push = |list: &mut Vec<String>, raw: &str| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return;
        }
        let with_v = if trimmed.starts_with('v') || trimmed.starts_with('V') {
            trimmed.to_string()
        } else {
            format!("v{}", normalize_version(trimmed))
        };
        if !list.iter().any(|item| item.eq_ignore_ascii_case(&with_v)) {
            list.push(with_v);
        }
    };
    if let Some(value) = wanted {
        push(&mut out, value);
    }
    if let Ok(value) = std::env::var("CPN_RELEASE_TAG") {
        push(&mut out, &value);
    }
    push(&mut out, TIP_CARGO_VERSION);
    // Recent published tips (newest first). Keep short; CDN probe is cheap vs API.
    for tip in [
        "v0.2.6-alpha.28",
        "v0.2.6-alpha.27",
        "v0.2.6-alpha.26",
        "v0.2.6-alpha.25",
        "v0.2.6-alpha.24",
    ] {
        push(&mut out, tip);
    }
    out
}

fn download_base(repo: &str, tag: &str) -> String {
    format!("https://github.com/{repo}/releases/download/{tag}")
}

fn asset(name: &str, base: &str) -> ReleaseAsset {
    ReleaseAsset {
        name: name.to_string(),
        browser_download_url: format!("{base}/{name}"),
        content_type: "application/octet-stream".into(),
        size: 0,
    }
}

fn parse_sha256sums_names(body: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let _hash = parts.next();
        let name = parts.next().unwrap_or("").trim_start_matches('*');
        if !name.is_empty() && !names.iter().any(|n| n == name) {
            names.push(name.to_string());
        }
    }
    names
}

async fn curl_text(url: &str) -> Result<(u16, String), String> {
    let tmp = std::env::temp_dir().join(format!("cpn-direct-sums-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    let output = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--location",
            "--max-time",
            "20",
            "-o",
            tmp.to_string_lossy().as_ref(),
            "-w",
            "%{http_code}",
            "-H",
            "User-Agent: cpn-installer",
            url,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("Could not download release asset index: {error}"))?;
    let status = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u16>()
        .unwrap_or(0);
    let body = std::fs::read_to_string(&tmp).unwrap_or_default();
    let _ = std::fs::remove_file(&tmp);
    if status == 0 && !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Direct release download failed ({})",
            stderr.trim().chars().take(160).collect::<String>()
        ));
    }
    Ok((status, body))
}

fn release_from_sums(repo: &str, tag: &str, sums_body: &str) -> Option<CpnRelease> {
    let base = download_base(repo, tag);
    let names = parse_sha256sums_names(sums_body);
    if names.is_empty() {
        return None;
    }
    let mut assets: Vec<ReleaseAsset> = names.iter().map(|n| asset(n, &base)).collect();
    if !assets.iter().any(|a| a.name == "SHA256SUMS") {
        assets.push(asset("SHA256SUMS", &base));
    }
    if !assets.iter().any(|a| a.name == "SHA256SUMS.asc") {
        assets.push(asset("SHA256SUMS.asc", &base));
    }
    if !assets.iter().any(|a| a.name == "RPM-GPG-KEY-CPN") {
        assets.push(asset("RPM-GPG-KEY-CPN", &base));
    }
    let assets_json: Vec<serde_json::Value> = assets
        .iter()
        .map(|a| {
            serde_json::json!({
                "name": a.name,
                "browser_download_url": a.browser_download_url,
                "content_type": a.content_type,
                "size": a.size,
            })
        })
        .collect();
    let value = serde_json::json!({
        "tag_name": tag,
        "name": tag,
        "published_at": "",
        "prerelease": tag.contains("alpha") || tag.contains("beta") || tag.contains("rc"),
        "draft": false,
        "html_url": format!("https://github.com/{repo}/releases/tag/{tag}"),
        "assets": assets_json,
    });
    parse_release(&value)
}

/// Probe CDN download URLs for a single release tag (no api.github.com).
pub async fn probe_tag(tag_or_version: &str) -> Result<CpnRelease, String> {
    let repo = github_repo();
    let tag = {
        let trimmed = tag_or_version.trim();
        if trimmed.starts_with('v') || trimmed.starts_with('V') {
            trimmed.to_string()
        } else {
            format!("v{}", normalize_version(trimmed))
        }
    };
    let url = format!("{}/SHA256SUMS", download_base(&repo, &tag));
    let (status, body) = curl_text(&url).await?;
    if !(200..300).contains(&status) || body.trim().is_empty() {
        return Err(format!("{tag}: HTTP {status} for SHA256SUMS"));
    }
    release_from_sums(&repo, &tag, &body)
        .ok_or_else(|| format!("{tag}: SHA256SUMS empty or unreadable"))
}

/// Probe CDN download URLs for a release tip (no api.github.com).
pub async fn probe_direct_release(wanted: Option<&str>) -> Result<CpnRelease, String> {
    let mut errors = Vec::new();
    for tag in tag_candidates(wanted) {
        match probe_tag(&tag).await {
            Ok(release) => return Ok(release),
            Err(error) => errors.push(error),
        }
    }
    Err(format!(
        "Could not resolve a release via direct download URLs ({})",
        errors.into_iter().take(4).collect::<Vec<_>>().join("; ")
    ))
}

/// Build a short release list from direct tip probes (newest first).
pub async fn list_releases_direct(limit: usize) -> Result<Vec<CpnRelease>, String> {
    let mut releases = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for tag in tag_candidates(None) {
        if releases.len() >= limit.max(1) {
            break;
        }
        match probe_tag(&tag).await {
            Ok(release) => {
                let key = normalize_version(&release.tag_name);
                if seen.insert(key) {
                    releases.push(release);
                }
            }
            Err(_) => continue,
        }
    }
    if releases.is_empty() {
        return Err(
            "No releases available via direct download URLs (set CPN_RELEASE_TAG or wait for GitHub API)."
                .into(),
        );
    }
    Ok(releases)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sums_names() {
        let body = "abc  cpn-installer\ndef *cpn-installer-0.2.6-0.alpha28.el9.x86_64.rpm\n";
        let names = parse_sha256sums_names(body);
        assert_eq!(names.len(), 2);
        assert_eq!(names[1], "cpn-installer-0.2.6-0.alpha28.el9.x86_64.rpm");
    }

    #[test]
    fn tag_candidates_include_tip() {
        let tags = tag_candidates(Some("0.2.6-alpha.28"));
        assert!(tags.iter().any(|t| t == "v0.2.6-alpha.28"));
        assert!(!tags.join(" ").contains('\u{2014}'));
    }

    #[test]
    fn release_from_sums_builds_assets() {
        let body = "aa  cpn-installer-0.2.6-0.alpha28.el9.x86_64.rpm\nbb  SHA256SUMS\n";
        let release = release_from_sums(
            "Control-Panel-Network/CPN-Control-Panel-Network",
            "v0.2.6-alpha.28",
            body,
        )
        .expect("release");
        assert_eq!(release.version, "0.2.6-alpha.28");
        assert!(release.checksums_asset.is_some());
        assert!(
            release
                .assets
                .iter()
                .any(|a| a.name.contains("el9") && a.browser_download_url.contains("/download/"))
        );
    }
}
