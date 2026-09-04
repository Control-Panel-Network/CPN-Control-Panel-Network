//! Backup destinations, schedules, and restore listing helpers.

use crate::backups::{BackupScope, list_backup_files, resolve_archive_dir};
use crate::paths::join_data;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BackupDestinations {
    pub local_enabled: bool,
    pub google_drive_note: String,
    pub remote_note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BackupSchedule {
    pub enabled: bool,
    pub cron: String,
    pub scope: String,
    pub domain: String,
}

fn destinations_path() -> PathBuf {
    join_data("backup-destinations.json")
}

fn schedule_path() -> PathBuf {
    join_data("backup-schedule.json")
}

pub fn load_destinations() -> BackupDestinations {
    let Ok(raw) = fs::read_to_string(destinations_path()) else {
        return BackupDestinations {
            local_enabled: true,
            google_drive_note: "Google Drive sync is not configured yet.".into(),
            remote_note: "Remote server transfer is not configured yet.".into(),
        };
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save_destinations(dest: &BackupDestinations) -> Result<(), String> {
    if let Some(parent) = destinations_path().parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Cannot create data dir: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(dest).map_err(|e| e.to_string())?;
    fs::write(destinations_path(), raw).map_err(|e| format!("Cannot save destinations: {e}"))
}

pub fn load_schedule() -> BackupSchedule {
    let Ok(raw) = fs::read_to_string(schedule_path()) else {
        return BackupSchedule {
            enabled: false,
            cron: "0 2 * * *".into(),
            scope: "panel".into(),
            domain: String::new(),
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
    let scope = BackupScope::parse(scope)?;
    let (dir, path_display) = resolve_archive_dir(scope, domain)?;
    Ok((path_display, list_backup_files(&dir)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn destinations_roundtrip() {
        with_test_data_dir(|| {
            let mut d = load_destinations();
            d.local_enabled = true;
            d.google_drive_note = "coming next".into();
            save_destinations(&d).unwrap();
            let loaded = load_destinations();
            assert!(loaded.local_enabled);
            assert_eq!(loaded.google_drive_note, "coming next");
        });
    }
}
