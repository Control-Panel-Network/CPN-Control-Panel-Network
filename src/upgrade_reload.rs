//! Detached panel reload after upgrade / repair / downgrade apply.

use crate::installer::AppState;
use crate::model::MaintenanceAction;

pub(crate) fn apply_reload_log_tag(action: MaintenanceAction, kind: &str) -> &'static str {
    match action {
        MaintenanceAction::Repair => "repair",
        MaintenanceAction::Upgrade if kind == "tip" => "upgrade_tip",
        _ => "upgrade",
    }
}

pub(crate) fn apply_reload_reason(action: MaintenanceAction, kind: &str) -> Option<&'static str> {
    match action {
        MaintenanceAction::ConfigOnly => None,
        MaintenanceAction::Repair if kind == "tip" => Some("post-tip-repair"),
        MaintenanceAction::Repair => Some("post-repair"),
        MaintenanceAction::Downgrade => Some("post-downgrade"),
        MaintenanceAction::Upgrade if kind == "tip" => Some("post-tip-upgrade"),
        MaintenanceAction::Upgrade => Some("post-upgrade"),
    }
}

pub(crate) fn will_schedule_reload(action: MaintenanceAction, kind: &str) -> bool {
    apply_reload_reason(action, kind).is_some() && crate::panel_service::running_under_systemd()
}

pub(crate) fn schedule_apply_reload(state: &AppState, action: MaintenanceAction, kind: &str) {
    let tag = apply_reload_log_tag(action, kind);
    let Some(reason) = apply_reload_reason(action, kind) else {
        let _ = crate::panel_maintenance_mode::clear_with_reason("completed");
        set_restart_scheduled(state, false);
        return;
    };
    if !crate::panel_service::running_under_systemd() {
        let _ = crate::panel_maintenance_mode::clear_with_reason("completed");
        crate::upgrade_tip_log::log_tagged(
            tag,
            "info",
            "Package apply finished in-process (no systemd detached reload)",
        );
        set_restart_scheduled(state, false);
        return;
    }
    let _ = crate::panel_maintenance_mode::mark_restarting(
        "Restarting the panel process after package apply",
    );
    match crate::panel_service::schedule_detached_panel_restart(reason) {
        Ok(()) => {
            let msg = format!("Scheduled detached cpn-installer.service reload ({reason})");
            crate::upgrade_tip_log::log_tagged(tag, "info", &msg);
            state.log(msg, "info");
            set_restart_scheduled(state, true);
        }
        Err(error) => {
            let msg = format!("Could not schedule detached panel reload: {error}");
            crate::upgrade_tip_log::log_tagged(tag, "error", &msg);
            state.log(format!("Warning: {msg}"), "error");
            let _ = crate::panel_maintenance_mode::clear_with_reason("restart-schedule-failed");
            set_restart_scheduled(state, false);
        }
    }
}

fn set_restart_scheduled(state: &AppState, value: bool) {
    let mut status = state.status.write().unwrap_or_else(|e| e.into_inner());
    status.restart_scheduled = value;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_reload_reasons_cover_repair_and_upgrade() {
        assert_eq!(
            apply_reload_reason(MaintenanceAction::Upgrade, "package"),
            Some("post-upgrade")
        );
        assert_eq!(
            apply_reload_reason(MaintenanceAction::Upgrade, "tip"),
            Some("post-tip-upgrade")
        );
        assert_eq!(
            apply_reload_reason(MaintenanceAction::Repair, "package"),
            Some("post-repair")
        );
        assert_eq!(
            apply_reload_reason(MaintenanceAction::Repair, "tip"),
            Some("post-tip-repair")
        );
        assert_eq!(
            apply_reload_reason(MaintenanceAction::Downgrade, "package"),
            Some("post-downgrade")
        );
        assert_eq!(
            apply_reload_reason(MaintenanceAction::ConfigOnly, "package"),
            None
        );
        assert_eq!(
            apply_reload_log_tag(MaintenanceAction::Repair, "package"),
            "repair"
        );
        assert_eq!(
            apply_reload_log_tag(MaintenanceAction::Upgrade, "tip"),
            "upgrade_tip"
        );
    }
}
