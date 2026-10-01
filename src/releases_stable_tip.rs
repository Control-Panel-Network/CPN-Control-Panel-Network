//! Fetch configured-repo branch tip (default `stable`) for commit-aware updates.

use crate::build_meta::{self, sha_equal, short_sha};
use crate::releases::github_repo;
use crate::releases_cache;
use std::process::Stdio;
use tokio::process::Command;

const GITHUB_API_VERSION: &str = "2022-11-28";
const DEFAULT_STABLE_BRANCH: &str = "stable";

#[derive(Debug, Clone, Default)]
pub struct StableTipInfo {
    pub branch: String,
    pub sha: String,
    pub short_sha: String,
    pub label: String,
    pub error: Option<String>,
}

pub fn stable_branch() -> String {
    std::env::var("CPN_STABLE_BRANCH")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_STABLE_BRANCH.to_string())
}

fn installer_user_agent() -> String {
    format!("CPN-Installer/{}", env!("CARGO_PKG_VERSION"))
}

async fn curl_github_json(url: &str) -> Result<(u16, String), String> {
    let tmp = std::env::temp_dir();
    let body_path = tmp.join(format!("cpn-gh-tip-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&body_path);
    let mut args: Vec<String> = vec![
        "--silent".into(),
        "--show-error".into(),
        "--location".into(),
        "--max-time".into(),
        "12".into(),
        "-o".into(),
        body_path.to_string_lossy().into_owned(),
        "-w".into(),
        "%{http_code}".into(),
        "-H".into(),
        "Accept: application/vnd.github+json".into(),
        "-H".into(),
        format!("User-Agent: {}", installer_user_agent()),
        "-H".into(),
        format!("X-GitHub-Api-Version: {GITHUB_API_VERSION}"),
    ];
    if let Some(token) = releases_cache::github_token() {
        args.push("-H".into());
        args.push(format!("Authorization: Bearer {token}"));
    }
    args.push(url.into());
    let output = Command::new("curl")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("Could not query GitHub commit tip: {error}"))?;
    let status = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u16>()
        .unwrap_or(0);
    let body = std::fs::read_to_string(&body_path).unwrap_or_default();
    let _ = std::fs::remove_file(&body_path);
    if status == 0 && !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "GitHub tip request failed ({})",
            stderr.trim().chars().take(160).collect::<String>()
        ));
    }
    Ok((status, body))
}

fn urlencoding_lite(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => out.push(ch),
            _ => {
                for byte in ch.to_string().as_bytes() {
                    out.push_str(&format!("%{byte:02X}"));
                }
            }
        }
    }
    out
}

pub async fn resolve_ref_sha(repo: &str, git_ref: &str) -> Result<String, String> {
    let trimmed = git_ref.trim().trim_start_matches("refs/heads/");
    if trimmed.is_empty() {
        return Err("Empty git ref".into());
    }
    let encoded = urlencoding_lite(trimmed);
    let url = format!("https://api.github.com/repos/{repo}/commits/{encoded}");
    let (status, body) = curl_github_json(&url).await?;
    if status != 200 {
        return Err(format!(
            "GitHub commits API returned HTTP {status} for {repo}@{trimmed}"
        ));
    }
    let value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|error| format!("Could not parse GitHub commit JSON: {error}"))?;
    let sha = value
        .get("sha")
        .and_then(|item| item.as_str())
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .ok_or_else(|| format!("GitHub commit response missing sha for {repo}@{trimmed}"))?;
    Ok(sha.to_string())
}

pub async fn fetch_stable_tip(repo: &str) -> StableTipInfo {
    let branch = stable_branch();
    match resolve_ref_sha(repo, &branch).await {
        Ok(sha) => {
            let short = short_sha(&sha);
            StableTipInfo {
                label: format!("{branch} @ {short}"),
                branch,
                short_sha: short,
                sha,
                error: None,
            }
        }
        Err(error) => StableTipInfo {
            branch,
            sha: String::new(),
            short_sha: String::new(),
            label: String::new(),
            error: Some(error),
        },
    }
}

pub async fn resolve_running_sha(
    repo: &str,
    installed_version: &str,
    installed_tag: &str,
    manifest_source_commit: Option<&str>,
) -> (Option<String>, String) {
    if let Some(embedded) = Some(build_meta::embedded_git_sha()).filter(|s| !s.is_empty()) {
        return (Some(embedded.to_string()), "build".into());
    }
    if let Some(manifest) = manifest_source_commit
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return (Some(manifest.to_string()), "manifest".into());
    }
    let candidates = [
        installed_tag.trim(),
        installed_version.trim(),
        &format!("v{}", installed_version.trim().trim_start_matches('v')),
    ];
    for candidate in candidates {
        if candidate.is_empty() {
            continue;
        }
        if let Ok(sha) = resolve_ref_sha(repo, candidate).await {
            return (Some(sha), "release-tag".into());
        }
    }
    (None, "unknown".into())
}

pub fn tip_update_available(running_sha: Option<&str>, tip_sha: &str) -> bool {
    let tip = tip_sha.trim();
    if tip.is_empty() {
        return false;
    }
    match running_sha.map(str::trim).filter(|s| !s.is_empty()) {
        Some(running) => !sha_equal(running, tip),
        None => false,
    }
}

pub fn parse_tip_upgrade_target(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.eq_ignore_ascii_case("stable") || trimmed.eq_ignore_ascii_case("tip") {
        return Some(stable_branch());
    }
    if let Some(rest) = trimmed
        .strip_prefix("stable@")
        .or_else(|| trimmed.strip_prefix("STABLE@"))
    {
        let sha = rest.trim();
        if build_meta::looks_like_git_sha(sha) {
            return Some(sha.to_string());
        }
        return Some(stable_branch());
    }
    if build_meta::looks_like_git_sha(trimmed) {
        return Some(trimmed.to_string());
    }
    None
}

pub fn configured_repo_for_tip() -> String {
    github_repo()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tip_targets() {
        assert_eq!(
            parse_tip_upgrade_target("stable").as_deref(),
            Some("stable")
        );
        assert_eq!(
            parse_tip_upgrade_target("stable@a7dc8f9").as_deref(),
            Some("a7dc8f9")
        );
        assert!(parse_tip_upgrade_target("v1.0.0").is_none());
    }

    #[test]
    fn tip_update_when_sha_differs() {
        assert!(tip_update_available(
            Some("f8728724a878e8cd79d7d04817be50830f2ca606"),
            "a7dc8f9638b262f7a9bef691da2e6f5ead8186bb"
        ));
        assert!(!tip_update_available(
            Some("a7dc8f9"),
            "a7dc8f9638b262f7a9bef691da2e6f5ead8186bb"
        ));
    }
}
