//! GitHub Releases HTTP fetch with disk cache and rate limiting.

use crate::releases::{CpnRelease, github_repo, normalize_version, parse_release};
use std::process::Stdio;
use tokio::process::Command;

const GITHUB_API_VERSION: &str = "2022-11-28";

struct GithubHttpResponse {
    status: u16,
    etag: Option<String>,
    body: String,
}

fn installer_user_agent() -> String {
    format!("CPN-Installer/{}", env!("CARGO_PKG_VERSION"))
}

fn http_error_message(status: u16, repo: &str) -> String {
    match status {
        401 => "GitHub Releases returned HTTP 401 (bad or missing token). Check /var/lib/cpn/secrets/github-token or CPN_GITHUB_TOKEN.".into(),
        404 => format!(
            "GitHub Releases returned HTTP 404 (repo not found or private without token). Repo: {repo}"
        ),
        403 => crate::releases_cache::friendly_rate_limit_message(403),
        429 => crate::releases_cache::friendly_rate_limit_message(429),
        other => format!("GitHub Releases request failed (HTTP {other}) for repo {repo}"),
    }
}

async fn curl_github_releases(url: &str, etag: Option<&str>) -> Result<GithubHttpResponse, String> {
    let tmp = std::env::temp_dir();
    let body_path = tmp.join(format!("cpn-gh-body-{}.json", std::process::id()));
    let hdr_path = tmp.join(format!("cpn-gh-hdr-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&body_path);
    let _ = std::fs::remove_file(&hdr_path);

    let mut args: Vec<String> = vec![
        "--silent".into(),
        "--show-error".into(),
        "--location".into(),
        "--max-time".into(),
        "12".into(),
        "-D".into(),
        hdr_path.to_string_lossy().into_owned(),
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
    if let Some(token) = crate::releases_cache::github_token() {
        args.push("-H".into());
        args.push(format!("Authorization: Bearer {token}"));
    }
    if let Some(tag) = etag.filter(|v| !v.is_empty()) {
        args.push("-H".into());
        args.push(format!("If-None-Match: {tag}"));
    }
    args.push(url.into());

    let output = Command::new("curl")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("Could not query GitHub Releases: {error}"))?;

    let status = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u16>()
        .unwrap_or(0);
    if status == 0 && !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = std::fs::remove_file(&body_path);
        let _ = std::fs::remove_file(&hdr_path);
        return Err(format!(
            "GitHub Releases request failed ({})",
            stderr.trim().chars().take(160).collect::<String>()
        ));
    }

    let body = std::fs::read_to_string(&body_path).unwrap_or_default();
    let headers = std::fs::read_to_string(&hdr_path).unwrap_or_default();
    let _ = std::fs::remove_file(&body_path);
    let _ = std::fs::remove_file(&hdr_path);

    let mut etag_out = None;
    for line in headers.lines() {
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("etag")
        {
            etag_out = Some(value.trim().to_string());
            break;
        }
    }
    Ok(GithubHttpResponse {
        status,
        etag: etag_out,
        body,
    })
}

fn parse_releases_json(body: &str, limit: usize) -> Result<Vec<CpnRelease>, String> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|error| format!("Invalid releases JSON: {error}"))?;
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

async fn direct_fallback_result(
    limit: usize,
    repo: &str,
    existing: Option<crate::releases_cache::ReleasesCacheFile>,
    api_error: &str,
) -> crate::releases_cache::ReleasesFetchResult {
    use crate::releases_cache::{mark_attempt, save_cache_for, store_success};

    match crate::releases_direct::list_releases_direct_for_repo(repo, limit.clamp(1, 5)).await {
        Ok(releases) => {
            let stored = store_success(repo, releases.clone(), None, existing);
            let _ = save_cache_for(repo, &stored);
            crate::releases_cache::ReleasesFetchResult {
                releases,
                from_cache: false,
                cache_age_secs: Some(0),
                rate_limited: api_error.contains("403") || api_error.contains("429"),
                retry_after_secs: None,
                note: Some(format!(
                    "GitHub API unavailable for {repo}; resolved tip via direct release download URLs."
                )),
                soft_error: Some(api_error.to_string()),
            }
        }
        Err(direct_error) => {
            if let Some(mut cache) = existing {
                mark_attempt(&mut cache);
                cache.last_error = Some(format!("{api_error}; {direct_error}"));
                let _ = save_cache_for(repo, &cache);
            }
            crate::releases_cache::ReleasesFetchResult {
                releases: Vec::new(),
                from_cache: false,
                cache_age_secs: None,
                rate_limited: api_error.contains("403") || api_error.contains("429"),
                retry_after_secs: None,
                note: None,
                soft_error: Some(format!("{api_error} Direct fallback: {direct_error}")),
            }
        }
    }
}

