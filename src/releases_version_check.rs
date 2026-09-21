//! Version check assembly (configured source + optional upstream tip).

use crate::releases::{
    VersionCheck, OFFICIAL_GITHUB_REPO, compare_versions, github_repo, package_source_label,
};
use crate::releases_fetch::list_releases_for_repo;
use crate::releases_source;
use std::cmp::Ordering;

async fn upstream_tip(force_network: bool) -> (Option<String>, Option<String>) {
    if releases_source::is_official_repo(&github_repo()) {
        return (None, None);
    }
    match list_releases_for_repo(OFFICIAL_GITHUB_REPO, 1, force_network).await {
        Ok(fetched) => {
            let latest = fetched.releases.first();
            (
                latest.map(|item| item.version.clone()),
                latest.map(|item| item.tag_name.clone()),
            )
        }
        Err(_) => (None, None),
    }
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
    let using_fork = !releases_source::is_official_repo(&repo);
    let token_configured = releases_source::github_token_configured();
    let upstream_repo = OFFICIAL_GITHUB_REPO.to_string();
    match list_releases_for_repo(&repo, 20, force_network).await {
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
            let (upstream_latest_version, upstream_latest_tag) = if using_fork {
                upstream_tip(force_network).await
            } else {
                (None, None)
            };
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
                using_fork,
                token_configured,
                upstream_repo,
                upstream_latest_version,
                upstream_latest_tag,
            }
        }
        Err(error) => {
            let (upstream_latest_version, upstream_latest_tag) = if using_fork {
                upstream_tip(false).await
            } else {
                (None, None)
            };
            VersionCheck {
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
                using_fork,
                token_configured,
                upstream_repo,
                upstream_latest_version,
                upstream_latest_tag,
            }
        }
    }
}
