//! Backup schedule load/save and restore candidate listing helpers.

use crate::panel_ops_backup_extra::BackupSchedule;
use crate::paths::join_data;
use std::fs;
use std::path::PathBuf;

fn schedule_path() -> PathBuf {
    join_data("backup-schedule.json")
}

pub fn load_schedule() -> BackupSchedule {
    let Ok(raw) = fs::read_to_string(schedule_path()) else {
        return BackupSchedule {
            enabled: false,
            cron: "0 2 * * *".into(),
            scope: "panel".into(),
            domain: String::new(),
            destination: "local".into(),
        };
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save_schedule(schedule: &BackupSchedule) -> Result<(), String> {
    if let Some(parent) = schedule_path().parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Cannot create data dir: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(schedule).map_err(|e| e.to_string())?;
    fs::write(schedule_path(), raw).map_err(|e| format!("Cannot save schedule: {e}"))
}

pub fn list_restore_candidates(
    scope: &str,
    domain: &str,
) -> Result<(String, Vec<(String, u64)>), String> {
    let (preferred, hits) =
        crate::backup_restore_scan::list_restore_archives_with_fallback(scope, domain)?;
    let display = preferred.unwrap_or_else(|| "panel + fallback locations".into());
    let files = hits
        .into_iter()
        .map(|h| (h.name, h.size))
        .collect::<Vec<_>>();
    Ok((display, files))
}
