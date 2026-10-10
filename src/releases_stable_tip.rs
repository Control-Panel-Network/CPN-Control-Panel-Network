//! Fetch configured-repo branch tip (default `stable`) for commit-aware updates.
//!
//! The branch comes from the saved update source (`/var/lib/cpn/update-source.json`,
//! Version Management > Update source > Branch), env `CPN_UPDATE_BRANCH` /
//! `CPN_STABLE_BRANCH` override it. `stable` is production; `dev` is lab only.

use crate::build_meta::{self, sha_equal, short_sha};
use crate::releases::github_repo;
use crate::releases_cache;
use crate::releases_source::{DEV_UPDATE_BRANCH, normalize_branch};
use std::process::Stdio;
use tokio::process::Command;

const GITHUB_API_VERSION: &str = "2022-11-28";

#[derive(Debug, Clone, Default)]
pub struct StableTipInfo {
    pub branch: String,
    pub sha: String,
    pub short_sha: String,
    pub label: String,
    pub error: Option<String>,
}

/// Branch followed by "Upgrade to latest commits" (saved setting or env override).
/// Name kept for existing callers; it is the configured update branch, not always `stable`.
pub fn stable_branch() -> String {
    crate::releases_source::configured_update_branch()
}

/// Alias with the newer name.
pub fn update_branch() -> String {
    stable_branch()
}

/// Branch name to show for a commit target: a branch ref is shown as-is, a
/// pinned SHA falls back to the configured update branch.
pub fn branch_label_for_ref(git_ref: &str) -> String {
    let trimmed = git_ref.trim();
    if trimmed.is_empty() || build_meta::looks_like_git_sha(trimmed) {
        return stable_branch();
    }
    trimmed.to_string()
}

fn installer_user_agent() -> String {
    format!("CPN-Installer/{}", env!("CARGO_PKG_VERSION"))
}

pub(crate) async fn curl_github_json(url: &str) -> Result<(u16, String), String> {
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

/// Tip of the configured update branch (`stable` unless the operator picked another).
pub async fn fetch_stable_tip(repo: &str) -> StableTipInfo {
    fetch_branch_tip(repo, &stable_branch()).await
}

/// Tip of an explicit branch.
pub async fn fetch_branch_tip(repo: &str, branch: &str) -> StableTipInfo {
    let branch = branch.trim().to_string();
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

/// Map a commit-style target to a git ref for `apply_tip_ref`.
///
/// Accepted forms (release tags such as `v1.4.0` return `None`):
/// - `tip` / `commits`: the configured update branch (saved on Version Management)
/// - `stable` / `dev`: that branch literally
/// - `branch:<name>` or `branch=<name>`: any valid branch name
/// - `<branch>@<sha>`: pinned commit (the UI sends this after a tip check);
///   an invalid SHA part falls back to `<branch>`
/// - a bare git SHA (7 to 40 hex chars)
pub fn parse_tip_upgrade_target(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.eq_ignore_ascii_case("tip") || trimmed.eq_ignore_ascii_case("commits") {
        return Some(stable_branch());
    }
    if trimmed.eq_ignore_ascii_case("stable") {
        return Some(crate::releases_source::DEFAULT_UPDATE_BRANCH.to_string());
    }
    if trimmed.eq_ignore_ascii_case(DEV_UPDATE_BRANCH) {
        return Some(DEV_UPDATE_BRANCH.to_string());
    }
    if let Some(rest) = trimmed
        .strip_prefix("branch:")
        .or_else(|| trimmed.strip_prefix("branch="))
    {
        if rest.trim().is_empty() {
            return None;
        }
        return normalize_branch(rest).ok();
    }
    if let Some((branch_part, sha_part)) = trimmed.split_once('@') {
        let branch = normalize_branch(branch_part).ok()?;
        if branch_part.trim().is_empty() {
            return None;
        }
        let sha = sha_part.trim();
        if build_meta::looks_like_git_sha(sha) {
            return Some(sha.to_string());
        }
        return Some(branch);
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
        assert!(parse_tip_upgrade_target("1.4.0").is_none());
        assert!(parse_tip_upgrade_target("").is_none());
    }

    #[test]
    fn parses_dev_and_named_branch_targets() {
        assert_eq!(parse_tip_upgrade_target("dev").as_deref(), Some("dev"));
        assert_eq!(parse_tip_upgrade_target("DEV").as_deref(), Some("dev"));
        assert_eq!(
            parse_tip_upgrade_target("dev@1b2b7090").as_deref(),
            Some("1b2b7090")
        );
        assert_eq!(
            parse_tip_upgrade_target("dev@not-a-sha").as_deref(),
            Some("dev"),
            "invalid sha part falls back to the branch"
        );
        assert_eq!(
            parse_tip_upgrade_target("branch:feat/x-1").as_deref(),
            Some("feat/x-1")
        );
        assert_eq!(
            parse_tip_upgrade_target("branch=release/2.0").as_deref(),
            Some("release/2.0")
        );
        assert!(parse_tip_upgrade_target("branch:").is_none());
        assert!(parse_tip_upgrade_target("branch:a..b").is_none());
        assert!(parse_tip_upgrade_target("@abc1234").is_none());
        assert!(parse_tip_upgrade_target("bad name@abc1234").is_none());
        assert_eq!(
            parse_tip_upgrade_target("a7dc8f9638b262f7a9bef691da2e6f5ead8186bb").as_deref(),
            Some("a7dc8f9638b262f7a9bef691da2e6f5ead8186bb")
        );
    }

    #[test]
    fn branch_label_for_ref_prefers_branch_names() {
        assert_eq!(branch_label_for_ref("dev"), "dev");
        assert_eq!(branch_label_for_ref("feat/x"), "feat/x");
        // A pinned SHA cannot name its branch; the configured branch is used.
        let configured = stable_branch();
        assert_eq!(branch_label_for_ref("a7dc8f9"), configured);
        assert_eq!(branch_label_for_ref(""), configured);
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
