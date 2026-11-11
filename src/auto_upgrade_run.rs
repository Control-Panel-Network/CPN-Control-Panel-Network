//! Automatic upgrade check / apply / background worker.

use crate::auto_upgrade::{
    AutoUpgradeMode, STARTUP_DELAY_SECS, ensure_default_auto_upgrade_config, interval_secs,
    load_auto_upgrade_config, now_unix, resolve_security_release, save_auto_upgrade_config,
};
use crate::installer::AppState;
use crate::login_service_gate::login_services_ready;
use crate::manifest::detect_existing_install;
use crate::model::{MaintenanceAction, MaintenanceRequest};
use crate::os_support::detect_guest_os;
use crate::releases;
use crate::upgrade::spawn_maintenance;
use crate::upgrade_busy;
use crate::upgrade_tip_log;
use serde::Serialize;
use std::sync::Arc;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize)]
pub struct AutoUpgradeCheckReport {
    pub enabled: bool,
    pub would_upgrade: bool,
    pub applied: bool,
    pub update_available: bool,
    pub target_version: Option<String>,
    pub target_tag: Option<String>,
    pub security_match: bool,
    pub skipped_reason: Option<String>,
    pub message: String,
}

fn is_root() -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::geteuid() == 0 }
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn remember(cfg: &mut crate::auto_upgrade::AutoUpgradeConfig, ok: bool, msg: &str) {
    cfg.last_check_at = now_unix();
    cfg.last_check_ok = ok;
    cfg.last_result = msg.to_string();
    let _ = save_auto_upgrade_config(cfg);
}

