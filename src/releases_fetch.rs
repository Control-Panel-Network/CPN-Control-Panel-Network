//! GitHub Releases HTTP fetch with disk cache and rate limiting.

use crate::releases::{
    CpnRelease, VersionCheck, compare_versions, github_repo, normalize_version,
    package_source_label, parse_release,
};
use std::cmp::Ordering;
use std::process::Stdio;
use tokio::process::Command;

struct GithubHttpResponse {
    status: u16,
    etag: Option<String>,
    body: String,
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
        "User-Agent: cpn-installer".into(),
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

pub async fn list_releases_cached(
    limit: usize,
    force_network: bool,
) -> Result<crate::releases_cache::ReleasesFetchResult, String> {
    use crate::releases_cache::{
        self, age_secs, cache_is_fresh, friendly_rate_limit_message, load_cache, mark_attempt,
        note_for_cached, save_cache, seconds_until_next_check, store_success,
    };

    let repo = github_repo();
    let existing = load_cache();

    if let Some(cache) = existing.as_ref() {
        if cache_is_fresh(cache, &repo) && !force_network {
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
        if force_network
            && let Some(wait) = seconds_until_next_check(cache)
        {
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
                let _ = save_cache(&updated);
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
            return Err(error);
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
        let _ = save_cache(&updated);
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

    if response.status == 403 || response.status == 429 {
        let msg = friendly_rate_limit_message(response.status);
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
            let _ = save_cache(&updated);
            return Ok(releases_cache::ReleasesFetchResult {
                releases,
                from_cache: true,
                cache_age_secs: Some(age),
                rate_limited: true,
                retry_after_secs: None,
                note: Some(note_for_cached(age, true, None)),
                soft_error: Some(msg),
            });
        }
        return Err(msg);
    }

    if response.status < 200 || response.status >= 300 {
        let msg = format!("GitHub Releases request failed (HTTP {})", response.status);
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
            let _ = save_cache(&updated);
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
        return Err(msg);
    }

    let releases = parse_releases_json(&response.body, limit)?;
    let stored = store_success(&repo, releases.clone(), response.etag, existing);
    let _ = save_cache(&stored);
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
    let releases = list_releases_cached(30, false).await?.releases;
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
    version_check_with_options(running_version, installed_version, false).await
}

pub async fn version_check_with_options(
    running_version: &str,
    installed_version: &str,
    force_network: bool,
) -> VersionCheck {
    let repo = github_repo();
    let source = package_source_label();
    match list_releases_cached(20, force_network).await {
        Ok(fetched) => {
            let latest = fetched.releases.first();
            let latest_version = latest.map(|item| item.version.clone());
            let latest_tag = latest.map(|item| item.tag_name.clone());
            let update_available = latest_version
                .as_ref()
                .map(|latest| compare_versions(installed_version, latest) == Ordering::Less)
                .unwrap_or(false);
            let downgrade_possible = fetched.releases.iter().any(|release| {
                compare_versions(&release.version, installed_version) == Ordering::Less
            });
            let error = fetched.soft_error.clone();
            VersionCheck {
                running_version: running_version.into(),
                installed_version: installed_version.into(),
                latest_version,
                latest_tag,
                update_available,
                downgrade_possible,
                repo,
                source,
                releases: fetched.releases,
                error,
                from_cache: fetched.from_cache,
                cache_age_secs: fetched.cache_age_secs,
                rate_limited: fetched.rate_limited,
                retry_after_secs: fetched.retry_after_secs,
                cache_note: fetched.note,
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
            from_cache: false,
            cache_age_secs: None,
            rate_limited: false,
            retry_after_secs: None,
            cache_note: None,
        },
    }
}