pub async fn list_releases_for_repo(
    repo: &str,
    limit: usize,
    force_network: bool,
) -> Result<crate::releases_cache::ReleasesFetchResult, String> {
    use crate::releases_cache::{
        self, age_secs, cache_is_fresh, load_cache_for, mark_attempt, note_for_cached,
        save_cache_for, seconds_until_next_check, store_success,
    };

    let repo = repo.trim();
    if repo.is_empty() {
        return Err("GitHub repo is empty.".into());
    }
    let existing = load_cache_for(repo);

    if let Some(cache) = existing.as_ref() {
        if cache_is_fresh(cache, repo) && !force_network {
            let age = age_secs(cache);
            let mut releases = cache.releases.clone();
            releases.truncate(limit.max(1));
            return Ok(releases_cache::ReleasesFetchResult {
                releases,
                from_cache: true,
                cache_age_secs: Some(age),
                rate_limited: false,
                retry_after_secs: None,
                note: Some(note_for_cached(age, false, None)),
                soft_error: None,
            });
        }
        if force_network && let Some(wait) = seconds_until_next_check(cache) {
            let age = age_secs(cache);
            let mut releases = cache.releases.clone();
            releases.truncate(limit.max(1));
            return Ok(releases_cache::ReleasesFetchResult {
                releases,
                from_cache: true,
                cache_age_secs: Some(age),
                rate_limited: true,
                retry_after_secs: Some(wait),
                note: Some(note_for_cached(age, true, Some(wait))),
                soft_error: None,
            });
        }
    }

    let url = format!("https://api.github.com/repos/{repo}/releases?per_page=30");
    let etag = existing.as_ref().and_then(|c| c.etag.clone());
    let response = match curl_github_releases(&url, etag.as_deref()).await {
        Ok(resp) => resp,
        Err(error) => {
            if let Some(cache) = existing.as_ref()
                && !cache.releases.is_empty()
                && cache.repo == repo
            {
                let age = age_secs(cache);
                let mut releases = cache.releases.clone();
                releases.truncate(limit.max(1));
                let mut updated = cache.clone();
                mark_attempt(&mut updated);
                updated.last_error = Some(error.clone());
                let _ = save_cache_for(repo, &updated);
                return Ok(releases_cache::ReleasesFetchResult {
                    releases,
                    from_cache: true,
                    cache_age_secs: Some(age),
                    rate_limited: false,
                    retry_after_secs: None,
                    note: Some(note_for_cached(age, false, None)),
                    soft_error: Some(error),
                });
            }
            return Ok(direct_fallback_result(limit, repo, existing, &error).await);
        }
    };

    if response.status == 304
        && let Some(cache) = existing.as_ref()
    {
        let mut updated = cache.clone();
        mark_attempt(&mut updated);
        updated.fetched_at_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(updated.fetched_at_unix);
        updated.last_error = None;
        if let Some(tag) = response.etag.clone() {
            updated.etag = Some(tag);
        }
        let _ = save_cache_for(repo, &updated);
        let mut releases = updated.releases.clone();
        releases.truncate(limit.max(1));
        return Ok(releases_cache::ReleasesFetchResult {
            releases,
            from_cache: true,
            cache_age_secs: Some(0),
            rate_limited: false,
            retry_after_secs: None,
            note: Some("Release list unchanged (HTTP 304); cache refreshed.".into()),
            soft_error: None,
        });
    }

    if response.status == 403
        || response.status == 429
        || response.status == 401
        || response.status == 404
    {
        let msg = http_error_message(response.status, repo);
        if let Some(cache) = existing.as_ref()
            && !cache.releases.is_empty()
            && cache.repo == repo
        {
            let age = age_secs(cache);
            let mut releases = cache.releases.clone();
            releases.truncate(limit.max(1));
            let mut updated = cache.clone();
            mark_attempt(&mut updated);
            updated.last_error = Some(msg.clone());
            let _ = save_cache_for(repo, &updated);
            return Ok(releases_cache::ReleasesFetchResult {
                releases,
                from_cache: true,
                cache_age_secs: Some(age),
                rate_limited: matches!(response.status, 403 | 429),
                retry_after_secs: None,
                note: Some(note_for_cached(
                    age,
                    matches!(response.status, 403 | 429),
                    None,
                )),
                soft_error: Some(msg),
            });
        }
        return Ok(direct_fallback_result(limit, repo, existing, &msg).await);
    }

    if response.status < 200 || response.status >= 300 {
        let msg = http_error_message(response.status, repo);
        if let Some(cache) = existing.as_ref()
            && !cache.releases.is_empty()
            && cache.repo == repo
        {
            let age = age_secs(cache);
            let mut releases = cache.releases.clone();
            releases.truncate(limit.max(1));
            let mut updated = cache.clone();
            mark_attempt(&mut updated);
            updated.last_error = Some(msg.clone());
            let _ = save_cache_for(repo, &updated);
            return Ok(releases_cache::ReleasesFetchResult {
                releases,
                from_cache: true,
                cache_age_secs: Some(age),
                rate_limited: false,
                retry_after_secs: None,
                note: Some(note_for_cached(age, false, None)),
                soft_error: Some(msg),
            });
        }
        return Ok(direct_fallback_result(limit, repo, existing, &msg).await);
    }

    let releases = parse_releases_json(&response.body, limit)?;
    let stored = store_success(repo, releases.clone(), response.etag, existing);
    let _ = save_cache_for(repo, &stored);
    Ok(releases_cache::ReleasesFetchResult {
        releases,
        from_cache: false,
        cache_age_secs: Some(0),
        rate_limited: false,
        retry_after_secs: None,
        note: None,
        soft_error: None,
    })
}