/// Check for a newer installable release. When `apply` is true and the Owner
/// toggle is on, start the same upgrade path as Version Management.
pub async fn run_auto_upgrade_check(
    state: Arc<AppState>,
    apply: bool,
) -> Result<AutoUpgradeCheckReport, String> {
    let mut cfg = load_auto_upgrade_config();
    if !cfg.enabled && apply {
        let msg = "Automatic updates are off. Owner can enable them under Version Management."
            .to_string();
        remember(&mut cfg, true, &msg);
        upgrade_tip_log::log_tagged_main_only("auto_upgrade", "info", &msg);
        return Ok(AutoUpgradeCheckReport {
            enabled: false,
            would_upgrade: false,
            applied: false,
            update_available: false,
            target_version: None,
            target_tag: None,
            security_match: false,
            skipped_reason: Some("disabled".into()),
            message: msg,
        });
    }

    if apply && !is_root() {
        let msg = "Automatic upgrade requires root (cpn-installer.service).".to_string();
        upgrade_tip_log::log_tagged_main_only("auto_upgrade", "error", &msg);
        return Err(msg);
    }

    if apply && !login_services_ready() {
        let msg = "Skipped automatic upgrade: core services are not healthy (login service gate)."
            .to_string();
        remember(&mut cfg, false, &msg);
        upgrade_tip_log::log_tagged_main_only("auto_upgrade", "error", &msg);
        return Ok(AutoUpgradeCheckReport {
            enabled: cfg.enabled,
            would_upgrade: false,
            applied: false,
            update_available: false,
            target_version: None,
            target_tag: None,
            security_match: false,
            skipped_reason: Some("unhealthy".into()),
            message: msg,
        });
    }

    upgrade_busy::heal_orphaned_busy(&state);
    let phase_busy = {
        let status = state.status.read().unwrap_or_else(|e| e.into_inner());
        upgrade_busy::is_busy_phase(status.phase)
    };
    if apply && (upgrade_busy::job_in_flight() || phase_busy) {
        let msg = "Skipped automatic upgrade: another maintenance job is already in progress."
            .to_string();
        upgrade_tip_log::log_tagged_main_only("auto_upgrade", "info", &msg);
        return Ok(AutoUpgradeCheckReport {
            enabled: cfg.enabled,
            would_upgrade: false,
            applied: false,
            update_available: false,
            target_version: None,
            target_tag: None,
            security_match: false,
            skipped_reason: Some("busy".into()),
            message: msg,
        });
    }

    let existing = detect_existing_install(VERSION);
    let guest = detect_guest_os().map_err(|e| format!("Could not detect guest OS: {e}"))?;
    // Dry-run (manual Check now) may use cached releases when GitHub rate-limits;
    // scheduled apply keeps a forced refresh.
    let check =
        releases::version_check_with_options(VERSION, &existing.package_version, apply).await;
    if let Some(err) = check.error.as_ref() {
        if check.releases.is_empty() {
            let msg = format!("Automatic update check failed: {err}");
            remember(&mut cfg, false, &msg);
            upgrade_tip_log::log_tagged_main_only("auto_upgrade", "error", &msg);
            return Err(msg);
        }
        upgrade_tip_log::log_tagged_main_only(
            "auto_upgrade",
            "info",
            &format!("Release list note (using cache): {err}"),
        );
    }

    let latest = check
        .releases
        .iter()
        .find(|r| {
            !r.draft
                && (cfg.include_prerelease || !r.prerelease)
                && releases::compatible_package_asset(r, &guest).is_some()
                && releases::compare_versions(&r.version, &existing.package_version)
                    == std::cmp::Ordering::Greater
        })
        .cloned();

    let Some(target) = latest else {
        let msg = format!(
            "No newer installable release than {} (mode={:?}, prerelease={}).",
            existing.package_version, cfg.mode, cfg.include_prerelease
        );
        remember(&mut cfg, true, &msg);
        upgrade_tip_log::log_tagged_main_only("auto_upgrade", "info", &msg);
        return Ok(AutoUpgradeCheckReport {
            enabled: cfg.enabled,
            would_upgrade: false,
            applied: false,
            update_available: false,
            target_version: None,
            target_tag: None,
            security_match: false,
            skipped_reason: Some("up_to_date".into()),
            message: msg,
        });
    };

    let security_match = resolve_security_release(&target).await;
    if cfg.mode == AutoUpgradeMode::Security && !security_match {
        let msg = format!(
            "Newer release {} available, but security-only mode found no security release metadata (and no tag/title security keywords); skipped.",
            target.tag_name
        );
        remember(&mut cfg, true, &msg);
        upgrade_tip_log::log_tagged_main_only("auto_upgrade", "info", &msg);
        return Ok(AutoUpgradeCheckReport {
            enabled: cfg.enabled,
            would_upgrade: false,
            applied: false,
            update_available: true,
            target_version: Some(target.version.clone()),
            target_tag: Some(target.tag_name.clone()),
            security_match: false,
            skipped_reason: Some("not_security".into()),
            message: msg,
        });
    }

    let would = cfg.enabled;
    if !apply || !would {
        let msg = format!(
            "Update available: {} (security_match={}). Automatic apply is {}.",
            target.tag_name,
            security_match,
            if would { "armed" } else { "off" }
        );
        remember(&mut cfg, true, &msg);
        upgrade_tip_log::log_tagged_main_only("auto_upgrade", "info", &msg);
        return Ok(AutoUpgradeCheckReport {
            enabled: cfg.enabled,
            would_upgrade: would,
            applied: false,
            update_available: true,
            target_version: Some(target.version.clone()),
            target_tag: Some(target.tag_name.clone()),
            security_match,
            skipped_reason: if would {
                Some("dry_run".into())
            } else {
                Some("disabled".into())
            },
            message: msg,
        });
    }

    let msg = format!(
        "Automatic upgrade starting for {} (mode={:?}).",
        target.tag_name, cfg.mode
    );
    cfg.last_applied_version = target.version.clone();
    remember(&mut cfg, true, &msg);
    upgrade_tip_log::log_tagged_main_only("auto_upgrade", "info", &msg);

    {
        let mut current = state.status.write().unwrap_or_else(|e| e.into_inner());
        if upgrade_busy::is_busy_phase(current.phase) {
            let busy = "Skipped automatic upgrade: became busy before start.".to_string();
            upgrade_tip_log::log_tagged_main_only("auto_upgrade", "info", &busy);
            return Ok(AutoUpgradeCheckReport {
                enabled: true,
                would_upgrade: true,
                applied: false,
                update_available: true,
                target_version: Some(target.version),
                target_tag: Some(target.tag_name),
                security_match,
                skipped_reason: Some("busy".into()),
                message: busy,
            });
        }
        upgrade_busy::mark_job_started();
        current.phase = "downloading";
        current.progress = 1;
        current.error = None;
        current.message = format!("Automatic upgrade to {}", target.tag_name);
    }

    let request = MaintenanceRequest {
        action: MaintenanceAction::Upgrade,
        version: Some(target.version.clone()),
        confirm_downgrade: false,
        reset_data: false,
        confirm_execute: true,
        bypass_docker: false,
    };
    tokio::spawn(spawn_maintenance(state, request));

    Ok(AutoUpgradeCheckReport {
        enabled: true,
        would_upgrade: true,
        applied: true,
        update_available: true,
        target_version: Some(target.version),
        target_tag: Some(target.tag_name),
        security_match,
        skipped_reason: None,
        message: msg,
    })
}

/// Background worker: wait, then check on an interval while the panel runs.
pub async fn spawn_auto_upgrade_worker(state: Arc<AppState>) {
    // Fresh installs / first start: seed enabled ON + All newer releases when
    // the config file is absent. Never clobber Owner-saved OFF.
    ensure_default_auto_upgrade_config();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(STARTUP_DELAY_SECS)).await;
        loop {
            let cfg = load_auto_upgrade_config();
            let wait = interval_secs(&cfg);
            if cfg.enabled {
                match run_auto_upgrade_check(state.clone(), true).await {
                    Ok(report) => {
                        upgrade_tip_log::log_tagged_main_only(
                            "auto_upgrade",
                            "info",
                            &format!("periodic: {}", report.message),
                        );
                    }
                    Err(err) => {
                        upgrade_tip_log::log_tagged_main_only(
                            "auto_upgrade",
                            "error",
                            &format!("periodic check error: {err}"),
                        );
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
        }
    });
}
