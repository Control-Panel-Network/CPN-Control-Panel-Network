//! Maintenance job for a commit / branch-tip target (`stable`, `dev`,
//! `branch:<name>`, `<branch>@<sha>`, SHA).
//!
//! Split out of `upgrade.rs`. Shares the post-install cleanup, binary check, and
//! session finish markers with the release-package path.

use crate::installer::AppState;
use crate::manifest::{ExistingInstall, installer_bin};
use crate::model::{MaintenanceAction, MaintenanceRequest};
use crate::upgrade_reload::{schedule_apply_reload, will_schedule_reload};
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::Command;

/// Allowlisted cleanup after the new binary/package is in place. Logs every
/// removed path and preservation note to the installer transcript.
pub(crate) async fn post_install_cleanup(state: &AppState) {
    state
        .progress("testing", 86, "Cleaning stale CPN packaging/staging")
        .await;
    let cleanup = crate::upgrade_cleanup::cleanup_stale_packaging();
    for path in &cleanup.removed {
        state.log(format!("cleanup removed: {path}"), "info");
    }
    for note in cleanup.notes.iter().chain(cleanup.skipped_preserved.iter()) {
        state.log(note.clone(), "info");
    }
}

/// `cpn-installer --version` must succeed after apply.
pub(crate) async fn verify_installer_binary(state: &AppState) -> Result<(), String> {
    state
        .progress("testing", 92, "Verifying installer binary")
        .await;
    let version_ok = Command::new(installer_bin())
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|status| status.success())
        .unwrap_or(false);
    if !version_ok {
        return Err("Post-maintenance version check failed".into());
    }
    Ok(())
}

/// Apply pending panel data migrations (warn only).
pub(crate) async fn apply_migrations(state: &AppState) {
    state
        .progress("verifying", 98, "Applying panel data migrations")
        .await;
    match crate::panel_migrate::run_pending_migrations() {
        Ok(applied) => {
            for id in applied {
                state.log(format!("migration applied: {id}"), "info");
            }
        }
        Err(error) => {
            state.log(format!("Warning: panel migrations: {error}"), "error");
        }
    }
}

pub(crate) async fn run_tip_maintenance(
    state: Arc<AppState>,
    request: &MaintenanceRequest,
    tip_ref: &str,
    existing: &ExistingInstall,
) -> Result<(), String> {
    let repo = crate::releases::github_repo();
    let branch = crate::releases_stable_tip::branch_label_for_ref(tip_ref);
    state.log(
        format!(
            "Maintenance {:?}: commit target {tip_ref} from {repo} (branch {branch})",
            request.action
        ),
        "info",
    );
    let source_label = if crate::panel_service::running_under_systemd() {
        "ui"
    } else {
        "cli"
    };
    if let Err(error) = crate::panel_maintenance_mode::begin(
        source_label,
        "Updating CPN Panel",
        &format!("Installing {branch} commits {tip_ref}"),
        Some(tip_ref),
    ) {
        state.log(
            format!("Warning: could not set panel maintenance flag: {error}"),
            "error",
        );
    }
    crate::upgrade::maybe_reset_data(request.reset_data)?;
    let docker_before = crate::upgrade_verify::snapshot_cpn_docker_running();
    let tip = match crate::upgrade_tip::apply_tip_ref(&state, &repo, tip_ref).await {
        Ok(tip) => tip,
        Err(error) => {
            crate::upgrade_tip_log::log_failure(&error, None);
            state.log(error.clone(), "error");
            return Err(error);
        }
    };
    let status_snapshot = state
        .status
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    crate::upgrade_tip::record_tip_install(
        &tip,
        status_snapshot.selected_server.or(existing.selected_server),
        status_snapshot.selected_mail.or(existing.selected_mail),
    )?;
    state.log(
        format!(
            "Installed {} ({}) from source tip {}",
            tip.package_version, tip.branch_label, tip.short_sha
        ),
        "info",
    );

    post_install_cleanup(&state).await;

    if matches!(request.action, MaintenanceAction::Upgrade) && request.bypass_docker {
        state
            .progress("installing", 90, "Refreshing CPN-managed Docker (--bypass)")
            .await;
        for note in crate::upgrade_verify::maybe_refresh_cpn_docker(true) {
            state.log(note, "info");
        }
    }

    verify_installer_binary(&state).await?;

    state
        .progress("verifying", 96, "Verifying services after upgrade")
        .await;
    let binary_replaced = !tip.skipped_rebuild;
    if tip.skipped_rebuild {
        state.log(
            "Skip-rebuild tip: verifying /login without restarting the panel".to_string(),
            "info",
        );
    }
    match crate::upgrade_verify::verify_after_upgrade(
        request.bypass_docker,
        &docker_before,
        binary_replaced,
    ) {
        Ok(report) => {
            for line in report.summary_lines() {
                state.log(line, "info");
            }
        }
        Err(error) => {
            state.log(error.clone(), "error");
            return Err(error);
        }
    }

    apply_migrations(&state).await;

    let message = format!("Updated to {} ({})", tip.branch_label, tip.package_version);
    state.progress("completed", 100, message.clone()).await;
    let restart_scheduled;
    {
        let mut status = state.status.write().unwrap_or_else(|e| e.into_inner());
        status.phase = "completed";
        status.progress = 100;
        status.error = None;
        status.message = message.clone();
        // Skip-rebuild: binary unchanged; do not bounce MainPID or leave UI
        // waiting on a detached reload that never needed to run.
        status.restart_scheduled = if tip.skipped_rebuild {
            false
        } else {
            will_schedule_reload(request.action, "tip")
        };
        restart_scheduled = status.restart_scheduled;
        if let Some(info) = status.maintenance.as_mut() {
            info.installed_version = tip.package_version.clone();
            info.running_sha = Some(tip.sha.clone());
            info.stable_tip_label = Some(tip.branch_label.clone());
            info.stable_update_available = false;
            info.update_available = false;
        }
        let _ = state.events.send(crate::model::InstallerEvent::Completed {
            status: status.clone(),
        });
    }

    if tip.skipped_rebuild {
        let _ = crate::panel_maintenance_mode::clear_with_reason("completed");
        state.log(
            "Skip-rebuild tip complete; panel reload not scheduled".to_string(),
            "info",
        );
    } else {
        schedule_apply_reload(&state, request.action, "tip");
    }
    crate::upgrade_session_log::finish_session_ok(&message, restart_scheduled);
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn tip_run_finishes_session_with_done_marker() {
        let src = include_str!("upgrade_tip_run.rs");
        assert!(src.contains("finish_session_ok(&message, restart_scheduled)"));
        assert!(src.contains("post_install_cleanup(&state)"));
        assert!(!src.contains('\u{2014}'));
    }
}