pub async fn list_releases_cached(
    limit: usize,
    force_network: bool,
) -> Result<crate::releases_cache::ReleasesFetchResult, String> {
    list_releases_for_repo(&github_repo(), limit, force_network).await
}

pub async fn list_releases(limit: usize) -> Result<Vec<CpnRelease>, String> {
    let fetched = list_releases_cached(limit, false).await?;
    if fetched.releases.is_empty() {
        if let Some(error) = fetched.soft_error.or(fetched.note) {
            return Err(error);
        }
        return Err("No GitHub releases available (empty list and no cache).".into());
    }
    Ok(fetched.releases)
}

pub async fn find_release(version_or_tag: &str) -> Result<CpnRelease, String> {
    let wanted = normalize_version(version_or_tag);
    let repo = github_repo();
    let releases = list_releases_for_repo(&repo, 30, false).await?.releases;
    if let Some(found) = releases.into_iter().find(|release| {
        normalize_version(&release.version) == wanted
            || normalize_version(&release.tag_name) == wanted
            || release.tag_name == version_or_tag
            || release.tag_name == format!("v{wanted}")
    }) {
        return Ok(found);
    }
    crate::releases_direct::probe_direct_release_for_repo(&repo, Some(version_or_tag))
        .await
        .map_err(|error| format!("No GitHub release found for version {version_or_tag}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_errors_are_actionable() {
        let msg = http_error_message(401, "Acme/Fork");
        assert!(msg.contains("401"));
        assert!(msg.contains("token"));
        let msg404 = http_error_message(404, "Acme/Missing");
        assert!(msg404.contains("404"));
        assert!(msg404.contains("Acme/Missing"));
        assert!(!msg.contains('\u{2014}'));
    }

    #[test]
    fn user_agent_includes_version() {
        assert!(installer_user_agent().starts_with("CPN-Installer/"));
    }
}
