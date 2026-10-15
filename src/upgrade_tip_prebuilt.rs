//! Actions tip binaries, then safe matching GitHub Release package, before source.

use crate::installer::AppState;
use crate::manifest::{ManifestSource, installer_bin};
use crate::upgrade_tip_artifacts;
use crate::upgrade_tip_log;
use crate::upgrade_tip_release_fallback;
use std::path::Path;

/// Try GitHub Actions artifacts for `sha`, then a binary-neutral release package.
/// Returns `Some((source, package_version))` when a prebuilt path installed successfully.
pub async fn try_actions_or_release_package(
    state: &AppState,
    repo: &str,
    sha: &str,
    work: &Path,
) -> Result<Option<(ManifestSource, String)>, String> {
    match upgrade_tip_artifacts::try_apply_actions_artifacts(repo, sha, work).await {
        Ok(true) => {
            if crate::upgrade_tip_verify::require_built_sha(Path::new(installer_bin()), sha).is_ok()
            {
                upgrade_tip_log::log_info(format!(
                    "Installed prebuilt tip binaries for {}",
                    crate::build_meta::short_sha(sha)
                ));
                return Ok(Some((
                    ManifestSource::Local,
                    env!("CARGO_PKG_VERSION").to_string(),
                )));
            }
            upgrade_tip_log::log_info(
                "Prebuilt tip SHA did not match; trying matching release package, then source",
            );
        }
        Ok(false) => {
            upgrade_tip_log::log_info(
                "No GitHub Actions tip binaries; trying matching release package before source build",
            );
        }
        Err(error) => {
            upgrade_tip_log::log_failure(&error, None);
            state.log(error, "error");
        }
    }

    match upgrade_tip_release_fallback::try_apply_matching_release_package(state, repo, sha).await {
        Ok(Some(pair)) => Ok(Some(pair)),
        Ok(None) => {
            upgrade_tip_log::log_info(
                "No safe matching release package for this tip; building from source",
            );
            Ok(None)
        }
        Err(error) => {
            upgrade_tip_log::log_info(format!(
                "Release package fallback failed ({error}); building from source"
            ));
            state.log(error, "info");
            Ok(None)
        }
    }
}
