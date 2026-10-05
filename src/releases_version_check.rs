//! Version check assembly (configured source + upstream tip + stable commit tip).

use crate::manifest;
use crate::releases::{
    OFFICIAL_GITHUB_REPO, RELEASE_LIST_LIMIT, VersionCheck, compare_versions, github_repo,
    package_source_label, pick_newest_publishable_release, releases_for_version_picker,
};
use crate::releases_fetch::list_releases_for_repo;
use crate::releases_source;
use crate::releases_stable_tip::{self, tip_update_available};
use std::cmp::Ordering;

async fn upstream_tip(force_network: bool) -> (Option<String>, Option<String>) {
    if releases_source::is_official_repo(&github_repo()) {
        return (None, None);
    }
    match list_releases_for_repo(OFFICIAL_GITHUB_REPO, 1, force_network).await {
        Ok(fetched) => {
            let latest = pick_newest_publishable_release(&fetched.releases);
            (
                latest.map(|item| item.version.clone()),
                latest.map(|item| item.tag_name.clone()),
            )
        }
        Err(_) => (None, None),
    }
}

type TipTuple = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    bool,
    Option<String>,
);

async fn attach_stable_tip(repo: &str, installed_version: &str) -> TipTuple {
    let tip = releases_stable_tip::fetch_stable_tip(repo).await;
    let tip_error = tip.error.clone();
    if tip.sha.is_empty() {
        return (
            None,
            None,
            Some(tip.branch),
            None,
            None,
            None,
            false,
            tip_error,
        );
    }
    let existing = manifest::detect_existing_install(env!("CARGO_PKG_VERSION"));
    let tag_for_resolve = if existing.release_tag.trim().is_empty() {
        let ver = installed_version.trim().trim_start_matches('v');
        if ver.is_empty() {
            String::new()
        } else {
            format!("v{ver}")
        }
    } else {
        existing.release_tag.clone()
    };
    let (running_sha, running_source) = releases_stable_tip::resolve_running_sha(
        repo,
        installed_version,
        &tag_for_resolve,
        manifest::manifest_source_commit().as_deref(),
    )
    .await;
    let stable_update = tip_update_available(running_sha.as_deref(), &tip.sha);
    (
        running_sha,
        Some(running_source),
        Some(tip.branch),
        Some(tip.sha),
        Some(tip.short_sha),
        Some(tip.label),
        stable_update,
        tip_error,
    )
}

#[allow(clippy::too_many_arguments)]
fn fill_version_check(
    running_version: &str,
    installed_version: &str,
    latest_version: Option<String>,
    latest_tag: Option<String>,
    release_update_available: bool,
    downgrade_possible: bool,
    repo: String,
    source: String,
    releases: Vec<crate::releases::CpnRelease>,
    error: Option<String>,
    from_cache: bool,
    cache_age_secs: Option<u64>,
    rate_limited: bool,
    retry_after_secs: Option<u64>,
    cache_note: Option<String>,
    using_fork: bool,
    token_configured: bool,
    upstream_repo: String,
    upstream_latest_version: Option<String>,
    upstream_latest_tag: Option<String>,
    tip: TipTuple,
) -> VersionCheck {
    let (
        running_sha,
        running_sha_source,
        stable_branch,
        stable_tip_sha,
        stable_tip_short,
        stable_tip_label,
        stable_update_available,
        tip_check_error,
    ) = tip;
    VersionCheck {
        running_version: running_version.into(),
        installed_version: installed_version.into(),
        latest_version,
        latest_tag,
        update_available: release_update_available || stable_update_available,
        downgrade_possible,
        repo,
        source,
        releases,
        error,
        from_cache,
        cache_age_secs,
        rate_limited,
        retry_after_secs,
        cache_note,
        using_fork,
        token_configured,
        upstream_repo,
        upstream_latest_version,
        upstream_latest_tag,
        running_sha,
        running_sha_source,
        stable_tip_sha,
        stable_tip_short,
        stable_branch,
        stable_tip_label,
        stable_update_available,
        release_update_available,
        tip_check_error,
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
    let tip = attach_stable_tip(&repo, installed_version).await;
    match list_releases_for_repo(&repo, RELEASE_LIST_LIMIT, force_network).await {
        Ok(fetched) => {
            let releases = releases_for_version_picker(fetched.releases);
            let latest = pick_newest_publishable_release(&releases);
            let latest_version = latest.map(|item| item.version.clone());
            let latest_tag = latest.map(|item| item.tag_name.clone());
            let release_update_available = latest_version
                .as_ref()
                .map(|latest| compare_versions(installed_version, latest) == Ordering::Less)
                .unwrap_or(false);
            let downgrade_possible = releases.iter().any(|release| {
                compare_versions(&release.version, installed_version) == Ordering::Less
            });
            let (upstream_latest_version, upstream_latest_tag) = if using_fork {
                upstream_tip(force_network).await
            } else {
                (None, None)
            };
            fill_version_check(
                running_version,
                installed_version,
                latest_version,
                latest_tag,
                release_update_available,
                downgrade_possible,
                repo,
                source,
                releases,
                fetched.soft_error.clone(),
                fetched.from_cache,
                fetched.cache_age_secs,
                fetched.rate_limited,
                fetched.retry_after_secs,
                fetched.note,
                using_fork,
                token_configured,
                upstream_repo,
                upstream_latest_version,
                upstream_latest_tag,
                tip,
            )
        }
        Err(error) => {
            let (upstream_latest_version, upstream_latest_tag) = if using_fork {
                upstream_tip(false).await
            } else {
                (None, None)
            };
            fill_version_check(
                running_version,
                installed_version,
                None,
                None,
                false,
                false,
                repo,
                source,
                Vec::new(),
                Some(error),
                false,
                None,
                false,
                None,
                None,
                using_fork,
                token_configured,
                upstream_repo,
                upstream_latest_version,
                upstream_latest_tag,
                tip,
            )
        }
    }
}
