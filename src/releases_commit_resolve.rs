//! Resolve git refs (branch tips, tags, SHAs) without a single point of failure.
//!
//! Order: GitHub commits API (optional token), last-good disk cache,
//! `git ls-remote`, then raw `.cpn-git-sha` on the branch tip.

use crate::build_meta;
use crate::paths;
use crate::releases_cache;
use crate::releases_stable_tip::curl_github_json;
use crate::upgrade_tip_log;
use serde::{Deserialize, Serialize};
use std::fs;
use std::process::Stdio;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::process::Command;

const SCHEMA: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommitTipCacheEntry {
    pub repo: String,
    pub git_ref: String,
    pub sha: String,
    pub fetched_at_unix: u64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommitTipCacheFile {
    pub schema_version: u32,
    pub entries: Vec<CommitTipCacheEntry>,
}

#[derive(Debug, Clone)]
pub struct ResolvedRef {
    pub sha: String,
    pub source: &'static str,
    pub rate_limited: bool,
    pub note: Option<String>,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn cache_path() -> std::path::PathBuf {
    paths::default_data_dir().join("github-commit-tip-cache.json")
}

pub fn load_cache() -> CommitTipCacheFile {
    let path = cache_path();
    let Ok(raw) = fs::read_to_string(&path) else {
        return CommitTipCacheFile::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save_cache(cache: &CommitTipCacheFile) -> Result<(), String> {
    let dir = paths::default_data_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    let path = cache_path();
    let mut file = CommitTipCacheFile {
        schema_version: SCHEMA,
        entries: cache.entries.clone(),
    };
    file.entries.retain(|e| build_meta::looks_like_git_sha(&e.sha));
    if file.entries.len() > 200 {
        file.entries.sort_by_key(|e| std::cmp::Reverse(e.fetched_at_unix));
        file.entries.truncate(200);
    }
    let body = serde_json::to_string_pretty(&file)
        .map_err(|e| format!("Could not serialize commit tip cache: {e}"))?;
    fs::write(&path, format!("{body}\n"))
        .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o644));
    }
    Ok(())
}

fn cache_lookup(cache: &CommitTipCacheFile, repo: &str, git_ref: &str) -> Option<CommitTipCacheEntry> {
    cache
        .entries
        .iter()
        .find(|e| e.repo == repo && e.git_ref == git_ref && build_meta::looks_like_git_sha(&e.sha))
        .cloned()
}

fn cache_store(
    cache: &mut CommitTipCacheFile,
    repo: &str,
    git_ref: &str,
    sha: &str,
    last_error: Option<String>,
) {
    let now = now_unix();
    if let Some(existing) = cache
        .entries
        .iter_mut()
        .find(|e| e.repo == repo && e.git_ref == git_ref)
    {
        existing.sha = sha.to_string();
        existing.fetched_at_unix = now;
        existing.last_error = last_error;
        return;
    }
    cache.entries.push(CommitTipCacheEntry {
        repo: repo.to_string(),
        git_ref: git_ref.to_string(),
        sha: sha.to_string(),
        fetched_at_unix: now,
        last_error,
    });
}

pub fn friendly_commits_rate_limit(status: u16) -> String {
    let token = releases_cache::github_token().is_some();
    let token_hint = if token {
        "Token is configured but GitHub still refused the request."
    } else {
        "Add an optional token under Version Management > Update source for higher API limits."
    };
    match status {
        403 => format!(
            "GitHub commits API returned HTTP 403 for this ref ({token_hint} Showing cached or anonymous fallbacks when available.)"
        ),
        429 => "GitHub commits API returned HTTP 429 (rate limited). Using cached or anonymous fallbacks when available.".into(),
        401 => "GitHub commits API returned HTTP 401. Check the configured GitHub token.".into(),
        other => format!("GitHub commits API returned HTTP {other}."),
    }
}

pub fn upgrade_failure_hint(api_status: Option<u16>, token_configured: bool) -> String {
    let mut hints: Vec<&str> = Vec::new();
    if matches!(api_status, Some(403 | 429)) && !token_configured {
        hints.push("optional GitHub token under Version Management > Update source");
    }
    hints.push("retry in a few minutes");
    hints.push("Upgrade to latest release instead of latest commits");
    format!("Next steps: {}.", hints.join("; "))
}

fn parse_commit_sha_json(body: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(body)
        .map_err(|error| format!("Could not parse GitHub commit JSON: {error}"))?;
    let sha = value
        .get("sha")
        .and_then(|item| item.as_str())
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .ok_or_else(|| "GitHub commit response missing sha".to_string())?;
    Ok(sha.to_string())
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

async fn resolve_via_api(repo: &str, trimmed: &str) -> Result<(String, u16), (u16, String)> {
    let encoded = urlencoding_lite(trimmed);
    let url = format!("https://api.github.com/repos/{repo}/commits/{encoded}");
    let (status, body) = curl_github_json(&url)
        .await
        .map_err(|error| (0, error))?;
    if status != 200 {
        return Err((status, friendly_commits_rate_limit(status)));
    }
    parse_commit_sha_json(&body)
        .map(|sha| (sha, status))
        .map_err(|e| (status, e))
}

/// Branch-like ref (not already a full/object SHA).
fn is_branch_like_ref(trimmed: &str) -> bool {
    !build_meta::looks_like_git_sha(trimmed)
}

async fn resolve_via_ls_remote(repo: &str, branch: &str) -> Result<String, String> {
    let url = format!("https://github.com/{repo}.git");
    let refname = format!("refs/heads/{branch}");
    upgrade_tip_log::log_info(format!("Resolving {repo}@{branch} via git ls-remote (no GitHub API)"));
    let output = Command::new("git")
        .args(["ls-remote", &url, &refname])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("git ls-remote unavailable: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "git ls-remote failed: {}",
            stderr.trim().chars().take(160).collect::<String>()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let mut parts = line.split_whitespace();
        let Some(sha) = parts.next() else {
            continue;
        };
        if build_meta::looks_like_git_sha(sha) {
            return Ok(sha.to_string());
        }
    }
    Err(format!("git ls-remote returned no SHA for {refname}"))
}

async fn resolve_via_raw_marker(repo: &str, branch: &str) -> Result<String, String> {
    let url = format!("https://raw.githubusercontent.com/{repo}/{branch}/.cpn-git-sha");
    upgrade_tip_log::log_info(format!("Trying raw branch marker {url}"));
    let output = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--location",
            "--max-time",
            "15",
            "-w",
            "%{http_code}",
            &url,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("Could not fetch raw .cpn-git-sha: {e}"))?;
    let combined = String::from_utf8_lossy(&output.stdout);
    if combined.len() < 3 {
        return Err("raw .cpn-git-sha response empty".into());
    }
    let split = combined.len().saturating_sub(3);
    let body = combined[..split].trim();
    let status = combined[split..].trim().parse::<u16>().unwrap_or(0);
    if status != 200 {
        return Err(format!("raw .cpn-git-sha HTTP {status}"));
    }
    let line = body.lines().next().unwrap_or(body).trim();
    if build_meta::looks_like_git_sha(line) {
        Ok(line.to_string())
    } else {
        Err("raw .cpn-git-sha did not contain a git SHA".into())
    }
}

/// Expand short SHAs using cache when the caller passed a 7-char pin from the UI.
fn full_sha_from_cache(cache: &CommitTipCacheFile, repo: &str, git_ref: &str, short: &str) -> Option<String> {
    let entry = cache_lookup(cache, repo, git_ref)?;
    if build_meta::sha_equal(&entry.sha, short) {
        Some(entry.sha.clone())
    } else {
        None
    }
}

pub async fn resolve_ref_resilient(repo: &str, git_ref: &str) -> Result<ResolvedRef, String> {
    let trimmed = git_ref.trim().trim_start_matches("refs/heads/");
    if trimmed.is_empty() {
        return Err("Empty git ref".into());
    }
    if build_meta::looks_like_git_sha(trimmed) && trimmed.len() >= 40 {
        return Ok(ResolvedRef {
            sha: trimmed.to_string(),
            source: "pinned-sha",
            rate_limited: false,
            note: None,
        });
    }

    let mut cache = load_cache();
    let mut api_status: Option<u16> = None;
    let mut api_error: Option<String> = None;

    match resolve_via_api(repo, trimmed).await {
        Ok((sha, _status)) => {
            cache_store(&mut cache, repo, trimmed, &sha, None);
            let _ = save_cache(&cache);
            return Ok(ResolvedRef {
                sha,
                source: "github-api",
                rate_limited: false,
                note: None,
            });
        }
        Err((status, msg)) => {
            api_status = Some(status);
            api_error = Some(msg.clone());
            upgrade_tip_log::log_info(format!(
                "GitHub commits API could not resolve {repo}@{trimmed}: {msg}"
            ));
        }
    }

    if let Some(entry) = cache_lookup(&cache, repo, trimmed) {
        let age = now_unix().saturating_sub(entry.fetched_at_unix);
        upgrade_tip_log::log_info(format!(
            "Using last-good commit tip cache for {repo}@{trimmed} (about {age}s old)"
        ));
        let note = api_error.as_ref().map(|e| format!("{e} Using cached SHA."));
        let _ = cache_store(
            &mut cache,
            repo,
            trimmed,
            &entry.sha,
            api_error.clone(),
        );
        let _ = save_cache(&cache);
        return Ok(ResolvedRef {
            sha: entry.sha.clone(),
            source: "cache",
            rate_limited: matches!(api_status, Some(403 | 429)),
            note,
        });
    }

    if is_branch_like_ref(trimmed) {
        if let Ok(sha) = resolve_via_ls_remote(repo, trimmed).await {
            let note = api_error.as_ref().map(|e| format!("{e} Resolved via git ls-remote."));
            cache_store(&mut cache, repo, trimmed, &sha, api_error.clone());
            let _ = save_cache(&cache);
            return Ok(ResolvedRef {
                sha,
                source: "ls-remote",
                rate_limited: matches!(api_status, Some(403 | 429)),
                note,
            });
        }
        if let Ok(sha) = resolve_via_raw_marker(repo, trimmed).await {
            let note = api_error
                .as_ref()
                .map(|e| format!("{e} Resolved via raw .cpn-git-sha."));
            cache_store(&mut cache, repo, trimmed, &sha, api_error.clone());
            let _ = save_cache(&cache);
            return Ok(ResolvedRef {
                sha,
                source: "raw-marker",
                rate_limited: matches!(api_status, Some(403 | 429)),
                note,
            });
        }
    } else if build_meta::looks_like_git_sha(trimmed) {
        if let Some(full) = full_sha_from_cache(&cache, repo, trimmed, trimmed) {
            return Ok(ResolvedRef {
                sha: full,
                source: "cache-short-sha",
                rate_limited: matches!(api_status, Some(403 | 429)),
                note: api_error.clone(),
            });
        }
    }

    let hint = upgrade_failure_hint(api_status, releases_cache::github_token().is_some());
    Err(format!(
        "Could not resolve commit ref {trimmed} for {repo}. {} {}",
        api_error.unwrap_or_else(|| "All resolution paths failed.".into()),
        hint
    ))
}

pub async fn resolve_ref_sha(repo: &str, git_ref: &str) -> Result<String, String> {
    resolve_ref_resilient(repo, git_ref)
        .await
        .map(|resolved| resolved.sha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limit_copy_has_no_em_dash() {
        assert!(!friendly_commits_rate_limit(403).contains('\u{2014}'));
        assert!(!upgrade_failure_hint(Some(403), false).contains('\u{2013}'));
    }

    #[test]
    fn branch_like_detection() {
        assert!(is_branch_like_ref("stable"));
        assert!(!is_branch_like_ref("a7dc8f9638b262f7a9bef691da2e6f5ead8186bb"));
    }

    #[test]
    fn parse_commit_json_extracts_sha() {
        let body = r#"{"sha":"abc123def456789012345678901234567890abcd"}"#;
        let sha = parse_commit_sha_json(body).unwrap();
        assert_eq!(sha, "abc123def456789012345678901234567890abcd");
    }
}
